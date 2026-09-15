use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum DiscStatus {
    Ok,
    Skip,
    Fail,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LogEvent {
    pub status: DiscStatus,
    pub path: String,
    pub message: String,
}

pub fn parse_log_line(line: &str) -> Option<LogEvent> {
    let mut parts = line.splitn(2, " | ");
    let _timestamp = parts.next()?;
    let rest = parts.next()?;

    let mut fields = rest.splitn(3, " | ");
    let status_raw = fields.next()?;
    let path = fields.next()?;
    let message = fields.next()?;

    let status = match status_raw.trim() {
        "OK" => DiscStatus::Ok,
        "SKIP" => DiscStatus::Skip,
        "FAIL" => DiscStatus::Fail,
        _ => return None,
    };

    Some(LogEvent {
        status,
        path: path.trim().to_string(),
        message: message.trim().to_string(),
    })
}

pub struct LogTailer {
    pub path: PathBuf,
    pub offset: u64,
}

impl LogTailer {
    pub fn new(path: PathBuf) -> Self {
        Self { path, offset: 0 }
    }

    pub fn read_new_lines(&mut self) -> io::Result<Vec<String>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        self.offset += buf.len() as u64;
        Ok(buf.lines().map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_ok_line() {
        let line = "Tue 09/15/2026  9:40:00.00 | OK    | C:\\Games\\PS1\\Track.cue | convertido y verificado";
        let event = parse_log_line(line).expect("should parse");
        assert_eq!(event.status, DiscStatus::Ok);
        assert_eq!(event.path, "C:\\Games\\PS1\\Track.cue");
        assert_eq!(event.message, "convertido y verificado");
    }

    #[test]
    fn parses_a_skip_line() {
        let line = "Tue 09/15/2026  9:40:01.00 | SKIP  | C:\\Games\\PS1\\Track.cue | ya existe .chd";
        let event = parse_log_line(line).expect("should parse");
        assert_eq!(event.status, DiscStatus::Skip);
        assert_eq!(event.message, "ya existe .chd");
    }

    #[test]
    fn parses_a_fail_line() {
        let line = "Tue 09/15/2026  9:40:02.00 | FAIL  | C:\\Games\\PS1\\Bad.cue | fallo la conversion";
        let event = parse_log_line(line).expect("should parse");
        assert_eq!(event.status, DiscStatus::Fail);
    }

    #[test]
    fn returns_none_for_section_markers() {
        assert_eq!(parse_log_line("==== Ejecucion iniciada Tue 09/15/2026 ===="), None);
        assert_eq!(parse_log_line("==== Resumen: Convertidos=1 Saltados=0 Fallidos=0 ===="), None);
    }

    #[test]
    fn returns_none_for_unknown_status() {
        let line = "Tue 09/15/2026  9:40:00.00 | WEIRD | path | message";
        assert_eq!(parse_log_line(line), None);
    }

    #[test]
    fn tailer_reads_only_new_lines_across_calls() {
        let dir = std::env::temp_dir().join(format!("chd_tailer_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("conversion_log.txt");
        fs::write(&log_path, "line one\n").unwrap();

        let mut tailer = LogTailer::new(log_path.clone());
        let first = tailer.read_new_lines().unwrap();
        assert_eq!(first, vec!["line one".to_string()]);

        let mut f = File::options().append(true).open(&log_path).unwrap();
        writeln!(f, "line two").unwrap();

        let second = tailer.read_new_lines().unwrap();
        assert_eq!(second, vec!["line two".to_string()]);

        // No new writes: reading again yields nothing.
        let third = tailer.read_new_lines().unwrap();
        assert!(third.is_empty());

        fs::remove_dir_all(&dir).unwrap();
    }
}
