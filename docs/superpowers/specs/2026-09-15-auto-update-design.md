# Auto-update via GitHub Releases — Design

Status: approved, pending implementation plan.

## Goal

Let users get new versions of CHD Converter without manually rebuilding
or re-downloading installers, using GitHub Releases as a serverless
update backend. Add a "check for updates" flow plus an opt-in
auto-install setting, and replace the current manual WSL build process
with a CI pipeline that builds, signs, and publishes releases.

## 1. CI/CD pipeline

`.github/workflows/release.yml`, triggered on `push` of a `v*` tag,
running on `windows-latest`. Uses `tauri-apps/tauri-action` to:

- install frontend deps, run `npm run tauri build`
- code-sign the produced `.exe`/`.msi`/NSIS setup with the existing
  self-signed certificate
- sign the update artifacts + `latest.json` with the separate updater
  signing key (see §2)
- create the GitHub Release for the pushed tag, uploading MSI, NSIS
  setup, and `latest.json` as release assets

This fully replaces the manual WSL → rsync → `cmd.exe` build process
used until now (and the recurring `\\?\` / signtool `PATH` friction
that came with it).

### Required GitHub secrets

- `WINDOWS_CERTIFICATE` — the existing self-signed `.pfx`, base64-encoded
- `WINDOWS_CERTIFICATE_PASSWORD` — its password (rotated — see GitHub Actions repository secrets, not committed here)
- `TAURI_SIGNING_PRIVATE_KEY` — the updater's minisign private key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — that key's password

## 2. Two separate signing concerns

These must not be conflated:

- **Code-signing certificate** (already exists): signs the `.exe`/`.msi`
  so Windows/SmartScreen can attribute the binary to an identity. Same
  self-signed cert already in use (thumbprint
  `ADEF2B14DE4A6012C0CD6AC765321D9EADFBA213`).
- **Updater signing key** (new): a minisign keypair generated via
  `tauri signer generate`, used only to sign `latest.json` and the
  update payload so `tauri-plugin-updater` can verify on the client
  that a downloaded update wasn't tampered with in transit. The public
  half is not secret and is embedded in `tauri.conf.json`
  (`plugins.updater.pubkey`). The private half only ever lives as a
  GitHub secret, used by CI at release time.

## 3. Backend (`tauri-plugin-updater`)

- Add `tauri-plugin-updater = "2"` and grant its permission in
  `capabilities/default.json`.
- New Tauri commands:
  - `check_for_update() -> Option<UpdateInfo>` — queries the current
    release's `latest.json` (fetched from
    `https://github.com/mitcheljimenez/chd-batch-converter/releases/latest/download/latest.json`).
    Returns `None` if already on the latest version. Network/GitHub
    failures during the silent startup check are swallowed (treated as
    "nothing new"); the manual button surfaces them instead (see §4).
  - `install_update()` — downloads and applies a previously-found
    update, then restarts the app.
- `Config` (in `settings.rs`) gains `auto_update_enabled: bool` (default
  `false`), persisted the same way `chdman_path` already is.
- **Guard**: neither the silent startup check nor the auto-install path
  may call `install_update()` while `RunState` shows a conversion in
  progress — a mid-run restart would kill a possibly hours-long job.
  The check is deferred, not dropped: it's retried on the next check
  (next app start, or the next manual button click).

## 4. Frontend UI

In `#settings-panel`, below the existing `chdman_path` field:

- A checkbox: **"Instalar actualizaciones automáticamente"** — loaded
  and saved alongside the rest of `Config` via the existing
  Guardar button.
- A button: **"Buscar actualizaciones"**, inside the same panel.

Behavior:

- **On app start**: silent background `check_for_update()`. If a new
  version is found:
  - checkbox ON → `install_update()` automatically (unless a
    conversion is running — see §3 guard), then notify "Se instaló la
    v0.1.3, se aplicará al reiniciar."
  - checkbox OFF → dialog: "Hay una actualización disponible (v0.1.3)"
    with **Instalar** / **Ahora no** buttons. Only "Instalar" triggers
    the download+install.
- **"Buscar actualizaciones" button**: same check, but always surfaces
  a result (unlike the silent startup check): "Ya tenés la última
  versión" when nothing's new, "No se pudo comprobar (sin conexión)" on
  a network failure, otherwise the same ON/OFF branching as above.

## 5. Versioning workflow

`tauri.conf.json` (`0.1.2`) and `Cargo.toml` (currently drifted at
`0.1.0`) get synced to the same version as part of this work, and kept
in sync going forward. Publishing a new version becomes:

```
1. Bump version in tauri.conf.json and Cargo.toml (same number)
2. git commit -m "Bump version to 0.1.3"
3. git tag v0.1.3
4. git push origin main --tags
```

That push alone triggers the full CI pipeline (§1). No more manual
WSL builds except for local testing before tagging.

## 6. Error handling / edge cases

- No internet / GitHub unreachable: silent check treats it as "no
  update"; manual button reports "No se pudo comprobar (sin conexión)".
- Tampered/invalid update signature: `tauri-plugin-updater` rejects
  the install by design (minisign verification) — no custom code
  needed.
- Update found mid-conversion: deferred per the §3 guard, retried on
  the next check.
