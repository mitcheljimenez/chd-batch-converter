use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

/// Whether `kind` ("cue" or "iso") should be converted with `createcd`
/// instead of the format its extension would normally imply. A `.cue` is
/// always `createcd` regardless of `force_cd` -- the flag only matters for
/// an `.iso`, where the caller can force CD format via a per-item override
/// (the UI's "convert as CD" choice for a disc that's really a PS1 game
/// wrongly ripped as `.iso`). Mirrors `convertir_a_chd.bat`'s
/// `FORMAT_OVERRIDES` handling, kept here as its own pure function so it's
/// testable without spawning chdman.
fn should_use_cd(kind: &str, force_cd: bool) -> bool {
    kind == "cue" || force_cd
}

/// Builds the `createcd`/`createdvd` argument list for converting
/// `disc_path` to `out`. Always pins `-np 1` (one compression thread) --
/// see the doc comment on the `-np` push site in `convert_disc` for why:
/// this crate's own parallelism is one chdman process per core, so letting
/// each process ALSO spin up its own per-core thread pool would
/// oversubscribe the CPU by roughly (cores squared) instead of just using
/// it. Kept as its own pure function so the presence of `-np 1` is
/// verifiable without spawning chdman.
fn build_convert_args(kind: &str, force_cd: bool, disc_path: &Path, out: &Path) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = Vec::new();
    if should_use_cd(kind, force_cd) {
        args.push("createcd".into());
    } else {
        // createdvd defaults to zstd compression, unreadable by
        // AetherSX2/NetherSX2 on Android -- -c zlib keeps DVD CHDs portable
        // there. Same rationale as convertir_a_chd.bat's own :process_disc.
        args.push("createdvd".into());
        args.push("-c".into());
        args.push("zlib".into());
    }
    args.push("-i".into());
    args.push(disc_path.as_os_str().to_owned());
    args.push("-o".into());
    args.push(out.as_os_str().to_owned());
    args.push("-np".into());
    args.push("1".into());
    args
}

/// Converts one disc (`.cue` or `.iso`) to a `.chd` next to itself, then
/// verifies the result, streaming chdman's own progress output through
/// `on_progress` the same way `extractor::extract_chd_with_progress` does
/// for extraction. Unlike the old `convertir_a_chd.bat`, this never checks
/// whether the destination `.chd` already exists -- callers are expected to
/// only queue discs `scanner::scan_folder` returned, which already excludes
/// those (so "skipped" as a per-run count no longer applies to this path;
/// see `RunRecord.skipped` staying 0 in `lib.rs::start_conversion`).
///
/// `active_pids` records the spawned chdman PID for the run's duration, so
/// `cancel_conversion` (running on a different thread, possibly mid-queue
/// for other workers) can `taskkill` it immediately regardless of how much
/// stdout is still to drain. A kill shows up here as a plain non-zero exit,
/// which this function reports as `CONVERT_FAILED`/`CONVERT_VERIFY_FAILED`
/// -- the caller distinguishes a genuine failure from a cancellation by
/// checking its own cancelled flag afterward (see `lib.rs`'s worker loop).
pub fn convert_disc(
    chdman_path: &Path,
    disc_path: &Path,
    kind: &str,
    force_cd: bool,
    active_pids: &Arc<Mutex<Vec<u32>>>,
    mut on_progress: impl FnMut(&str, f32),
) -> Result<PathBuf, String> {
    let out = disc_path.with_extension("chd");
    // `verify` has no `-np` equivalent (it isn't in verify's own option
    // list per chdman's docs), so the thread-oversubscription guard below
    // only applies to the convert step.
    let convert_args = build_convert_args(kind, force_cd, disc_path, &out);

    run_with_progress(chdman_path, &convert_args, active_pids, &mut on_progress)
        .map_err(|_| "CONVERT_FAILED".to_string())?;

    let verify_args: Vec<std::ffi::OsString> = vec!["verify".into(), "-i".into(), out.as_os_str().to_owned()];
    run_with_progress(chdman_path, &verify_args, active_pids, &mut on_progress)
        .map_err(|_| "CONVERT_VERIFY_FAILED".to_string())?;

    Ok(out)
}

