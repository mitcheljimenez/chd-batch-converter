# CHD Converter UI — Design Spec

**Date:** 2026-09-15
**Status:** Approved for implementation

## Purpose

Provide a modern, desktop GUI for `convertir_a_chd.bat` (see
`docs/superpowers/specs/2026-09-14-chd-batch-converter-design.md`), so the
user can pick a folder, watch conversion progress live, cancel mid-run, and
review past runs, without touching a console. The GUI must ship as a
standalone Windows `.exe` requiring no runtime installation on the end
user's machine.

## Non-goals

- No reimplementation of discovery/conversion/verify rules. The `.bat`
  remains the single source of truth for what gets converted, how, and
  when something is skipped or fails — the UI only orchestrates and
  displays it.
- No macOS/Linux support — Windows only, matching the underlying `.bat`
  and `chdman.exe`.
- No editing of `chdman`'s advanced compression flags in this version
  (out of scope; the UI only lets the user point at a `chdman.exe` path).
- No packaging/installer polish (code signing, auto-update) — a plain
  `.exe` produced by `tauri build` is sufficient for this version.

## Tech stack

**Tauri** (Rust backend + WebView2-rendered HTML/CSS/JS frontend).
Chosen over Electron (same visual capability, but 80-150MB binaries vs.
Tauri's few-MB output, since Tauri reuses the OS's built-in WebView2
instead of bundling Chromium) and over Python+PySide6 (harder to reach the
"Discord/Spotify-like" polish the user wants, and PyInstaller binaries are
more prone to antivirus false positives). Build toolchain (Rust, Node) is
installed on the user's real Windows machine (via WSL2 interop) by whoever
builds the app; the *shipped* `.exe` needs nothing extra on any Windows
10/11 machine (WebView2 is preinstalled on both).

## Architecture

Two halves, as in any Tauri app:

- **Backend (Rust):** never reimplements conversion logic. It shells out to
  `convertir_a_chd.bat` (the existing, already-tested script) as a child
  process pointed at the user-chosen folder, with `CHDMAN_OVERRIDE` set to
  the configured `chdman.exe` path (reusing the exact test seam already
  built into the `.bat` for `tests/run_scenario.sh`). While the process
  runs, the backend tails `conversion_log.txt` incrementally (remembering
  its last-read byte offset, so a disconnected/reconnected frontend never
  loses or duplicates lines) and emits a structured event per new log line.
- **Frontend (HTML/CSS/JS):** folder picker, live per-game status table,
  overall progress bar, cancel button, run-history panel, and a
  `chdman.exe` path setting. Purely reactive to backend events — holds no
  conversion logic of its own.

## Components & data flow

1. **Folder picker:** native Windows dialog (Tauri's dialog plugin). On
   selection, the frontend asks the backend for a pre-scan: a recursive,
   read-only count of `.cue`/`.iso` files (no conversion yet) that
   populates the table with each detected game (name, folder, status
   "Pendiente").
2. **"Convertir todo" button:** triggers the backend to launch
   `convertir_a_chd.bat` for real over that folder. As the backend tails
   `conversion_log.txt`, it parses each `OK | path | message` /
   `SKIP | ... ` / `FAIL | ...` line into a typed event (`disc-updated`)
   the frontend uses to update that row's icon (✅/⏭️/❌) and the overall
   progress bar (against the count from step 1).
3. **"Cancelar" button:** visible only while a run is active. Kills the
   process tree (`taskkill /T /F` on the spawned `cmd.exe`'s PID, which
   also kills any in-flight `chdman.exe`). Games already completed keep
   their status; games not yet reached revert to "Pendiente" (safe to
   re-run later — the `.bat`'s own skip-if-exists logic handles resuming).
4. **History:** every completed run (cancelled or not) is appended as an
   entry to a `historial.json` the backend manages directly (timestamp,
   folder, converted/skipped/failed counts) — not parsed from the
   free-text log, for reliability. A side panel lists past runs.
5. **Settings:** a `chdman.exe` path field, persisted to a `config.json`
   next to the UI's `.exe`. If unset, the backend falls back to the path
   next to `convertir_a_chd.bat` (which ships alongside the UI).

## Error handling

- **`chdman.exe` not configured or path invalid:** detected by the UI
  *before* starting a run — "Convertir todo" stays disabled with a clear
  message, rather than letting the `.bat`'s own guard fail mid-run.
- **`.bat` process exits non-zero for reasons other than user cancel:**
  the backend stops tailing and the UI shows an error banner with the
  actual message from the log, not a generic failure.
- **Cancellation:** treated as its own status ("Cancelado por el
  usuario") in both the table and history — never conflated with a real
  failure.
- **No games found in the chosen folder:** "Convertir todo" stays
  disabled with an explanatory message; the pre-scan already knows the
  count is zero.
- **Log grows while the frontend isn't listening** (app minimized, etc.):
  the backend's incremental byte-offset tracking means reconnecting picks
  up exactly where it left off.

## Testing approach

- **Backend (Rust) unit tests:** for the log-line parser (raw line →
  typed status struct) and the incremental-tail-offset logic — genuine
  unit tests, since these are pure Rust functions, unlike the `.bat`
  itself.
- **Manual integration testing:** run the UI against the same fixture
  folders already used for the `.bat`'s own test suite (e.g.
  `nested_multidisc`, `cue_bang`) and confirm the table, progress bar, and
  history match what the console-based tests already established as
  correct.
- **Cancellation:** verified manually against a larger test folder, since
  reliably automating mid-run process-killing is impractical here.
- No reimplementation of the `.bat`'s own 10-scenario regression suite —
  that coverage is inherited for free by shelling out to the same script.
