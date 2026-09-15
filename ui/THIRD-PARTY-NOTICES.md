# Third-Party Notices

This application bundles a binary it does not own the copyright to:

## chdman.exe

- **Location in this repo:** `ui/src-tauri/build-assets/chdman.exe`
- **Source project:** [MAME](https://github.com/mamedev/mame) (the CHD tooling,
  `chdman`, ships as part of the MAME project)
- **License:** GPL-2.0-or-later (MAME is multi-licensed; the majority of the
  codebase, including the CHD tools, is GPL-2.0-or-later — see MAME's own
  [LICENSE.md](https://github.com/mamedev/mame/blob/master/LICENSE.md) for the
  authoritative, file-by-file breakdown)
- **Why it's bundled:** used as a fallback binary so the app works without the
  user having to locate and configure a `chdman.exe` path manually on first
  run. A user-configured path (Settings) always takes priority over this
  bundled copy.
- **Source availability:** the exact MAME source corresponding to this binary
  is publicly available at https://github.com/mamedev/mame under the same
  license. This repository does not modify chdman's source or binary in any
  way; it is redistributed unmodified.

This notice satisfies GPL-2.0-or-later's attribution/source-availability
requirements for redistributing this specific binary. It does not relicense
any other part of this repository, which remains under the license in the
top-level `LICENSE` file.
