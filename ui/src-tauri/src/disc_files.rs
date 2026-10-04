use std::path::{Path, PathBuf};

/// Every file that makes up the disc at `disc_path`: the `.cue`/`.gdi`
/// index itself plus each track file it references, or just the `.iso`.
/// Only files that actually exist are returned, so a malformed or partial
/// dump never makes a caller act on a path that isn't there. Used to size
/// a disc before conversion and to know exactly what to remove afterwards.
pub fn disc_files(disc_path: &Path, kind: &str) -> Vec<PathBuf> {
    let mut files = vec![disc_path.to_path_buf()];
    let referenced = match kind {
        "cue" => std::fs::read_to_string(disc_path)
            .map(|text| cue_track_names(&text))
            .unwrap_or_default(),
        "gdi" => std::fs::read_to_string(disc_path)
            .map(|text| gdi_track_names(&text))
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let dir = disc_path.parent().unwrap_or(Path::new("."));
    for name in referenced {
        let path = dir.join(name);
        if !files.contains(&path) {
            files.push(path);
        }
    }
    files.retain(|p| p.is_file());
    files
}

/// Sends `files` to the system trash (Recycle Bin / Trash / freedesktop
/// trash) rather than deleting them outright, so a mistake stays
/// recoverable. On macOS this uses NSFileManager instead of the crate's
/// default (asking Finder via AppleScript), which would pop up an extra
/// "wants to control Finder" permission prompt.
pub fn move_to_trash(files: &[PathBuf]) -> Result<(), String> {
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    ctx.delete_all(files).map_err(|e| e.to_string())
}

/// Total size in bytes of `files` (missing files count as 0).
pub fn total_size(files: &[PathBuf]) -> u64 {
    files
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

/// Track file names from a `.cue` sheet's `FILE "name" TYPE` lines (the
/// name may also be unquoted when it has no spaces).
fn cue_track_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.get(..5).is_some_and(|kw| kw.eq_ignore_ascii_case("FILE ")) {
                return None;
            }
            let after = line[5..].trim_start();
            if let Some(quoted) = after.strip_prefix('"') {
                quoted.split('"').next().map(str::to_string)
            } else {
                after.split_whitespace().next().map(str::to_string)
            }
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// Track file names from a Dreamcast `.gdi`: a track count line, then one
/// `<n> <lba> <type> <sector size> <file> <offset>` line per track, where
/// `<file>` is quoted when it contains spaces.
fn gdi_track_names(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            if let Some(start) = line.find('"') {
                line[start + 1..].split('"').next().map(str::to_string)
            } else {
                line.split_whitespace().nth(4).map(str::to_string)
            }
        })
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chd_disc_files_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_quoted_and_unquoted_cue_file_lines() {
        let cue = "FILE \"Game (Track 1).bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\nfile Track2.bin BINARY\r\n  TRACK 02 AUDIO\r\n";
        assert_eq!(cue_track_names(cue), vec!["Game (Track 1).bin", "Track2.bin"]);
    }

    #[test]
    fn parses_quoted_and_unquoted_gdi_track_lines() {
        let gdi = "3\r\n1 0 4 2352 track01.bin 0\r\n2 756 0 2352 \"track 02.raw\" 0\r\n3 45000 4 2352 track03.bin 0\r\n";
        assert_eq!(gdi_track_names(gdi), vec!["track01.bin", "track 02.raw", "track03.bin"]);
    }

    #[test]
    fn cue_disc_includes_the_sheet_and_every_existing_track() {
        let dir = temp_dir("cue");
        fs::write(dir.join("Game.cue"), "FILE \"Game (Track 1).bin\" BINARY\nFILE \"Game (Track 2).bin\" BINARY\nFILE \"Missing.bin\" BINARY\n").unwrap();
        fs::write(dir.join("Game (Track 1).bin"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("Game (Track 2).bin"), vec![0u8; 50]).unwrap();
        fs::write(dir.join("Unrelated.bin"), vec![0u8; 999]).unwrap();

        let files = disc_files(&dir.join("Game.cue"), "cue");
        let names: Vec<_> = files.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["Game.cue", "Game (Track 1).bin", "Game (Track 2).bin"]);
        assert_eq!(total_size(&files), fs::metadata(dir.join("Game.cue")).unwrap().len() + 150);
    }

    #[test]
    fn gdi_disc_includes_the_index_and_its_tracks() {
        let dir = temp_dir("gdi");
        fs::write(dir.join("Sonic.gdi"), "2\n1 0 4 2352 track01.bin 0\n2 756 0 2352 track02.raw 0\n").unwrap();
        fs::write(dir.join("track01.bin"), b"a").unwrap();
        fs::write(dir.join("track02.raw"), b"b").unwrap();

        let files = disc_files(&dir.join("Sonic.gdi"), "gdi");
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn iso_disc_is_just_the_iso() {
        let dir = temp_dir("iso");
        fs::write(dir.join("Game.iso"), b"iso").unwrap();
        fs::write(dir.join("Game.cue"), b"not ours").unwrap();
        assert_eq!(disc_files(&dir.join("Game.iso"), "iso"), vec![dir.join("Game.iso")]);
    }
}
