use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use walkdir::WalkDir;

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct ScannedChd {
    pub name: String,
    pub folder: String,
    pub kind: String,
}

/// Recursively finds every `.chd` under `root` (including nested
/// subfolders) that hasn't already been extracted, and classifies each as
/// "cd", "dvd", or "unknown" by running `chdman info` on it (see
/// `classify_chd_info`). "Already extracted" means a `.iso` (dvd) or
/// `.cue`/`.bin` (cd) sibling already sits next to it -- `extract_chd_with_
/// progress` would refuse those anyway (`EXTRACT_DEST_EXISTS`), so leaving
/// them out keeps the list to files actually worth acting on, the same way
/// `scanner::scan_folder` already excludes a `.cue`/`.iso` that already has
/// a sibling `.chd`. Caller must have already verified `chdman_path` exists
/// (as `resolve_chdman_path` in lib.rs does before this is ever called): a
/// missing/misconfigured chdman would otherwise make `chdman info` fail to
/// spawn for every single `.chd` found, silently collapsing the whole scan
/// to "unknown" instead of surfacing the real problem.
pub fn scan_chds(root: &Path, chdman_path: &Path) -> Vec<ScannedChd> {
    let mut results = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_chd = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("chd"))
            .unwrap_or(false);
        if !is_chd {
            continue;
        }
        let kind = Command::new(chdman_path)
            .args(["info", "-i"])
            .arg(path)
            .creation_flags(crate::CREATE_NO_WINDOW)
            .output()
            .map(|out| classify_chd_info(&String::from_utf8_lossy(&out.stdout)).to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        if already_extracted(path, &kind) {
            continue;
        }
        results.push(ScannedChd {
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            folder: path.parent().unwrap().to_string_lossy().to_string(),
            kind,
        });
    }
    results
}

/// Whether `chd_path`'s extraction target(s) already exist next to it: a
/// `.iso` for a dvd-kind CHD, or a `.cue`/`.bin` for a cd-kind one (either
/// alone counts -- `extract_chd_with_progress` refuses as soon as one of
/// the pair exists). An "unknown" kind has no defined target, so it's never
/// considered already-extracted here.
fn already_extracted(chd_path: &Path, kind: &str) -> bool {
    match kind {
        "dvd" => chd_path.with_extension("iso").exists(),
        "cd" => chd_path.with_extension("cue").exists() || chd_path.with_extension("bin").exists(),
        _ => false,
    }
}

/// Extracts `chd_path` back to its original format next to itself: `.cue`
/// + `.bin` for a CD-type CHD, or `.iso` for a DVD-type CHD, then runs
/// `chdman verify` against the source `.chd` to confirm it isn't corrupt
/// (the same check `createcd`/`createdvd` already do after compressing --
/// extraction had no equivalent safety net before this). Never overwrites
/// an existing output file -- refuses before spawning chdman at all, since
/// `kind` must already be known via `scan_chds`/`classify_chd_info` rather
/// than guessed here (chdman's extractors don't validate the input CHD's
/// type, and would silently write garbage if run against the wrong one).
///
/// `on_progress` is called for every "Extracting, X%"/"Verifying, X%" line
/// chdman prints, so a caller can drive a live progress bar instead of
/// blocking silently for the whole operation (chdman is not asked to run
/// concurrently with anything else here -- the caller is expected to run
/// this from a background thread if it must keep its own UI thread free,
/// the same way `start_conversion` runs the batch script from one).
pub fn extract_chd_with_progress(
    chdman_path: &Path,
    chd_path: &Path,
    kind: &str,
    mut on_progress: impl FnMut(&str, f32),
) -> Result<PathBuf, String> {
    let output = match kind {
        "cd" => {
            let cue = chd_path.with_extension("cue");
            let bin = chd_path.with_extension("bin");
            if cue.exists() {
                return Err(format!("EXTRACT_DEST_EXISTS:{}", cue.display()));
            }
            if bin.exists() {
                return Err(format!("EXTRACT_DEST_EXISTS:{}", bin.display()));
            }
            run_with_progress(chdman_path, "extractcd", chd_path, Some(&cue), &mut on_progress)
                .map_err(|_| "EXTRACT_FAILED".to_string())?;
            cue
        }
        "dvd" => {
            let iso = chd_path.with_extension("iso");
            if iso.exists() {
                return Err(format!("EXTRACT_DEST_EXISTS:{}", iso.display()));
            }
            run_with_progress(chdman_path, "extractdvd", chd_path, Some(&iso), &mut on_progress)
                .map_err(|_| "EXTRACT_FAILED".to_string())?;
            iso
        }
        _ => return Err("EXTRACT_UNKNOWN_FORMAT".to_string()),
    };

    run_with_progress(chdman_path, "verify", chd_path, None, &mut on_progress)
        .map_err(|_| "EXTRACT_VERIFY_FAILED".to_string())?;

    Ok(output)
}

