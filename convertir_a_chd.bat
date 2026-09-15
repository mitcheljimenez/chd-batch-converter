@echo off
setlocal enabledelayedexpansion

set "ROOT=%~dp0"
if defined CDHMAN_OVERRIDE (
    set "CDHMAN=%CDHMAN_OVERRIDE%"
) else (
    set "CDHMAN=%ROOT%cdhman.exe"
)
set "LOG=%ROOT%conversion_log.txt"

if not exist "%CDHMAN%" (
    echo ERROR: cdhman.exe no se encontro junto a este script ^(se esperaba en "%CDHMAN%"^).
    echo Descarga cdhman.exe del paquete de herramientas de MAME y colocalo en esta carpeta.
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
