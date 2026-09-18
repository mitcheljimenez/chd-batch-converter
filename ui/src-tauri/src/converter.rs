use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

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
    let use_cd = should_use_cd(kind, force_cd);

    let mut convert_args: Vec<std::ffi::OsString> = Vec::new();
    if use_cd {
        convert_args.push("createcd".into());
    } else {
        // createdvd defaults to zstd compression, unreadable by
        // AetherSX2/NetherSX2 on Android -- -c zlib keeps DVD CHDs portable
        // there. Same rationale as convertir_a_chd.bat's own :process_disc.
        convert_args.push("createdvd".into());
        convert_args.push("-c".into());
        convert_args.push("zlib".into());
    }
    convert_args.push("-i".into());
    convert_args.push(disc_path.as_os_str().to_owned());
    convert_args.push("-o".into());
    convert_args.push(out.as_os_str().to_owned());

    run_with_progress(chdman_path, &convert_args, active_pids, &mut on_progress)
        .map_err(|_| "CONVERT_FAILED".to_string())?;

    let verify_args: Vec<std::ffi::OsString> = vec!["verify".into(), "-i".into(), out.as_os_str().to_owned()];
    run_with_progress(chdman_path, &verify_args, active_pids, &mut on_progress)
        .map_err(|_| "CONVERT_VERIFY_FAILED".to_string())?;

    Ok(out)
}

/// Runs `chdman` with `args`, streaming its stdout line-by-line (splitting
/// on chdman's bare '\r' progress-overwrite trick, same as
/// `extractor::run_with_progress`) and calling `on_progress(phase, percent)`
/// for every "Compressing, X% complete..."/"Verifying, X% complete..." line.
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
        .stderr(Stdio::null())
        .creation_flags(crate::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| ())?;

    let pid = child.id();
    active_pids.lock().unwrap().push(pid);

    let mut stdout = child.stdout.take().ok_or(())?;
    let mut partial = String::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stdout.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        partial.push_str(&String::from_utf8_lossy(&chunk[..n]));
        let normalized = partial.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<&str> = normalized.split('\n').collect();
        let tail = lines.pop().unwrap_or("").to_string();
        for line in lines {
            if let Some((phase, percent)) = parse_convert_progress(line) {
                on_progress(phase, percent);
            }
        }
        partial = tail;
    }

    let status = child.wait().map_err(|_| ())?;
    active_pids.lock().unwrap().retain(|&p| p != pid);

    if status.success() {
        Ok(())
    } else {
        Err(())
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
