# CHD Converter UI

Desktop UI for `../convertir_a_chd.bat`. Built with Tauri.

## Build

```
cd ui
npm install
npm run tauri build
```

The finished `.exe` is at `ui/src-tauri/target/release/chd-converter-ui.exe` (or
similar, per `tauri.conf.json`'s `productName`). It bundles `convertir_a_chd.bat`
and a fallback `chdman.exe` alongside itself at build time, so it works out of
the box — set a different path in the app's Settings panel if you want to use
your own `chdman.exe` instead. See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)
for the bundled `chdman.exe`'s license.

## Usage

1. Run the built `.exe`.
2. Open Settings and set the path to your `chdman.exe`.
3. Click "Elegir carpeta" and pick your ROMs folder.
4. Review the detected games, then click "Convertir todo".
5. Watch live progress; use "Cancelar" to stop early if needed.
6. Check "Historial" any time for past runs.

## Develop

```
cd ui
npm run tauri dev
```