/// Runs `chdman` with `args`, streaming its output line-by-line (splitting
/// on chdman's bare '\r' progress-overwrite trick) and calling
/// `on_progress(phase, percent)` for every "Compressing, X%
/// complete..."/"Verifying, X% complete..." line. chdman writes its progress
/// lines to **stderr** (confirmed in its own source: `progress()` writes to
/// `std::cerr` and flushes on every call) -- stdout carries only the
/// occasional summary line. Both streams are piped and read from their own
/// thread so a slow/quiet stdout never blocks stderr's progress lines (or
/// vice versa); the two threads feed a shared channel that this function
/// drains on the caller's thread, since `on_progress` closes over a
/// `tauri::Window` and isn't required to be `Send`.
///
/// Registers the spawned PID in `active_pids` for the run's duration so an
/// external `taskkill` can reach it.
fn run_with_progress(
    chdman_path: &Path,
    args: &[std::ffi::OsString],
    active_pids: &Arc<Mutex<Vec<u32>>>,
    on_progress: &mut impl FnMut(&str, f32),
) -> Result<(), ()> {
    let mut child = Command::new(chdman_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(crate::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| ())?;

    let pid = child.id();
    active_pids.lock().unwrap().push(pid);

    let stdout = child.stdout.take().ok_or(())?;
    let stderr = child.stderr.take().ok_or(())?;

    let (tx, rx) = mpsc::channel();
    let tx_stderr = tx.clone();
    let stdout_reader = thread::spawn(move || stream_progress(stdout, tx));
    let stderr_reader = thread::spawn(move || stream_progress(stderr, tx_stderr));

    for (phase, percent) in rx {
        on_progress(phase, percent);
    }

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();

    let status = child.wait().map_err(|_| ())?;
    active_pids.lock().unwrap().retain(|&p| p != pid);

    if status.success() {
        Ok(())
    } else {
        Err(())
    }
}

/// Reads `reader` to EOF, parsing out "Compressing/Verifying, X% complete"
/// lines and sending each as `(phase, percent)` through `tx`. Runs on its
/// own thread (see `run_with_progress`) so it never has to wait its turn
/// behind the other stream.
fn stream_progress(mut reader: impl Read, tx: mpsc::Sender<(&'static str, f32)>) {
    let mut partial = String::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = reader.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        partial.push_str(&String::from_utf8_lossy(&chunk[..n]));
        let normalized = partial.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<&str> = normalized.split('\n').collect();
        let tail = lines.pop().unwrap_or("").to_string();
        for line in lines {
            if let Some(parsed) = parse_convert_progress(line) {
                let _ = tx.send(parsed);
            }
        }
        partial = tail;
    }
}

/// Parses chdman's own progress output for conversion, e.g. "Compressing,
/// 42.9% complete... (ratio=51.1%)" or "Verifying, 71.7% complete...".
fn parse_convert_progress(line: &str) -> Option<(&'static str, f32)> {
    let trimmed = line.trim_start();
    let (phase, rest) = if let Some(r) = trimmed.strip_prefix("Compressing, ") {
        ("compressing", r)
    } else if let Some(r) = trimmed.strip_prefix("Verifying, ") {
        ("verifying", r)
    } else {
        return None;
    };
    let percent_str = rest.split('%').next()?;
    let percent: f32 = percent_str.trim().parse().ok()?;
    Some((phase, percent))
}

/// How many discs to convert concurrently: one worker per available CPU
/// core, capped at the number of discs actually queued (no point starting
/// idle workers that would just find an empty queue).
pub fn worker_count(queued: usize) -> usize {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    std::cmp::max(1, std::cmp::min(cores, queued))
}

#[cfg(test)]
mod should_use_cd_tests {
    use super::should_use_cd;

    #[test]
    fn a_cue_is_always_createcd_regardless_of_the_override() {
        assert!(should_use_cd("cue", false));
        assert!(should_use_cd("cue", true));
    }

    #[test]
    fn an_iso_is_createdvd_unless_forced_to_cd() {
        assert!(!should_use_cd("iso", false));
        assert!(should_use_cd("iso", true));
    }
}

#[cfg(test)]
mod build_convert_args_tests {
    use super::build_convert_args;
    use std::path::Path;

    fn args_as_strings(args: &[std::ffi::OsString]) -> Vec<String> {
        args.iter().map(|a| a.to_string_lossy().to_string()).collect()
    }

    #[test]
    fn every_invocation_pins_to_a_single_compression_thread() {
        // The whole point: this crate parallelizes at the process level (one
        // chdman per core, see worker_count), so every single chdman process
        // must be capped to one compression thread of its own -- otherwise
        // N processes times chdman's own default per-core thread pool would
        // oversubscribe the CPU by roughly N² instead of just using it.
        let cue_args = args_as_strings(&build_convert_args("cue", false, Path::new("Game.cue"), Path::new("Game.chd")));
        assert!(cue_args.windows(2).any(|w| w == ["-np", "1"]), "{:?}", cue_args);

        let iso_args = args_as_strings(&build_convert_args("iso", false, Path::new("Game.iso"), Path::new("Game.chd")));
        assert!(iso_args.windows(2).any(|w| w == ["-np", "1"]), "{:?}", iso_args);
    }

    #[test]
    fn a_cue_uses_createcd() {
        let args = args_as_strings(&build_convert_args("cue", false, Path::new("Game.cue"), Path::new("Game.chd")));
        assert_eq!(args[0], "createcd");
    }

    #[test]
    fn an_iso_defaults_to_createdvd_with_zlib() {
        let args = args_as_strings(&build_convert_args("iso", false, Path::new("Game.iso"), Path::new("Game.chd")));
        assert_eq!(args[0], "createdvd");
        assert!(args.windows(2).any(|w| w == ["-c", "zlib"]), "{:?}", args);
    }

    #[test]
    fn an_iso_forced_to_cd_uses_createcd_instead() {
        let args = args_as_strings(&build_convert_args("iso", true, Path::new("Game.iso"), Path::new("Game.chd")));
        assert_eq!(args[0], "createcd");
    }
}

#[cfg(test)]
mod parse_convert_progress_tests {
    use super::parse_convert_progress;

    #[test]
    fn parses_a_compressing_line() {
        let (phase, percent) = parse_convert_progress("Compressing, 42.9% complete... (ratio=51.1%)  ").expect("should parse");
        assert_eq!(phase, "compressing");
        assert!((percent - 42.9).abs() < 0.001);
    }

    #[test]
    fn parses_a_verifying_line() {
        let (phase, percent) = parse_convert_progress("Verifying, 71.7% complete...").expect("should parse");
        assert_eq!(phase, "verifying");
        assert!((percent - 71.7).abs() < 0.001);
    }

    #[test]
    fn ignores_unrelated_lines() {
        assert_eq!(parse_convert_progress("chdman - MAME Compressed Hunks of Data (CHD) manager 0.289"), None);
        assert_eq!(parse_convert_progress("Compression complete ... final ratio = 60.3%"), None);
    }
}

#[cfg(test)]
mod worker_count_tests {
    use super::worker_count;

    #[test]
    fn never_starts_more_workers_than_queued_discs() {
        assert_eq!(worker_count(0), 1); // an empty queue still gets one worker, which just finds nothing and exits immediately
        assert_eq!(worker_count(1), 1);
    }

    #[test]
    fn caps_at_available_cores_for_a_large_queue() {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        assert_eq!(worker_count(10_000), cores);
    }
}
