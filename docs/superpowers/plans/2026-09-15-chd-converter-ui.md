# CHD Converter UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri desktop app (`ui/`) that wraps `convertir_a_chd.bat`: pick a folder, watch per-game conversion status live, cancel mid-run, and review run history — without reimplementing any conversion/verify logic.

**Architecture:** A Rust backend spawns `convertir_a_chd.bat` as a child process against a user-chosen folder, tails `conversion_log.txt` incrementally, and emits parsed status events to an HTML/CSS/JS frontend that renders a live table, progress bar, cancel button, settings, and history panel.

**Tech Stack:** Tauri (Rust + WebView2), vanilla HTML/CSS/JS (no frontend framework — YAGNI for this scope), `walkdir`/`serde`/`serde_json` Rust crates.

**Spec:** `docs/superpowers/specs/2026-09-15-chd-converter-ui-design.md` (and the underlying script's spec, `docs/superpowers/specs/2026-09-14-chd-batch-converter-design.md`)

## Global Constraints

- The UI never reimplements discovery/conversion/verify rules — `convertir_a_chd.bat` remains the single source of truth (spec: Non-goals).
- Ships as a standalone Windows `.exe` requiring no runtime installed on the end user's machine (spec: Tech stack).
- `chdman.exe` path is user-configurable via the UI, persisted in `config.json` (spec: Components §5).
- Run history is tracked in a structured `historial.json`, not parsed from the free-text log (spec: Components §4).
- The backend's log-tailing must be resumable by byte offset — no lost or duplicated lines on reconnect (spec: Error handling).
- Cancellation is its own status, never conflated with a real failure (spec: Error handling).
- This plan builds on Windows via WSL2 interop (`/mnt/c/WINDOWS/system32/cmd.exe`, `wslpath -w`) — the same mechanism used throughout the `.bat`'s own plan (`docs/superpowers/plans/2026-09-14-chd-batch-converter.md`).

---

## Task 1: Add an optional root-folder argument to `convertir_a_chd.bat`

**Why this comes first:** the existing script always scans `%~dp0` — its own file's directory. The UI needs to point it at an arbitrary user-chosen folder while the `.bat` itself ships bundled with the UI elsewhere on disk. This is a small, backward-compatible addition: an optional first argument overrides `ROOT`; omitting it (every existing double-click and test usage) behaves exactly as before.

**Files:**
- Modify: `convertir_a_chd.bat:4`

**Interfaces:**
- Produces: `convertir_a_chd.bat "<folder>"` scans `<folder>` instead of the script's own directory; `convertir_a_chd.bat` (no args) is unchanged.

- [ ] **Step 1: Write a failing regression check**

```bash
cd /home/jimrod/chd-batch-converter
WORK=/mnt/c/temp/chd_root_arg_test
rm -rf "$WORK"
mkdir -p "$WORK/engine" "$WORK/target/game"
cp convertir_a_chd.bat "$WORK/engine/"
cp tests/mock_chdman.bat "$WORK/engine/"
printf 'FILE "Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > "$WORK/target/game/Track.cue"
echo "fake bin" > "$WORK/target/game/Track.bin"
WINENGINE=$(wslpath -w "$WORK/engine")
WINTARGET=$(wslpath -w "$WORK/target")
cd "$WORK/engine"
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINENGINE && set CHDMAN_OVERRIDE=$WINENGINE\\mock_chdman.bat && convertir_a_chd.bat \"$WINTARGET\""
test -f "$WORK/target/game/Track.chd" && echo "UNEXPECTED PASS" || echo "EXPECTED FAIL: root arg not supported yet"
```

Expected: `EXPECTED FAIL: root arg not supported yet` (the `.chd` isn't created in `target/` because the script still scans `engine/`, where there's nothing to convert).

- [ ] **Step 2: Add the root-argument override**

In `convertir_a_chd.bat`, replace line 4:

```bat
set "ROOT=%~dp0"
```

with:

```bat
if not "%~1"=="" (
    set "ROOT=%~1"
    if not "%ROOT:~-1%"=="\" set "ROOT=%ROOT%\"
) else (
    set "ROOT=%~dp0"
)
```

This preserves the default (`%~dp0`, always trailing-backslash-terminated) when no argument is given, and normalizes a caller-supplied path to always end in `\` so `%ROOT%chdman.exe` / `%ROOT%conversion_log.txt` concatenate correctly regardless of how the caller formatted the argument.

- [ ] **Step 3: Re-run the check from Step 1 to confirm it now passes**

```bash
cd /home/jimrod/chd-batch-converter
WORK=/mnt/c/temp/chd_root_arg_test
rm -rf "$WORK"
mkdir -p "$WORK/engine" "$WORK/target/game"
cp convertir_a_chd.bat "$WORK/engine/"
cp tests/mock_chdman.bat "$WORK/engine/"
printf 'FILE "Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > "$WORK/target/game/Track.cue"
echo "fake bin" > "$WORK/target/game/Track.bin"
WINENGINE=$(wslpath -w "$WORK/engine")
WINTARGET=$(wslpath -w "$WORK/target")
cd "$WORK/engine"
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINENGINE && set CHDMAN_OVERRIDE=$WINENGINE\\mock_chdman.bat && convertir_a_chd.bat \"$WINTARGET\""
test -f "$WORK/target/game/Track.chd" && echo "PASS: converted into target, not engine dir" || echo "FAIL"
test -f "$WORK/target/conversion_log.txt" && echo "PASS: log written into target dir" || echo "FAIL"
test ! -f "$WORK/engine/conversion_log.txt" && echo "PASS: no log left behind in engine dir" || echo "FAIL"
```

Expected: all three lines print `PASS: ...`.

- [ ] **Step 4: Re-run the full existing scenario suite (regression) to confirm no-arg behavior is untouched**

```bash
cd /home/jimrod/chd-batch-converter
for s in cue_basic cue_skip cue_fail cue_verify_fail iso_basic iso_ignored_with_cue nested_multidisc cue_bang iso_skip_logged cue_parens; do
  echo "=== $s ==="
  bash tests/run_scenario.sh "$s" 2>&1 | tail -3
done
```

Expected: identical output to the counts already established (`cue_basic`: Convertidos=1; `cue_skip`: Saltados=1; `cue_fail`: Fallidos=1; `cue_verify_fail`: Fallidos=1; `iso_basic`: Convertidos=1; `iso_ignored_with_cue`: Convertidos=1 Saltados=1; `nested_multidisc`: Convertidos=4 Saltados=1; `cue_bang`: Convertidos=1; `iso_skip_logged`: Convertidos=1 Saltados=1; `cue_parens`: Convertidos=1).

- [ ] **Step 5: Clean up and commit**

```bash
rm -rf /mnt/c/temp/chd_root_arg_test
cd /home/jimrod/chd-batch-converter
git add convertir_a_chd.bat
git commit -m "Support an optional root-folder argument in convertir_a_chd.bat

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 2: Windows toolchain and Tauri project scaffold

**Files:**
- Create: `ui/` (generated Tauri project — `ui/src-tauri/`, `ui/src/`, `ui/package.json`, `ui/src-tauri/Cargo.toml`, `ui/src-tauri/tauri.conf.json`)
- Create: `ui/README.md`

**Interfaces:**
- Produces: a buildable Tauri project at `ui/` with `npm run tauri build` producing a Windows `.exe` under `ui/src-tauri/target/release/`.
- Consumes: nothing (first UI task; independent of Task 1's `.bat` change, but built to house it in later tasks).

- [ ] **Step 1: Check for existing Node.js and Rust on the real Windows install**

```bash
/mnt/c/WINDOWS/system32/cmd.exe /c "node --version" 2>&1
/mnt/c/WINDOWS/system32/cmd.exe /c "cargo --version" 2>&1
```

If either prints "not recognized", install it:

```bash
# Node.js LTS (skip if already present)
/mnt/c/WINDOWS/system32/cmd.exe /c "winget install OpenJS.NodeJS.LTS --silent --accept-package-agreements --accept-source-agreements"
# Rust (skip if already present) — rustup-init via winget
/mnt/c/WINDOWS/system32/cmd.exe /c "winget install Rustlang.Rustup --silent --accept-package-agreements --accept-source-agreements"
```

After either install, open a **fresh** `cmd.exe` invocation (PATH changes don't apply to already-running shells) and re-run the version checks above to confirm both are now found.

- [ ] **Step 2: Scaffold the Tauri project**

```bash
mkdir -p /home/jimrod/chd-batch-converter/ui
cd /home/jimrod/chd-batch-converter/ui
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && npm create tauri-app@latest . -- --yes --template vanilla --manager npm"
```

If the scaffolding tool prompts interactively despite `--yes`, answer: app name `chd-converter-ui`, window title `CHD Converter`, package manager `npm`, UI template `vanilla` (no framework — matches the plan's YAGNI choice).

- [ ] **Step 3: Install dependencies and verify a build compiles**

```bash
cd /home/jimrod/chd-batch-converter/ui
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && npm install"
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && npm run tauri build"
```

Expected: the build succeeds and produces an `.exe`. Confirm it exists:

```bash
find /home/jimrod/chd-batch-converter/ui/src-tauri/target/release -maxdepth 1 -iname "*.exe"
```

Expected: prints a path to a `.exe` file (the scaffold's default "Hello World" app).

- [ ] **Step 4: Add the two Rust dependencies this plan will need**

Edit `ui/src-tauri/Cargo.toml`, adding to the `[dependencies]` section (alongside whatever the scaffold already put there — do not remove existing entries):

```toml
walkdir = "2"
```

(`serde`/`serde_json` are already included by the Tauri scaffold for its own IPC — reuse those, don't add a duplicate version.)

- [ ] **Step 5: Write `ui/README.md`**

```markdown
# CHD Converter UI

Desktop UI for `../convertir_a_chd.bat`. Built with Tauri.

## Build

```
cd ui
npm install
npm run tauri build
```

The finished `.exe` is at `ui/src-tauri/target/release/chd-converter-ui.exe` (or
similar, per `tauri.conf.json`'s `productName`). It bundles `convertir_a_chd.bat`
alongside itself at build time (see Task 6) — copy `chdman.exe` next to the
`.exe` (or set its path in the app's Settings panel) before running conversions.

## Develop

```
cd ui
npm run tauri dev
```
```

- [ ] **Step 6: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add ui/
git commit -m "Scaffold Tauri UI project

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 3: Log-line parser and incremental tailer (pure Rust, unit tested)

**Files:**
- Create: `ui/src-tauri/src/log_tail.rs`
- Modify: `ui/src-tauri/src/main.rs` (add `mod log_tail;`)

**Interfaces:**
- Produces:
  - `pub enum DiscStatus { Ok, Skip, Fail }`
  - `pub struct LogEvent { pub status: DiscStatus, pub path: String, pub message: String }`
  - `pub fn parse_log_line(line: &str) -> Option<LogEvent>`
  - `pub struct LogTailer { pub path: std::path::PathBuf, pub offset: u64 }` with `pub fn new(path: PathBuf) -> Self` and `pub fn read_new_lines(&mut self) -> std::io::Result<Vec<String>>`
- Consumes: nothing new (pure module).

This module parses lines written by `convertir_a_chd.bat`'s `:log` subroutine, which look like:
`Tue 09/15/2026  9:40:00.00 | OK    | C:\Games\PS1\Track.cue | convertido y verificado`
(timestamp, then `STATUS | path | message`, all separated by `" | "`). Section-marker lines like `==== Ejecucion iniciada ... ====` or `==== Resumen: ... ====` have no such structure and must parse to `None`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src-tauri/src/log_tail.rs`:

```rust
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum DiscStatus {
    Ok,
    Skip,
    Fail,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogEvent {
    pub status: DiscStatus,
    pub path: String,
    pub message: String,
}

pub fn parse_log_line(line: &str) -> Option<LogEvent> {
    todo!()
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
        todo!()
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
```

Add `mod log_tail;` near the top of `ui/src-tauri/src/main.rs` (below any existing `mod` lines the scaffold generated, above `fn main()`).

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo test log_tail"
```

Expected: compile failure or panic from the two `todo!()` bodies.

- [ ] **Step 3: Implement `parse_log_line` and `LogTailer::read_new_lines`**

Replace the two `todo!()` bodies in `ui/src-tauri/src/log_tail.rs`:

```rust
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
```

```rust
    pub fn read_new_lines(&mut self) -> io::Result<Vec<String>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        self.offset += buf.len() as u64;
        Ok(buf.lines().map(|s| s.to_string()).collect())
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo test log_tail"
```

Expected: `test result: ok. 6 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add ui/src-tauri/src/log_tail.rs ui/src-tauri/src/main.rs
git commit -m "Add log-line parser and incremental tailer

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 4: Folder pre-scan (recursive `.cue`/`.iso` enumeration)

**Files:**
- Create: `ui/src-tauri/src/scanner.rs`
- Modify: `ui/src-tauri/src/main.rs` (add `mod scanner;`)

**Interfaces:**
- Consumes: nothing from earlier tasks (independent pure module, like Task 3).
- Produces:
  - `#[derive(serde::Serialize, Debug, Clone, PartialEq)] pub struct ScannedDisc { pub name: String, pub folder: String, pub kind: String }` (`kind` is `"cue"` or `"iso"`)
  - `pub fn scan_folder(root: &std::path::Path) -> Vec<ScannedDisc>`

This mirrors `convertir_a_chd.bat`'s own discovery rule exactly: every `.cue` counts; a `.iso` counts only if no `.cue` exists anywhere in that `.iso`'s own directory. This is a *display-only* pre-scan (it does not touch chdman or write any files) — its only job is populating the table before the user presses "Convertir todo".

- [ ] **Step 1: Write the failing tests**

Create `ui/src-tauri/src/scanner.rs`:

```rust
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct ScannedDisc {
    pub name: String,
    pub folder: String,
    pub kind: String,
}

pub fn scan_folder(root: &Path) -> Vec<ScannedDisc> {
    todo!()
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
    fn finds_cue_and_bare_iso_but_skips_iso_next_to_cue() {
        let dir = std::env::temp_dir().join(format!("chd_scanner_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        write(&dir.join("GameA/Track.cue"), "FILE \"Track.bin\" BINARY\n");
        write(&dir.join("GameB/Disc.iso"), "fake iso");
        write(&dir.join("GameC/Disc.iso"), "fake iso");
        write(&dir.join("GameC/Other.cue"), "FILE \"Other.bin\" BINARY\n");

        let mut results = scan_folder(&dir);
        results.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(results.len(), 2, "GameC's Disc.iso must be excluded: {:?}", results);
        assert_eq!(results[0].name, "Disc.iso");
        assert_eq!(results[0].kind, "iso");
        assert_eq!(results[1].name, "Other.cue");
        assert_eq!(results[1].kind, "cue");

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
```

Add `mod scanner;` to `ui/src-tauri/src/main.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo test scanner"
```

Expected: compile/panic failure from `todo!()`.

- [ ] **Step 3: Implement `scan_folder`**

```rust
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

    let mut results: Vec<ScannedDisc> = cues
        .iter()
        .map(|p| ScannedDisc {
            name: p.file_name().unwrap().to_string_lossy().to_string(),
            folder: p.parent().unwrap().to_string_lossy().to_string(),
            kind: "cue".to_string(),
        })
        .collect();

    for iso in &isos {
        let dir = iso.parent().unwrap();
        let has_cue = cues.iter().any(|c| c.parent().unwrap() == dir);
        if !has_cue {
            results.push(ScannedDisc {
                name: iso.file_name().unwrap().to_string_lossy().to_string(),
                folder: dir.to_string_lossy().to_string(),
                kind: "iso".to_string(),
            });
        }
    }

    results
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo test scanner"
```

Expected: `test result: ok. 2 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add ui/src-tauri/src/scanner.rs ui/src-tauri/src/main.rs
git commit -m "Add recursive folder pre-scan for .cue/.iso discovery

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 5: Config and history persistence

**Files:**
- Create: `ui/src-tauri/src/settings.rs`
- Modify: `ui/src-tauri/src/main.rs` (add `mod settings;`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)] pub struct Config { pub chdman_path: String }`
  - `pub fn load_config(app_dir: &std::path::Path) -> Config`
  - `pub fn save_config(app_dir: &std::path::Path, config: &Config) -> std::io::Result<()>`
  - `#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)] pub struct RunRecord { pub timestamp: String, pub folder: String, pub converted: u32, pub skipped: u32, pub failed: u32, pub cancelled: bool }`
  - `pub fn load_history(app_dir: &std::path::Path) -> Vec<RunRecord>`
  - `pub fn append_history(app_dir: &std::path::Path, record: RunRecord) -> std::io::Result<()>`

Both files live in `app_dir` (a directory the Tauri command handlers resolve via `tauri::api::path::app_config_dir`, wired up in Task 6) as `config.json` and `historial.json` respectively.

- [ ] **Step 1: Write the failing tests**

Create `ui/src-tauri/src/settings.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    pub chdman_path: String,
}

pub fn load_config(app_dir: &Path) -> Config {
    todo!()
}

pub fn save_config(app_dir: &Path, config: &Config) -> io::Result<()> {
    todo!()
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
    todo!()
}

pub fn append_history(app_dir: &Path, record: RunRecord) -> io::Result<()> {
    todo!()
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
        let config = Config { chdman_path: "C:\\Tools\\chdman.exe".to_string() };
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
}
```

Add `mod settings;` to `ui/src-tauri/src/main.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo test settings"
```

Expected: compile/panic failure from `todo!()`.

- [ ] **Step 3: Implement the four functions**

```rust
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
```

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo test settings"
```

Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add ui/src-tauri/src/settings.rs ui/src-tauri/src/main.rs
git commit -m "Add config.json and historial.json persistence

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 6: Run/cancel commands wiring subprocess + tailer + events

**Files:**
- Modify: `ui/src-tauri/src/main.rs`
- Create: `ui/src-tauri/build-assets/` (holds a copy of `convertir_a_chd.bat` bundled into the build — see Step 1)

**Interfaces:**
- Consumes: `scanner::scan_folder` (Task 4), `log_tail::{LogTailer, parse_log_line, DiscStatus, LogEvent}` (Task 3), `settings::{Config, load_config, save_config, RunRecord, load_history, append_history}` (Task 5).
- Produces: four Tauri commands invokable from the frontend as `invoke("prescan", { root })`, `invoke("start_conversion", { root })`, `invoke("cancel_conversion")`, `invoke("get_config")`/`invoke("save_config", { chdmanPath })`, `invoke("get_history")`; and one emitted event, `"disc-updated"`, whose payload is `LogEvent` (serialized), plus a `"run-finished"` event with payload `{ converted: u32, skipped: u32, failed: u32, cancelled: bool }`.

- [ ] **Step 1: Bundle `convertir_a_chd.bat` into the Tauri build**

Copy the script into the UI project so it ships with the `.exe`:

```bash
mkdir -p /home/jimrod/chd-batch-converter/ui/src-tauri/build-assets
cp /home/jimrod/chd-batch-converter/convertir_a_chd.bat /home/jimrod/chd-batch-converter/ui/src-tauri/build-assets/
```

In `ui/src-tauri/tauri.conf.json`, add `"build-assets/convertir_a_chd.bat"` to the `"bundle"` → `"resources"` array (create that array if the scaffold didn't add one). This makes the file available at runtime next to the installed app via Tauri's resource-resolution API (`tauri::api::path::resource_dir`).

- [ ] **Step 2: Add application state and the four commands to `main.rs`**

Add near the top of `ui/src-tauri/src/main.rs` (after the `mod` declarations):

```rust
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::Manager;

use log_tail::{parse_log_line, LogTailer};
use scanner::scan_folder;
use settings::{append_history, load_config, load_history, save_config, Config, RunRecord};

struct RunState(Mutex<Option<Child>>);

#[tauri::command]
fn prescan(root: String) -> Vec<scanner::ScannedDisc> {
    scan_folder(std::path::Path::new(&root))
}

#[tauri::command]
fn get_config(app_handle: tauri::AppHandle) -> Config {
    let app_dir = app_handle.path_resolver().app_config_dir().expect("app config dir resolvable");
    load_config(&app_dir)
}

#[tauri::command]
fn set_config(app_handle: tauri::AppHandle, chdman_path: String) -> Result<(), String> {
    let app_dir = app_handle.path_resolver().app_config_dir().expect("app config dir resolvable");
    save_config(&app_dir, &Config { chdman_path }).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_history(app_handle: tauri::AppHandle) -> Vec<RunRecord> {
    let app_dir = app_handle.path_resolver().app_config_dir().expect("app config dir resolvable");
    load_history(&app_dir)
}

#[tauri::command]
fn cancel_conversion(state: tauri::State<RunState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(child) = guard.as_mut() {
        let pid = child.id();
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
        *guard = None;
    }
    Ok(())
}

#[tauri::command]
fn start_conversion(
    window: tauri::Window,
    app_handle: tauri::AppHandle,
    root: String,
    state: tauri::State<RunState>,
) -> Result<(), String> {
    let config = get_config(app_handle.clone());
    if config.chdman_path.is_empty() {
        return Err("chdman.exe path not configured".to_string());
    }

    let script_path = app_handle
        .path_resolver()
        .resolve_resource("build-assets/convertir_a_chd.bat")
        .ok_or("bundled convertir_a_chd.bat not found")?;

    let log_path = std::path::Path::new(&root).join("conversion_log.txt");
    let _ = std::fs::remove_file(&log_path); // start each run from a clean log for the tailer's offset to make sense

    let mut cmd = Command::new(&script_path);
    cmd.arg(&root)
        .env("CHDMAN_OVERRIDE", &config.chdman_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    *state.0.lock().map_err(|e| e.to_string())? = Some(child);

    let app_dir = app_handle.path_resolver().app_config_dir().expect("app config dir resolvable");
    let root_for_thread = root.clone();

    thread::spawn(move || {
        let mut tailer = LogTailer::new(log_path.clone());
        let mut converted = 0u32;
        let mut skipped = 0u32;
        let mut failed = 0u32;

        loop {
            if let Ok(lines) = tailer.read_new_lines() {
                for line in lines {
                    if let Some(event) = parse_log_line(&line) {
                        match event.status {
                            log_tail::DiscStatus::Ok => converted += 1,
                            log_tail::DiscStatus::Skip => skipped += 1,
                            log_tail::DiscStatus::Fail => failed += 1,
                        }
                        let _ = window.emit("disc-updated", &event);
                    }
                }
            }

            // Stop when the process has exited AND we've drained the log.
            let still_running = unsafe {
                // A lightweight "is this PID still alive" check via tasklist,
                // since std::process::Child doesn't expose non-blocking wait
                // across a Mutex boundary cleanly here.
                Command::new("tasklist")
                    .args(["/FI", &format!("PID eq {}", pid)])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                    .unwrap_or(false)
            };

            if !still_running {
                // One final drain in case the process wrote its last lines
                // between our last read and it exiting.
                if let Ok(lines) = tailer.read_new_lines() {
                    for line in lines {
                        if let Some(event) = parse_log_line(&line) {
                            match event.status {
                                log_tail::DiscStatus::Ok => converted += 1,
                                log_tail::DiscStatus::Skip => skipped += 1,
                                log_tail::DiscStatus::Fail => failed += 1,
                            }
                            let _ = window.emit("disc-updated", &event);
                        }
                    }
                }
                break;
            }

            thread::sleep(Duration::from_millis(300));
        }

        let record = RunRecord {
            timestamp: chrono_like_timestamp(),
            folder: root_for_thread,
            converted,
            skipped,
            failed,
            cancelled: false, // Task 7 refines this once cancel is wired to set a flag the thread can read
        };
        let _ = append_history(&app_dir, record.clone());
        let _ = window.emit("run-finished", &record);
    });

    Ok(())
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("{}", secs) // Unix timestamp; the frontend formats it for display (Task 7)
}
```

Register the state and commands in the existing `tauri::Builder::default()` chain in `fn main()` (the scaffold generates this — extend it, don't replace it):

```rust
.manage(RunState(Mutex::new(None)))
.invoke_handler(tauri::generate_handler![
    prescan,
    get_config,
    set_config,
    get_history,
    start_conversion,
    cancel_conversion
])
```

**Note on the cancel/history interaction:** this task's `cancelled` field is hardcoded `false` — Task 7 (frontend) tracks whether the user pressed Cancel and, since the background thread already reports a `RunRecord` unconditionally on process exit (including an exit caused by `taskkill`), the simplest correct fix is a small follow-up in Task 7: have `cancel_conversion` set a shared `AtomicBool` the background thread checks before writing `cancelled: false`, flipping it to `true` when set. Implement that flag now as part of this task rather than deferring it further:

Add above `struct RunState`:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

struct RunState(Mutex<Option<Child>>, AtomicBool);
```

Update every `state.0` reference above to stay as-is (unchanged field index), update `.manage(RunState(Mutex::new(None)))` to `.manage(RunState(Mutex::new(None), AtomicBool::new(false)))`, set `state.1.store(true, Ordering::SeqCst)` inside `cancel_conversion` right before clearing the guard, and in `start_conversion`, reset it at the top (`state.1.store(false, Ordering::SeqCst)`) and read it via a cloned `Arc` into the spawned thread — since `tauri::State` isn't `'static`, capture the flag by extracting it into an `Arc<AtomicBool>` stored in `RunState` instead of a bare `AtomicBool`:

```rust
struct RunState(Mutex<Option<Child>>, std::sync::Arc<AtomicBool>);
```

and in `start_conversion`, before spawning the thread:

```rust
let cancelled_flag = state.1.clone();
cancelled_flag.store(false, Ordering::SeqCst);
```

then inside the thread closure, replace `cancelled: false` with `cancelled: cancelled_flag.load(Ordering::SeqCst)`, and in `cancel_conversion`, add `state.1.store(true, Ordering::SeqCst);` before the `taskkill` call.

**Known scope limitation (documented, not silently dropped):** the spec's "`.bat` process exits non-zero mid-run" error-banner requirement is only reachable today via the missing-`chdman.exe` guard, which `start_conversion` already checks *before* spawning (so it can never actually fire mid-run in the current script). If `chdman.exe` is deleted or becomes inaccessible after that check but before/during the run, the current design has no distinct "run-error" event — the background thread will simply report whatever the log accumulated, same as a normal finish. Adding a true mid-run exit-code check would require passing the spawned `Child`'s ownership into the tailer thread (instead of `tasklist`-polling by PID) so it can call `child.wait()` and inspect the exit status directly — a reasonable v2 follow-up, but out of scope here per YAGNI, since the precondition that would trigger it (chdman disappearing mid-run) is rare and the missing-file guard already covers the common case.

- [ ] **Step 3: Build to confirm it compiles**

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo build"
```

Expected: builds with no errors (warnings about unused fields are acceptable at this stage — Task 7 wires the frontend that uses everything).

- [ ] **Step 4: Manual integration test against a real fixture**

```bash
WORK=/mnt/c/temp/chd_ui_backend_test
rm -rf "$WORK"
cp -r /home/jimrod/chd-batch-converter/tests/fixtures/nested_multidisc "$WORK"
```

Since there's no frontend yet to drive `invoke()`, verify this task's logic with a standalone Rust integration test instead. Create `ui/src-tauri/tests/start_conversion_smoke.rs`:

```rust
// This is a smoke test of the *process-spawning + tailing* pattern used by
// start_conversion, exercised directly against the mock chdman rather than
// through Tauri's IPC layer (which needs a running window to invoke).
use std::process::{Command, Stdio};
use std::{fs, thread, time::Duration};

#[test]
fn spawns_bat_and_tail_sees_all_expected_lines() {
    let repo_root = std::env::current_dir().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let bat = repo_root.join("convertir_a_chd.bat");
    let mock = repo_root.join("tests/mock_chdman.bat");
    assert!(bat.exists(), "expected {:?} to exist", bat);
    assert!(mock.exists(), "expected {:?} to exist", mock);

    let work = std::env::temp_dir().join("chd_ui_backend_integration_test");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(work.join("game")).unwrap();
    fs::write(
        work.join("game/Track.cue"),
        "FILE \"Track.bin\" BINARY\nTRACK 01 MODE1/2352\nINDEX 01 00:00:00\n",
    )
    .unwrap();
    fs::write(work.join("game/Track.bin"), "fake bin").unwrap();

    let mut child = Command::new(&bat)
        .arg(&work)
        .env("CHDMAN_OVERRIDE", &mock)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn convertir_a_chd.bat");

    child.wait().expect("wait on child");
    thread::sleep(Duration::from_millis(200)); // allow final log flush

    let log = fs::read_to_string(work.join("conversion_log.txt")).unwrap();
    assert!(log.contains("OK    | "), "expected an OK line in log:\n{}", log);
    assert!(work.join("game/Track.chd").exists());

    fs::remove_dir_all(&work).unwrap();
}
```

Run it:

```bash
cd /home/jimrod/chd-batch-converter/ui/src-tauri
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && cargo test --test start_conversion_smoke"
```

Expected: `test result: ok. 1 passed; 0 failed`. This is the critical proof that spawning the `.bat` with `Stdio::null()` (Task's earlier design decision for handling `pause`) doesn't hang — if it hangs, the test framework's own timeout will surface that clearly, which is exactly the risk this step exists to catch.

- [ ] **Step 5: Clean up and commit**

```bash
rm -rf /mnt/c/temp/chd_ui_backend_test
cd /home/jimrod/chd-batch-converter
git add ui/
git commit -m "Wire prescan/start/cancel commands with log-tailing and history

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 7: Frontend shell — folder picker, live table, progress bar

**Files:**
- Modify: `ui/src/index.html`
- Modify: `ui/src/main.js`
- Modify: `ui/src/styles.css` (filename per scaffold's vanilla template — adjust if the generated name differs, e.g. `style.css`)
- Modify: `ui/src-tauri/tauri.conf.json` (enable the dialog plugin's `open` permission for folder selection)

**Interfaces:**
- Consumes: the four commands and two events from Task 6 (`prescan`, `start_conversion`, `"disc-updated"`, `"run-finished"`).
- Produces: a working folder-pick → scan → convert → live-update flow. Settings/history panels come in Task 8.

- [ ] **Step 1: Enable the folder-dialog permission**

In `ui/src-tauri/tauri.conf.json`, under `"tauri"` → `"allowlist"`, ensure `"dialog"` is enabled with `"open": true` (add the block if the scaffold didn't include it):

```json
"dialog": {
  "open": true
}
```

- [ ] **Step 2: Write the dark theme base (Discord/Spotify-inspired)**

Replace the contents of `ui/src/styles.css`:

```css
:root {
  --bg: #1e1f22;
  --surface: #2b2d31;
  --surface-hover: #35373c;
  --text: #f2f3f5;
  --text-dim: #949ba4;
  --accent: #5865f2;
  --ok: #23a55a;
  --fail: #f23f42;
  --skip: #949ba4;
}

* { box-sizing: border-box; }

body {
  margin: 0;
  background: var(--bg);
  color: var(--text);
  font-family: "Segoe UI", system-ui, sans-serif;
}

#app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  padding: 24px;
  gap: 16px;
}

.toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
}

button {
  background: var(--accent);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 10px 18px;
  font-size: 14px;
  cursor: pointer;
  transition: filter 0.15s ease;
}

button:hover { filter: brightness(1.1); }
button:disabled { background: var(--surface-hover); color: var(--text-dim); cursor: not-allowed; }

button.secondary { background: var(--surface-hover); }

#folder-label {
  color: var(--text-dim);
  font-size: 13px;
}

#progress-bar-track {
  height: 8px;
  border-radius: 4px;
  background: var(--surface);
  overflow: hidden;
}

#progress-bar-fill {
  height: 100%;
  background: var(--accent);
  width: 0%;
  transition: width 0.2s ease;
}

#disc-table {
  flex: 1;
  overflow-y: auto;
  background: var(--surface);
  border-radius: 8px;
  padding: 8px;
}

.disc-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
}

.disc-row:hover { background: var(--surface-hover); }

.disc-status-icon { width: 20px; text-align: center; }
.disc-name { flex: 1; }
.disc-message { color: var(--text-dim); font-size: 12px; }

.status-ok { color: var(--ok); }
.status-fail { color: var(--fail); }
.status-skip { color: var(--skip); }
.status-pending { color: var(--text-dim); }
```

- [ ] **Step 3: Write the HTML shell**

Replace the `<body>` contents of `ui/src/index.html`:

```html
<div id="app">
  <div class="toolbar">
    <button id="pick-folder-btn">Elegir carpeta</button>
    <span id="folder-label">Ninguna carpeta seleccionada</span>
    <button id="convert-btn" disabled>Convertir todo</button>
    <button id="cancel-btn" class="secondary" style="display:none">Cancelar</button>
  </div>
  <div id="progress-bar-track"><div id="progress-bar-fill"></div></div>
  <div id="disc-table"></div>
</div>
<script type="module" src="/main.js"></script>
```

- [ ] **Step 4: Write the frontend logic**

Replace the contents of `ui/src/main.js`:

```javascript
const { invoke } = window.__TAURI__.tauri;
const { open } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;

let currentFolder = null;
let discs = []; // [{ name, folder, kind, status: "pending"|"ok"|"skip"|"fail", message: "" }]

const pickFolderBtn = document.getElementById("pick-folder-btn");
const folderLabel = document.getElementById("folder-label");
const convertBtn = document.getElementById("convert-btn");
const cancelBtn = document.getElementById("cancel-btn");
const progressFill = document.getElementById("progress-bar-fill");
const discTable = document.getElementById("disc-table");

function renderTable() {
  discTable.innerHTML = "";
  for (const disc of discs) {
    const row = document.createElement("div");
    row.className = "disc-row";
    const icon = { pending: "•", ok: "✅", skip: "⏭️", fail: "❌" }[disc.status];
    row.innerHTML = `
      <span class="disc-status-icon status-${disc.status}">${icon}</span>
      <span class="disc-name">${disc.name}</span>
      <span class="disc-message">${disc.message ?? ""}</span>
    `;
    discTable.appendChild(row);
  }
}

function updateProgress() {
  const done = discs.filter((d) => d.status !== "pending").length;
  const pct = discs.length === 0 ? 0 : Math.round((done / discs.length) * 100);
  progressFill.style.width = `${pct}%`;
}

pickFolderBtn.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;

  currentFolder = selected;
  folderLabel.textContent = selected;

  const scanned = await invoke("prescan", { root: selected });
  discs = scanned.map((d) => ({ ...d, status: "pending", message: "" }));
  renderTable();
  updateProgress();
  convertBtn.disabled = discs.length === 0;
});

convertBtn.addEventListener("click", async () => {
  try {
    await invoke("start_conversion", { root: currentFolder });
    convertBtn.style.display = "none";
    cancelBtn.style.display = "inline-block";
  } catch (err) {
    alert(`No se pudo iniciar la conversion: ${err}`);
  }
});

cancelBtn.addEventListener("click", async () => {
  await invoke("cancel_conversion");
});

// Windows paths use backslashes; the backend reports the source path exactly
// as chdman/cmd.exe see it, so we match on a case-insensitive suffix rather
// than requiring an exact string match against the pre-scan's own path join.
function matchDiscByPath(path) {
  const normalized = path.toLowerCase().replace(/\\/g, "/");
  return discs.find((d) => normalized.endsWith(d.name.toLowerCase()) && normalized.includes(d.folder.toLowerCase().replace(/\\/g, "/")));
}

listen("disc-updated", (event) => {
  const { status, path, message } = event.payload;
  const disc = matchDiscByPath(path);
  if (disc) {
    disc.status = status.toLowerCase(); // Rust enum serializes as "Ok" | "Skip" | "Fail"
    disc.message = message;
    renderTable();
    updateProgress();
  }
});

listen("run-finished", () => {
  convertBtn.style.display = "inline-block";
  cancelBtn.style.display = "none";
});
```

- [ ] **Step 5: Build and manually verify against `nested_multidisc`**

```bash
cd /home/jimrod/chd-batch-converter/ui
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && npm run tauri build"
```

Then, with the built `.exe`, manually: launch it, click "Elegir carpeta" and pick a copy of `tests/fixtures/nested_multidisc`, confirm the table populates with 5 pending rows (2 from `GameA`, 1 from `GameB`, 1 from `GameC`, 1 loose at root), configure a `chdman.exe` path (a `Settings` UI isn't built until Task 8 — for this manual check only, temporarily write `config.json` by hand into the app's config dir with a path to `tests/mock_chdman.bat`, then delete it after), click "Convertir todo", and confirm rows update live and the progress bar reaches 100%.

- [ ] **Step 6: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add ui/
git commit -m "Add frontend shell: folder picker, live table, progress bar

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 8: Settings and history panels

**Files:**
- Modify: `ui/src/index.html`
- Modify: `ui/src/main.js`
- Modify: `ui/src/styles.css`

**Interfaces:**
- Consumes: `get_config`/`set_config`/`get_history` commands from Task 6.
- Produces: a working Settings panel (chdman.exe path) and History panel (past runs), both toggleable from the toolbar.

- [ ] **Step 1: Add the panel markup**

In `ui/src/index.html`, add inside `#app`, after the toolbar `<div>` and before `#progress-bar-track`:

```html
<div class="toolbar">
  <button id="settings-btn" class="secondary">Configuracion</button>
  <button id="history-btn" class="secondary">Historial</button>
</div>

<div id="settings-panel" class="panel" style="display:none">
  <label for="chdman-path-input">Ruta de chdman.exe</label>
  <input id="chdman-path-input" type="text" placeholder="C:\Tools\chdman.exe" />
  <button id="save-settings-btn">Guardar</button>
</div>

<div id="history-panel" class="panel" style="display:none"></div>
```

- [ ] **Step 2: Add panel styles**

Append to `ui/src/styles.css`:

```css
.panel {
  background: var(--surface);
  border-radius: 8px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.panel label { color: var(--text-dim); font-size: 13px; }

.panel input {
  background: var(--bg);
  border: 1px solid var(--surface-hover);
  border-radius: 6px;
  padding: 8px 10px;
  color: var(--text);
  font-size: 14px;
}

.history-row {
  display: flex;
  gap: 12px;
  padding: 8px;
  border-bottom: 1px solid var(--surface-hover);
  font-size: 13px;
}

.history-row .history-folder { flex: 1; color: var(--text-dim); }
```

- [ ] **Step 3: Wire the panels in `main.js`**

Append to `ui/src/main.js`:

```javascript
const settingsBtn = document.getElementById("settings-btn");
const settingsPanel = document.getElementById("settings-panel");
const chdmanPathInput = document.getElementById("chdman-path-input");
const saveSettingsBtn = document.getElementById("save-settings-btn");
const historyBtn = document.getElementById("history-btn");
const historyPanel = document.getElementById("history-panel");

settingsBtn.addEventListener("click", async () => {
  historyPanel.style.display = "none";
  const isHidden = settingsPanel.style.display === "none";
  if (isHidden) {
    const config = await invoke("get_config");
    chdmanPathInput.value = config.chdman_path;
  }
  settingsPanel.style.display = isHidden ? "flex" : "none";
});

saveSettingsBtn.addEventListener("click", async () => {
  await invoke("set_config", { chdmanPath: chdmanPathInput.value });
  settingsPanel.style.display = "none";
});

historyBtn.addEventListener("click", async () => {
  settingsPanel.style.display = "none";
  const isHidden = historyPanel.style.display === "none";
  if (isHidden) {
    const history = await invoke("get_history");
    historyPanel.innerHTML = history
      .slice()
      .reverse()
      .map((run) => {
        const date = new Date(Number(run.timestamp) * 1000).toLocaleString();
        const status = run.cancelled
          ? "Cancelado"
          : `${run.converted} convertidos, ${run.skipped} saltados, ${run.failed} fallidos`;
        return `<div class="history-row"><span class="history-folder">${run.folder}</span><span>${date}</span><span>${status}</span></div>`;
      })
      .join("");
  }
  historyPanel.style.display = isHidden ? "flex" : "none";
});
```

- [ ] **Step 4: Build and manually verify**

```bash
cd /home/jimrod/chd-batch-converter/ui
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && npm run tauri build"
```

Launch the built `.exe`: open Settings, type a path to a real `chdman.exe` (or `tests/mock_chdman.bat` for a dry run), save, run a conversion against a copy of `tests/fixtures/cue_basic`, then open History and confirm the run appears with correct counts and a readable timestamp.

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add ui/
git commit -m "Add settings and history panels

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 9: End-to-end smoke test with the real `chdman.exe` and packaging polish

**Files:**
- Modify: `ui/README.md`
- Modify: `ui/src-tauri/tauri.conf.json` (app metadata: `productName`, `identifier`, window title/size)

**Interfaces:**
- Consumes: the fully wired app from Tasks 1-8.
- Produces: a verified, distributable `.exe`.

- [ ] **Step 1: Set final app metadata**

In `ui/src-tauri/tauri.conf.json`, set (adjust key paths to match the generated schema version — Tauri 1.x nests these under `"package"`/`"tauri"`, Tauri 2.x differently; use whichever the scaffold from Task 2 produced):
- `productName`: `"CHD Converter"`
- window `title`: `"CHD Converter"`, `width`: `900`, `height`: `640`
- a stable `identifier` (reverse-domain style is conventional, but any unique string works for a local, unsigned build), e.g. `"com.local.chd-converter-ui"`

- [ ] **Step 2: Real-binary end-to-end smoke test**

```bash
WORK=/mnt/c/temp/chd_ui_real_smoke
rm -rf "$WORK"
mkdir -p "$WORK/game"
cd "$WORK/game"
dd if=/dev/urandom of=Track.bin bs=2352 count=75 status=none
printf 'FILE "Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > Track.cue
```

Launch the built `ui/src-tauri/target/release/*.exe`, set the chdman path in Settings to the repo's real `/home/jimrod/chd-batch-converter/chdman.exe` (translate to its Windows path when entering it in the UI, e.g. via `wslpath -w`), pick `$WORK` as the folder, run the conversion, and confirm: the table shows one row transitioning from pending to ✅, the progress bar reaches 100%, `Track.chd` exists in `$WORK/game/`, and the History panel shows one entry with `Convertidos=1`.

```bash
rm -rf /mnt/c/temp/chd_ui_real_smoke
```

- [ ] **Step 3: Update `ui/README.md` with final usage instructions**

Add a "Usage" section after "Build":

```markdown
## Usage

1. Run the built `.exe`.
2. Open Settings and set the path to your `chdman.exe`.
3. Click "Elegir carpeta" and pick your ROMs folder.
4. Review the detected games, then click "Convertir todo".
5. Watch live progress; use "Cancelar" to stop early if needed.
6. Check "Historial" any time for past runs.
```

- [ ] **Step 4: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add ui/
git commit -m "Finalize app metadata and document usage

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```
