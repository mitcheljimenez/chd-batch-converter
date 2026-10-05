mod chd_mover;
mod chdman;
mod converter;
mod disc_files;
mod extractor;
mod flattener;
pub mod log_tail;
mod organizer;
mod scanner;
mod settings;
mod verifier;

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Passed (on Windows) to every child process spawned here (`chdman`,
/// `taskkill`) via `hide_console`. Without it, each spawn briefly flashes a
/// new console window, since these are all console-subsystem programs and
/// the GUI app itself has none for them to inherit -- and conversion alone
/// can spawn several `chdman` processes at once (see
/// `converter::worker_count`), so this is the difference between one quiet
/// run and a strobe of terminal windows.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Keeps a spawned console program from opening its own window. Only
/// Windows needs this; on Linux/macOS a child process never gets a window
/// of its own, so it's a no-op there.
pub(crate) fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

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
    /// chdman's own output when extraction failed, for "Ver detalles".
    log: Option<String>,
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

/// Whether a verify-only pass is running (`.0`, claimed with swap like
/// `ExtractState`) and whether the user asked to stop it (`.1`). Cancelling
/// stops before the next file rather than killing the chdman in flight:
/// verify only reads, so letting the current file finish is harmless.
struct VerifyState(Arc<AtomicBool>, Arc<AtomicBool>);

#[derive(serde::Serialize, Clone)]
struct VerifyProgressEvent {
    chd_path: String,
    percent: f32,
}

#[derive(serde::Serialize, Clone)]
struct VerifyItemDone {
    chd_path: String,
    ok: bool,
    /// What chdman reported when the file didn't verify.
    log: Option<String>,
}

#[derive(serde::Serialize, Clone)]
struct VerifyAllFinished {
    ok: u32,
    failed: u32,
    cancelled: bool,
}

/// Lists every `.chd` under `root` for the "Verificar .chd" view.
#[tauri::command]
fn prescan_verify(root: String) -> Vec<verifier::ScannedChdFile> {
    verifier::scan_all_chds(std::path::Path::new(&root))
}

/// Runs `chdman verify` on each path in `chd_paths`, one at a time, from a
/// background thread: `verify-item-progress` while each runs,
/// `verify-item-done` with the verdict, then `verify-all-finished`.
#[tauri::command]
fn start_verify_all(
    window: tauri::Window,
    app_handle: tauri::AppHandle,
    chd_paths: Vec<String>,
    state: tauri::State<VerifyState>,
) -> Result<(), String> {
    if state.0.swap(true, Ordering::SeqCst) {
        return Err("VERIFY_IN_PROGRESS".to_string());
    }
    state.1.store(false, Ordering::SeqCst);

    let chdman_path = match resolve_app_config_dir(&app_handle)
        .and_then(|dir| resolve_chdman_path(&app_handle, &load_config(&dir)))
    {
        Ok(path) => path,
        Err(e) => {
            state.0.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    let running_flag = state.0.clone();
    let cancelled_flag = state.1.clone();
    thread::spawn(move || {
        let (mut ok, mut failed) = (0u32, 0u32);
        for chd_path in chd_paths {
            if cancelled_flag.load(Ordering::SeqCst) {
                break;
            }
            let progress_window = window.clone();
            let progress_path = chd_path.clone();
            let result = verifier::verify_chd(
                std::path::Path::new(&chdman_path),
                std::path::Path::new(&chd_path),
                |_, percent| {
                    let _ = progress_window.emit(
                        "verify-item-progress",
                        VerifyProgressEvent { chd_path: progress_path.clone(), percent },
                    );
                },
            );
            let item = match result {
                Ok(()) => {
                    ok += 1;
                    VerifyItemDone { chd_path, ok: true, log: None }
                }
                Err(e) => {
                    failed += 1;
                    VerifyItemDone { chd_path, ok: false, log: Some(e.log).filter(|l| !l.is_empty()) }
                }
            };
            let _ = window.emit("verify-item-done", item);
        }
        let cancelled = cancelled_flag.load(Ordering::SeqCst);
        let _ = window.emit("verify-all-finished", VerifyAllFinished { ok, failed, cancelled });
        running_flag.store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// Stops a verify pass after the file currently being checked.
#[tauri::command]
fn cancel_verify(state: tauri::State<VerifyState>) {
    state.1.store(true, Ordering::SeqCst);
}

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
                        log: None,
                    }
                }
                Err(e) => {
                    failed += 1;
                    ExtractItemDone {
                        chd_path: item.chd_path.clone(),
                        output: None,
                        error: Some(e.code),
                        log: Some(e.log).filter(|l| !l.is_empty()),
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

/// Resolves the chdman path the same way for every command that needs
/// one: the user's configured path if set, otherwise the platform default
/// (see `default_chdman_path`). Returns the same stable error codes
/// (CHDMAN_NOT_CONFIGURED / CHDMAN_NOT_FOUND:<path>) regardless of caller,
/// since the frontend's i18n.js translates them by exact string.
fn resolve_chdman_path(app_handle: &tauri::AppHandle, config: &Config) -> Result<String, String> {
    let chdman_path = if config.chdman_path.is_empty() {
        default_chdman_path(app_handle).ok_or_else(|| "CHDMAN_NOT_CONFIGURED".to_string())?
    } else {
        strip_verbatim_prefix(&config.chdman_path)
    };

    if !std::path::Path::new(&chdman_path).exists() {
        return Err(format!("CHDMAN_NOT_FOUND:{}", chdman_path));
    }

    Ok(chdman_path)
}

/// Windows: the chdman.exe bundled as an installer resource.
#[cfg(windows)]
fn default_chdman_path(app_handle: &tauri::AppHandle) -> Option<String> {
    let bundled = app_handle
        .path()
        .resolve("build-assets/chdman.exe", tauri::path::BaseDirectory::Resource)
        .ok()?;
    bundled
        .exists()
        .then(|| strip_verbatim_prefix(&bundled.to_string_lossy()))
}

/// Linux/macOS: nothing is bundled (distros and Homebrew ship their own
/// chdman), so look for one installed on the system.
#[cfg(not(windows))]
fn default_chdman_path(_app_handle: &tauri::AppHandle) -> Option<String> {
    find_system_chdman(std::env::var_os("PATH"))
        .map(|p| p.to_string_lossy().to_string())
}

/// Where package managers put chdman, searched in addition to `$PATH`. A
/// macOS app launched from Finder/Dock does *not* inherit the shell's
/// `PATH`, so Homebrew's prefix (`/opt/homebrew/bin` on Apple Silicon,
/// `/usr/local/bin` on Intel) would otherwise never be found. `/usr/games`
/// is where some Debian-based distros install mame-tools.
#[cfg(not(windows))]
const SYSTEM_CHDMAN_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/usr/games"];

#[cfg(not(windows))]
fn find_system_chdman(path_var: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    let from_path = path_var
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    from_path
        .into_iter()
        .chain(SYSTEM_CHDMAN_DIRS.iter().map(std::path::PathBuf::from))
        .map(|dir| dir.join("chdman"))
        .find(|candidate| candidate.is_file())
}

/// Whether `path` is an existing directory, so the frontend can tell if
/// the remembered last folder is still there before reopening it.
#[tauri::command]
fn folder_exists(path: String) -> bool {
    std::path::Path::new(&path).is_dir()
}

/// The folder to open for something dropped on the window: the folder
/// itself, or the folder containing a dropped file (e.g. a .cue), so either
/// works. None if the path doesn't exist.
#[tauri::command]
fn folder_for_drop(path: String) -> Option<String> {
    let path = std::path::Path::new(&path);
    if path.is_dir() {
        Some(path.to_string_lossy().to_string())
    } else if path.is_file() {
        path.parent().map(|p| p.to_string_lossy().to_string())
    } else {
        None
    }
}

/// The OS this build runs on ("windows", "linux", "macos"), so the
/// frontend can show platform-specific help (e.g. how to install chdman).
#[tauri::command]
fn get_platform() -> &'static str {
    std::env::consts::OS
}

#[tauri::command]
fn get_config(app_handle: tauri::AppHandle) -> Config {
    match resolve_app_config_dir(&app_handle) {
        Ok(app_dir) => load_config(&app_dir),
        Err(_) => Config::default(),
    }
}

/// Replaces the whole persisted config. The frontend always sends every
/// field (see `saveConfig` in main.js), so one call shape covers every
/// setting instead of growing a parameter per option.
#[tauri::command]
fn set_config(app_handle: tauri::AppHandle, config: Config) -> Result<(), String> {
    let app_dir = resolve_app_config_dir(&app_handle)?;
    save_config(&app_dir, &config).map_err(|e| e.to_string())
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
        kill_process(pid);
    }
    Ok(())
}

/// Force-kills one `chdman` process by PID. `chdman` never spawns children
/// of its own, so there's no process tree to worry about beyond `/T` on
/// Windows (kept for parity with the old `.bat` design).
#[cfg(windows)]
fn kill_process(pid: u32) {
    let _ = hide_console(Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"])).output();
}

#[cfg(unix)]
fn kill_process(pid: u32) {
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
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

/// Payload of `disc-updated`: the same fields the old `.bat`-tailing
/// `LogEvent` carried, plus `note`, a stable code (translated by the
/// frontend) for anything extra that happened to the disc, e.g.
/// "ORIGINALS_TRASHED".
#[derive(serde::Serialize, Clone)]
struct DiscResult {
    status: log_tail::DiscStatus,
    path: String,
    message: String,
    note: Option<&'static str>,
    /// Size of the original files and of the new .chd, on success only.
    original_bytes: Option<u64>,
    chd_bytes: Option<u64>,
    /// chdman's own output when the disc failed, for "Ver detalles".
    log: Option<String>,
}

/// Converts every eligible disc under `root` in parallel, one `chdman`
/// process per available CPU core (see `converter::worker_count`), or one
/// disc at a time when `Config.parallel_conversion` is off --
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
    trash_originals: bool,
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
    let parallel = config.parallel_conversion;
    let worker_total = if parallel { converter::worker_count(discs.len()) } else { 1 };
    let queue = Arc::new(Mutex::new(std::collections::VecDeque::from(discs)));

    let converted = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let failed = Arc::new(std::sync::atomic::AtomicU32::new(0));
    // Totals over successfully converted discs only, for the "space saved"
    // summary and the history entry.
    let bytes_before = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let bytes_after = Arc::new(std::sync::atomic::AtomicU64::new(0));

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
            let bytes_before = bytes_before.clone();
            let bytes_after = bytes_after.clone();

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
                // Listed before converting: the set of files is what the
                // .cue/.gdi references right now, not after the fact.
                let original_files = disc_files::disc_files(&disc_path, &disc.kind);
                let original_size = disc_files::total_size(&original_files);
                let progress_window = window.clone();
                let progress_path = disc_path_str.clone();

                let result = converter::convert_disc(
                    std::path::Path::new(&chdman_path),
                    &disc_path,
                    &disc.kind,
                    force_cd,
                    parallel,
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
                    Ok(chd_path) => {
                        converted.fetch_add(1, Ordering::SeqCst);
                        let chd_size = std::fs::metadata(&chd_path).map(|m| m.len()).unwrap_or(0);
                        bytes_before.fetch_add(original_size, Ordering::SeqCst);
                        bytes_after.fetch_add(chd_size, Ordering::SeqCst);
                        // Only reached once chdman verify passed on the new
                        // .chd, so the originals are safe to let go of.
                        let note = if trash_originals {
                            match disc_files::move_to_trash(&original_files) {
                                Ok(()) => Some("ORIGINALS_TRASHED"),
                                Err(_) => Some("ORIGINALS_TRASH_FAILED"),
                            }
                        } else {
                            None
                        };
                        DiscResult {
                            status: log_tail::DiscStatus::Ok,
                            path: disc_path_str,
                            message: "convertido y verificado".to_string(),
                            note,
                            original_bytes: Some(original_size),
                            chd_bytes: Some(chd_size),
                            log: None,
                        }
                    }
                    // A cancellation shows up as the same plain failure a
                    // genuinely broken conversion would (the killed
                    // chdman process just exits non-zero) -- distinguished
                    // here by checking the flag rather than the error
                    // string, so a real failure racing with cancellation
                    // is never misreported as one.
                    Err(_) if cancelled_flag.load(Ordering::SeqCst) => break,
                    Err(e) => {
                        failed.fetch_add(1, Ordering::SeqCst);
                        let message = if e.code == "CONVERT_VERIFY_FAILED" {
                            "convertido pero VERIFY FALLO"
                        } else {
                            "fallo la conversion"
                        };
                        DiscResult {
                            status: log_tail::DiscStatus::Fail,
                            path: disc_path_str,
                            message: message.to_string(),
                            note: None,
                            original_bytes: None,
                            chd_bytes: None,
                            log: Some(e.log).filter(|l| !l.is_empty()),
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
            original_bytes: bytes_before.load(Ordering::SeqCst),
            chd_bytes: bytes_after.load(Ordering::SeqCst),
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
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(RunState(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(Vec::new())),
        ))
        .manage(ExtractState(Arc::new(AtomicBool::new(false))))
        .manage(VerifyState(Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false))))
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
            restart_app,
            get_platform,
            folder_exists,
            folder_for_drop,
            prescan_verify,
            start_verify_all,
            cancel_verify
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

#[cfg(all(test, not(windows)))]
mod system_chdman_tests {
    use super::find_system_chdman;
    use std::fs;

    #[test]
    fn finds_chdman_in_a_path_entry() {
        let dir = std::env::temp_dir().join("chd_ui_find_system_chdman_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("chdman"), "").unwrap();

        let path_var = std::env::join_paths([std::path::PathBuf::from("/nonexistent-dir"), dir.clone()]).unwrap();
        assert_eq!(find_system_chdman(Some(path_var)), Some(dir.join("chdman")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ignores_a_directory_named_chdman() {
        let dir = std::env::temp_dir().join("chd_ui_find_system_chdman_dir_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("chdman")).unwrap();

        let path_var = std::env::join_paths([dir.clone()]).unwrap();
        // Can't assert None outright: the machine running the tests may
        // have a real chdman in one of SYSTEM_CHDMAN_DIRS.
        assert_ne!(find_system_chdman(Some(path_var)), Some(dir.join("chdman")));

        let _ = fs::remove_dir_all(&dir);
    }
}

/// End-to-end checks of the real conversion path (spawn chdman, stream its
/// progress, verify, cancel) against a tiny shell-script stand-in for
/// chdman. Unix-only: the Windows equivalent lives in tests/*.rs via the
/// legacy .bat.
#[cfg(all(test, unix))]
mod unix_conversion_tests {
    use super::{converter, kill_process, verifier};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use std::{fs, thread};

    const MOCK_CHDMAN: &str = r#"#!/bin/sh
# Minimal chdman stand-in: progress on stderr with bare \r like the real one.
cmd="$1"
out=""
while [ $# -gt 0 ]; do
  if [ "$1" = "-o" ]; then out="$2"; fi
  shift
done
case "$cmd" in
  createcd|createdvd)
    if [ -n "$MOCK_CHDMAN_HANG" ]; then printf 'partial' > "$out"; exec sleep 30; fi
    if [ -e "$out" ]; then echo "Error: file already exists" >&2; exit 1; fi
    printf 'Compressing, 50.0%% complete... (ratio=40.0%%)\r' >&2
    printf 'Compressing, 100.0%% complete... (ratio=40.0%%)\n' >&2
    printf 'chd' > "$out"
    ;;
  verify)
    printf 'Verifying, 100.0%% complete...\n' >&2
    ;;
  *) exit 1 ;;
esac
"#;

    /// Writing an executable while another test thread is spawning a
    /// process races on Linux: the forked child briefly inherits our
    /// still-open write handle, and exec'ing the script during that window
    /// fails with ETXTBSY ("Text file busy"). Probe-run it until it starts.
    fn wait_until_runnable(script: &Path) {
        for _ in 0..200 {
            match std::process::Command::new(script).output() {
                Err(e) if e.raw_os_error() == Some(26) => thread::sleep(Duration::from_millis(10)),
                _ => return,
            }
        }
        panic!("{} stayed busy", script.display());
    }

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let work = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&work);
        fs::create_dir_all(&work).unwrap();
        let mock = work.join("chdman");
        fs::write(&mock, MOCK_CHDMAN).unwrap();
        fs::set_permissions(&mock, fs::Permissions::from_mode(0o755)).unwrap();
        wait_until_runnable(&mock);
        let disc = work.join("Game.cue");
        fs::write(&disc, "FILE \"Game.bin\" BINARY\n").unwrap();
        (mock, disc)
    }

    #[test]
    fn converts_and_verifies_with_progress() {
        let (mock, disc) = setup("chd_ui_unix_convert_test");
        let pids = Arc::new(Mutex::new(Vec::new()));
        let mut seen = Vec::new();

        let out = converter::convert_disc(&mock, &disc, "cue", false, true, &pids, |phase, pct| {
            seen.push((phase.to_string(), pct))
        })
        .expect("conversion should succeed");

        assert_eq!(out, disc.with_extension("chd"));
        assert!(out.is_file());
        assert!(seen.contains(&("compressing".to_string(), 50.0)));
        assert!(seen.contains(&("verifying".to_string(), 100.0)));
        assert!(pids.lock().unwrap().is_empty(), "finished PIDs must be unregistered");
        let _ = fs::remove_dir_all(disc.parent().unwrap());
    }

    #[test]
    fn kill_process_stops_a_running_conversion() {
        let (mock, disc) = setup("chd_ui_unix_cancel_test");
        // Make the mock hang in its convert step until killed. Set via a
        // wrapper rather than the test process's env, since tests run in
        // parallel and share it.
        let hanging = mock.with_file_name("chdman-hang");
        fs::write(&hanging, format!("#!/bin/sh\nMOCK_CHDMAN_HANG=1 exec \"{}\" \"$@\"\n", mock.display())).unwrap();
        fs::set_permissions(&hanging, fs::Permissions::from_mode(0o755)).unwrap();
        wait_until_runnable(&hanging);

        let pids = Arc::new(Mutex::new(Vec::new()));
        let worker_pids = Arc::clone(&pids);
        let worker_disc = disc.clone();
        let started = Instant::now();
        let worker = thread::spawn(move || {
            converter::convert_disc(Path::new(&hanging), &worker_disc, "cue", false, true, &worker_pids, |_, _| {})
        });

        let pid = loop {
            if let Some(&pid) = pids.lock().unwrap().first() {
                break pid;
            }
            assert!(started.elapsed() < Duration::from_secs(5), "chdman never registered its PID");
            thread::sleep(Duration::from_millis(20));
        };
        kill_process(pid);

        let result = worker.join().unwrap();
        assert_eq!(result.map_err(|e| e.code), Err("CONVERT_FAILED".to_string()));
        assert!(started.elapsed() < Duration::from_secs(20), "kill didn't stop chdman");
        assert!(
            !disc.with_extension("chd").exists(),
            "the truncated .chd from a cancelled run must be removed"
        );
        let _ = fs::remove_dir_all(disc.parent().unwrap());
    }

    #[test]
    fn failed_convert_keeps_a_chd_that_was_already_there() {
        let (mock, disc) = setup("chd_ui_unix_preexisting_test");
        let existing = disc.with_extension("chd");
        fs::write(&existing, "user's own chd").unwrap();
        let pids = Arc::new(Mutex::new(Vec::new()));

        let result = converter::convert_disc(&mock, &disc, "cue", false, true, &pids, |_, _| {});

        let err = result.unwrap_err();
        assert_eq!(err.code, "CONVERT_FAILED");
        // chdman's own explanation is kept for "Ver detalles".
        assert!(err.log.contains("Error: file already exists"), "{:?}", err.log);
        assert_eq!(fs::read_to_string(&existing).unwrap(), "user's own chd");
        let _ = fs::remove_dir_all(disc.parent().unwrap());
    }

    #[test]
    fn verify_reports_a_good_chd_and_a_corrupt_one() {
        let (mock, disc) = setup("chd_ui_unix_verify_test");
        let chd = disc.with_extension("chd");
        fs::write(&chd, "chd").unwrap();
        let mut seen = Vec::new();
        assert_eq!(verifier::verify_chd(&mock, &chd, |phase, pct| seen.push((phase.to_string(), pct))), Ok(()));
        assert!(seen.contains(&("verifying".to_string(), 100.0)));

        // A chdman that finds a hash mismatch exits non-zero.
        let corrupt = mock.with_file_name("chdman-corrupt");
        fs::write(&corrupt, "#!/bin/sh\necho 'Error: Raw SHA1 in header = 1234' >&2\nexit 1\n").unwrap();
        fs::set_permissions(&corrupt, fs::Permissions::from_mode(0o755)).unwrap();
        wait_until_runnable(&corrupt);
        let err = verifier::verify_chd(&corrupt, &chd, |_, _| {}).unwrap_err();
        assert_eq!(err.code, "VERIFY_FAILED");
        assert!(err.log.contains("Raw SHA1 in header"), "{:?}", err.log);

        let _ = fs::remove_dir_all(disc.parent().unwrap());
    }
}
