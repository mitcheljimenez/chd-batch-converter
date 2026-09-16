use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// File types a multi-disc game might be stored as. Matches the set the
/// original ES-DE Multi-Disc ROM Organizer project supported.
const DISC_EXTENSIONS: &[&str] = &["chd", "iso", "cue", "bin", "pbp", "rvz", "zip"];

#[derive(serde::Serialize, Clone, Default)]
pub struct OrganizeSummary {
    pub games_organized: u32,
    pub files_moved: u32,
    pub skipped_already_organized: u32,
}

/// Finds a disc-number marker ("Disc 1", "(Disk 2)", "CD 3", case-insensitive)
/// in a filename stem and returns the byte span it occupies (including a
/// wrapping "(" / ")" pair, if present) plus the parsed number.
///
/// Scans for the keyword anywhere in the stem rather than assuming it's the
/// last parenthesized group — the original project split on the first "("
/// instead, which mis-detects the base name for a title like
/// "Game (USA) (Disc 1)" (it would keep only "Game" and drop "(USA)").
fn find_disc_marker(stem: &str) -> Option<(usize, usize, u32)> {
    let lower = stem.to_lowercase();
    let bytes = lower.as_bytes();

    for keyword in ["disc", "disk", "cd"] {
        let mut search_from = 0;
        while let Some(rel_idx) = lower[search_from..].find(keyword) {
            let keyword_start = search_from + rel_idx;
            let after_keyword = keyword_start + keyword.len();

            let mut i = after_keyword;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            let digits_start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }

            if i > digits_start {
                if let Ok(number) = stem[digits_start..i].parse::<u32>() {
                    let mut marker_start = keyword_start;
                    let mut marker_end = i;
                    if marker_start > 0 && bytes[marker_start - 1] == b'(' {
                        marker_start -= 1;
                    }
                    if marker_end < bytes.len() && bytes[marker_end] == b')' {
                        marker_end += 1;
                    }
                    return Some((marker_start, marker_end, number));
                }
            }

            search_from = keyword_start + keyword.len();
        }
    }
    None
}

/// Strips the disc-number marker out of a filename stem, returning the game's
/// base name (trimmed of the whitespace the marker leaves behind) and the
/// disc number. Returns None for a stem with no recognizable disc marker —
/// those files are left untouched rather than swept up by a loose prefix
/// match (the original project's `move "basename*.*"` could grab an
/// unrelated file that merely started with the same text, e.g. a stray
/// "Game Manual.pdf" next to "Game (Disc 1).chd").
fn extract_base_name(stem: &str) -> Option<(String, u32)> {
    let (start, end, number) = find_disc_marker(stem)?;
    let base = format!("{}{}", &stem[..start], &stem[end..]);
    let trimmed = base.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some((trimmed.to_string(), number))
    }
}

struct Candidate {
    path: PathBuf,
    file_name: String,
    extension: String,
    disc_number: u32,
}

