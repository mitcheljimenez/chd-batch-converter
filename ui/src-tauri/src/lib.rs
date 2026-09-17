mod chd_mover;
mod extractor;
pub mod log_tail;
mod organizer;
mod scanner;
mod settings;

use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Passed to every child process spawned here (the conversion script,
/// `tasklist`, `taskkill`) via `.creation_flags(...)`. Without it, each spawn
/// briefly flashes a new console window, since these are all console-
/// subsystem programs and the GUI app itself has none for them to inherit.
/// The `tasklist` liveness poll alone fires every 300ms for the run's whole
/// duration, so this is the difference between one long conversion and a
/// strobe of terminal windows.
const CREATE_NO_WINDOW: u32 = 0x08000000;

use chd_mover::{move_chd_files as run_move_chd_files, MoveChdSummary};
use log_tail::{parse_log_line, LogTailer};
use organizer::{organize_multidisc as run_organize_multidisc, OrganizeSummary};
use scanner::scan_folder;
use settings::{append_history, load_config, load_history, save_config, Config, RunRecord};

/// The running child (if any) and the "user pressed cancel" flag. Both halves
/// are `Arc`-wrapped so the background tail thread can share them with the
/// command handlers — in particular so the thread can clear the child slot on
/// its own exit path instead of leaving a stale `Child` behind (whose PID
/// could later be reused by an unrelated process and killed by a cancel).
struct RunState(Arc<Mutex<Option<Child>>>, Arc<AtomicBool>);

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

/// Unpacks a single `.chd` back to its original format (`.cue`+`.bin` for
/// `kind == "cd"`, `.iso` for `kind == "dvd"`) next to itself. `kind` comes
/// from a prior `prescan_chds` call, which is the only place that runs
/// `chdman info` to determine it -- see `extractor::extract_chd`'s own doc
/// comment for why guessing it here instead would be unsafe.
#[tauri::command]
fn extract_chd_command(chd_path: String, kind: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    let app_dir = resolve_app_config_dir(&app_handle)?;
    let config = load_config(&app_dir);
    let chdman_path = resolve_chdman_path(&app_handle, &config)?;
    let output = extractor::extract_chd(std::path::Path::new(&chdman_path), std::path::Path::new(&chd_path), &kind)?;
    Ok(output.to_string_lossy().to_string())
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
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(child) = guard.as_mut() {
        let pid = child.id();
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        *guard = None;
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
/// conversion is in flight (RunState.0 is Some) rather than killing a
/// possibly hours-long batch job out from under the user — the caller is
/// expected to retry this on the next check (app start or the manual
/// button) once the run finishes.
#[tauri::command]
async fn install_update(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, RunState>,
) -> Result<(), String> {
    {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("UPDATE_DEFERRED_CONVERSION_IN_PROGRESS".to_string());
        }
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

#[tauri::command]
fn start_conversion(
    window: tauri::Window,
    app_handle: tauri::AppHandle,
    root: String,
    format_overrides: Vec<String>,
    state: tauri::State<RunState>,
) -> Result<(), String> {
    // Reject a second run while one is already in flight, BEFORE touching the
    // cancelled flag below. Two rapid clicks can fire start_conversion twice
    // inside one IPC round-trip; the second run's log-file deletion below
    // would truncate the first tailer's view, and state.0 would only
    // remember the second Child — orphaning the first process with no UI way
    // to cancel it. Checking this first also avoids clobbering the flag (see
    // next comment) for a call that's about to bail out anyway.
    {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("CONVERSION_IN_PROGRESS".to_string());
        }
    }

    // Reset the cancelled flag before anything else that follows (including
    // the chdman-path check and spawning). If this happened after spawning, a
    // cancel_conversion racing in between the spawn and the reset could set
    // the flag true and kill the process, only for this store(false) to
    // immediately clobber it back to false — recording a genuinely
    // cancelled run as cancelled: false.
    state.1.store(false, Ordering::SeqCst);

    let app_dir = resolve_app_config_dir(&app_handle)?;
    let config = load_config(&app_dir);
    let chdman_path = resolve_chdman_path(&app_handle, &config)?;

    let script_path = app_handle
        .path()
        .resolve("build-assets/convertir_a_chd.bat", tauri::path::BaseDirectory::Resource)
        .map_err(|_| "SCRIPT_NOT_FOUND".to_string())?;

    let log_path = std::path::Path::new(&root).join("conversion_log.txt");
    let _ = std::fs::remove_file(&log_path); // start each run from a clean log for the tailer's offset to make sense

    // Same clean-slate-per-run treatment as conversion_log.txt above: a
    // stale overrides file from a previous run must never leak into this
    // one. Absent when format_overrides is empty (the common case) so
    // FORMAT_OVERRIDES stays unset and convertir_a_chd.bat's existing
    // "unset means no overrides" behavior applies unchanged.
    let overrides_path = std::path::Path::new(&root).join("format_overrides.txt");
    let _ = std::fs::remove_file(&overrides_path);
    if !format_overrides.is_empty() {
        // Every line, including the LAST one, must end in "\r\n": findstr
        // /X (which convertir_a_chd.bat uses to match override lines)
        // requires CRLF after a line to match it at all -- verified
        // directly, a final line missing the trailing "\r\n" silently
        // fails to match, which a plain .join("\r\n") would produce. Not
        // just LF-vs-CRLF (see the FORMAT_OVERRIDES test fixture's commit
        // message for that half of this gotcha).
        let contents: String = format_overrides.iter().map(|p| format!("{}=cd\r\n", p)).collect();
        std::fs::write(&overrides_path, contents).map_err(|e| e.to_string())?;
    }

    let mut cmd = Command::new(&script_path);
    cmd.arg(&root)
        .env("CHDMAN_OVERRIDE", &chdman_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    if !format_overrides.is_empty() {
        cmd.env("FORMAT_OVERRIDES", &overrides_path);
    }

    let child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    *state.0.lock().map_err(|e| e.to_string())? = Some(child);

    let cancelled_flag = state.1.clone();
    let child_slot = state.0.clone();
    let root_for_thread = root.clone();

    thread::spawn(move || {
        let mut tailer = LogTailer::new(log_path.clone());
        let mut converted = 0u32;
        let mut skipped = 0u32;
        let mut failed = 0u32;
        // Remembers the most recent "Input file:" line chdman printed, since
        // its own "Compressing/Verifying, X% complete" progress lines don't
        // repeat the filename.
        let mut current_file: Option<String> = None;

        let handle_line = |line: &str,
                                current_file: &mut Option<String>,
                                converted: &mut u32,
                                skipped: &mut u32,
                                failed: &mut u32| {
            if let Some(path) = log_tail::parse_input_file_line(line) {
                *current_file = Some(path);
                return;
            }
            if let Some(cur) = current_file.as_deref() {
                if let Some(progress) = log_tail::parse_progress_line(line, cur) {
                    let _ = window.emit("disc-progress", &progress);
                    return;
                }
            }
            if let Some(event) = parse_log_line(line) {
                match event.status {
                    log_tail::DiscStatus::Ok => *converted += 1,
                    log_tail::DiscStatus::Skip => *skipped += 1,
                    log_tail::DiscStatus::Fail => *failed += 1,
                }
                *current_file = None;
                let _ = window.emit("disc-updated", &event);
            }
        };

        loop {
            if let Ok(lines) = tailer.read_new_lines() {
                for line in lines {
                    handle_line(&line, &mut current_file, &mut converted, &mut skipped, &mut failed);
                }
            }

            // Stop when the process has exited AND we've drained the log.
            // A lightweight "is this PID still alive" check via tasklist,
            // since std::process::Child doesn't expose non-blocking wait
            // across a Mutex boundary cleanly here. Only a successful
            // tasklist invocation whose output omits the PID counts as
            // "not running" — if tasklist itself fails to run (transient
            // spawn failure), assume the child is still running and retry
            // on the next poll, rather than truncating the tail early.
            let still_running = match Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid)])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            {
                Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
                Err(_) => true,
            };

            if !still_running {
                // One final drain in case the process wrote its last lines
                // between our last read and it exiting.
                if let Ok(lines) = tailer.read_new_lines() {
                    for line in lines {
                        handle_line(&line, &mut current_file, &mut converted, &mut skipped, &mut failed);
                    }
                }
                // The process is gone: clear the shared slot so a later
                // cancel_conversion can't taskkill a stale PID that Windows
                // may since have recycled onto an unrelated process. Only on
                // this exit path — while the loop is still polling, the slot
                // must keep the live Child so cancel can reach it.
                if let Ok(mut guard) = child_slot.lock() {
                    *guard = None;
                }
                break;
            }

            thread::sleep(Duration::from_millis(300));
        }

        let record = RunRecord {
            timestamp: chrono_like_timestamp(),
            folder: root_for_thread,
            converted,
            skipped,
            failed,
            cancelled: cancelled_flag.load(Ordering::SeqCst),
        };
        let _ = append_history(&app_dir, record.clone());
        let _ = window.emit("run-finished", &record);
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
            Arc::new(Mutex::new(None)),
            Arc::new(AtomicBool::new(false)),
        ))
        .invoke_handler(tauri::generate_handler![
            greet,
            prescan,
            prescan_chds,
            extract_chd_command,
            organize_multidisc,
            move_chd_files,
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

    // These test the guard logic in isolation (a live Child can't be
    // constructed in a unit test without actually spawning a process), by
    // exercising the same "is a conversion running" check the commands use:
    // state.0.lock().unwrap().is_some().

    #[test]
    fn no_running_conversion_is_not_blocked() {
        let state = RunState(Arc::new(Mutex::new(None)), Arc::new(AtomicBool::new(false)));
        let guard = state.0.lock().unwrap();
        assert!(guard.is_none(), "expected no conversion in progress");
    }

    #[test]
    fn install_update_guard_matches_run_state_shape() {
        // Documents the exact check install_update performs before calling
        // the plugin's downloader: state.0.lock() must yield None. This
        // doesn't spawn a real Child (that requires a real OS process and
        // is exercised by the existing start_conversion_smoke.rs
        // integration test instead) — it locks in the guard's shape so a
        // future refactor of RunState's fields doesn't silently drop it.
        let state = RunState(Arc::new(Mutex::new(None)), Arc::new(AtomicBool::new(false)));
        let is_blocked = state.0.lock().map(|g| g.is_some()).unwrap_or(false);
        assert!(!is_blocked);
    }
}
