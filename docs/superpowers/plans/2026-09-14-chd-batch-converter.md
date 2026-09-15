# CHD Batch Converter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `convertir_a_chd.bat`, a self-contained Windows batch script that recursively finds PS1 `.bin`/`.cue` and PS2/PSP `.iso` disc images, converts each to a verified `.chd` next to its source using `chdman.exe`, and logs the results.

**Architecture:** A single batch file at the repo root does discovery (via `for /r`), conversion, and verification through one `:process_disc` subroutine shared by both the `.cue` and `.iso` code paths, writing to a `conversion_log.txt`. Tests run the *real* script against a *mock* `chdman.bat` (selected via an env var seam, `CHDMAN_OVERRIDE`, that defaults to the real `chdman.exe` path so end-user behavior is unchanged) executed through `cmd.exe` via WSL2 interop against fixtures under `/mnt/c/temp/`.

**Tech Stack:** Windows `cmd.exe` batch scripting (`.bat`), bash test harness (`tests/run_scenario.sh`) invoking `cmd.exe` through WSL2 interop, git/GitHub for version control.

**Spec:** `docs/superpowers/specs/2026-09-14-chd-batch-converter-design.md`

## Global Constraints

- `chdman.exe` must live in the same folder as the script; if missing, the script errors and exits before touching any files (spec §Environment, §7).
- Source `.bin`/`.cue`/`.iso` files are never modified, moved, or deleted (spec §5).
- `.cue` → `chdman createcd -i "<file>.cue" -o "<file>.chd"` (spec §2).
- `.iso` (only when no `.cue` exists in the same directory) → `chdman createdvd -i "<file>.iso" -o "<file>.chd"` (spec §1, §2).
- Output `.chd` is written next to its source file (spec §2).
- If `<file>.chd` already exists, skip conversion and verification for that file (spec §3).
- After a successful conversion, run `chdman verify -i "<file>.chd"` and record pass/fail (spec §4).
- All results are logged to `conversion_log.txt` at the script's root with timestamp, folder, file, conversion result, verification result, plus an end-of-run summary (spec §6).
- A failed conversion/verification for one file must not abort the batch — the script continues with the next file (spec §7).
- Recursion covers the script's own folder and all nested subfolders, any depth (spec §Environment).

---

## Task 1: Scaffolding, chdman-missing guard, and the test harness

**Files:**
- Create: `convertir_a_chd.bat`
- Create: `tests/mock_chdman.bat`
- Create: `tests/run_scenario.sh`
- Create: `.gitignore`

**Interfaces:**
- Produces: `convertir_a_chd.bat` reads env var `CHDMAN_OVERRIDE` (full path to a `chdman`-compatible executable/script); if unset, defaults to `%~dp0chdman.exe`. Exits `/b 1` with an error message on stdout if the resolved chdman path does not exist.
- Produces: `tests/mock_chdman.bat <createcd|createdvd|verify> -i <input> [-o <output>]` — writes `MOCK_CHDMAN <cmd> input=... output=...` to stdout; for `createcd`/`createdvd`, writes `FAKE_CHD_CONTENT for <input>` into `<output>` and exits 0, UNLESS `<input>` contains `BADCUE` or `BADISO` (case-insensitive), in which case it prints `mock convert: FAILED` and exits 1 without creating the output file. For `verify`, exits 0 printing `mock verify: ok` UNLESS `<input>` contains `BADVERIFY` (case-insensitive), in which case it prints `mock verify: FAILED` and exits 1.
- Produces: `tests/run_scenario.sh <scenario_name>` — copies `tests/fixtures/<scenario_name>/` plus the current `convertir_a_chd.bat` and `tests/mock_chdman.bat` into a fresh `/mnt/c/temp/chd_test_<scenario_name>/`, runs `convertir_a_chd.bat` there via `cmd.exe` with `CHDMAN_OVERRIDE` pointed at the copied mock, and leaves the resulting tree in place at `/mnt/c/temp/chd_test_<scenario_name>/` for the caller to inspect (that path is also printed as the last line, prefixed `RESULT_DIR:`).
- Consumes: nothing (first task).

- [ ] **Step 1: Write the mock chdman fixture**

Create `tests/mock_chdman.bat`:

