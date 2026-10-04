use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

/// A chdman step that failed: the stable error `code` the frontend
/// translates (e.g. "CONVERT_FAILED"), plus `log`, whatever chdman printed
/// besides its progress lines, shown on demand so a failure can be
/// diagnosed without leaving the app.
#[derive(Debug, Clone, PartialEq)]
pub struct ChdmanError {
    pub code: String,
    pub log: String,
}

impl ChdmanError {
    pub fn new(code: impl Into<String>, log: impl Into<String>) -> Self {
        ChdmanError { code: code.into(), log: log.into() }
    }

    /// An error decided before chdman ever ran, so there's no log.
    pub fn without_log(code: impl Into<String>) -> Self {
        ChdmanError::new(code, String::new())
    }
}

/// Keeps the captured log bounded: a pathological run can print a lot, and
/// only the end of it (where chdman reports what went wrong) matters.
const MAX_LOG_LINES: usize = 200;

enum Output {
    Progress(&'static str, f32),
    Line(String),
}

/// Runs `chdman` with `args`, calling `on_progress(phase, percent)` for each
/// progress line `parse` recognizes. chdman writes progress to **stderr**
/// (its `progress()` writes to `std::cerr` and flushes every call) and the
/// occasional summary to stdout, overwriting progress in place with bare
/// '\r'. Both streams are read on their own thread so neither can block
/// the other, feeding one channel that's drained here on the caller's
/// thread (`on_progress` closes over a `tauri::Window`, which needn't be
/// `Send`).
///
/// When `active_pids` is given, the child's PID is registered there while
/// it runs so `cancel_conversion` can kill it.
///
/// On failure (spawn error or non-zero exit) returns every non-progress
/// line chdman printed (the last `MAX_LOG_LINES`), for display.
pub fn run(
    chdman_path: &Path,
    args: &[std::ffi::OsString],
    active_pids: Option<&Arc<Mutex<Vec<u32>>>>,
    parse: fn(&str) -> Option<(&'static str, f32)>,
    on_progress: &mut impl FnMut(&str, f32),
) -> Result<(), String> {
    let mut command = Command::new(chdman_path);
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = crate::hide_console(&mut command)
        .spawn()
        .map_err(|e| format!("{}: {}", chdman_path.display(), e))?;

    let pid = child.id();
    if let Some(pids) = active_pids {
        pids.lock().unwrap().push(pid);
    }

    let (tx, rx) = mpsc::channel();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(thread::spawn(move || stream(stdout, parse, tx)));
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(thread::spawn(move || stream(stderr, parse, tx)));
    }
    drop(tx);

    let mut log: Vec<String> = Vec::new();
    for output in rx {
        match output {
            Output::Progress(phase, percent) => on_progress(phase, percent),
            Output::Line(line) => {
                log.push(line);
                if log.len() > MAX_LOG_LINES {
                    log.remove(0);
                }
            }
        }
    }
    for reader in readers {
        let _ = reader.join();
    }

    let status = child.wait();
    if let Some(pids) = active_pids {
        pids.lock().unwrap().retain(|&p| p != pid);
    }
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            // A plain exit code when there is one; otherwise (killed by a
            // signal on Unix) the OS's own description.
            log.push(match status.code() {
                Some(code) => format!("(chdman exit code {})", code),
                None => format!("(chdman {})", status),
            });
            Err(log.join("\n"))
        }
        Err(e) => {
            log.push(e.to_string());
            Err(log.join("\n"))
        }
    }
}

/// Reads `reader` to EOF, splitting on '\n' and chdman's bare '\r', and
/// sends each line as progress (when `parse` recognizes it) or as a log
/// line (when it has any text).
fn stream(mut reader: impl Read, parse: fn(&str) -> Option<(&'static str, f32)>, tx: mpsc::Sender<Output>) {
    let mut partial = String::new();
    let mut chunk = [0u8; 4096];
    let send_line = |line: &str| {
        if let Some((phase, percent)) = parse(line) {
            let _ = tx.send(Output::Progress(phase, percent));
        } else if !line.trim().is_empty() {
            let _ = tx.send(Output::Line(line.trim_end().to_string()));
        }
    };
    loop {
        let n = reader.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        partial.push_str(&String::from_utf8_lossy(&chunk[..n]));
        let normalized = partial.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<&str> = normalized.split('\n').collect();
        // The last element is whatever came after the final newline: an
        // incomplete line still being written, held back for the next read.
        let tail = lines.pop().unwrap_or("").to_string();
        for line in lines {
            send_line(line);
        }
        partial = tail;
    }
    // A last line without a trailing newline (e.g. chdman's final error).
    send_line(&partial);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test(line: &str) -> Option<(&'static str, f32)> {
        let rest = line.trim_start().strip_prefix("Working, ")?;
        rest.split('%').next()?.trim().parse().ok().map(|p| ("working", p))
    }

    fn collect(input: &str) -> (Vec<(&'static str, f32)>, Vec<String>) {
        let (tx, rx) = mpsc::channel();
        stream(input.as_bytes(), parse_test, tx);
        let (mut progress, mut lines) = (Vec::new(), Vec::new());
        for output in rx {
            match output {
                Output::Progress(phase, pct) => progress.push((phase, pct)),
                Output::Line(line) => lines.push(line),
            }
        }
        (progress, lines)
    }

    #[test]
    fn separates_progress_from_log_lines() {
        let (progress, lines) = collect(
            "Input file: Game.cue\nWorking, 10.0% complete...\rWorking, 55.5% complete...\r\n\nError: file not found",
        );
        assert_eq!(progress, vec![("working", 10.0), ("working", 55.5)]);
        assert_eq!(lines, vec!["Input file: Game.cue", "Error: file not found"]);
    }
}
