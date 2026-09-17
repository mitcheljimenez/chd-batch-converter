# CHD Converter UI

Desktop UI for `../convertir_a_chd.bat`. Built with Tauri.

**Current version:** 0.1.14

## Install

Download the latest installer (`.msi` or the NSIS `.exe`) from
[GitHub Releases](https://github.com/mitcheljimenez/chd-batch-converter/releases/latest) —
no build step needed. The app checks for new versions on its own (see
[Auto-update](#auto-update) below), so once it's installed you generally
don't need to come back here for updates.

The installer is signed with a self-signed certificate, so Windows
SmartScreen may warn about an "unknown publisher" the first time you run
it — that's expected for a project this size (a certificate from a public
CA costs money this project doesn't spend). Click "More info" → "Run
anyway" to proceed.

## Usage

1. Run the installed app. It defaults to Spanish; switch to English (or
   back) from the "Idioma"/"Language" selector in Settings — it applies
   and saves immediately.
2. Open Settings and set the path to your `chdman.exe` (bundled by
   default, so this step is optional unless you want to use your own).
3. Click "Elegir carpeta" and pick your ROMs folder.
4. On the "Convertir" section, review the detected games, then click
   "Convertir todo". PS2 `.iso` games default to DVD format with `zlib`
   compression, which plays correctly both on PC (PCSX2) and on Android
   (NetherSX2/AetherSX2). Each `.iso` row also has a "DVD (zlib)"/"CD"
   dropdown if you want to force CD format for a specific game instead —
   you shouldn't normally need to, but it's there as an escape hatch.
5. Watch live progress; use "Cancelar" to stop early if needed.
6. Check "Historial" any time for past runs.

## Organize multi-disc games

The "Organizar multi-disco" section groups multi-disc games (files with a
`Disc 1`/`Disc 2`, `Disk 1`/`Disk 2`, or `CD 1`/`CD 2` marker in the name)
into a `<Game>.m3u/` folder each, with a generated `.m3u` playlist inside —
the layout ES-DE expects for a multi-disc entry. It recurses through
however deeply nested your folders are, and re-running it skips games it
already organized rather than reprocessing them.

Supported file types: `.chd`, `.rvz`, `.iso`, `.cue`, `.bin`, `.pbp`,
`.zip`.

If you're going to copy the organized folders onto an Android device's
storage, pick "Almacenamiento interno" or "Tarjeta SD externa" (the "?"
tooltip explains how to find your SD card's ID on the device) so `.rvz`
(GameCube/Wii, played with Dolphin) playlists get the absolute Android
path they need — every other format uses portable relative paths that
work as-is on both the PC and after copying to Android.

This feature is a rewritten, Windows-only take on
[ItsRetroPup/ES-DE-Multi-Disc-ROM-Organizer](https://github.com/ItsRetroPup/ES-DE-Multi-Disc-ROM-Organizer)
— credit to [ItsRetroPup](https://github.com/ItsRetroPup) for the
original concept and compatibility research (in particular, that Dolphin
needs absolute paths while other cores don't). The scanning/grouping logic
here is a from-scratch Rust implementation, not a port of their script.

## Extraer .chd

The "Extraer .chd" section finds every `.chd` in your chosen folder,
detects whether each one is CD format (`.bin`/`.cue`) or DVD format
(`.iso`) by inspecting it with `chdman info`, and lets you unpack it back
to its original files with a per-row "Extraer" button. This is mainly
useful for recovering a `.chd` an older version of this app converted with
`createdvd`'s default Zstandard compression (unreadable by NetherSX2/
AetherSX2 on Android): extract it back to `.iso`, then re-run "Convertir
todo" on that folder to get a fresh, DVD-zlib `.chd`. The original `.chd`
is never deleted or modified — extraction is refused instead of
overwriting if the destination file already exists.

## Auto-update

The app checks GitHub Releases for new versions automatically:

- On startup, silently — if "Install updates automatically" is checked, it
  downloads and installs a newer version on its own (asking nothing) and
  shows the release notes once it's done; otherwise it asks before
  installing.
- Any time via the "Check for updates" button in Settings, which always
  shows a result (up to date, found something, or couldn't check).

That checkbox (like the language selector) saves itself as soon as you
toggle it — no need to click "Save" separately for either.

## Build from source

```
cd ui
npm install
npm run tauri build
```

The finished `.exe` is at `ui/src-tauri/target/release/chd-converter-ui.exe` (or
similar, per `tauri.conf.json`'s `productName`). It bundles `convertir_a_chd.bat`
and a fallback `chdman.exe` alongside itself at build time, so it works out of
the box. See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for the bundled
`chdman.exe`'s license.

## Develop

```
cd ui
npm run tauri dev
```

## Releasing a new version

See [`docs/RELEASING.md`](../docs/RELEASING.md) — publishing is a tag
push (`git tag vX.Y.Z && git push --tags`), which triggers a GitHub
Actions workflow that builds, signs, and publishes the release
automatically. No manual build needed for a release.

`main` is protected (changes from anyone other than the repo owner
require a pull request), and only the repo owner can push `v*` tags —
external contributions go through a PR, and only the owner cuts
releases.