```bat
@echo off
rem Mimics chdman.exe's CLI surface for automated tests. Not real chdman.
setlocal enabledelayedexpansion
set "CMD=%~1"
set "INPUT="
set "OUTPUT="
shift

:parse
if "%~1"=="" goto :run
if /I "%~1"=="-i" (
    set "INPUT=%~2"
    shift
    shift
    goto :parse
)
if /I "%~1"=="-o" (
    set "OUTPUT=%~2"
    shift
    shift
    goto :parse
)
shift
goto :parse

:run
echo MOCK_CHDMAN %CMD% input=%INPUT% output=%OUTPUT%

if /I "%CMD%"=="verify" (
    echo %INPUT% | findstr /I "BADVERIFY" >nul
    if not errorlevel 1 (
        echo mock verify: FAILED
        exit /b 1
    )
    echo mock verify: ok
    exit /b 0
)

echo %INPUT% | findstr /I "BADCUE BADISO" >nul
if not errorlevel 1 (
    echo mock convert: FAILED
    exit /b 1
)
echo FAKE_CHD_CONTENT for %INPUT%> "%OUTPUT%"
exit /b 0
```

- [ ] **Step 2: Write the test harness script**

Create `tests/run_scenario.sh`:

```bash
#!/bin/bash
# Usage: tests/run_scenario.sh <scenario_name>
# Runs convertir_a_chd.bat for real (via cmd.exe/WSL2 interop) against the
# fixture tree in tests/fixtures/<scenario_name>/, using tests/mock_chdman.bat
# in place of a real chdman.exe. Leaves the result tree in place and prints
# its path as "RESULT_DIR:<path>".
set -euo pipefail

SCENARIO="$1"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$REPO_DIR/tests/fixtures/$SCENARIO"

if [ ! -d "$FIXTURE_DIR" ]; then
    echo "No such fixture: $FIXTURE_DIR" >&2
    exit 1
fi

WORK="/mnt/c/temp/chd_test_${SCENARIO}"
rm -rf "$WORK"
mkdir -p "$WORK"
cp -r "$FIXTURE_DIR/." "$WORK/"
cp "$REPO_DIR/convertir_a_chd.bat" "$WORK/"
cp "$REPO_DIR/tests/mock_chdman.bat" "$WORK/mock_chdman.bat"

cd "$WORK"
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && set CHDMAN_OVERRIDE=$WINDIR\\mock_chdman.bat && convertir_a_chd.bat"

echo "RESULT_DIR:$WORK"
```

- [ ] **Step 3: Make the harness executable and add a .gitignore**

```bash
chmod +x /home/jimrod/chd-batch-converter/tests/run_scenario.sh
```

Create `.gitignore`:

```
chdman.exe
tests/fixtures/**/*.chd
tests/fixtures/**/conversion_log.txt
```

(`chdman.exe` is a third-party MAME binary the user placed at the repo root for local testing — it must never be committed.)

