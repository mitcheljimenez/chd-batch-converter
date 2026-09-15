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
alongside itself at build time (see Task 6) — copy `chdman.exe` next to the
`.exe` (or set its path in the app's Settings panel) before running conversions.

## Develop

```
cd ui
npm run tauri dev
```
