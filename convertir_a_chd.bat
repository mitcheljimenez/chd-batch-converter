@echo off
setlocal

if not "%~1"=="" (
    set "ROOT=%~1"
) else (
    set "ROOT=%~dp0"
)
rem The trailing-backslash normalization below must happen in a subroutine
rem (not inline in the if/else above): reading %ROOT% for the substring
rem check inside the same parenthesized block that just set it would see
rem the pre-block (empty) value, since cmd.exe expands % variables once
rem when it parses a parenthesized block, before any of the block's own
rem "set" commands run. A CALL'd label parses fresh, so it sees the
rem up-to-date value. (This app runs it with a folder argument via
rem start_conversion; the original double-click/no-arg flow never hit
rem this code path, so the bug was previously dormant.)
call :ensure_trailing_backslash
rem Test-only hook: tests/run_scenario.sh points this at tests/mock_chdman.bat
rem so the suite can run without a real chdman.exe.
if defined CHDMAN_OVERRIDE (
    set "CHDMAN=%CHDMAN_OVERRIDE%"
) else (
    set "CHDMAN=%ROOT%chdman.exe"
)
set "LOG=%ROOT%conversion_log.txt"

if not exist "%CHDMAN%" (
    echo ERROR: chdman.exe no se encontro junto a este script ^(se esperaba en "%CHDMAN%"^).
    echo Descarga chdman.exe del paquete de herramientas de MAME y colocalo en esta carpeta.
    pause >nul
    exit /b 1
)

set /a COUNT_CONVERTED=0
set /a COUNT_SKIPPED=0
set /a COUNT_FAILED=0

call :log "==== Ejecucion iniciada %DATE% %TIME% ===="

for /r "%ROOT%" %%F in (*.cue) do (
    call :process_disc "%%~fF" "%%~dpF" "%%~nF" createcd
)

rem Only skip an .iso when a .cue with the SAME base name sits next to it --
rem the same game, already available as cue/bin -- not merely because some
rem unrelated .cue exists in the same folder. A flat folder that mixes a PS1
rem game (.cue) with a PS2 game (.iso) must still convert the .iso.
for /r "%ROOT%" %%F in (*.iso) do (
    if exist "%%~dpF%%~nF.cue" (
        call :log "SKIP  | %%~fF | .iso ignorado: existe %%~nF.cue (mismo juego)"
        set /a COUNT_SKIPPED+=1
    ) else (
        call :process_disc "%%~fF" "%%~dpF" "%%~nF" createdvd
    )
)

call :log "==== Resumen: Convertidos=%COUNT_CONVERTED% Saltados=%COUNT_SKIPPED% Fallidos=%COUNT_FAILED% ===="
echo.
echo Listo. Convertidos=%COUNT_CONVERTED%  Saltados=%COUNT_SKIPPED%  Fallidos=%COUNT_FAILED%
echo Ver "%LOG%" para el detalle.
pause >nul
exit /b 0

:ensure_trailing_backslash
if not "%ROOT:~-1%"=="\" set "ROOT=%ROOT%\"
goto :eof

:log
setlocal disabledelayedexpansion
set "MSG=%~1"
setlocal enabledelayedexpansion
echo %DATE% %TIME% ^| !MSG!>> "%LOG%"
endlocal
endlocal
goto :eof

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
