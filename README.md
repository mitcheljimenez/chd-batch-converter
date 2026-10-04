# CHD Batch Converter

`.bat` script to batch-convert PS1 (`.bin`/`.cue`) and PS2/PSP (`.iso`) game
collections to `.chd` using `chdman.exe` (from MAME), verifying every
generated file.

## Usage

1. Download `chdman.exe` (part of the MAME tools package) and place it in
   the same folder as `convertir_a_chd.bat`.
2. Put `convertir_a_chd.bat` in the root folder of your game collection
   (whether it uses per-game subfolders or loose files).
3. Double-click `convertir_a_chd.bat` (or run it from a console).
4. Check `conversion_log.txt`, generated next to the script, for the
   details of each conversion and verification.

## Behavior

- Recursively walks every subfolder (and the root folder itself).
- `.cue` → converted with `chdman createcd`.
- `.iso` (only if there's no same-named `.cue` next to it, i.e. the same game
  isn't already available as cue/bin) → converted with `chdman createdvd`. An
  unrelated `.cue` for a different game in the same folder does not block it.
- The resulting `.chd` is written next to the original file.
- If the `.chd` already exists, that conversion is skipped (so the script
  can be re-run without repeating work).
- Every generated `.chd` is verified with `chdman verify`.
- Original files (`.bin`/`.cue`/`.iso`) are never modified, moved, or
  deleted. (The desktop app can optionally send them to the trash once
  their `.chd` is verified.)

## Desktop app

If you'd rather not use the command line, there's a desktop app with a
graphical interface (Windows, Linux and macOS) in [`ui/`](ui/) that also
handles Dreamcast (`.gdi`) dumps, with folder selection, live
progress, conversion history, automatic updates, **parallel conversion**
(several `chdman` processes run at once, one per CPU core, instead of
converting discs one at a time), an ES-DE multi-disc
game organizer (credit to
[ItsRetroPup/ES-DE-Multi-Disc-ROM-Organizer](https://github.com/ItsRetroPup/ES-DE-Multi-Disc-ROM-Organizer)
for the original concept — see [`ui/README.md`](ui/README.md#organize-multi-disc-games)
for details), and a batch `.chd` extractor with progress bars and
post-extraction verification (see
[`ui/README.md`](ui/README.md#extraer-chd)) for recovering an older,
Android-incompatible `.chd` back to its original files. Ready-to-run
installers are available on
[GitHub Releases](https://github.com/mitcheljimenez/chd-batch-converter/releases/latest).

## Roadmap

Rough priority order, highest first — not commitments or dates, just
where effort would likely pay off most:

1. **A real code-signing certificate.** The self-signed one works but
   still shows an "unknown publisher" warning on every fresh Windows
   install, and the macOS build is only ad-hoc signed (not notarized), so
   Gatekeeper needs a one-time "Open Anyway". A certificate from a public
   CA / an Apple Developer account would remove both, at a real recurring
   cost.
2. **More languages** if there's demand — the Settings selector already
   supports adding a language as a self-contained dictionary in
   `ui/src/i18n.js`, so this is mostly translation work, not plumbing.
3. **Config profiles** for people who juggle more than one ROMs
   directory or chdman path (e.g. separate PC and handheld libraries).

Done: **parallel conversion** in the desktop app — see [`ui/README.md`](ui/README.md)
for details. (The plain `.bat` script above still converts one disc at a
time; only the desktop app runs several `chdman` processes concurrently.)

Done: **Linux and macOS builds of the desktop app** — see
[`ui/README.md`](ui/README.md#linux-and-macos).

Done: **a "verify only" pass** — "Verificar .chd" in the desktop app
re-runs `chdman verify` over existing `.chd` files.

Done: **chdman's output on failures** — a failed conversion, extraction
or verification has a "Ver detalles" link that expands what chdman
printed.

Done: **CI tests on every push** (`cargo test` on Windows, Linux and
macOS — `.github/workflows/ci.yml`).

## Tests

`tests/run_scenario.sh <name>` runs `convertir_a_chd.bat` for real (via
`cmd.exe`/WSL2 interop) against the fixtures in `tests/fixtures/<name>/`,
using `tests/mock_chdman.bat` instead of a real `chdman.exe`.
