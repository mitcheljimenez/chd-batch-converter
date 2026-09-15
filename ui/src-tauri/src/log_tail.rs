use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
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
    /// Trailing bytes of an incomplete (newline-less) line held back from the
    /// previous read, to be prepended to the next chunk. chdman's own stdout is
    /// redirected into the same log file the `.bat`'s `:log` subroutine appends
    /// to, so a poll can easily land between a line's start and its `\n`.
    partial: String,
}

impl LogTailer {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            partial: String::new(),
        }
    }

    pub fn read_new_lines(&mut self) -> io::Result<Vec<String>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        // Read RAW BYTES: conversion_log.txt is written by cmd.exe's `echo` in
        // the OEM console codepage (CP850/437), not UTF-8, so an accented
        // filename ("Edición Azul.cue") yields bytes that `read_to_string`
        // rejects outright — which would leave the offset stuck and kill the
        // tail permanently. Decode leniently instead: a bad byte degrades to
        // U+FFFD rather than freezing the UI.
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        // Advance by the RAW byte count, computed BEFORE lossy decoding.
        // `from_utf8_lossy` can change the byte length (each invalid byte
        // becomes a 3-byte replacement char), so using the decoded length
        // here would silently desync the file offset.
        self.offset += buf.len() as u64;
        let chunk = String::from_utf8_lossy(&buf);

        let mut combined = std::mem::take(&mut self.partial);
        combined.push_str(&chunk);

        if let Some(last_newline) = combined.rfind('\n') {
            let (complete, rest) = combined.split_at(last_newline + 1);
            let lines = complete.lines().map(|s| s.to_string()).collect();
            self.partial = rest.to_string();
            Ok(lines)
        } else {
            self.partial = combined;
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

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

    #[test]
    fn tailer_survives_non_utf8_bytes_and_advances_by_raw_byte_count() {
        let dir = std::env::temp_dir().join(format!("chd_tailer_oem_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("conversion_log.txt");

        // "Edici" + 0xA2 (CP850/437 'ó', invalid as UTF-8) + "n\n" — exactly
        // what cmd.exe's `echo` produces for an accented filename.
        let raw: Vec<u8> = vec![0x45, 0x64, 0x69, 0x63, 0x69, 0xA2, 0x6E, 0x0A];
        fs::write(&log_path, &raw).unwrap();

        let mut tailer = LogTailer::new(log_path.clone());
        let first = tailer.read_new_lines().expect("must not error on non-UTF-8 input");
        assert_eq!(first.len(), 1);
        assert!(first[0].starts_with("Edici"));
        assert!(first[0].ends_with('n'));
        // The offset must track RAW bytes (8), not the lossy-decoded string's
        // length (10, since U+FFFD is 3 bytes where the input byte was 1).
        assert_eq!(tailer.offset, raw.len() as u64);

        // A subsequent write must be seen exactly once — not skipped (offset
        // too far ahead) and not partly re-read (offset too far behind).
        let mut f = File::options().append(true).open(&log_path).unwrap();
        writeln!(f, "line two").unwrap();
        drop(f);

        let second = tailer.read_new_lines().unwrap();
        assert_eq!(second, vec!["line two".to_string()]);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tailer_buffers_partial_line_until_newline_arrives() {
        let dir = std::env::temp_dir().join(format!("chd_tailer_partial_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("conversion_log.txt");

        // First poll lands mid-write: the line has no trailing newline yet.
        fs::write(&log_path, "Tue 09/15/2026 | OK    | C:\\Games\\Tr").unwrap();

        let mut tailer = LogTailer::new(log_path.clone());
        let first = tailer.read_new_lines().unwrap();
        assert!(first.is_empty(), "a partial line must not be emitted yet");

        // The writer finishes the line.
        let mut f = File::options().append(true).open(&log_path).unwrap();
        write!(f, "ack.cue | convertido\n").unwrap();
        drop(f);

        let second = tailer.read_new_lines().unwrap();
        assert_eq!(
            second,
            vec!["Tue 09/15/2026 | OK    | C:\\Games\\Track.cue | convertido".to_string()]
        );
        // And it parses as a real event — the whole point of not splitting it.
        assert_eq!(parse_log_line(&second[0]).unwrap().status, DiscStatus::Ok);

        // Nothing duplicated on a follow-up read.
        assert!(tailer.read_new_lines().unwrap().is_empty());

        fs::remove_dir_all(&dir).unwrap();
    }
}
