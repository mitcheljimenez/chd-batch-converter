use std::path::Path;
use walkdir::WalkDir;

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct ScannedChdFile {
    pub name: String,
    pub folder: String,
}

/// Every `.chd` under `root` (any depth), sorted by path so the list is
/// stable between scans. Unlike `extractor::scan_chds` this doesn't run
/// `chdman info` on each file: verifying doesn't need to know the format,
/// so a large library lists instantly.
pub fn scan_all_chds(root: &Path) -> Vec<ScannedChdFile> {
    let mut paths: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension()
                .map(|ext| ext.eq_ignore_ascii_case("chd"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| ScannedChdFile {
            name: p.file_name().unwrap().to_string_lossy().to_string(),
            folder: p.parent().unwrap().to_string_lossy().to_string(),
        })
        .collect()
}

/// Runs `chdman verify` on one `.chd`, which re-reads every hunk and checks
/// it against the SHA-1 stored in the file -- the way to catch a copy that
/// got corrupted after it was made (a bad transfer, a failing drive or SD
/// card). Streams "Verifying, X%" through `on_progress`.
pub fn verify_chd(chdman_path: &Path, chd_path: &Path, mut on_progress: impl FnMut(&str, f32)) -> Result<(), String> {
    crate::extractor::run_with_progress(chdman_path, "verify", chd_path, None, &mut on_progress)
        .map_err(|_| "VERIFY_FAILED".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_every_chd_at_any_depth_case_insensitively() {
        let dir = std::env::temp_dir().join(format!("chd_verifier_scan_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("PS1/Crash")).unwrap();
        fs::write(dir.join("PS1/Crash/Crash.chd"), "a").unwrap();
        fs::write(dir.join("Top.CHD"), "b").unwrap();
        fs::write(dir.join("PS1/Crash/Crash.cue"), "c").unwrap();

        let found = scan_all_chds(&dir);
        let names: Vec<_> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["Crash.chd", "Top.CHD"]);

        let _ = fs::remove_dir_all(&dir);
    }
}