/// Recursively finds multi-disc game files under `root` and groups each
/// game's discs into its own "<Game>.m3u/" folder (created directly under
/// `destination`, never inside wherever the discs happened to be nested
/// under `root`) alongside an ".m3u" playlist listing them in disc order —
/// matching the layout ES-DE expects for multi-disc entries.
///
/// `destination` is always flattened to one level: every "<Game>.m3u/"
/// folder lands directly under it, regardless of how deep the source discs
/// were nested under `root`. Games used to be organized in place (next to
/// wherever their discs were found), which meant re-running against a large,
/// deeply-nested collection scattered dozens of ".m3u" folders throughout the
/// tree instead of one place the user can point ES-DE at.
///
/// Runs in two passes (scan everything, then move) rather than moving files
/// while still walking the tree: the original project's `for /r` walk
/// mutated the directory tree it was iterating (creating new folders and
/// moving files mid-walk), which is exactly the kind of thing that produces
/// inconsistent results on deeply nested folder structures. Two passes make
/// that class of bug impossible here.
pub fn organize_multidisc(root: &Path, destination: &Path, android_base: Option<&str>) -> Result<OrganizeSummary, String> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;

    let mut groups: HashMap<(PathBuf, String), Vec<Candidate>> = HashMap::new();

    let walker = WalkDir::new(root).into_iter().filter_entry(|entry| {
        // Never descend into a folder we (or a previous run) already
        // organized — reprocessing it would try to move its files into
        // themselves and corrupt the playlist. When `destination` sits
        // inside `root`, this is also what keeps a later re-run from ever
        // re-scanning what a prior run already moved there: `destination`
        // itself holds nothing but "<Game>.m3u" folders, which this filter
        // skips, and it's created (empty) before the scan below even starts,
        // so the very first run never sees loose files inside it either.
        !entry
            .file_name()
            .to_str()
            .map(|name| name.to_lowercase().ends_with(".m3u"))
            .unwrap_or(false)
    });

    for entry in walker {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !DISC_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        let Some((base_name, disc_number)) = extract_base_name(stem) else {
            continue;
        };
        let parent = path.parent().unwrap_or(root).to_path_buf();
        let file_name = entry.file_name().to_string_lossy().to_string();
        groups.entry((parent, base_name)).or_default().push(Candidate {
            path: path.to_path_buf(),
            file_name,
            extension,
            disc_number,
        });
    }

    let mut summary = OrganizeSummary::default();

    for ((_parent, base_name), mut candidates) in groups {
        // Flattened: always directly under `destination`, never nested under
        // wherever the source discs lived under `root`. Two different source
        // folders producing the same base name (rare) land on the same
        // target and the second is safely counted as already-organized
        // below, rather than risking any cross-game clobbering.
        let game_folder = destination.join(format!("{}.m3u", base_name));
        if game_folder.exists() {
            summary.skipped_already_organized += 1;
            continue;
        }

        candidates.sort_by_key(|c| c.disc_number);

        fs::create_dir_all(&game_folder).map_err(|e| e.to_string())?;

        // Only computed when needed, for building an absolute Android path
        // below. With output always flattened to one level under
        // `destination`, this is just the game's own folder name — no
        // intermediate nesting to account for.
        let relative_game_folder = format!("{}.m3u", base_name);

        let mut playlist_lines = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            let dest_path = game_folder.join(&candidate.file_name);
            fs::rename(&candidate.path, &dest_path).map_err(|e| e.to_string())?;

            // Matches the original project's compatibility finding: Dolphin
            // (GameCube/Wii, .rvz) needs an absolute Android path to detect
            // multi-disc sets, while every other core (DuckStation included)
            // works fine — and more portably — with a plain relative
            // filename next to the .m3u.
            let line = if candidate.extension == "rvz" {
                match android_base {
                    Some(base) => format!(
                        "{}/{}/{}",
                        base.trim_end_matches('/'),
                        relative_game_folder,
                        candidate.file_name
                    ),
                    None => candidate.file_name.clone(),
                }
            } else {
                candidate.file_name.clone()
            };
            playlist_lines.push(line);
            summary.files_moved += 1;
        }

        // LF-only, no trailing blank line: matches the playlist convention
        // ES-DE and its emulator cores (DuckStation, Dolphin, etc.) expect,
        // and keeps the file byte-identical however it's produced next
        // (Windows text APIs default to CRLF, which some cores mishandle).
        let playlist_content = playlist_lines.join("\n") + "\n";
        let playlist_path = game_folder.join(format!("{}.m3u", base_name));
        fs::write(&playlist_path, playlist_content).map_err(|e| e.to_string())?;

        summary.games_organized += 1;
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chd_organizer_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extracts_base_name_and_disc_number_from_simple_marker() {
        assert_eq!(
            extract_base_name("Final Fantasy VII (Disc 1)"),
            Some(("Final Fantasy VII".to_string(), 1))
        );
    }

    #[test]
    fn extracts_base_name_when_another_parenthetical_precedes_the_marker() {
        // The bug in the original project: splitting on the first "("
        // would have kept only "Game" here, losing "(USA)".
        assert_eq!(
            extract_base_name("Game (USA) (Disc 2)"),
            Some(("Game (USA)".to_string(), 2))
        );
    }

    #[test]
    fn extracts_base_name_from_bare_disk_marker_without_parens() {
        assert_eq!(extract_base_name("Some Game Disk 3"), Some(("Some Game".to_string(), 3)));
    }

    #[test]
    fn returns_none_for_a_filename_with_no_disc_marker() {
        assert_eq!(extract_base_name("Single Disc Game"), None);
    }

    #[test]
    fn groups_and_moves_discs_from_a_deeply_nested_folder_flattened_into_destination() {
        let root = temp_dir("nested");
        let destination = temp_dir("nested_dest");
        let nested = root.join("psx").join("collection").join("subfolder");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Chrono Cross (Disc 1).chd"), b"disc1").unwrap();
        fs::write(nested.join("Chrono Cross (Disc 2).chd"), b"disc2").unwrap();

        let summary = organize_multidisc(&root, &destination, None).unwrap();

        assert_eq!(summary.games_organized, 1);
        assert_eq!(summary.files_moved, 2);

        // Lands directly under `destination`, not nested under
        // psx/collection/subfolder the way it was found under `root`.
        let game_folder = destination.join("Chrono Cross.m3u");
        assert!(game_folder.join("Chrono Cross (Disc 1).chd").exists());
        assert!(game_folder.join("Chrono Cross (Disc 2).chd").exists());
        assert!(!nested.join("Chrono Cross (Disc 1).chd").exists());

        let playlist = fs::read_to_string(game_folder.join("Chrono Cross.m3u")).unwrap();
        assert_eq!(playlist, "Chrono Cross (Disc 1).chd\nChrono Cross (Disc 2).chd\n");
        assert!(!playlist.contains('\r'));

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn does_not_reprocess_an_already_organized_game_folder() {
        let root = temp_dir("idempotent");
        let destination = temp_dir("idempotent_dest");
        fs::write(root.join("Game (Disc 1).chd"), b"disc1").unwrap();
        fs::write(root.join("Game (Disc 2).chd"), b"disc2").unwrap();

        let first = organize_multidisc(&root, &destination, None).unwrap();
        assert_eq!(first.games_organized, 1);

        // Simulate leftover loose files with the same base name after a
        // first run — a re-run must not try to fold them into the folder
        // that already exists, and must not error out either.
        fs::write(root.join("Game (Disc 3).chd"), b"disc3").unwrap();
        let second = organize_multidisc(&root, &destination, None).unwrap();
        assert_eq!(second.games_organized, 0);
        assert_eq!(second.skipped_already_organized, 1);

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn leaves_a_same_prefix_file_with_no_disc_marker_untouched() {
        // The original project's `move "basename*.*"` would sweep this up
        // too, since it starts with "Game" — a real risk with sequels or
        // bonus-disc files that share a prefix but aren't part of the set.
        let root = temp_dir("prefix_safety");
        let destination = temp_dir("prefix_safety_dest");
        fs::write(root.join("Game (Disc 1).chd"), b"disc1").unwrap();
        fs::write(root.join("Game Extras.chd"), b"not a disc").unwrap();

        organize_multidisc(&root, &destination, None).unwrap();

        assert!(root.join("Game Extras.chd").exists());
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn uses_absolute_android_path_for_rvz_when_a_base_is_given() {
        let root = temp_dir("android_rvz");
        let destination = temp_dir("android_rvz_dest");
        let nested = root.join("gc");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Resident Evil (Disc 1).rvz"), b"disc1").unwrap();
        fs::write(nested.join("Resident Evil (Disc 2).rvz"), b"disc2").unwrap();

        organize_multidisc(&root, &destination, Some("/storage/emulated/0/ROMs")).unwrap();

        let playlist = fs::read_to_string(
            destination.join("Resident Evil.m3u").join("Resident Evil.m3u"),
        )
        .unwrap();
        // Flattened: the android path is just the game's own folder name,
        // not the "gc" subfolder it used to live under before flattening.
        assert_eq!(
            playlist,
            "/storage/emulated/0/ROMs/Resident Evil.m3u/Resident Evil (Disc 1).rvz\n\
             /storage/emulated/0/ROMs/Resident Evil.m3u/Resident Evil (Disc 2).rvz\n"
        );

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn keeps_relative_paths_for_non_rvz_even_with_an_android_base() {
        let root = temp_dir("android_relative");
        let destination = temp_dir("android_relative_dest");
        fs::write(root.join("Game (Disc 1).chd"), b"disc1").unwrap();
        fs::write(root.join("Game (Disc 2).chd"), b"disc2").unwrap();

        organize_multidisc(&root, &destination, Some("/storage/emulated/0/ROMs")).unwrap();

        let playlist = fs::read_to_string(destination.join("Game.m3u").join("Game.m3u")).unwrap();
        assert_eq!(playlist, "Game (Disc 1).chd\nGame (Disc 2).chd\n");

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&destination).unwrap();
    }

    #[test]
    fn destination_can_be_a_subfolder_of_root_without_recursing_into_itself() {
        // A common real choice: organize output goes into "<root>/Organized".
        // The walker must not descend into that folder mid-walk and treat
        // freshly-moved files as new candidates to move again.
        let root = temp_dir("dest_inside_root");
        let destination = root.join("Organized");
        fs::write(root.join("Game (Disc 1).chd"), b"disc1").unwrap();
        fs::write(root.join("Game (Disc 2).chd"), b"disc2").unwrap();

        let summary = organize_multidisc(&root, &destination, None).unwrap();

        assert_eq!(summary.games_organized, 1);
        assert_eq!(summary.files_moved, 2);
        assert!(destination.join("Game.m3u").join("Game (Disc 1).chd").exists());

        fs::remove_dir_all(&root).unwrap();
    }
}
