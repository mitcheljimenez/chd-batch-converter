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
- `.iso` (only if there's no `.cue` in that same folder) → converted with
  `chdman createdvd`.
- The resulting `.chd` is written next to the original file.
- If the `.chd` already exists, that conversion is skipped (so the script
  can be re-run without repeating work).
- Every generated `.chd` is verified with `chdman verify`.
- Original files (`.bin`/`.cue`/`.iso`) are never modified, moved, or
  deleted.

## Desktop app

If you'd rather not use the command line, there's a desktop app with a
graphical interface (Windows) in [`ui/`](ui/), with folder selection, live
progress, conversion history, automatic updates, and an ES-DE multi-disc
game organizer (credit to
[ItsRetroPup/ES-DE-Multi-Disc-ROM-Organizer](https://github.com/ItsRetroPup/ES-DE-Multi-Disc-ROM-Organizer)
for the original concept — see [`ui/README.md`](ui/README.md#organize-multi-disc-games)
for details). Ready-to-run installers are available on
[GitHub Releases](https://github.com/mitcheljimenez/chd-batch-converter/releases/latest).

## Roadmap

Rough priority order, highest first — not commitments or dates, just
where effort would likely pay off most:

1. **Linux and macOS builds of the desktop app.** Tauri already targets
   both; the real work is replacing `convertir_a_chd.bat`'s Windows-only
   pieces (`cmd.exe`/batch, `\\?\`-prefixed paths, `taskkill`) with a
   cross-platform conversion path, and — for macOS — code-signing and
   notarization so Gatekeeper doesn't block the app outright (Windows
   SmartScreen at least lets you click through).
2. **A real code-signing certificate.** The self-signed one works but
   still shows an "unknown publisher" warning on every fresh install;
   a certificate from a public CA would remove that, at a real
   recurring cost.
3. **Parallel conversion.** Discs currently convert one at a time;
   running a few `chdman` processes concurrently would meaningfully
   speed up large libraries on multi-core machines.
4. **A "verify only" pass** — re-run `chdman verify` against existing
   `.chd` files without reconverting, useful after a drive move or to
   catch bit rot.
5. **CI smoke tests on every push**, not just the release pipeline —
   catch a broken build before it's tagged, not after.
6. **More languages** if there's demand — the Settings selector already
   supports adding a language as a self-contained dictionary in
   `ui/src/i18n.js`, so this is mostly translation work, not plumbing.
7. **A conversion log viewer in the app** — right now a failure only
   shows a short message; being able to expand it to the full
   `conversion_log.txt` for that game would help diagnosing chdman
   errors without leaving the app.
8. **Config profiles** for people who juggle more than one ROMs
   directory or chdman path (e.g. separate PC and handheld libraries).

## Tests

`tests/run_scenario.sh <name>` runs `convertir_a_chd.bat` for real (via
`cmd.exe`/WSL2 interop) against the fixtures in `tests/fixtures/<name>/`,
using `tests/mock_chdman.bat` instead of a real `chdman.exe`.
