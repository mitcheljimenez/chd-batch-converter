use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

fn default_language() -> String {
    "es".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub chdman_path: String,
    #[serde(default)]
    pub auto_update_enabled: bool,
    #[serde(default = "default_language")]
    pub language: String,
    /// Convert several discs at once (one chdman per CPU core) instead of
    /// one at a time. Defaults to true so configs saved before this option
    /// existed keep the parallel behavior they already had.
    #[serde(default = "default_true")]
    pub parallel_conversion: bool,
    /// After a disc converts and verifies, send its original files (the
    /// .cue/.gdi and its tracks, or the .iso) to the system trash. Off by
    /// default: originals are never touched unless the user opts in.
    #[serde(default)]
    pub trash_originals: bool,
    /// The folder last chosen, reopened on the next launch ("" for none).
    #[serde(default)]
    pub last_folder: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            chdman_path: String::new(),
            auto_update_enabled: false,
            language: default_language(),
            parallel_conversion: true,
            trash_originals: false,
            last_folder: String::new(),
        }
    }
}

pub fn load_config(app_dir: &Path) -> Config {
    let path = app_dir.join("config.json");
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save_config(app_dir: &Path, config: &Config) -> io::Result<()> {
    fs::create_dir_all(app_dir)?;
    let path = app_dir.join("config.json");
    let contents = serde_json::to_string_pretty(config).expect("Config always serializes");
    fs::write(path, contents)
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RunRecord {
    pub timestamp: String,
    pub folder: String,
    pub converted: u32,
    pub skipped: u32,
    pub failed: u32,
    pub cancelled: bool,
    /// Combined size of the originals and of the resulting .chd files, over
    /// the discs this run converted. 0 for runs recorded before this was
    /// tracked.
    #[serde(default)]
    pub original_bytes: u64,
    #[serde(default)]
    pub chd_bytes: u64,
}

pub fn load_history(app_dir: &Path) -> Vec<RunRecord> {
    let path = app_dir.join("historial.json");
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn append_history(app_dir: &Path, record: RunRecord) -> io::Result<()> {
    fs::create_dir_all(app_dir)?;
    let mut records = load_history(app_dir);
    records.push(record);
    let path = app_dir.join("historial.json");
    let contents = serde_json::to_string_pretty(&records).expect("Vec<RunRecord> always serializes");
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("chd_settings_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_config_yields_default() {
        let dir = temp_dir("missing_config");
        let config = load_config(&dir);
        assert_eq!(config.chdman_path, "");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_then_load_config_round_trips() {
        let dir = temp_dir("round_trip");
        let config = Config {
            chdman_path: "C:\\Tools\\chdman.exe".to_string(),
            auto_update_enabled: false,
            language: "es".to_string(),
            parallel_conversion: true,
            trash_originals: false,
            last_folder: String::new(),
        };
        save_config(&dir, &config).unwrap();
        let loaded = load_config(&dir);
        assert_eq!(loaded.chdman_path, "C:\\Tools\\chdman.exe");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_history_yields_empty_vec() {
        let dir = temp_dir("missing_history");
        assert_eq!(load_history(&dir), Vec::new());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn append_history_accumulates_records_in_order() {
        let dir = temp_dir("append_history");
        let r1 = RunRecord {
            timestamp: "2026-09-15T10:00:00".to_string(),
            folder: "C:\\Games\\PS1".to_string(),
            converted: 3,
            skipped: 1,
            failed: 0,
            cancelled: false,
            original_bytes: 3_000,
            chd_bytes: 1_200,
        };
        let r2 = RunRecord {
            timestamp: "2026-09-15T11:00:00".to_string(),
            folder: "C:\\Games\\PS2".to_string(),
            converted: 0,
            skipped: 0,
            failed: 0,
            cancelled: true,
            original_bytes: 0,
            chd_bytes: 0,
        };
        append_history(&dir, r1.clone()).unwrap();
        append_history(&dir, r2.clone()).unwrap();

        let loaded = load_history(&dir);
        assert_eq!(loaded, vec![r1, r2]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_config_defaults_auto_update_to_false() {
        let dir = temp_dir("missing_config_auto_update");
        let config = load_config(&dir);
        assert_eq!(config.auto_update_enabled, false);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_then_load_config_round_trips_auto_update_flag() {
        let dir = temp_dir("round_trip_auto_update");
        let config = Config {
            chdman_path: "C:\\Tools\\chdman.exe".to_string(),
            auto_update_enabled: true,
            language: "es".to_string(),
            parallel_conversion: true,
            trash_originals: false,
            last_folder: String::new(),
        };
        save_config(&dir, &config).unwrap();
        let loaded = load_config(&dir);
        assert_eq!(loaded.auto_update_enabled, true);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_config_defaults_language_to_es() {
        let dir = temp_dir("missing_config_language");
        let config = load_config(&dir);
        assert_eq!(config.language, "es");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_then_load_config_round_trips_language() {
        let dir = temp_dir("round_trip_language");
        let config = Config {
            chdman_path: "C:\\Tools\\chdman.exe".to_string(),
            auto_update_enabled: false,
            language: "en".to_string(),
            parallel_conversion: true,
            trash_originals: false,
            last_folder: String::new(),
        };
        save_config(&dir, &config).unwrap();
        let loaded = load_config(&dir);
        assert_eq!(loaded.language, "en");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_config_defaults_to_parallel_conversion() {
        let dir = temp_dir("default_parallel");
        // A config saved before the option existed has no such key.
        fs::write(dir.join("config.json"), r#"{"chdman_path":"","auto_update_enabled":false,"language":"es"}"#).unwrap();
        assert!(load_config(&dir).parallel_conversion);
        assert!(Config::default().parallel_conversion);
    }

    #[test]
    fn save_then_load_config_round_trips_sequential_mode() {
        let dir = temp_dir("roundtrip_sequential");
        let config = Config {
            parallel_conversion: false,
            ..Config::default()
        };
        save_config(&dir, &config).unwrap();
        assert!(!load_config(&dir).parallel_conversion);
    }

    #[test]
    fn history_saved_before_sizes_were_tracked_still_loads() {
        let dir = temp_dir("old_history");
        fs::write(
            dir.join("historial.json"),
            r#"[{"timestamp":"1","folder":"C:\\Games","converted":2,"skipped":0,"failed":0,"cancelled":false}]"#,
        )
        .unwrap();
        let loaded = load_history(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!((loaded[0].original_bytes, loaded[0].chd_bytes), (0, 0));
    }
}
