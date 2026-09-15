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
