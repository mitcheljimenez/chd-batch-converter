use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

fn default_language() -> String {
    "es".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub chdman_path: String,
    #[serde(default)]
    pub auto_update_enabled: bool,
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            chdman_path: String::new(),
            auto_update_enabled: false,
            language: default_language(),
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
        };
        let r2 = RunRecord {
            timestamp: "2026-09-15T11:00:00".to_string(),
            folder: "C:\\Games\\PS2".to_string(),
            converted: 0,
            skipped: 0,
            failed: 0,
            cancelled: true,
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
        };
        save_config(&dir, &config).unwrap();
        let loaded = load_config(&dir);
        assert_eq!(loaded.language, "en");
        fs::remove_dir_all(&dir).unwrap();
    }
}