(Fixtures' generated `.chd`/log files are test byproducts written under `tests/fixtures/<scenario>/` only if a scenario is run in-place; the real runs happen under `/mnt/c/temp/`, which is outside the repo, so this is a defensive ignore, not the primary mechanism.)

- [ ] **Step 4: Write the chdman-missing-guard fixture**

Create empty directory `tests/fixtures/missing_chdman/` (no files needed — the test harness will NOT be used for this one since we're testing absence of `mock_chdman.bat`). Instead, write a dedicated bash check as part of Step 6 below that runs the real script directly without a mock present.

- [ ] **Step 5: Write `convertir_a_chd.bat` with only the chdman-presence guard**

Create `convertir_a_chd.bat`:

```bat
@echo off
setlocal enabledelayedexpansion

set "ROOT=%~dp0"
if defined CHDMAN_OVERRIDE (
    set "CHDMAN=%CHDMAN_OVERRIDE%"
) else (
    set "CHDMAN=%ROOT%chdman.exe"
)
set "LOG=%ROOT%conversion_log.txt"

if not exist "%CHDMAN%" (
    echo ERROR: chdman.exe no se encontro junto a este script ^(se esperaba en "%CHDMAN%"^).
    echo Descarga chdman.exe del paquete de herramientas de MAME y colocalo en esta carpeta.
    exit /b 1
)

set /a COUNT_CONVERTED=0
set /a COUNT_SKIPPED=0
set /a COUNT_FAILED=0

call :log "==== Ejecucion iniciada %DATE% %TIME% ===="

call :log "==== Resumen: Convertidos=!COUNT_CONVERTED! Saltados=!COUNT_SKIPPED! Fallidos=!COUNT_FAILED! ===="
echo.
echo Listo. Convertidos=!COUNT_CONVERTED!  Saltados=!COUNT_SKIPPED!  Fallidos=!COUNT_FAILED!
echo Ver "%LOG%" para el detalle.
exit /b 0

:log
echo %DATE% %TIME% ^| %~1>> "%LOG%"
goto :eof
```

This is intentionally a stub for discovery/conversion (added in Tasks 2-4) — it already implements the guard, logging subroutine, and summary output shape end to end so it's independently testable.

- [ ] **Step 6: Test the chdman-missing guard and the summary/log plumbing**

```bash
mkdir -p /mnt/c/temp/chd_test_no_chdman
cp /home/jimrod/chd-batch-converter/convertir_a_chd.bat /mnt/c/temp/chd_test_no_chdman/
cd /mnt/c/temp/chd_test_no_chdman
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && convertir_a_chd.bat"; echo "EXIT_CODE:$?"
test ! -f conversion_log.txt && echo "PASS: no log written when chdman missing" || echo "FAIL: log was written"
```

Expected: prints an `ERROR: chdman.exe no se encontro...` line, `EXIT_CODE:1`, and `PASS: no log written when chdman missing`.

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh missing_chdman 2>&1 | tail -20
grep -q "Resumen: Convertidos=0 Saltados=0 Fallidos=0" /mnt/c/temp/chd_test_missing_chdman/conversion_log.txt && echo "PASS: summary logged with zero counts" || echo "FAIL"
```

Expected: `PASS: summary logged with zero counts` (this run has the mock present via the harness, so the guard passes and we're validating the empty-run summary/log plumbing).

- [ ] **Step 7: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add -A
git commit -m "Add chdman guard, log/summary plumbing, and test harness

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 2: `.cue` discovery, skip-if-exists, and conversion (no verify yet)

**Files:**
- Modify: `convertir_a_chd.bat`
- Create: `tests/fixtures/cue_basic/game/Track.cue`
- Create: `tests/fixtures/cue_basic/game/Track.bin`
- Create: `tests/fixtures/cue_skip/game/Track.cue`
- Create: `tests/fixtures/cue_skip/game/Track.chd`
- Create: `tests/fixtures/cue_fail/game/BADCUE_Track.cue`

**Interfaces:**
- Consumes: `:log` subroutine and `CHDMAN`/`LOG`/`COUNT_*` variables from Task 1.
- Produces: `:process_disc "<src>" "<dir>" "<base>" <subcmd>` subroutine — on entry, if `<dir><base>.chd` exists, logs `SKIP  | <src> | ya existe .chd` and increments `COUNT_SKIPPED`; otherwise calls `chdman <subcmd> -i "<src>" -o "<dir><base>.chd"`, and on failure logs `FAIL  | <src> | fallo la conversion` and increments `COUNT_FAILED`; on success (this task) logs `OK    | <src> | convertido` and increments `COUNT_CONVERTED`. (Verification is added in Task 3, replacing the "on success" branch.)

- [ ] **Step 1: Create fixtures**

```bash
mkdir -p /home/jimrod/chd-batch-converter/tests/fixtures/cue_basic/game
echo "fake bin" > /home/jimrod/chd-batch-converter/tests/fixtures/cue_basic/game/Track.bin
printf 'FILE "Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > /home/jimrod/chd-batch-converter/tests/fixtures/cue_basic/game/Track.cue

mkdir -p /home/jimrod/chd-batch-converter/tests/fixtures/cue_skip/game
printf 'FILE "Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > /home/jimrod/chd-batch-converter/tests/fixtures/cue_skip/game/Track.cue
echo "pre-existing chd" > /home/jimrod/chd-batch-converter/tests/fixtures/cue_skip/game/Track.chd

mkdir -p /home/jimrod/chd-batch-converter/tests/fixtures/cue_fail/game
printf 'FILE "BADCUE_Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > /home/jimrod/chd-batch-converter/tests/fixtures/cue_fail/game/BADCUE_Track.cue
```

- [ ] **Step 2: Run the not-yet-implemented scenario to confirm it currently does nothing**

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh cue_basic
test -f /mnt/c/temp/chd_test_cue_basic/game/Track.chd && echo "UNEXPECTED PASS" || echo "EXPECTED FAIL: no .chd yet"
```

Expected: `EXPECTED FAIL: no .chd yet` (discovery isn't wired up).

- [ ] **Step 3: Add `.cue` discovery and `:process_disc` to `convertir_a_chd.bat`**

Replace the two `call :log "==== Resumen..."` block's preceding blank area — insert the discovery loop right after the `call :log "==== Ejecucion iniciada..."` line and before the summary log line — and add the `:process_disc` subroutine after `:log`:

```bat
for /r "%ROOT%" %%F in (*.cue) do (
    call :process_disc "%%~fF" "%%~dpF" "%%~nF" createcd
)
```

```bat
:process_disc
setlocal
set "SRC=%~1"
set "DIR=%~2"
set "BASE=%~3"
set "SUBCMD=%~4"
set "OUT=%DIR%%BASE%.chd"

if exist "%OUT%" (
    call :log "SKIP  | %SRC% | ya existe .chd"
    endlocal
    set /a COUNT_SKIPPED+=1
    goto :eof
)

call "%CHDMAN%" %SUBCMD% -i "%SRC%" -o "%OUT%" >> "%LOG%" 2>&1
if errorlevel 1 (
    call :log "FAIL  | %SRC% | fallo la conversion"
    endlocal
    set /a COUNT_FAILED+=1
    goto :eof
)

call :log "OK    | %SRC% | convertido"
endlocal
set /a COUNT_CONVERTED+=1
goto :eof
```

- [ ] **Step 4: Run the three `.cue` scenarios and verify**

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh cue_basic
test -f /mnt/c/temp/chd_test_cue_basic/game/Track.chd && echo "PASS: chd created" || echo "FAIL: chd missing"
grep -q "OK.*Track.cue.*convertido" /mnt/c/temp/chd_test_cue_basic/conversion_log.txt && echo "PASS: OK logged" || echo "FAIL: OK not logged"

bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh cue_skip
grep -q "SKIP.*Track.cue.*ya existe" /mnt/c/temp/chd_test_cue_skip/conversion_log.txt && echo "PASS: skip logged" || echo "FAIL: skip not logged"
grep -q "pre-existing chd" /mnt/c/temp/chd_test_cue_skip/game/Track.chd && echo "PASS: original chd untouched" || echo "FAIL: chd was overwritten"

bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh cue_fail
test ! -f /mnt/c/temp/chd_test_cue_fail/game/BADCUE_Track.chd && echo "PASS: no chd on failure" || echo "FAIL: chd created despite failure"
grep -q "FAIL.*BADCUE_Track.cue.*fallo la conversion" /mnt/c/temp/chd_test_cue_fail/conversion_log.txt && echo "PASS: failure logged" || echo "FAIL: failure not logged"
```

Expected: all six lines print `PASS: ...`.

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add -A
git commit -m "Add .cue discovery, skip-if-exists, and createcd conversion

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 3: Verification step after conversion

**Files:**
- Modify: `convertir_a_chd.bat`
- Create: `tests/fixtures/cue_verify_fail/game/BADVERIFY_Track.cue`

**Interfaces:**
- Consumes: `:process_disc` from Task 2 (its "on success" branch is replaced).
- Produces: `:process_disc` now also runs `chdman verify -i "<out>"` after a successful conversion; verify failure logs `FAIL  | <src> | convertido pero VERIFY FALLO` and increments `COUNT_FAILED` instead of `COUNT_CONVERTED`; verify success logs `OK    | <src> | convertido y verificado` and increments `COUNT_CONVERTED`.

- [ ] **Step 1: Create the verify-fail fixture**

```bash
mkdir -p /home/jimrod/chd-batch-converter/tests/fixtures/cue_verify_fail/game
printf 'FILE "BADVERIFY_Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > /home/jimrod/chd-batch-converter/tests/fixtures/cue_verify_fail/game/BADVERIFY_Track.cue
```

(The mock's convert step doesn't fail on `BADVERIFY` in the input name, only on `BADCUE`/`BADISO`, so this produces a `.chd` that then fails verification — matching how a real corrupt-but-created `.chd` would behave.)

- [ ] **Step 2: Run it against the current script to confirm verify isn't checked yet**

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh cue_verify_fail
grep -q "OK.*BADVERIFY_Track.cue.*convertido" /mnt/c/temp/chd_test_cue_verify_fail/conversion_log.txt && echo "EXPECTED: currently logs OK (no verify yet)" || echo "UNEXPECTED"
```

Expected: `EXPECTED: currently logs OK (no verify yet)`.

- [ ] **Step 3: Add the verify call to `:process_disc`**

Replace the success branch:

```bat
call :log "OK    | %SRC% | convertido"
endlocal
set /a COUNT_CONVERTED+=1
goto :eof
```

with:

```bat
call "%CHDMAN%" verify -i "%OUT%" >> "%LOG%" 2>&1
if errorlevel 1 (
    call :log "FAIL  | %SRC% | convertido pero VERIFY FALLO"
    endlocal
    set /a COUNT_FAILED+=1
    goto :eof
)

call :log "OK    | %SRC% | convertido y verificado"
endlocal
set /a COUNT_CONVERTED+=1
goto :eof
```

- [ ] **Step 4: Re-run all `.cue` scenarios (regression) plus the new one**

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh cue_basic
grep -q "OK.*Track.cue.*convertido y verificado" /mnt/c/temp/chd_test_cue_basic/conversion_log.txt && echo "PASS: basic still converts+verifies" || echo "FAIL"

bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh cue_verify_fail
grep -q "FAIL.*BADVERIFY_Track.cue.*VERIFY FALLO" /mnt/c/temp/chd_test_cue_verify_fail/conversion_log.txt && echo "PASS: verify failure logged" || echo "FAIL"
test -f /mnt/c/temp/chd_test_cue_verify_fail/game/BADVERIFY_Track.chd && echo "PASS: chd kept despite failed verify (spec: never delete)" || echo "FAIL: chd was deleted"
```

Expected: all three print `PASS: ...`.

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add -A
git commit -m "Verify each converted .chd and only count it on verify success

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 4: `.iso` discovery (skip when a `.cue` is present) and `createdvd` conversion

**Files:**
- Modify: `convertir_a_chd.bat`
- Create: `tests/fixtures/iso_basic/game/Disc.iso`
- Create: `tests/fixtures/iso_ignored_with_cue/game/Disc.iso`
- Create: `tests/fixtures/iso_ignored_with_cue/game/Other.cue`

**Interfaces:**
- Consumes: `:process_disc` from Task 3 (reused unchanged, called with `createdvd`).
- Produces: a second discovery loop over `*.iso` that checks for any `*.cue` in the same directory before calling `:process_disc`.

- [ ] **Step 1: Create fixtures**

```bash
mkdir -p /home/jimrod/chd-batch-converter/tests/fixtures/iso_basic/game
echo "fake iso" > /home/jimrod/chd-batch-converter/tests/fixtures/iso_basic/game/Disc.iso

mkdir -p /home/jimrod/chd-batch-converter/tests/fixtures/iso_ignored_with_cue/game
echo "fake iso" > /home/jimrod/chd-batch-converter/tests/fixtures/iso_ignored_with_cue/game/Disc.iso
printf 'FILE "Other.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > /home/jimrod/chd-batch-converter/tests/fixtures/iso_ignored_with_cue/game/Other.cue
```

- [ ] **Step 2: Confirm `.iso` is currently ignored entirely**

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh iso_basic
test ! -f /mnt/c/temp/chd_test_iso_basic/game/Disc.chd && echo "EXPECTED: no chd yet for iso" || echo "UNEXPECTED"
```

Expected: `EXPECTED: no chd yet for iso`.

- [ ] **Step 3: Add `.iso` discovery to `convertir_a_chd.bat`**

Insert right after the existing `.cue` `for /r` loop:

```bat
for /r "%ROOT%" %%F in (*.iso) do (
    set "HASCUE="
    for %%C in ("%%~dpF*.cue") do set "HASCUE=1"
    if not defined HASCUE (
        call :process_disc "%%~fF" "%%~dpF" "%%~nF" createdvd
    )
)
```

- [ ] **Step 4: Run both `.iso` scenarios**

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh iso_basic
test -f /mnt/c/temp/chd_test_iso_basic/game/Disc.chd && echo "PASS: iso converted" || echo "FAIL"
grep -q "MOCK_CHDMAN createdvd input=.*Disc.iso" /mnt/c/temp/chd_test_iso_basic/conversion_log.txt && echo "PASS: createdvd used" || echo "FAIL: wrong subcommand"

bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh iso_ignored_with_cue
test ! -f /mnt/c/temp/chd_test_iso_ignored_with_cue/game/Disc.chd && echo "PASS: iso skipped when cue present" || echo "FAIL: iso was converted"
test -f /mnt/c/temp/chd_test_iso_ignored_with_cue/game/Other.chd && echo "PASS: cue still converted" || echo "FAIL"
```

Expected: all four lines print `PASS: ...`.

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add -A
git commit -m "Add .iso discovery via createdvd, skipping folders with a .cue

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 5: Nested folders, multi-disc, and end-to-end summary

**Files:**
- Create: `tests/fixtures/nested_multidisc/GameA/Disc1.cue`
- Create: `tests/fixtures/nested_multidisc/GameA/Disc2.cue`
- Create: `tests/fixtures/nested_multidisc/Sub/GameB/Track.cue`
- Create: `tests/fixtures/nested_multidisc/Sub/GameB/Track.chd` (pre-existing, to hit the skip path)
- Create: `tests/fixtures/nested_multidisc/Sub/Deeper/GameC/Disc.iso`
- Create: `tests/fixtures/nested_multidisc/LooseTrack.cue` (loose file directly at the fixture root)

**Interfaces:**
- Consumes: the full `convertir_a_chd.bat` from Tasks 1-4, unmodified — this task is a pure integration test proving the pieces work together, plus a check of the final summary line.

- [ ] **Step 1: Create the nested/multi-disc/loose-file fixture tree**

```bash
BASE=/home/jimrod/chd-batch-converter/tests/fixtures/nested_multidisc

mkdir -p "$BASE/GameA"
printf 'FILE "Disc1.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > "$BASE/GameA/Disc1.cue"
printf 'FILE "Disc2.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > "$BASE/GameA/Disc2.cue"

mkdir -p "$BASE/Sub/GameB"
printf 'FILE "Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > "$BASE/Sub/GameB/Track.cue"
echo "pre-existing chd" > "$BASE/Sub/GameB/Track.chd"

mkdir -p "$BASE/Sub/Deeper/GameC"
echo "fake iso" > "$BASE/Sub/Deeper/GameC/Disc.iso"

printf 'FILE "LooseTrack.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > "$BASE/LooseTrack.cue"
```

- [ ] **Step 2: Run the scenario**

```bash
bash /home/jimrod/chd-batch-converter/tests/run_scenario.sh nested_multidisc
```

- [ ] **Step 3: Verify every leaf converted/skipped correctly**

```bash
R=/mnt/c/temp/chd_test_nested_multidisc
test -f "$R/GameA/Disc1.chd" && echo "PASS: GameA Disc1" || echo "FAIL"
test -f "$R/GameA/Disc2.chd" && echo "PASS: GameA Disc2" || echo "FAIL"
grep -q "pre-existing chd" "$R/Sub/GameB/Track.chd" && echo "PASS: GameB skip preserved original" || echo "FAIL"
test -f "$R/Sub/Deeper/GameC/Disc.chd" && echo "PASS: deeply nested iso converted" || echo "FAIL"
test -f "$R/LooseTrack.chd" && echo "PASS: loose root-level cue converted" || echo "FAIL"
grep -q "Resumen: Convertidos=4 Saltados=1 Fallidos=0" "$R/conversion_log.txt" && echo "PASS: summary counts correct" || echo "FAIL: summary mismatch"
```

Expected: all six lines print `PASS: ...`. (Counts: Disc1, Disc2, GameC/Disc.iso, LooseTrack = 4 converted; GameB/Track = 1 skipped; 0 failed.)

- [ ] **Step 4: Smoke test against the real chdman.exe (not the mock)**

The user has placed a real `chdman.exe` (MAME 0.289) at
`/home/jimrod/chd-batch-converter/chdman.exe`. Run one small real
conversion end to end, without `CHDMAN_OVERRIDE`, to prove the script
works against the genuine binary and not just the mock's simulated
behavior.

```bash
WORK=/mnt/c/temp/chd_real_smoke
rm -rf "$WORK"
mkdir -p "$WORK/game"
cd "$WORK/game"
dd if=/dev/urandom of=Track.bin bs=2352 count=75 status=none
printf 'FILE "Track.bin" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n' > Track.cue
cp /home/jimrod/chd-batch-converter/convertir_a_chd.bat "$WORK/"
cp /home/jimrod/chd-batch-converter/chdman.exe "$WORK/"
cd "$WORK"
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && convertir_a_chd.bat"
test -f "$WORK/game/Track.chd" && echo "PASS: real chd created" || echo "FAIL"
grep -q "OK.*Track.cue.*convertido y verificado" "$WORK/conversion_log.txt" && echo "PASS: real conversion + verify succeeded" || echo "FAIL"
```

Expected: both lines print `PASS: ...` (a 75-sector random-data track is
enough for `chdman createcd` to accept and for `chdman verify` to
confirm hunk-by-hunk against the real binary). Clean up afterwards:

```bash
rm -rf /mnt/c/temp/chd_real_smoke
```

- [ ] **Step 5: Commit**

```bash
cd /home/jimrod/chd-batch-converter
git add -A
git commit -m "Add end-to-end nested/multi-disc integration test fixture

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

---

## Task 6: README and push to GitHub

**Files:**
- Create: `README.md`

**Interfaces:**
- Consumes: nothing new — documents the finished script from Tasks 1-5.

- [ ] **Step 1: Write the README**

Create `README.md`:

```markdown
# CHD Batch Converter

Script `.bat` para convertir en lote colecciones de PS1 (`.bin`/`.cue`) y
PS2/PSP (`.iso`) a `.chd` usando `chdman.exe` (de MAME), verificando cada
archivo generado.

## Uso

1. Descarga `chdman.exe` (paquete de herramientas de MAME) y colócalo en la
   misma carpeta que `convertir_a_chd.bat`.
2. Coloca `convertir_a_chd.bat` en la carpeta raíz de tu colección de juegos
   (ya sea con subcarpetas por juego, o con los archivos sueltos).
3. Haz doble clic en `convertir_a_chd.bat` (o corre `convertir_a_chd.bat`
   desde una consola).
4. Revisa `conversion_log.txt`, generado junto al script, para el detalle de
   cada conversión y verificación.

## Comportamiento

- Recorre recursivamente todas las subcarpetas (y la carpeta raíz misma).
- `.cue` → convertido con `chdman createcd`.
- `.iso` (solo si no hay ningún `.cue` en esa misma carpeta) → convertido
  con `chdman createdvd`.
- El `.chd` resultante se escribe junto al archivo original.
- Si el `.chd` ya existe, se salta esa conversión (permite re-correr el
  script sin repetir trabajo).
- Cada `.chd` generado se verifica con `chdman verify`.
- Los archivos originales (`.bin`/`.cue`/`.iso`) nunca se modifican, mueven
  ni borran.

## Diseño y plan de implementación

Ver `docs/superpowers/specs/2026-09-14-chd-batch-converter-design.md` y
`docs/superpowers/plans/2026-09-14-chd-batch-converter.md`.

## Pruebas

`tests/run_scenario.sh <nombre>` corre `convertir_a_chd.bat` de verdad (vía
`cmd.exe`/interop de WSL2) contra los fixtures en `tests/fixtures/<nombre>/`,
usando `tests/mock_chdman.bat` en lugar de un `chdman.exe` real.
```

- [ ] **Step 2: Commit the README**

```bash
cd /home/jimrod/chd-batch-converter
git add -A
git commit -m "Add README

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CwA6Mc4Q7Z2GK2VihZHZkL"
```

- [ ] **Step 3: Create the GitHub repo and push**

```bash
cd /home/jimrod/chd-batch-converter
gh repo create chd-batch-converter --private --source=. --remote=origin
git push -u origin main
```

If `gh` is not authenticated, this step will fail with an auth error — in that case, stop and ask the user to run `gh auth login` (or provide an existing remote URL), then re-run the two commands above.

- [ ] **Step 4: Verify the push**

```bash
git log --oneline -1
git remote -v
```

Expected: `origin` points at the new GitHub repo, and the latest commit matches the local `HEAD`.
