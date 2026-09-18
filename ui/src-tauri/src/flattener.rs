use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(serde::Serialize, Clone, Default)]
pub struct FlattenSummary {
    pub files_moved: u32,
    pub folders_removed: u32,
    pub renamed_due_to_collision: u32,
}

/// A folder is left untouched (not descended into, not emptied) when its own
/// name contains ".m3u" anywhere -- these are the per-game folders
/// `organize_multidisc` builds for multi-disc sets, and their discs must stay
/// grouped together rather than getting scattered back into `root` by this
/// unrelated cleanup pass.
fn is_multidisc_folder(name: &str) -> bool {
    name.to_lowercase().contains(".m3u")
}

/// Never descends into (or yields) a directory whose own name is a
/// multi-disc marker -- shared between the move pass and the empty-folder
/// cleanup pass below so both agree on what's off-limits.
fn skip_multidisc_dirs(entry: &walkdir::DirEntry) -> bool {
    !entry.file_type().is_dir()
        || !entry
            .file_name()
            .to_str()
            .map(is_multidisc_folder)
            .unwrap_or(false)
}

/// Resolves a non-colliding path for `file_name` directly under
/// `destination`: the plain join if nothing is there yet, otherwise a
/// " (2)", " (3)", ... suffix inserted before the extension -- same
/// approach as `chd_mover::move_chd_files`, generalized to any extension
/// (or none) since flattening isn't limited to `.chd` files.
fn unique_dest_path(destination: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let dest_path = destination.join(file_name);
    if !dest_path.exists() {
        return dest_path;
    }

    let name_path = Path::new(file_name);
    let stem = name_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let extension = name_path.extension().map(|e| e.to_string_lossy().to_string());

    let mut n = 2;
    loop {
        let candidate_name = match &extension {
            Some(ext) => format!("{} ({}).{}", stem, n, ext),
            None => format!("{} ({})", stem, n),
        };
        let candidate = destination.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Moves every file sitting inside a subfolder of `root` directly into
/// `root` itself (flattening one or more levels of "<Game>/<rom file>"
/// nesting down to just "<rom file>"), then removes whatever subfolders end
/// up empty as a result. Any folder whose name contains ".m3u" is skipped
/// entirely -- not descended into, not touched, not removed -- since those
/// are `organize_multidisc`'s multi-disc game folders and must keep their
/// discs grouped together rather than being flattened back into `root`.
///
/// Two passes, same reasoning as `organizer::organize_multidisc` and
/// `chd_mover::move_chd_files`: collect every file to move first, then move
/// them, rather than moving files while still walking `root` -- a
/// single-pass walk could revisit a just-moved file as if it were a new
/// candidate, or trip over a directory that became empty mid-walk.
/// Similarly, folders are only ever removed in a separate pass after every
/// move has already happened.
pub fn flatten_folders(root: &Path) -> Result<FlattenSummary, String> {
    let mut summary = FlattenSummary::default();

    let mut files_to_move = Vec::new();
    for entry in WalkDir::new(root).min_depth(1).into_iter().filter_entry(skip_multidisc_dirs) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        // Already directly under `root` -- nothing to flatten.
        if path.parent() == Some(root) {
            continue;
        }
        files_to_move.push(path.to_path_buf());
    }

    for path in files_to_move {
        let file_name = match path.file_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let dest_path = unique_dest_path(root, &file_name);
        if dest_path != root.join(&file_name) {
            summary.renamed_due_to_collision += 1;
        }
        fs::rename(&path, &dest_path).map_err(|e| e.to_string())?;
        summary.files_moved += 1;
    }

    // Deepest folders first, so a parent that only became empty because its
    // last child subfolder was just removed still gets cleaned up in the
    // same pass instead of being left behind as a now-pointless empty shell.
    let mut dirs: Vec<PathBuf> = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(skip_multidisc_dirs)
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .map(|e| e.path().to_path_buf())
        .collect();
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    for dir in dirs {
        let is_empty = fs::read_dir(&dir).map(|mut it| it.next().is_none()).unwrap_or(false);
        if is_empty && fs::remove_dir(&dir).is_ok() {
            summary.folders_removed += 1;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chd_flattener_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn moves_a_rom_out_of_its_game_folder_and_removes_the_now_empty_folder() {
        let root = temp_dir("basic");
        let game_folder = root.join("Super Game");
        fs::create_dir_all(&game_folder).unwrap();
        fs::write(game_folder.join("Super Game.zip"), b"rom").unwrap();

        let summary = flatten_folders(&root).unwrap();

        assert_eq!(summary.files_moved, 1);
        assert_eq!(summary.folders_removed, 1);
        assert!(root.join("Super Game.zip").exists());
        assert!(!game_folder.exists());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn flattens_nested_subfolders_directly_into_root() {
        let root = temp_dir("nested");
        let nested = root.join("SNES").join("Chrono Trigger");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Chrono Trigger.sfc"), b"rom").unwrap();

        let summary = flatten_folders(&root).unwrap();

        assert_eq!(summary.files_moved, 1);
        assert!(root.join("Chrono Trigger.sfc").exists());
        assert!(!root.join("SNES").exists());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn leaves_a_folder_with_m3u_in_its_name_completely_untouched() {
        let root = temp_dir("multidisc");
        let multidisc_folder = root.join("Chrono Cross.m3u");
        fs::create_dir_all(&multidisc_folder).unwrap();
        fs::write(multidisc_folder.join("Chrono Cross (Disc 1).chd"), b"disc1").unwrap();
        fs::write(multidisc_folder.join("Chrono Cross (Disc 2).chd"), b"disc2").unwrap();
        fs::write(multidisc_folder.join("Chrono Cross.m3u"), b"playlist").unwrap();

        let summary = flatten_folders(&root).unwrap();

        assert_eq!(summary.files_moved, 0);
        assert_eq!(summary.folders_removed, 0);
        assert!(multidisc_folder.join("Chrono Cross (Disc 1).chd").exists());
        assert!(multidisc_folder.join("Chrono Cross (Disc 2).chd").exists());
        assert!(!root.join("Chrono Cross (Disc 1).chd").exists());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn leaves_files_already_directly_in_root_untouched() {
        let root = temp_dir("already_flat");
        fs::write(root.join("Loose Game.zip"), b"rom").unwrap();

        let summary = flatten_folders(&root).unwrap();

        assert_eq!(summary.files_moved, 0);
        assert!(root.join("Loose Game.zip").exists());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn renames_on_filename_collision_instead_of_overwriting() {
        let root = temp_dir("collision");
        fs::create_dir_all(root.join("RegionA")).unwrap();
        fs::create_dir_all(root.join("RegionB")).unwrap();
        fs::write(root.join("RegionA").join("Game.zip"), b"region a content").unwrap();
        fs::write(root.join("RegionB").join("Game.zip"), b"region b content").unwrap();

        let summary = flatten_folders(&root).unwrap();

        assert_eq!(summary.files_moved, 2);
        assert_eq!(summary.renamed_due_to_collision, 1);
        assert!(root.join("Game.zip").exists());
        assert!(root.join("Game (2).zip").exists());
        let contents: Vec<String> = vec![
            fs::read_to_string(root.join("Game.zip")).unwrap(),
            fs::read_to_string(root.join("Game (2).zip")).unwrap(),
        ];
        assert!(contents.contains(&"region a content".to_string()));
        assert!(contents.contains(&"region b content".to_string()));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn does_not_remove_a_folder_that_still_has_leftover_content() {
        // A subfolder that (for whatever reason) also contains its own
        // subfolder with an .m3u name must not be removed -- it isn't
        // actually empty, since that nested folder (and its contents) is
        // deliberately left alone.
        let root = temp_dir("leftover");
        let parent = root.join("Collection");
        let multidisc = parent.join("Game.m3u");
        fs::create_dir_all(&multidisc).unwrap();
        fs::write(multidisc.join("Game (Disc 1).chd"), b"disc1").unwrap();
        fs::write(parent.join("Loose.zip"), b"rom").unwrap();

        let summary = flatten_folders(&root).unwrap();

        assert_eq!(summary.files_moved, 1);
        assert!(root.join("Loose.zip").exists());
        // "Collection" still holds "Game.m3u/", so it must survive.
        assert!(parent.exists());
        assert!(multidisc.join("Game (Disc 1).chd").exists());

        fs::remove_dir_all(&root).unwrap();
    }
}
