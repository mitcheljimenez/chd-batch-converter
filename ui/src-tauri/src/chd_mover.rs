use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(serde::Serialize, Clone, Default)]
pub struct MoveChdSummary {
    pub files_moved: u32,
    pub renamed_due_to_collision: u32,
}

/// Recursively finds every `.chd` under `root` (however deeply nested) and
/// moves it directly into `destination`, separating it from whatever
/// `.bin`/`.cue`/`.iso` siblings it was converted from -- those are left
/// exactly where they were, untouched.
///
/// Output is always flattened to one level under `destination`, the same
/// way `organize_multidisc` flattens its own output: the point is to end up
/// with every finished `.chd` in one place, not scattered across however the
/// source collection happens to be organized.
///
/// A name collision at the destination (two different source folders each
/// holding a same-named `.chd`, e.g. two regions of the same game) is never
/// silently overwritten -- it's renamed with a " (2)", " (3)", ... suffix,
/// since the two files are not guaranteed to be the same content.
pub fn move_chd_files(root: &Path, destination: &Path) -> Result<MoveChdSummary, String> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;

    // Deliberately NOT canonicalized: WalkDir yields paths built by joining
    // onto `root` exactly as given, so comparing against a canonicalized
    // `destination` (which on Windows gains a `\\?\` prefix) would silently
    // never match, and a second run would "rediscover" files it already
    // moved into `destination` as if they were new candidates.
    let destination = destination.to_path_buf();

    // Two passes, same reasoning as organize_multidisc: collect every match
    // first, then move -- moving files into `destination` while still
    // walking `root` would be actively dangerous here, since `destination`
    // can itself sit inside `root` and a single-pass walk could re-visit
    // files it just moved in as if they were new candidates.
    let mut chd_paths = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        // Never pick up a `.chd` already sitting inside `destination` --
        // relevant when `destination` is a subfolder of `root` and this
        // runs a second time.
        if path.starts_with(&destination) {
            continue;
        }
        let is_chd = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("chd"))
            .unwrap_or(false);
        if is_chd {
            chd_paths.push(path.to_path_buf());
        }
    }

    let mut summary = MoveChdSummary::default();
    for path in chd_paths {
        let file_name = match path.file_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let mut dest_path = destination.join(&file_name);

        if dest_path.exists() {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut n = 2;
            loop {
                let candidate = destination.join(format!("{} ({}).chd", stem, n));
                if !candidate.exists() {
                    dest_path = candidate;
                    break;
                }
                n += 1;
            }
            summary.renamed_due_to_collision += 1;
        }

        fs::rename(&path, &dest_path).map_err(|e| e.to_string())?;
        summary.files_moved += 1;
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("chd_mover_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn moves_chd_files_from_nested_folders_into_flat_destination() {
        let root = temp_dir("nested");
        let destination = temp_dir("nested_dest");
        let nested = root.join("psx").join("GameA");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Track.chd"), b"chd content").unwrap();

        let summary = move_chd_files(&root, &destination).unwrap();

        assert_eq!(summary.files_moved, 1);
        assert!(destination.join("Track.chd").exists());
        assert!(!nested.join("Track.chd").exists());

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn leaves_bin_cue_iso_siblings_untouched() {
        let root = temp_dir("siblings");
        let destination = temp_dir("siblings_dest");
        fs::write(root.join("Track.chd"), b"chd").unwrap();
        fs::write(root.join("Track.cue"), b"cue").unwrap();
        fs::write(root.join("Track.bin"), b"bin").unwrap();
        fs::write(root.join("Other.iso"), b"iso").unwrap();

        let summary = move_chd_files(&root, &destination).unwrap();

        assert_eq!(summary.files_moved, 1);
        assert!(destination.join("Track.chd").exists());
        assert!(root.join("Track.cue").exists());
        assert!(root.join("Track.bin").exists());
        assert!(root.join("Other.iso").exists());

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn renames_on_filename_collision_instead_of_overwriting() {
        let root = temp_dir("collision");
        let destination = temp_dir("collision_dest");
        fs::create_dir_all(root.join("RegionA")).unwrap();
        fs::create_dir_all(root.join("RegionB")).unwrap();
        fs::write(root.join("RegionA").join("Game.chd"), b"region a content").unwrap();
        fs::write(root.join("RegionB").join("Game.chd"), b"region b content").unwrap();

        let summary = move_chd_files(&root, &destination).unwrap();

        assert_eq!(summary.files_moved, 2);
        assert_eq!(summary.renamed_due_to_collision, 1);
        assert!(destination.join("Game.chd").exists());
        assert!(destination.join("Game (2).chd").exists());
        // Neither original's content was clobbered.
        let contents: Vec<String> = vec![
            fs::read_to_string(destination.join("Game.chd")).unwrap(),
            fs::read_to_string(destination.join("Game (2).chd")).unwrap(),
        ];
        assert!(contents.contains(&"region a content".to_string()));
        assert!(contents.contains(&"region b content".to_string()));

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn destination_can_be_a_subfolder_of_root_without_reprocessing_itself() {
        let root = temp_dir("dest_inside_root");
        let destination = root.join("CHDs");
        fs::write(root.join("Track.chd"), b"chd content").unwrap();

        let summary = move_chd_files(&root, &destination).unwrap();
        assert_eq!(summary.files_moved, 1);
        assert!(destination.join("Track.chd").exists());

        // Re-running must not pick the already-moved file back up (it isn't
        // there anymore) nor error out over destination existing.
        let second = move_chd_files(&root, &destination).unwrap();
        assert_eq!(second.files_moved, 0);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn creates_destination_folder_if_missing() {
        let root = temp_dir("create_dest");
        let destination = temp_dir("create_dest_target");
        fs::remove_dir_all(&destination).unwrap(); // must not exist yet
        fs::write(root.join("Track.chd"), b"chd content").unwrap();

        move_chd_files(&root, &destination).unwrap();

        assert!(destination.join("Track.chd").exists());
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn empty_folder_yields_no_moves() {
        let root = temp_dir("empty");
        let destination = temp_dir("empty_dest");

        let summary = move_chd_files(&root, &destination).unwrap();
        assert_eq!(summary.files_moved, 0);

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }
}
