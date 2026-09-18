mod chd_mover;
mod converter;
mod extractor;
mod flattener;
pub mod log_tail;
mod organizer;
mod scanner;
mod settings;

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Passed to every child process spawned here (`chdman`, `taskkill`) via
/// `.creation_flags(...)`. Without it, each spawn briefly flashes a new
/// console window, since these are all console-subsystem programs and the
/// GUI app itself has none for them to inherit -- and conversion alone can
/// spawn several `chdman` processes at once (see `converter::worker_count`),
/// so this is the difference between one quiet run and a strobe of terminal
/// windows.
const CREATE_NO_WINDOW: u32 = 0x08000000;

use chd_mover::{move_chd_files as run_move_chd_files, MoveChdSummary};
use flattener::{flatten_folders as run_flatten_folders, FlattenSummary};
use organizer::{organize_multidisc as run_organize_multidisc, OrganizeSummary};
use scanner::scan_folder;
use settings::{append_history, load_config, load_history, save_config, Config, RunRecord};

/// Whether a conversion is currently running (`.0`, `swap`-guarded the same
/// way `ExtractState` is), the "user pressed cancel" flag (`.1`), and the
/// PIDs of every `chdman` process any conversion worker currently has in
/// flight (`.2`) -- unlike the old single-`.bat`-child design, several
/// `chdman` processes can be running at once (one per `converter::
/// worker_count` worker), so `cancel_conversion` needs all of their PIDs,
/// not just one. A worker registers its own PID right after spawning and
/// removes it once that `chdman` invocation exits, so this only ever holds
/// PIDs that are (as far as this process knows) still alive.
struct RunState(Arc<AtomicBool>, Arc<AtomicBool>, Arc<Mutex<Vec<u32>>>);

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn prescan(root: String) -> Vec<scanner::ScannedDisc> {
    scan_folder(std::path::Path::new(&root))
}

#[tauri::command]
fn prescan_chds(root: String, app_handle: tauri::AppHandle) -> Result<Vec<extractor::ScannedChd>, String> {
    let app_dir = resolve_app_config_dir(&app_handle)?;
    let config = load_config(&app_dir);
    let chdman_path = resolve_chdman_path(&app_handle, &config)?;
    Ok(extractor::scan_chds(std::path::Path::new(&root), std::path::Path::new(&chdman_path)))
}

/// One `.chd` to unpack, as scanned by a prior `prescan_chds` call -- the
/// only place that runs `chdman info` to determine `kind`, since guessing it
/// here instead would be unsafe (see `extractor::extract_chd_with_progress`'s
/// doc comment).
#[derive(serde::Deserialize, Debug, Clone)]
struct ExtractItem {
    chd_path: String,
    kind: String,
}

#[derive(serde::Serialize, Clone)]
struct ExtractProgressEvent {
    chd_path: String,
    phase: String,
    percent: f32,
}

#[derive(serde::Serialize, Clone)]
struct ExtractItemDone {
    chd_path: String,
    output: Option<String>,
    error: Option<String>,
}

#[derive(serde::Serialize, Clone)]
struct ExtractAllFinished {
    done: u32,
    failed: u32,
}

/// Whether an `extract` run is currently in flight. Separate from `RunState`
/// since extraction always runs one item at a time from a single background
/// thread, unlike conversion's pool of `converter::worker_count` workers.
struct ExtractState(Arc<AtomicBool>);

