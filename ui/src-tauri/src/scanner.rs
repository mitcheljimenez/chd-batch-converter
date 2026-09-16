use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct ScannedDisc {
    pub name: String,
    pub folder: String,
    pub kind: String,
}

pub fn scan_folder(root: &Path) -> Vec<ScannedDisc> {
    let mut cues: Vec<PathBuf> = Vec::new();
    let mut isos: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().to_path_buf();
        match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) {
            Some(ext) if ext == "cue" => cues.push(path),
            Some(ext) if ext == "iso" => isos.push(path),
            _ => {}
        }
    }

    // Same rule convertir_a_chd.bat itself uses to skip a file at conversion
    // time (an existing <base name>.chd next to it) — applied here too so a
    // file that's already done never shows up as something to convert.
    let already_converted = |p: &Path| p.with_extension("chd").exists();

    let mut results: Vec<ScannedDisc> = cues
        .iter()
        .filter(|p| !already_converted(p))
        .map(|p| ScannedDisc {
            name: p.file_name().unwrap().to_string_lossy().to_string(),
            folder: p.parent().unwrap().to_string_lossy().to_string(),
            kind: "cue".to_string(),
        })
        .collect();

    for iso in &isos {
        let dir = iso.parent().unwrap();
        let stem = iso.file_stem();
        let has_matching_cue = cues
            .iter()
            .any(|c| c.parent().unwrap() == dir && c.file_stem() == stem);
        if !has_matching_cue && !already_converted(iso) {
            results.push(ScannedDisc {
                name: iso.file_name().unwrap().to_string_lossy().to_string(),
                folder: dir.to_string_lossy().to_string(),
                kind: "iso".to_string(),
            });
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn finds_cue_and_bare_iso_but_skips_iso_with_matching_cue_name() {
        let dir = std::env::temp_dir().join(format!("chd_scanner_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        write(&dir.join("GameA/Track.cue"), "FILE \"Track.bin\" BINARY\n");
        write(&dir.join("GameB/Disc.iso"), "fake iso");
        write(&dir.join("GameC/Disc.iso"), "fake iso");
        write(&dir.join("GameC/Disc.cue"), "FILE \"Disc.bin\" BINARY\n");

        let mut results = scan_folder(&dir);
        results.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(results.len(), 3, "GameC's Disc.iso must be excluded (same base name as Disc.cue): {:?}", results);
        assert_eq!(results[0].name, "Disc.cue");
        assert_eq!(results[0].kind, "cue");
        assert_eq!(results[1].name, "Disc.iso");
        assert_eq!(results[1].kind, "iso");
        assert_eq!(results[2].name, "Track.cue");
        assert_eq!(results[2].kind, "cue");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn converts_iso_sharing_a_flat_folder_with_an_unrelated_cue() {
        // Regression test: a flat folder mixing a PS1 game (.cue) and a
        // PS2 game (.iso) with different base names must not suppress the
        // .iso just because some .cue exists in the same directory — only
        // a same-named .cue (i.e. the same game already ripped as cue/bin)
        // should suppress it.
        let dir = std::env::temp_dir().join(format!("chd_scanner_flat_mixed_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        write(&dir.join("Other.cue"), "FILE \"Other.bin\" BINARY\n");
        write(&dir.join("Disc.iso"), "fake iso");

        let mut results = scan_folder(&dir);
        results.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(results.len(), 2, "unrelated cue must not suppress the iso: {:?}", results);
        assert_eq!(results[0].name, "Disc.iso");
        assert_eq!(results[0].kind, "iso");
        assert_eq!(results[1].name, "Other.cue");
        assert_eq!(results[1].kind, "cue");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn excludes_cue_and_iso_that_already_have_a_sibling_chd() {
        let dir = std::env::temp_dir().join(format!("chd_scanner_already_chd_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        write(&dir.join("GameA/Track.cue"), "FILE \"Track.bin\" BINARY\n");
        write(&dir.join("GameA/Track.chd"), "fake chd");
        write(&dir.join("GameB/Disc.iso"), "fake iso");
        write(&dir.join("GameB/Disc.chd"), "fake chd");
        write(&dir.join("GameC/Other.cue"), "FILE \"Other.bin\" BINARY\n");

        let results = scan_folder(&dir);

        assert_eq!(results.len(), 1, "only GameC's still-pending Other.cue should remain: {:?}", results);
        assert_eq!(results[0].name, "Other.cue");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_folder_yields_no_discs() {
        let dir = std::env::temp_dir().join(format!("chd_scanner_empty_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        assert_eq!(scan_folder(&dir), Vec::new());

        fs::remove_dir_all(&dir).unwrap();
    }
}
