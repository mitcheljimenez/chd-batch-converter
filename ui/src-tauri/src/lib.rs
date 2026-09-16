pub mod log_tail;
mod scanner;
mod settings;

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};

use log_tail::{parse_log_line, LogTailer};
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

#[tauri::command]
fn get_config(app_handle: tauri::AppHandle) -> Config {
    match resolve_app_config_dir(&app_handle) {
        Ok(app_dir) => load_config(&app_dir),
        Err(_) => Config::default(),
    }
}

#[tauri::command]
fn set_config(app_handle: tauri::AppHandle, chdman_path: String) -> Result<(), String> {
    let app_dir = resolve_app_config_dir(&app_handle)?;
    save_config(&app_dir, &Config { chdman_path }).map_err(|e| e.to_string())
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
            .output();
        *guard = None;
    }
    Ok(())
}

#[tauri::command]
fn start_conversion(
    window: tauri::Window,
    app_handle: tauri::AppHandle,
    root: String,
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
            return Err("Ya hay una conversión en curso".to_string());
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

    // Pre-run guards (spec): the configured path must exist, and an unset path
    // falls back to a chdman.exe bundled next to convertir_a_chd.bat.
    let chdman_path = if config.chdman_path.is_empty() {
        let fallback = app_handle
            .path()
            .resolve("build-assets/chdman.exe", tauri::path::BaseDirectory::Resource)
            .map_err(|_| "chdman.exe path not configured".to_string())?;
        if !fallback.exists() {
            return Err("chdman.exe path not configured".to_string());
        }
        strip_verbatim_prefix(&fallback.to_string_lossy())
    } else {
        strip_verbatim_prefix(&config.chdman_path)
    };

    if !std::path::Path::new(&chdman_path).exists() {
        return Err(format!(
            "chdman.exe no encontrado en la ruta configurada: {}",
            chdman_path
        ));
    }

    let script_path = app_handle
        .path()
        .resolve("build-assets/convertir_a_chd.bat", tauri::path::BaseDirectory::Resource)
        .map_err(|_| "bundled convertir_a_chd.bat not found".to_string())?;

    let log_path = std::path::Path::new(&root).join("conversion_log.txt");
    let _ = std::fs::remove_file(&log_path); // start each run from a clean log for the tailer's offset to make sense

    let mut cmd = Command::new(&script_path);
    cmd.arg(&root)
        .env("CHDMAN_OVERRIDE", &chdman_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

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

        loop {
            if let Ok(lines) = tailer.read_new_lines() {
                for line in lines {
                    if let Some(event) = parse_log_line(&line) {
                        match event.status {
                            log_tail::DiscStatus::Ok => converted += 1,
                            log_tail::DiscStatus::Skip => skipped += 1,
                            log_tail::DiscStatus::Fail => failed += 1,
                        }
                        let _ = window.emit("disc-updated", &event);
                    }
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
                        if let Some(event) = parse_log_line(&line) {
                            match event.status {
                                log_tail::DiscStatus::Ok => converted += 1,
                                log_tail::DiscStatus::Skip => skipped += 1,
                                log_tail::DiscStatus::Fail => failed += 1,
                            }
                            let _ = window.emit("disc-updated", &event);
                        }
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
        .manage(RunState(
            Arc::new(Mutex::new(None)),
            Arc::new(AtomicBool::new(false)),
        ))
        .invoke_handler(tauri::generate_handler![
            greet,
            prescan,
            get_config,
            set_config,
            get_history,
            start_conversion,
            cancel_conversion
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
