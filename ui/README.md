# CHD Converter UI

Desktop UI for `../convertir_a_chd.bat`. Built with Tauri.

**Current version:** 0.1.15

## Install

Download the latest installer from
[GitHub Releases](https://github.com/mitcheljimenez/chd-batch-converter/releases/latest) —
no build step needed. The app checks for new versions on its own (see
[Auto-update](#auto-update) below), so once it's installed you generally
don't need to come back here for updates.

| Platform | File |
|---|---|
| Windows | `CHD.Converter_x.y.z_x64-setup.exe` |
| Linux | `.AppImage` (auto-updates) or `.deb` (Debian/Ubuntu; does **not** auto-update — install the new `.deb` by hand) |
| macOS (Apple Silicon) | `CHD.Converter_x.y.z_aarch64.dmg` |
| macOS (Intel) | `CHD.Converter_x.y.z_x64.dmg` |

Linux and macOS need `chdman` installed separately — see
[Linux and macOS](#linux-and-macos).

The installer is signed with a self-signed certificate, so Windows
SmartScreen may warn about an "unknown publisher" the first time you run
it — that's expected for a project this size (a certificate from a public
CA costs money this project doesn't spend). Click "More info" → "Run
anyway" to proceed.

## Linux and macOS

The Windows installer bundles `chdman.exe`; on Linux and macOS, install
`chdman` with your package manager instead:

| System | Command |
|---|---|
| Debian / Ubuntu / Mint | `sudo apt install mame-tools` |
| Arch / SteamOS | `sudo pacman -S mame-tools` |
| Fedora | `sudo dnf install mame-tools` |
| macOS (Homebrew) | `brew install rom-tools` |

With the chdman path in Settings left empty, the app looks for `chdman`
on `$PATH` and in `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin` and
`/usr/games` (a macOS app opened from Finder doesn't see your shell's
`PATH`, hence the fixed list). If yours is somewhere else, set its full
path in Settings.

To run the **AppImage**: `chmod +x CHD.Converter_*.AppImage` and open it.

**macOS: first launch.** The app isn't notarized by Apple (that needs a
paid developer account), only ad-hoc signed, so Gatekeeper blocks the
first launch with an "unidentified developer" / "can't be verified"
message:

1. Drag the app from the `.dmg` into Applications and try to open it once
   (it will be blocked).
2. Open **System Settings → Privacy & Security**, scroll down and click
   **Open Anyway** next to the message about CHD Converter, then confirm.
   (On macOS 14 and earlier, right-click the app → Open → Open also works.)

Or, from Terminal: `xattr -cr "/Applications/CHD Converter.app"`.
You only need to do this once; later auto-updates open normally.

## Usage

1. Run the installed app. It defaults to Spanish; switch to English (or
   back) from the "Idioma"/"Language" selector in Settings — it applies
   and saves immediately.
2. Open Settings and set the path to your `chdman` (on Windows it's
   bundled, and on Linux/macOS the one your package manager installed is
   found automatically, so this step is optional unless you want to use
   your own).
3. Click "Elegir carpeta" and pick your ROMs folder, or just drag the
   folder (or any file inside it) onto the window. The app remembers it
   and reopens it next time.
4. On the "Convertir" section, review the detected games, then click
   "Convertir todo". PS2 `.iso` games default to DVD format with `zlib`
   compression, which plays correctly both on PC (PCSX2) and on Android
   (NetherSX2/AetherSX2). Each `.iso` row also has a "DVD (zlib)"/"CD"
   dropdown if you want to force CD format for a specific game instead —
   you shouldn't normally need to, but it's there as an escape hatch.
   Dreamcast dumps (`.gdi` plus its tracks) are converted too, with
   `chdman createcd`, and "Extraer .chd" turns a Dreamcast `.chd` back
   into a `.gdi` with its tracks. "Aplanar carpetas" leaves folders that
   hold a `.gdi` untouched, since Dreamcast tracks usually share generic
   names (`track01.bin`, ...) that would collide.
   Conversion runs **in parallel** by default: one `chdman` process per
   CPU core converts at once, instead of one disc at a time, so a large
   library finishes noticeably faster on a multi-core machine. Prefer one
   disc at a time (in order, with the PC more responsive meanwhile)?
   Uncheck "Convertir varios discos a la vez" in Settings; that single
   `chdman` then uses every core on its own.
   Want the space back? Check "Mover los archivos originales a la
   papelera tras convertir y verificar" (right under the buttons): once a
   disc converts **and** `chdman verify` passes on its new `.chd`, its
   originals (the `.cue` and every track it lists, the `.gdi` and its
   tracks, or the `.iso`) go to the system trash / Recycle Bin, never
   deleted outright, so you can still restore them. Off by default, and a
   disc that fails or is cancelled always keeps its originals.
5. Watch live progress; use "Cancelar" to stop early if needed. Each
   converted disc shows its size before and after (e.g. `700 MB → 450 MB
   (-36 %)`), and the run ends with the total space saved, which is also
   kept in "Historial". If you're in another app when a conversion or
   extraction finishes, a system notification tells you how it went.
   If a disc fails, click "Ver detalles" on its row to see exactly what
   `chdman` reported (the same works in "Extraer .chd" and
   "Verificar .chd").
6. Check "Historial" any time for past runs.
7. Need to undo a conversion (e.g. to recover a `.chd` made unreadable on
   Android by an old version of this app)? See [Extraer .chd](#extraer-chd)
   below.

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

The "Extraer .chd" section finds every `.chd` in your chosen folder and its
subfolders that doesn't already have its `.iso`/`.bin`/`.cue` sitting next
to it, detects whether each one is CD format (`.bin`/`.cue`) or DVD format
(`.iso`) by inspecting it with `chdman info`, and lets you unpack it back
to its original files — either one at a time with a per-row "Extraer"
button, or all at once with "Extraer todos". Both run in the background
with a live progress bar (per-row and overall), the same as the Convertir
tab, so the UI never blocks or looks stuck while chdman works through a
file. After each extraction, `chdman verify` also checks the source `.chd`
itself isn't corrupt — the same safety check `createcd`/`createdvd`
already run after compressing during conversion. This is mainly useful
for recovering a `.chd` an older version of this app converted with
`createdvd`'s default Zstandard compression (unreadable by NetherSX2/
AetherSX2 on Android): extract it back to `.iso`, then re-run "Convertir
todo" on that folder to get a fresh, DVD-zlib `.chd`. The original `.chd`
is never deleted or modified — extraction is refused instead of
overwriting if the destination file already exists.

## Verificar .chd

Checks that `.chd` files you already have are still intact, without
converting or extracting anything: `chdman verify` re-reads each one and
compares it with the SHA-1 stored inside it. Handy after copying a
library to another drive, an SD card or a handheld. Each file shows
"intacto" or "DAÑADO" (a corrupt one should be reconverted from its
original), "Cancelar" stops after the file in progress, and nothing is
ever modified.

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

The bundles land in `ui/src-tauri/target/release/bundle/` (NSIS `.exe`
on Windows, `.AppImage`/`.deb` on Linux, `.app`/`.dmg` on macOS). Shared
settings live in `tauri.conf.json`; per-platform ones in
`tauri.windows.conf.json`, `tauri.linux.conf.json` and
`tauri.macos.conf.json`, which Tauri merges on top automatically. The
Windows build bundles a fallback `chdman.exe` (and the legacy
`convertir_a_chd.bat`), so it works out of the box.

On Linux, building needs the WebKitGTK toolchain first:
`sudo apt install libwebkit2gtk-4.1-dev build-essential file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev xdg-utils`. See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for the bundled
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
