# CHD Batch Converter — Design Spec

**Date:** 2026-09-14
**Status:** Approved for implementation

## Purpose

Provide a self-contained Windows `.bat` script that uses `chdman.exe`
(bundled with MAME) to bulk-convert PS1/PS2/PSP disc images
(`.bin`/`.cue` and `.iso`) into compressed, playable `.chd` files,
without requiring the user to run any command by hand per game.

A future phase may wrap this logic in a GUI, but that is out of scope
for this spec.

## Non-goals

- No GUI in this phase (see "Future UI" below for direction only).
- No modification, deletion, or relocation of source `.bin`/`.cue`/`.iso`
  files.
- No support for other console formats (no-intro N64, GameCube/Wii,
  Saturn, etc.) — scope is PS1/PS2/PSP disc images only.
- No parallel/multi-threaded conversion — sequential processing is
  acceptable for this phase.

## Environment & Assumptions

- Target OS: Windows (`.bat`, `cmd.exe`).
- `chdman.exe` lives in the **same folder** as the script. If it is
  missing, the script prints an error and exits before processing
  anything.
- The script is placed at the root of a ROMs collection. That root may
  contain:
  - subfolders, one per game (typical case), and/or
  - loose `.bin`/`.cue`/`.iso` files directly in the root (fallback
    case).
- Recursion depth is unbounded: the script walks the full directory
  tree under its own location, so it works for one level of
  subfolders (the common case) as well as deeper nesting.

## Behavior

### 1. Discovery

For every directory under the script's location (including the
script's own directory):

- `.chd` files are ignored for conversion purposes (they only matter
  for the "already converted" check in step 3).
- `.cue` files are treated as PS1 disc images (their companion `.bin`
  files are referenced by the `.cue` and are not iterated directly).
- `.iso` files **without** an associated `.cue` in the same directory
  are treated as PS2/PSP disc images.
- A directory may contain multiple `.cue`/`.iso` files (multi-disc
  games, e.g. `Disc1.cue`, `Disc2.cue`); each one is converted and
  verified independently.

### 2. Conversion

- `.cue` → `chdman createcd -i "<file>.cue" -o "<file>.chd"`
- `.iso` → `chdman createdvd -i "<file>.iso" -o "<file>.chd"`
- Output `.chd` is written **next to** its source file, i.e. inside
  the same (possibly nested) folder.

### 3. Skip already-converted games

- Before converting, the script checks whether `<file>.chd` already
  exists in the target directory. If it does, the conversion (and
  verification) for that file is skipped and logged as "skipped".
- This makes the script safe to re-run over a partially processed
  collection.

### 4. Verification

- After a successful conversion, the script runs
  `chdman verify -i "<file>.chd"`.
- The pass/fail result of the verification is recorded per file.
- A failed verification does **not** delete the `.chd` or the source
  files; it is flagged in the log and in the on-screen summary so the
  user can investigate.

### 5. Source files

- `.bin`, `.cue`, and `.iso` files are never modified, moved, or
  deleted by this script, regardless of conversion/verification
  outcome.

### 6. Logging

- A `conversion_log.txt` file is created/appended at the root folder
  (where the `.bat` lives).
- Each entry includes: timestamp, folder, source file, conversion
  result, verification result.
- At the end of the run, a summary is printed to the console (and
  appended to the log): counts of converted, skipped, and failed
  items.

### 7. Error handling

- Missing `chdman.exe` next to the script → print a clear error and
  exit immediately, before touching any files.
- A failed `createcd`/`createdvd` call is logged as a failure for that
  file and the script continues with the next file (one bad game must
  not abort the whole batch).

## Testing approach

- Manual verification using a small set of dummy/sample `.bin`/`.cue`
  and `.iso` files arranged in:
  - a root folder with no subfolders (loose files case),
  - a root folder with one level of per-game subfolders,
  - a root folder with nested subfolders (depth 2+),
  - a folder with a multi-disc game (two `.cue` files),
  - a folder where a `.chd` already exists (skip case),
  - a folder where a source file is corrupt/invalid (failure case).
- Confirm `conversion_log.txt` content and on-screen summary match
  expectations for each case.
- No automated test framework is introduced — this is a CLI batch
  script validated by manual runs against representative folder
  layouts.

## Future UI (direction only, not part of this implementation)

Once the `.bat` is validated, a lightweight GUI (e.g. PowerShell
WinForms, or Python + Tkinter/PyQt) could let the user pick the root
folder, show live progress, and surface the same
`conversion_log.txt`. The GUI would shell out to the same conversion
logic rather than reimplementing it, to avoid duplicating the
`chdman` invocation rules above.
