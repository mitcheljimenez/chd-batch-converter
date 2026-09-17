use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct ScannedChd {
    pub name: String,
    pub folder: String,
    pub kind: String,
}

/// Recursively finds every `.chd` under `root` and classifies each as
/// "cd", "dvd", or "unknown" by running `chdman info` on it (see
/// `classify_chd_info`).
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
        results.push(ScannedChd {
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            folder: path.parent().unwrap().to_string_lossy().to_string(),
            kind,
        });
    }
    results
}

/// Extracts `chd_path` back to its original format next to itself: `.cue`
/// + `.bin` for a CD-type CHD, or `.iso` for a DVD-type CHD. Never
/// overwrites an existing output file -- refuses before spawning chdman at
/// all, since `kind` must already be known via `scan_chds`/
/// `classify_chd_info` rather than guessed here (chdman's extractors don't
/// validate the input CHD's type, and would silently write garbage if run
/// against the wrong one).
pub fn extract_chd(chdman_path: &Path, chd_path: &Path, kind: &str) -> Result<PathBuf, String> {
    match kind {
        "cd" => {
            let cue = chd_path.with_extension("cue");
            let bin = chd_path.with_extension("bin");
            if cue.exists() {
                return Err(format!("EXTRACT_DEST_EXISTS:{}", cue.display()));
            }
            if bin.exists() {
                return Err(format!("EXTRACT_DEST_EXISTS:{}", bin.display()));
            }
            run_extract(chdman_path, "extractcd", chd_path, &cue)?;
            Ok(cue)
        }
        "dvd" => {
            let iso = chd_path.with_extension("iso");
            if iso.exists() {
                return Err(format!("EXTRACT_DEST_EXISTS:{}", iso.display()));
            }
            run_extract(chdman_path, "extractdvd", chd_path, &iso)?;
            Ok(iso)
        }
        _ => Err("EXTRACT_UNKNOWN_FORMAT".to_string()),
    }
}

fn run_extract(chdman_path: &Path, subcommand: &str, chd_path: &Path, output: &Path) -> Result<(), String> {
    let status = Command::new(chdman_path)
        .arg(subcommand)
        .arg("-i")
        .arg(chd_path)
        .arg("-o")
        .arg(output)
        .creation_flags(crate::CREATE_NO_WINDOW)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("EXTRACT_FAILED".to_string());
    }
    Ok(())
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
        // if extract_chd tried to spawn it, this would fail with a spawn
        // error instead of the unknown-format error.
        let bogus_chdman = dir.join("does_not_exist.exe");

        let result = extract_chd(&bogus_chdman, &chd, "unknown");

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

        let result = extract_chd(&bogus_chdman, &chd, "cd");

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

        let result = extract_chd(&bogus_chdman, &chd, "dvd");

        assert!(matches!(result, Err(ref msg) if msg.starts_with("EXTRACT_DEST_EXISTS:")));
        fs::remove_dir_all(&dir).unwrap();
    }
}
