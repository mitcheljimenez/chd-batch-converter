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

if [ "$SCENARIO" = "missing_chdman" ]; then
    # This scenario exercises the real "chdman.exe missing" guard, which
    # requires CHDMAN_OVERRIDE to be UNDEFINED and no chdman.exe present.
    # Run the script directly, without the mock or the override.
    cd "$WORK"
    WINDIR=$(wslpath -w "$PWD")
    /mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && convertir_a_chd.bat"

    echo "RESULT_DIR:$WORK"
    exit 0
fi

cp "$REPO_DIR/tests/mock_chdman.bat" "$WORK/mock_chdman.bat"

# A fixture can opt into testing FORMAT_OVERRIDES by shipping a
# .format_override_target file holding a fixture-relative, Windows-style
# path (e.g. "game\Disc.iso") to the .iso it wants forced to CD format.
# The absolute path can't be baked into the fixture itself (it depends on
# $WORK, which varies per run), so it's resolved here instead.
FORMAT_OVERRIDES_ARG=""
TARGET_FILE="$FIXTURE_DIR/.format_override_target"
if [ -f "$TARGET_FILE" ]; then
    REL_TARGET=$(cat "$TARGET_FILE")
    ABS_TARGET_WIN=$(wslpath -w "$WORK/${REL_TARGET//\\//}")
    # CRLF, not bash's default LF: findstr /X (used by convertir_a_chd.bat
    # to match an override line) silently fails to match an LF-only line.
    printf '%s=cd\r\n' "$ABS_TARGET_WIN" > "$WORK/format_overrides.txt"
    FORMAT_OVERRIDES_ARG=$(wslpath -w "$WORK/format_overrides.txt")
fi

# Create a wrapper script to set CHDMAN_OVERRIDE (and, if given, FORMAT_OVERRIDES) and run the main script
cat > "$WORK/run_with_mock.bat" << 'BATCHEOF'
@echo off
set "CHDMAN_OVERRIDE=%~1"
if not "%~2"=="" set "FORMAT_OVERRIDES=%~2"
call convertir_a_chd.bat
BATCHEOF

cd "$WORK"
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && run_with_mock.bat $WINDIR\\mock_chdman.bat $FORMAT_OVERRIDES_ARG"

echo "RESULT_DIR:$WORK"
