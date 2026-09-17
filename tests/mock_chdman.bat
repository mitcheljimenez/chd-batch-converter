@echo off
rem Mimics chdman.exe's CLI surface for automated tests. Not real chdman.
rem No delayed expansion here: this file never uses !VAR! syntax, and
rem enabling it would corrupt %INPUT%/%OUTPUT% values that contain "!"
rem (a real Tomba!.cue-style filename), since two such values combined
rem on one line supply the "!...!" pair delayed expansion looks for.
setlocal
set "CMD=%~1"
echo MOCK_CHDMAN_RAWARGS: %*
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
    echo "%INPUT%"| findstr /I /C:"BADVERIFY" >nul
    if not errorlevel 1 (
        echo mock verify: FAILED
        exit /b 1
    )
    echo mock verify: ok
    exit /b 0
)

echo "%INPUT%"| findstr /I /C:"BADCUE" /C:"BADISO" >nul
if not errorlevel 1 (
    echo mock convert: FAILED
    exit /b 1
)
echo FAKE_CHD_CONTENT for %INPUT%> "%OUTPUT%"
exit /b 0
