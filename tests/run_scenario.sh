#!/bin/bash
# Usage: tests/run_scenario.sh <scenario_name>
# Runs convertir_a_chd.bat for real (via cmd.exe/WSL2 interop) against the
# fixture tree in tests/fixtures/<scenario_name>/, using tests/mock_chdman.bat
# in place of a real cdhman.exe. Leaves the result tree in place and prints
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
cp "$REPO_DIR/tests/mock_chdman.bat" "$WORK/mock_cdhman.bat"

# Create a wrapper script to set CHDMAN_OVERRIDE and run the main script
cat > "$WORK/run_with_mock.bat" << 'BATCHEOF'
@echo off
set "CHDMAN_OVERRIDE=%~1"
call convertir_a_chd.bat
BATCHEOF

cd "$WORK"
WINDIR=$(wslpath -w "$PWD")
/mnt/c/WINDOWS/system32/cmd.exe /c "cd /d $WINDIR && run_with_mock.bat $WINDIR\\mock_cdhman.bat"

echo "RESULT_DIR:$WORK"