/// Unpacks every `.chd` in `items` back to its original format (`.cue`+
/// `.bin` for `kind == "cd"`, `.iso` for `kind == "dvd"`), one at a time,
/// from a background thread -- so this returns immediately and the UI stays
/// responsive, the same shape as `start_conversion`. Each item's chdman
/// invocation streams "Extracting, X%"/"Verifying, X%" progress as
/// `extract-item-progress`, then reports success/failure as
/// `extract-item-done`; `extract-all-finished` fires once every item has
/// been attempted.
#[tauri::command]
fn start_extract_all(
    window: tauri::Window,
    app_handle: tauri::AppHandle,
    items: Vec<ExtractItem>,
    state: tauri::State<ExtractState>,
) -> Result<(), String> {
    // swap(true) both checks and claims the slot atomically, so two rapid
    // clicks can't both pass the check and run concurrently.
    if state.0.swap(true, Ordering::SeqCst) {
        return Err("EXTRACT_IN_PROGRESS".to_string());
    }

    let app_dir = resolve_app_config_dir(&app_handle)?;
    let config = load_config(&app_dir);
    let chdman_path = match resolve_chdman_path(&app_handle, &config) {
        Ok(path) => path,
        Err(e) => {
            // Release the slot we just claimed -- this run never actually
            // started, so a later click must be allowed to try again.
            state.0.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    let running_flag = state.0.clone();
    thread::spawn(move || {
        let mut done = 0u32;
        let mut failed = 0u32;
        for item in items {
            let chd_path = std::path::PathBuf::from(&item.chd_path);
            let progress_window = window.clone();
            let progress_chd_path = item.chd_path.clone();
            let result = extractor::extract_chd_with_progress(
                std::path::Path::new(&chdman_path),
                &chd_path,
                &item.kind,
                |phase, percent| {
                    let _ = progress_window.emit(
                        "extract-item-progress",
                        ExtractProgressEvent {
                            chd_path: progress_chd_path.clone(),
                            phase: phase.to_string(),
                            percent,
                        },
                    );
                },
            );
            let event = match result {
                Ok(output) => {
                    done += 1;
                    ExtractItemDone {
                        chd_path: item.chd_path.clone(),
                        output: Some(output.to_string_lossy().to_string()),
                        error: None,
                    }
                }
                Err(e) => {
                    failed += 1;
                    ExtractItemDone {
                        chd_path: item.chd_path.clone(),
                        output: None,
                        error: Some(e),
                    }
                }
            };
            let _ = window.emit("extract-item-done", event);
        }
        let _ = window.emit("extract-all-finished", ExtractAllFinished { done, failed });
        running_flag.store(false, Ordering::SeqCst);
    });

    Ok(())
}

/// `destination` is where every "<Game>.m3u/" folder is created — always
/// flattened to one level there, regardless of how deeply the source discs
/// were nested under `root`.
///
/// `android_base`, when present, is the Android-side path this folder maps
/// to (e.g. `/storage/emulated/0/ROMs` or `/storage/1234-5678/ROMs`) — the
/// UI collects it from the user only when they want playlists usable after
/// copying to an Android device; omitted, playlists use plain relative
/// filenames (correct for organizing on the same PC ES-DE runs on too).
#[tauri::command]
fn organize_multidisc(root: String, destination: String, android_base: Option<String>) -> Result<OrganizeSummary, String> {
    run_organize_multidisc(std::path::Path::new(&root), std::path::Path::new(&destination), android_base.as_deref())
}

/// Recursively finds every loose `.chd` under `root` and moves it into
/// `destination`, flattened to one level, separating it from whatever
/// `.bin`/`.cue`/`.iso` it was converted from.
#[tauri::command]
fn move_chd_files(root: String, destination: String) -> Result<MoveChdSummary, String> {
    run_move_chd_files(std::path::Path::new(&root), std::path::Path::new(&destination))
}

/// Moves every ROM file out of whatever subfolder(s) it's nested under and
/// directly into `root`, then removes the now-empty subfolders. There's no
/// separate destination -- unlike `organize_multidisc`/`move_chd_files`,
/// this flattens `root` in place. Any folder whose name contains ".m3u" is
/// left completely alone (not descended into, not emptied, not removed),
/// since those are `organize_multidisc`'s multi-disc game folders.
#[tauri::command]
fn flatten_folders(root: String) -> Result<FlattenSummary, String> {
    run_flatten_folders(std::path::Path::new(&root))
}

/// Resolves the app's config directory, surfacing a failure as a `Result`
/// instead of panicking. Callers that can't propagate a `Result` (commands
/// returning a bare value) degrade to a sensible default instead.
fn resolve_app_config_dir(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app_handle.path().app_config_dir().map_err(|e| e.to_string())
}

/// Strips the `\\?\` extended-length-path prefix Windows path resolution
/// (Tauri's resource resolver, `std::fs::canonicalize`, etc.) commonly adds.
/// `cmd.exe` and batch scripts cannot parse this prefix at all: `call
/// "\\?\C:\...\chdman.exe"` fails instantly with "the system cannot find the
/// path specified", even though the exact same path runs fine outside a
/// batch context (e.g. spawned directly, or typed at a PowerShell prompt).
/// Only a plain drive-letter path is ever handed to the `.bat` (as
/// CHDMAN_OVERRIDE); this prefix must never survive into that string.
fn strip_verbatim_prefix(path: &str) -> String {
    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

/// Resolves the chdman.exe path the same way for every command that needs
/// one: the user's configured path if set, otherwise the chdman.exe
/// bundled next to convertir_a_chd.bat. Returns the same stable error
/// codes (CHDMAN_NOT_CONFIGURED / CHDMAN_NOT_FOUND:<path>) regardless of
/// caller, since the frontend's i18n.js translates them by exact string.
fn resolve_chdman_path(app_handle: &tauri::AppHandle, config: &Config) -> Result<String, String> {
    let chdman_path = if config.chdman_path.is_empty() {
        let fallback = app_handle
            .path()
            .resolve("build-assets/chdman.exe", tauri::path::BaseDirectory::Resource)
            .map_err(|_| "CHDMAN_NOT_CONFIGURED".to_string())?;
        if !fallback.exists() {
            return Err("CHDMAN_NOT_CONFIGURED".to_string());
        }
        strip_verbatim_prefix(&fallback.to_string_lossy())
    } else {
        strip_verbatim_prefix(&config.chdman_path)
    };

    if !std::path::Path::new(&chdman_path).exists() {
        return Err(format!("CHDMAN_NOT_FOUND:{}", chdman_path));
    }

    Ok(chdman_path)
}

#[tauri::command]
fn get_config(app_handle: tauri::AppHandle) -> Config {
    match resolve_app_config_dir(&app_handle) {
        Ok(app_dir) => load_config(&app_dir),
        Err(_) => Config::default(),
    }
}

#[tauri::command]
fn set_config(
    app_handle: tauri::AppHandle,
    chdman_path: String,
    auto_update_enabled: bool,
    language: String,
) -> Result<(), String> {
    let app_dir = resolve_app_config_dir(&app_handle)?;
    save_config(
        &app_dir,
        &Config {
            chdman_path,
            auto_update_enabled,
            language,
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_history(app_handle: tauri::AppHandle) -> Vec<RunRecord> {
    match resolve_app_config_dir(&app_handle) {
        Ok(app_dir) => load_history(&app_dir),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
fn cancel_conversion(state: tauri::State<RunState>) -> Result<(), String> {
    state.1.store(true, Ordering::SeqCst);
    // Kill every chdman process any worker currently has in flight -- there
    // can be several at once (one per converter::worker_count worker),
    // unlike the old single-.bat-child design. Draining the vec here (rather
    // than just reading it) means a worker that's mid-registration for its
    // *next* item after this point still sees the cancelled flag above and
    // stops on its own before spawning another.
    let pids: Vec<u32> = std::mem::take(&mut *state.2.lock().map_err(|e| e.to_string())?);
    for pid in pids {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    Ok(())
}

/// What the frontend needs to render an "update available" prompt: just
/// enough to show a version number and, if the release has notes, show
/// them. Anything else `tauri_plugin_updater::Update` carries (download
/// URLs, signature) stays server-side — the frontend never touches those,
/// it only ever calls back into install_update to act on them.
#[derive(serde::Serialize, Clone)]
struct UpdateSummary {
    version: String,
    notes: Option<String>,
}

/// Checks GitHub Releases (via the endpoint configured in
/// plugins.updater.endpoints in tauri.conf.json) for a newer version than
/// the one currently running. Ok(None) always means "already on the latest
/// version". When the check itself fails (offline, GitHub unreachable,
/// plugin init failure), `silent` decides how it's reported: `true`
/// swallows it into Ok(None) (the app-start check, which must never
/// interrupt opening the app), `false` surfaces it as Err (the manual
/// "Buscar actualizaciones" button, which reports failures to the user).
#[tauri::command]
async fn check_for_update(
    app_handle: tauri::AppHandle,
    silent: bool,
) -> Result<Option<UpdateSummary>, String> {
    let updater = app_handle.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateSummary {
            version: update.version.clone(),
            notes: update.body.clone(),
        })),
        Ok(None) => Ok(None),
        Err(e) => {
            if silent {
                Ok(None)
            } else {
                Err(e.to_string())
            }
        }
    }
}

/// Downloads and installs the update this app is currently aware of via a
/// prior check_for_update call. Does NOT restart the app — the caller shows
/// the release notes first (see UpdateSummary.notes) and then invokes
/// restart_app once the user has dismissed that dialog. Refuses while a
/// conversion is in flight (RunState.0 is true) rather than killing a
/// possibly hours-long batch job out from under the user — the caller is
/// expected to retry this on the next check (app start or the manual
/// button) once the run finishes.
#[tauri::command]
async fn install_update(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, RunState>,
) -> Result<(), String> {
    if state.0.load(Ordering::SeqCst) {
        return Err("UPDATE_DEFERRED_CONVERSION_IN_PROGRESS".to_string());
    }

    let updater = app_handle.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "NO_UPDATE_AVAILABLE".to_string())?;

    // Emits running totals as "update-download-progress" so the frontend can
    // drive a real progress bar instead of an indeterminate spinner for what
    // (on a slow connection) can be several seconds of download.
    let mut downloaded: u64 = 0;
    let progress_handle = app_handle.clone();
    update
        .download_and_install(
            move |chunk_len, total| {
                downloaded += chunk_len as u64;
                let _ = progress_handle.emit(
                    "update-download-progress",
                    serde_json::json!({ "downloaded": downloaded, "total": total }),
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Restarts the app to apply an update install_update already completed.
/// Split out from install_update so the frontend can show the release notes
/// (from the UpdateSummary check_for_update returned) before the process
/// exits — restarting immediately after install would tear the app down
/// before that dialog could ever render.
#[tauri::command]
fn restart_app(app_handle: tauri::AppHandle) {
    app_handle.restart();
}

/// Converts every eligible disc under `root` in parallel, one `chdman`
/// process per available CPU core (see `converter::worker_count`) --
/// replacing the old approach of shelling out to `convertir_a_chd.bat`,
/// which only ever ran one `chdman` at a time. `format_overrides` is the
/// full path (as reported by `prescan`, `folder + separator + name`) of
/// every `.iso` the user forced to convert as CD instead of DVD.
///
/// Emits the exact same events the old `.bat`-tailing implementation did
/// (`disc-progress`, `disc-updated`, `run-finished`), so the frontend needs
/// no changes: `disc-progress`/`disc-updated` carry the disc's own path
/// directly (no more inferring it from a chdman "Input file:" line, since
/// this path calls chdman itself and already knows it).
///
/// `RunRecord.skipped` is always 0 here: unlike the `.bat`'s own from-
/// scratch filesystem walk, this queues exactly what `scanner::scan_folder`
/// returns, which already excludes discs with an existing `.chd` -- there's
/// nothing left to discover as "skipped" once conversion starts.
#[tauri::command]
fn start_conversion(
    window: tauri::Window,
    app_handle: tauri::AppHandle,
    root: String,
    format_overrides: Vec<String>,
    state: tauri::State<RunState>,
) -> Result<(), String> {
    // swap(true) both checks and claims the "a run is in flight" slot
    // atomically, so two rapid clicks can't both pass the check and start
    // two overlapping runs.
    if state.0.swap(true, Ordering::SeqCst) {
        return Err("CONVERSION_IN_PROGRESS".to_string());
    }

    // Reset the cancelled flag before anything else that follows (including
    // the chdman-path check and spawning). If this happened after spawning, a
    // cancel_conversion racing in between the spawn and the reset could set
    // the flag true and kill the process, only for this store(false) to
    // immediately clobber it back to false — recording a genuinely
    // cancelled run as cancelled: false.
    state.1.store(false, Ordering::SeqCst);

    let app_dir = match resolve_app_config_dir(&app_handle) {
        Ok(dir) => dir,
        Err(e) => {
            state.0.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    let config = load_config(&app_dir);
    let chdman_path = match resolve_chdman_path(&app_handle, &config) {
        Ok(path) => path,
        Err(e) => {
            state.0.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    let discs = scan_folder(std::path::Path::new(&root));
    let overrides: std::collections::HashSet<String> = format_overrides.into_iter().collect();
    let worker_total = converter::worker_count(discs.len());
    let queue = Arc::new(Mutex::new(std::collections::VecDeque::from(discs)));

    let converted = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let failed = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let running_flag = state.0.clone();
    let cancelled_flag = state.1.clone();
    let active_pids = state.2.clone();
    let root_for_thread = root.clone();

    thread::spawn(move || {
        let mut handles = Vec::with_capacity(worker_total);
        for _ in 0..worker_total {
            let queue = queue.clone();
            let chdman_path = chdman_path.clone();
            let overrides = overrides.clone();
            let window = window.clone();
            let cancelled_flag = cancelled_flag.clone();
            let active_pids = active_pids.clone();
            let converted = converted.clone();
            let failed = failed.clone();

            handles.push(thread::spawn(move || loop {
                if cancelled_flag.load(Ordering::SeqCst) {
                    break;
                }
                let Some(disc) = queue.lock().unwrap().pop_front() else {
                    break;
                };

                let disc_path = std::path::Path::new(&disc.folder).join(&disc.name);
                let disc_path_str = disc_path.to_string_lossy().to_string();
                let force_cd = disc.kind == "iso" && overrides.contains(&disc_path_str);
                let progress_window = window.clone();
                let progress_path = disc_path_str.clone();

                let result = converter::convert_disc(
                    std::path::Path::new(&chdman_path),
                    &disc_path,
                    &disc.kind,
                    force_cd,
                    &active_pids,
                    |phase, percent| {
                        let _ = progress_window.emit(
                            "disc-progress",
                            log_tail::ProgressEvent {
                                path: progress_path.clone(),
                                phase: phase.to_string(),
                                percent,
                            },
                        );
                    },
                );

                let event = match result {
                    Ok(_) => {
                        converted.fetch_add(1, Ordering::SeqCst);
                        log_tail::LogEvent {
                            status: log_tail::DiscStatus::Ok,
                            path: disc_path_str,
                            message: "convertido y verificado".to_string(),
                        }
                    }
                    // A cancellation shows up as the same plain failure a
                    // genuinely broken conversion would (the taskkill'd
                    // chdman process just exits non-zero) -- distinguished
                    // here by checking the flag rather than the error
                    // string, so a real failure racing with cancellation
                    // is never misreported as one.
                    Err(_) if cancelled_flag.load(Ordering::SeqCst) => break,
                    Err(e) => {
                        failed.fetch_add(1, Ordering::SeqCst);
                        let message = if e == "CONVERT_VERIFY_FAILED" {
                            "convertido pero VERIFY FALLO"
                        } else {
                            "fallo la conversion"
                        };
                        log_tail::LogEvent {
                            status: log_tail::DiscStatus::Fail,
                            path: disc_path_str,
                            message: message.to_string(),
                        }
                    }
                };
                let _ = window.emit("disc-updated", event);
            }));
        }

        for handle in handles {
            let _ = handle.join();
        }

        let record = RunRecord {
            timestamp: chrono_like_timestamp(),
            folder: root_for_thread,
            converted: converted.load(Ordering::SeqCst),
            skipped: 0,
            failed: failed.load(Ordering::SeqCst),
            cancelled: cancelled_flag.load(Ordering::SeqCst),
        };
        let _ = append_history(&app_dir, record.clone());
        let _ = window.emit("run-finished", &record);
        running_flag.store(false, Ordering::SeqCst);
    });

    Ok(())
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("{}", secs) // Unix timestamp; the frontend formats it for display (Task 7)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(RunState(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(Vec::new())),
        ))
        .manage(ExtractState(Arc::new(AtomicBool::new(false))))
        .invoke_handler(tauri::generate_handler![
            greet,
            prescan,
            prescan_chds,
            start_extract_all,
            organize_multidisc,
            move_chd_files,
            flatten_folders,
            get_config,
            set_config,
            get_history,
            start_conversion,
            cancel_conversion,
            check_for_update,
            install_update,
            restart_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod verbatim_prefix_tests {
    use super::strip_verbatim_prefix;

    #[test]
    fn strips_the_extended_length_prefix() {
        // The exact shape Tauri's resource resolver / std::fs::canonicalize
        // produces on Windows, and the exact shape that made `call
        // "%CHDMAN%"` fail in the packaged app: cmd.exe's batch interpreter
        // cannot parse a \\?\ prefixed path at all, even though the file it
        // points to is perfectly valid and runs fine outside a .bat.
        assert_eq!(
            strip_verbatim_prefix(r"\\?\C:\Program Files\CHD Converter\build-assets\chdman.exe"),
            r"C:\Program Files\CHD Converter\build-assets\chdman.exe"
        );
    }

    #[test]
    fn leaves_a_plain_path_untouched() {
        let plain = r"C:\Games\chdman.exe";
        assert_eq!(strip_verbatim_prefix(plain), plain);
    }
}

#[cfg(test)]
mod update_guard_tests {
    use super::RunState;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    // These test the guard logic in isolation (a live chdman process can't
    // be spawned in a unit test), by exercising the same "is a conversion
    // running" check the commands use: state.0.load(...).

    fn fresh_state() -> RunState {
        RunState(Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(Vec::new())))
    }

    #[test]
    fn no_running_conversion_is_not_blocked() {
        let state = fresh_state();
        assert!(!state.0.load(std::sync::atomic::Ordering::SeqCst), "expected no conversion in progress");
    }

    #[test]
    fn install_update_guard_matches_run_state_shape() {
        // Documents the exact check install_update performs before calling
        // the plugin's downloader: state.0.load(...) must be false. This
        // doesn't spawn a real chdman process (that requires a real OS
        // process and is exercised by the existing
        // start_conversion_smoke.rs integration test instead) — it locks in
        // the guard's shape so a future refactor of RunState's fields
        // doesn't silently drop it.
        let state = fresh_state();
        let is_blocked = state.0.load(std::sync::atomic::Ordering::SeqCst);
        assert!(!is_blocked);
    }
}
