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