/// Runs `chdman <subcommand> -i chd_path [-o output]`, streaming its output
/// line-by-line (splitting on chdman's bare '\r' progress-overwrite trick)
/// and calling `on_progress(phase, percent)` for every "Extracting, X%
/// complete..." or "Verifying, X% complete..." line it prints. chdman writes
/// its progress lines to **stderr**, flushing on every call (per its own
/// source: `progress()` writes to `std::cerr`) -- stdout only carries the
/// occasional summary line, so both streams are piped and read on their own
/// thread, feeding a shared channel this function drains on the caller's
/// thread (since `on_progress` closes over a `tauri::Window` and isn't
/// required to be `Send`). Returns `Err(())` on a spawn failure or non-zero
/// exit; the caller maps that to the specific stable error code for its
/// context (extraction vs. verify).
fn run_with_progress(
    chdman_path: &Path,
    subcommand: &str,
    chd_path: &Path,
    output: Option<&Path>,
    on_progress: &mut impl FnMut(&str, f32),
) -> Result<(), ()> {
    let mut command = Command::new(chdman_path);
    command.arg(subcommand).arg("-i").arg(chd_path);
    if let Some(out_path) = output {
        command.arg("-o").arg(out_path);
    }

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(crate::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| ())?;

    let stdout = child.stdout.take().ok_or(())?;
    let stderr = child.stderr.take().ok_or(())?;

    let (tx, rx) = mpsc::channel();
    let tx_stderr = tx.clone();
    let stdout_reader = thread::spawn(move || stream_phase_progress(stdout, tx));
    let stderr_reader = thread::spawn(move || stream_phase_progress(stderr, tx_stderr));

    for (phase, percent) in rx {
        on_progress(phase, percent);
    }

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();

    let status = child.wait().map_err(|_| ())?;
    if status.success() {
        Ok(())
    } else {
        Err(())
    }
}

/// Reads `reader` to EOF, parsing out "Extracting/Verifying, X% complete"
/// lines and sending each as `(phase, percent)` through `tx`. Runs on its
/// own thread (see `run_with_progress`) so it never has to wait its turn
/// behind the other stream.
fn stream_phase_progress(mut reader: impl Read, tx: mpsc::Sender<(&'static str, f32)>) {
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
        // The last element is whatever came after the final '\n' (possibly
        // empty) -- an incomplete line still being written, held back for
        // the next read.
        let tail = lines.pop().unwrap_or("").to_string();
        for line in lines {
            if let Some(parsed) = parse_phase_progress(line) {
                let _ = tx.send(parsed);
            }
        }
        partial = tail;
    }
}

