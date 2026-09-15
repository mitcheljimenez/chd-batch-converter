@echo off
setlocal

if not "%~1"=="" (
    set "ROOT=%~1"
    if not "%ROOT:~-1%"=="\" (
        set "ROOT=%ROOT%\"
    )
) else (
    set "ROOT=%~dp0"
)
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

for /r "%ROOT%" %%F in (*.iso) do (
    set "HASCUE="
    for %%C in ("%%~dpF*.cue") do set "HASCUE=1"
    if defined HASCUE (
        call :log "SKIP  | %%~fF | .iso ignorado: hay un .cue en la misma carpeta"
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
