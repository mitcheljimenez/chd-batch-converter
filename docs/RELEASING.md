# Releasing a new version

## One-time setup (already partially done; documented here for reference)

1. Generate the updater signing keypair (separate from the code-signing
   certificate — see the design spec at
   `docs/superpowers/specs/2026-09-15-auto-update-design.md` §2 for why
   these are two different things):

   ```
   npx tauri signer generate -w ~/.tauri/chd-converter.key
   ```

   This prints a public key. Paste it into
   `ui/src-tauri/tauri.conf.json`'s `plugins.updater.pubkey`, replacing
   the `REPLACE_WITH_REAL_PUBKEY_FROM_TAURI_SIGNER_GENERATE` placeholder,
   and commit that change.

2. Make sure `ui/src-tauri/tauri.conf.json`'s `bundle.createUpdaterArtifacts`
   is set to `true`. Tauri v2 only produces the signed `.sig` files and
   `latest.json` that the release workflow publishes and the updater
   endpoint serves when this flag is set — without it, `tauri build`
   bundles the app normally but silently omits the updater artifacts,
   and the updater endpoint 404s forever.

3. Add these secrets under the repo's Settings → Secrets and variables →
   Actions:

   | Secret | Value |
   |---|---|
   | `WINDOWS_CERTIFICATE` | base64 of the existing `.pfx` — on the machine that has it: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("C:\temp\chd-converter-signing.pfx")) \| Set-Clipboard`, then paste |
   | `WINDOWS_CERTIFICATE_PASSWORD` | the `.pfx` password |
   | `TAURI_SIGNING_PRIVATE_KEY` | contents of `~/.tauri/chd-converter.key` from step 1 |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password chosen when generating that key in step 1 |

## Publishing a new version

1. Bump the version number in both `ui/src-tauri/tauri.conf.json` and
   `ui/src-tauri/Cargo.toml` (same number in both).
2. Commit: `git commit -am "Bump version to 0.1.3"`
3. Tag: `git tag v0.1.3`
4. Push both: `git push origin main --tags`

Pushing the tag alone triggers `.github/workflows/release.yml`, which
runs one job per platform (Windows, Linux on `ubuntu-22.04`, macOS
Apple Silicon and macOS Intel; `fail-fast: false`, so one failing doesn't
cancel the rest). Each job builds, updater-signs, and uploads to the
**same** GitHub Release named after the tag; `tauri-action` merges every
platform into a single `latest.json` (`windows-x86_64`, `linux-x86_64`,
`darwin-aarch64`, `darwin-x86_64`). Attached:

- Windows: NSIS setup `.exe` (code-signed with the self-signed cert).
- Linux: `.AppImage` (+ `.sig`, used by the updater) and `.deb` (no
  auto-update).
- macOS: one `.dmg` per architecture plus `.app.tar.gz` (+ `.sig`) for the
  updater. Ad-hoc signed only (`signingIdentity: "-"` in
  `tauri.macos.conf.json`), not notarized — no Apple secrets needed.

The same `TAURI_SIGNING_PRIVATE_KEY` signs the updater artifacts for every
platform. Only the Windows job imports the code-signing certificate.

### Test release

A tag with a hyphen (e.g. `v0.2.0-rc1`) is published as a **prerelease**,
which `releases/latest` (and therefore the updater) ignores. Use one to
check that every platform's assets and all four `latest.json` entries
show up before cutting the real version. Delete the prerelease and its
tag afterwards.