/// Parses chdman's own progress output for extraction/verification, e.g.
/// "Extracting, 42.9% complete... " or "Verifying, 71.7% complete...".
fn parse_phase_progress(line: &str) -> Option<(&'static str, f32)> {
    let trimmed = line.trim_start();
    let (phase, rest) = if let Some(r) = trimmed.strip_prefix("Extracting, ") {
        ("extracting", r)
    } else if let Some(r) = trimmed.strip_prefix("Verifying, ") {
        ("verifying", r)
    } else {
        return None;
    };
    let percent_str = rest.split('%').next()?;
    let percent: f32 = percent_str.trim().parse().ok()?;
    Some((phase, percent))
}

/// Classifies a `chdman info` invocation's stdout as `"cd"` (created via
/// `createcd`, carries CD track metadata), `"dvd"` (created via `createdvd`,
/// carries a bare DVD tag), or `"unknown"` (neither pattern found -- not a
/// CHD chdman recognizes, or a format this app doesn't handle). This must be
/// known upfront before calling `extractcd`/`extractdvd`: those commands
/// don't validate that the input CHD matches, and silently write garbage
/// output if run against the wrong kind.
pub fn classify_chd_info(info_output: &str) -> &'static str {
    if info_output.contains("Tag='CHT2'") || info_output.contains("Tag='CHTR'") {
        "cd"
    } else if info_output.contains("Tag='DVD '") {
        "dvd"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod already_extracted_tests {
    use super::already_extracted;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chd_already_extracted_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dvd_is_already_extracted_when_a_sibling_iso_exists() {
        let dir = temp_dir("dvd_iso_exists");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();
        fs::write(dir.join("Game.iso"), b"existing iso").unwrap();

        assert!(already_extracted(&chd, "dvd"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dvd_is_not_already_extracted_without_a_sibling_iso() {
        let dir = temp_dir("dvd_no_iso");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();

        assert!(!already_extracted(&chd, "dvd"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cd_is_already_extracted_when_only_a_sibling_cue_exists() {
        let dir = temp_dir("cd_cue_only");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();
        fs::write(dir.join("Game.cue"), b"existing cue").unwrap();

        assert!(already_extracted(&chd, "cd"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cd_is_already_extracted_when_only_a_sibling_bin_exists() {
        let dir = temp_dir("cd_bin_only");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();
        fs::write(dir.join("Game.bin"), b"existing bin").unwrap();

        assert!(already_extracted(&chd, "cd"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cd_is_not_already_extracted_without_cue_or_bin() {
        let dir = temp_dir("cd_neither");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();

        assert!(!already_extracted(&chd, "cd"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_kind_is_never_considered_already_extracted() {
        let dir = temp_dir("unknown_kind");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();
        fs::write(dir.join("Game.iso"), b"existing iso").unwrap();

        assert!(!already_extracted(&chd, "unknown"));
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod classify_tests {
    use super::classify_chd_info;

    // Captured verbatim from `chdman info` (0.289) against a real CD-type
    // CHD produced by `createcd`.
    const CD_INFO: &str = "chdman - MAME Compressed Hunks of Data (CHD) manager 0.289 (mame0289)\nInput file:   test_cd.chd\nFile Version: 5\nLogical size: 244,800 bytes\nHunk Size:    19,584 bytes\nTotal Hunks:  13\nUnit Size:    2,448 bytes\nTotal Units:  100\nCompression:  cdlz (CD LZMA), cdzl (CD Deflate), cdfl (CD FLAC)\nCHD size:     293 bytes\nRatio:        0.1%\nSHA1:         1135febcf39f68f8ee90df246c0bb0f31517f53c\nData SHA1:    0288a92a2ce4f642d981dbe146a897dc31e659ac\nMetadata:     Tag='CHT2'  Index=0  Length=86 bytes\n              TRACK:1 TYPE:MODE1 SUBTYPE:NONE FRAMES:100 PREGAP:0 PGTYPE:M\n";

    // Captured verbatim from `chdman info` against a real DVD-type CHD
    // produced by `createdvd -c zlib`.
    const DVD_INFO: &str = "chdman - MAME Compressed Hunks of Data (CHD) manager 0.289 (mame0289)\nInput file:   test_dvd.chd\nFile Version: 5\nLogical size: 204,800 bytes\nHunk Size:    4,096 bytes\nTotal Hunks:  50\nUnit Size:    2,048 bytes\nTotal Units:  100\nCompression:  zlib (Deflate)\nCHD size:     188 bytes\nRatio:        0.1%\nSHA1:         8f206ee70c071466ee32ab91976cd519e5ae169d\nData SHA1:    e125b3b7525bd2a65d9bc7591f6cfedaf57a034a\nMetadata:     Tag='DVD '  Index=0  Length=1 bytes\n              .\n";

    const MALFORMED_INFO: &str = "chdman - MAME Compressed Hunks of Data (CHD) manager 0.289 (mame0289)\nError opening file: NOT A CHD\n";

    #[test]
    fn detects_cd_from_track_metadata_tag() {
        assert_eq!(classify_chd_info(CD_INFO), "cd");
    }

    #[test]
    fn detects_dvd_from_dvd_metadata_tag() {
        assert_eq!(classify_chd_info(DVD_INFO), "dvd");
    }

    #[test]
    fn reports_unknown_for_unrecognized_output() {
        assert_eq!(classify_chd_info(MALFORMED_INFO), "unknown");
    }

    #[test]
    fn reports_unknown_for_empty_output() {
        assert_eq!(classify_chd_info(""), "unknown");
    }
}

#[cfg(test)]
mod extract_chd_tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chd_extractor_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn refuses_unknown_kind_without_spawning_chdman() {
        let dir = temp_dir("unknown_kind");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();
        // A nonexistent chdman path proves no subprocess was attempted --
        // if extract_chd_with_progress tried to spawn it, this would fail
        // with a spawn error instead of the unknown-format error.
        let bogus_chdman = dir.join("does_not_exist.exe");

        let result = extract_chd_with_progress(&bogus_chdman, &chd, "unknown", |_, _| {});

        assert_eq!(result, Err("EXTRACT_UNKNOWN_FORMAT".to_string()));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_cd_extract_when_cue_already_exists() {
        let dir = temp_dir("cue_exists");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();
        fs::write(dir.join("Game.cue"), b"existing cue").unwrap();
        let bogus_chdman = dir.join("does_not_exist.exe");

        let result = extract_chd_with_progress(&bogus_chdman, &chd, "cd", |_, _| {});

        assert!(matches!(result, Err(ref msg) if msg.starts_with("EXTRACT_DEST_EXISTS:")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_dvd_extract_when_iso_already_exists() {
        let dir = temp_dir("iso_exists");
        let chd = dir.join("Game.chd");
        fs::write(&chd, b"fake").unwrap();
        fs::write(dir.join("Game.iso"), b"existing iso").unwrap();
        let bogus_chdman = dir.join("does_not_exist.exe");

        let result = extract_chd_with_progress(&bogus_chdman, &chd, "dvd", |_, _| {});

        assert!(matches!(result, Err(ref msg) if msg.starts_with("EXTRACT_DEST_EXISTS:")));
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod parse_phase_progress_tests {
    use super::parse_phase_progress;

    #[test]
    fn parses_an_extracting_line() {
        let (phase, percent) = parse_phase_progress("Extracting, 42.9% complete... ").expect("should parse");
        assert_eq!(phase, "extracting");
        assert!((percent - 42.9).abs() < 0.001);
    }

    #[test]
    fn parses_a_verifying_line() {
        let (phase, percent) = parse_phase_progress("Verifying, 71.7% complete...").expect("should parse");
        assert_eq!(phase, "verifying");
        assert!((percent - 71.7).abs() < 0.001);
    }

    #[test]
    fn ignores_unrelated_lines() {
        assert_eq!(parse_phase_progress("chdman - MAME Compressed Hunks of Data (CHD) manager 0.289"), None);
        assert_eq!(parse_phase_progress("Extraction complete"), None);
    }
}
