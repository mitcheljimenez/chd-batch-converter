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
