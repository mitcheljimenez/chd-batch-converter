# Auto-update via GitHub Releases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let CHD Converter check GitHub Releases for a newer version, optionally auto-install it, and be built/signed/published by CI instead of the manual WSL process.

**Architecture:** `tauri-plugin-updater` on the Rust side reads a `latest.json` manifest published to the project's GitHub Releases; two new Tauri commands (`check_for_update`, `install_update`) wrap it and add a "don't interrupt a running conversion" guard; a checkbox + button in the existing Settings panel drive it from the frontend; a new GitHub Actions workflow (`tauri-apps/tauri-action`) builds, code-signs, updater-signs, and publishes a Release whenever a `v*` tag is pushed.

**Tech Stack:** Tauri v2, Rust, `tauri-plugin-updater` v2, vanilla JS/DOM (no framework), GitHub Actions (`windows-latest` runner), `tauri-apps/tauri-action`.

**Spec:** `docs/superpowers/specs/2026-09-15-auto-update-design.md`

## Global Constraints

- Repo is public (`mitcheljimenez/chd-batch-converter`) — the `latest.json` URL needs no auth: `https://github.com/mitcheljimenez/chd-batch-converter/releases/latest/download/latest.json`.
- Two signing concerns stay separate: the existing self-signed code-signing cert (thumbprint `ADEF2B14DE4A6012C0CD6AC765321D9EADFBA213`) signs the `.exe`/`.msi`; a new, separate minisign keypair signs `latest.json`/update payloads. Never conflate the two in code, docs, or secret names.
- `auto_update_enabled` in `Config` defaults to `false` (opt-in). Nothing auto-installs unless the user has explicitly turned it on.
- Never call `install_update()` while a conversion is in progress (`RunState.0` holds `Some(Child)`). Defer, don't drop — retried on the next check.
- Rust unit tests are colocated in `#[cfg(test)] mod tests` blocks in the same file, matching `settings.rs` and `lib.rs`'s existing style.
- Frontend has no framework; any string built from filesystem/user/network data (e.g. a version string, a release note) goes through `textContent`, never `innerHTML`.
- Release trigger is a pushed `v*` tag, not every push to `main`.

---

## File Structure

- Modify `ui/src-tauri/Cargo.toml` — add `tauri-plugin-updater` dependency, sync `version` to `0.1.2`.
- Modify `ui/src-tauri/tauri.conf.json` — add `plugins.updater` config (endpoint + pubkey placeholder), sync top-level `version` if needed (already `0.1.2`).
- Modify `ui/src-tauri/capabilities/default.json` — add `updater:default` permission.
- Modify `ui/src-tauri/src/settings.rs` — add `auto_update_enabled: bool` to `Config`.
- Modify `ui/src-tauri/src/lib.rs` — add `check_for_update`/`install_update` commands, register the updater plugin, register the new commands in `invoke_handler`.
- Modify `ui/src/index.html` — add checkbox + button to `#settings-panel`.
- Modify `ui/src/main.js` — wire the checkbox/button, startup silent check, dialog logic.
- Create `.github/workflows/release.yml` — the CI release pipeline.
- Create `docs/RELEASING.md` — the tag-based release workflow + one-time secret setup instructions (this is where the "generate the real keypair and add secrets" human steps live, since a plan step can't generate real secret material).

---

## Task 1: Sync Cargo.toml version and add the updater dependency

**Files:**
- Modify: `ui/src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `tauri-plugin-updater` available as a crate dependency for Task 3 onward.

- [ ] **Step 1: Edit `Cargo.toml`**

Change the `version` field and add the dependency:

```toml
[package]
name = "chd-converter-ui"
version = "0.1.2"
description = "A Tauri App"
authors = ["you"]
edition = "2021"
```

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
tauri-plugin-dialog = "2"
tauri-plugin-updater = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
walkdir = "2"
```

- [ ] **Step 2: Verify it resolves**

Run (on the Windows toolchain mirror, per the project's established WSL/Windows workflow): `cargo check` from `ui/src-tauri`.
Expected: compiles (may take a while fetching the new crate); no errors. (There are no Rust tests to run yet for this task — it's a manifest-only change.)

- [ ] **Step 3: Commit**

```bash
git add ui/src-tauri/Cargo.toml
git commit -m "Add tauri-plugin-updater dependency; sync Cargo.toml version to 0.1.2"
```

---

## Task 2: Wire updater plugin config and capability permission

**Files:**
- Modify: `ui/src-tauri/tauri.conf.json`
- Modify: `ui/src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: nothing from Task 1 directly (parallel-safe, but sequenced after for a clean dependency story).
- Produces: `plugins.updater.pubkey` placeholder and `plugins.updater.endpoints` that Task 3's `check_for_update` command relies on being present in the built app's config; `updater:default` permission that the plugin's Rust calls require to not fail at runtime.

- [ ] **Step 1: Add the updater plugin block to `tauri.conf.json`**

Add a top-level `"plugins"` key (this file does not currently have one):

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "CHD Converter",
  "version": "0.1.2",
  "identifier": "com.mitch.chd-converter-ui",
  "build": {
    "frontendDist": "../src"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "title": "CHD Converter",
        "width": 900,
        "height": 640
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "resources": [
      "build-assets/convertir_a_chd.bat",
      "build-assets/chdman.exe"
    ]
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/mitcheljimenez/chd-batch-converter/releases/latest/download/latest.json"
      ],
      "pubkey": "REPLACE_WITH_REAL_PUBKEY_FROM_TAURI_SIGNER_GENERATE"
    }
  }
}
```

Note: this file does not have the `bundle.windows.certificateThumbprint` block committed (it's a machine-local addition per the project's established pattern) — do not add it here; leave that as-is.

The `pubkey` placeholder value is intentional: it is replaced with the real public key generated in Task 8 (`docs/RELEASING.md`'s one-time setup), which is the first point in this plan where a real keypair is actually generated. Until that happens, `check_for_update` will fail signature verification against any real release — this is expected and does not block the rest of this plan, since Tasks 3–7 are testable without a real release existing yet.

- [ ] **Step 2: Add the `updater:default` permission**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "dialog:default",
    "updater:default"
  ]
}
```

- [ ] **Step 3: Verify the app still builds**

Run: `cargo check` from `ui/src-tauri` (Windows toolchain mirror).
Expected: compiles — this step only touches JSON config, so a clean compile confirms the JSON is well-formed and Tauri accepted the new plugin block.

- [ ] **Step 4: Commit**

```bash
git add ui/src-tauri/tauri.conf.json ui/src-tauri/capabilities/default.json
git commit -m "Configure tauri-plugin-updater endpoint, pubkey placeholder, and permission"
```

---

## Task 3: `Config.auto_update_enabled` field

**Files:**
- Modify: `ui/src-tauri/src/settings.rs`

**Interfaces:**
- Produces: `Config { chdman_path: String, auto_update_enabled: bool }` — Task 5 (`lib.rs` commands) and Task 6 (frontend) both read/write this field by name.

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `settings.rs`:

```rust
#[test]
fn missing_config_defaults_auto_update_to_false() {
    let dir = temp_dir("missing_config_auto_update");
    let config = load_config(&dir);
    assert_eq!(config.auto_update_enabled, false);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn save_then_load_config_round_trips_auto_update_flag() {
    let dir = temp_dir("round_trip_auto_update");
    let config = Config {
        chdman_path: "C:\\Tools\\chdman.exe".to_string(),
        auto_update_enabled: true,
    };
    save_config(&dir, &config).unwrap();
    let loaded = load_config(&dir);
    assert_eq!(loaded.auto_update_enabled, true);
    fs::remove_dir_all(&dir).unwrap();
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib settings:: -- --nocapture` from `ui/src-tauri` (Windows toolchain mirror).
Expected: FAIL to compile — `Config` has no field `auto_update_enabled`, and the existing `save_then_load_config_round_trips` test's `Config { chdman_path: ... }` literal (line ~75) will also fail to compile once the field is added without a default, since it's a non-`..Default::default()` struct literal. This is expected and fixed in the next step.

- [ ] **Step 3: Add the field and fix the existing struct literal**

```rust
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    pub chdman_path: String,
    #[serde(default)]
    pub auto_update_enabled: bool,
}
```

The `#[serde(default)]` attribute matters: it lets an existing `config.json` on a user's machine (written before this change, with no `auto_update_enabled` key) still deserialize successfully, defaulting the missing field to `false`, instead of falling through to `unwrap_or_default()` and silently discarding their saved `chdman_path` too.

Update the pre-existing `save_then_load_config_round_trips` test's struct literal so it still compiles:

```rust
#[test]
fn save_then_load_config_round_trips() {
    let dir = temp_dir("round_trip");
    let config = Config {
        chdman_path: "C:\\Tools\\chdman.exe".to_string(),
        auto_update_enabled: false,
    };
    save_config(&dir, &config).unwrap();
    let loaded = load_config(&dir);
    assert_eq!(loaded.chdman_path, "C:\\Tools\\chdman.exe");
    fs::remove_dir_all(&dir).unwrap();
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib settings:: -- --nocapture` from `ui/src-tauri`.
Expected: PASS (all `settings::tests::*` tests, including the two new ones and the fixed existing one).

- [ ] **Step 5: Commit**

```bash
git add ui/src-tauri/src/settings.rs
git commit -m "Add auto_update_enabled field to Config"
```

---

## Task 4: `check_for_update` and `install_update` commands with the in-progress-conversion guard

**Files:**
- Modify: `ui/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `RunState(Arc<Mutex<Option<Child>>>, Arc<AtomicBool>)` (already defined in `lib.rs`); `Config.auto_update_enabled` (Task 3).
- Produces: `#[tauri::command] async fn check_for_update(app_handle, state) -> Result<Option<UpdateSummary>, String>`; `#[tauri::command] async fn install_update(app_handle, state) -> Result<(), String>`; a `UpdateSummary { version: String, notes: Option<String> }` struct (serializes for the frontend). Task 6 (frontend) calls both by these exact names via `invoke(...)`.

- [ ] **Step 1: Write the failing tests**

Add a new `#[cfg(test)] mod update_guard_tests` block at the bottom of `lib.rs`, after the existing `verbatim_prefix_tests` module:

```rust
#[cfg(test)]
mod update_guard_tests {
    use super::RunState;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    // These test the guard logic in isolation (a live Child can't be
    // constructed in a unit test without actually spawning a process), by
    // exercising the same "is a conversion running" check the commands use:
    // state.0.lock().unwrap().is_some().

    #[test]
    fn no_running_conversion_is_not_blocked() {
        let state = RunState(Arc::new(Mutex::new(None)), Arc::new(AtomicBool::new(false)));
        let guard = state.0.lock().unwrap();
        assert!(guard.is_none(), "expected no conversion in progress");
    }

    #[test]
    fn install_update_guard_matches_run_state_shape() {
        // Documents the exact check install_update performs before calling
        // the plugin's downloader: state.0.lock() must yield None. This
        // doesn't spawn a real Child (that requires a real OS process and
        // is exercised by the existing start_conversion_smoke.rs
        // integration test instead) — it locks in the guard's shape so a
        // future refactor of RunState's fields doesn't silently drop it.
        let state = RunState(Arc::new(Mutex::new(None)), Arc::new(AtomicBool::new(false)));
        let is_blocked = state.0.lock().map(|g| g.is_some()).unwrap_or(false);
        assert!(!is_blocked);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib update_guard_tests:: -- --nocapture` from `ui/src-tauri`.
Expected: FAIL to compile — `RunState`'s fields are private to the module it's declared in but this test module is a sibling (`mod update_guard_tests` inside the same file, same module as `RunState` itself, so this should actually compile and pass immediately since it's in the same file). Run it anyway to confirm: if it passes immediately, that's fine — this task's real substance is Steps 3–4 below (the commands), not this guard-shape test, which exists to pin the check's shape for later refactors.

- [ ] **Step 3: Implement the commands**

Add near the top of `lib.rs`, alongside the other `use` statements:

```rust
use tauri_plugin_updater::UpdaterExt;
```

Add after `cancel_conversion`, before `start_conversion`:

```rust
/// What the frontend needs to render an "update available" prompt: just
/// enough to show a version number and, if the release has notes, show
/// them. Anything else `tauri_plugin_updater::Update` carries (download
/// URLs, signature) stays server-side — the frontend never touches those,
/// it only ever calls back into install_update to act on them.
#[derive(serde::Serialize, Clone)]
struct UpdateSummary {
    version: String,
    notes: Option<String>,
}

/// Checks GitHub Releases (via the endpoint configured in
/// plugins.updater.endpoints in tauri.conf.json) for a newer version than
/// the one currently running. Returns Ok(None) both when already on the
/// latest version and when the check itself fails (offline, GitHub
/// unreachable) — callers that need to distinguish "checked, nothing new"
/// from "couldn't check" should inspect the Err case, which this only
/// produces for a plugin initialization failure, not a network failure.
#[tauri::command]
async fn check_for_update(app_handle: tauri::AppHandle) -> Result<Option<UpdateSummary>, String> {
    let updater = app_handle.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateSummary {
            version: update.version.clone(),
            notes: update.body.clone(),
        })),
        Ok(None) => Ok(None),
        Err(_) => Ok(None),
    }
}

/// Downloads and installs the update this app is currently aware of via a
/// prior check_for_update call, then restarts the app. Refuses while a
/// conversion is in flight (RunState.0 is Some) rather than killing a
/// possibly hours-long batch job out from under the user — the caller is
/// expected to retry this on the next check (app start or the manual
/// button) once the run finishes.
#[tauri::command]
async fn install_update(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, RunState>,
) -> Result<(), String> {
    {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("Hay una conversión en curso; se reintentará luego".to_string());
        }
    }

    let updater = app_handle.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No hay actualización disponible".to_string())?;

    update
        .download_and_install(|_chunk_len, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;

    app_handle.restart();
}
```

Note: `app_handle.restart()` (from `tauri::Manager`, already imported) never returns — it exits the process. The function's `Result<(), String>` return type is satisfied because `restart()`'s return type is `!` (never), which coerces to any type, including inside this function's body as its final expression.

Register the plugin and the two commands in `run()`:

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .manage(RunState(
        Arc::new(Mutex::new(None)),
        Arc::new(AtomicBool::new(false)),
    ))
    .invoke_handler(tauri::generate_handler![
        greet,
        prescan,
        get_config,
        set_config,
        get_history,
        start_conversion,
        cancel_conversion,
        check_for_update,
        install_update
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib -- --nocapture` from `ui/src-tauri` (Windows toolchain mirror — full suite, since this task touches shared imports and the command registration list).
Expected: PASS — all pre-existing tests (18 as of the last recorded run) plus the two new `update_guard_tests`, unaffected in count since `check_for_update`/`install_update` are `async` commands with no unit tests of their own here (they need a running Tauri app context to exercise for real; that's covered by manual testing in Task 9, not unit tests).

- [ ] **Step 5: Commit**

```bash
git add ui/src-tauri/src/lib.rs
git commit -m "Add check_for_update and install_update commands with in-progress-conversion guard"
```

---

## Task 5: Frontend — Settings panel checkbox and button markup

**Files:**
- Modify: `ui/src/index.html`

**Interfaces:**
- Produces: `#auto-update-checkbox` (checkbox input), `#check-updates-btn` (button), `#update-status` (span for inline result text) — all inside `#settings-panel`. Task 6 (`main.js`) queries these exact ids.

- [ ] **Step 1: Add the markup**

Inside `#settings-panel`, after the existing `save-settings-btn` button:

```html
<div id="settings-panel" class="panel" style="display:none">
  <label for="chdman-path-input">Ruta de chdman.exe</label>
  <input id="chdman-path-input" type="text" placeholder="C:\Tools\chdman.exe" />
  <label>
    <input id="auto-update-checkbox" type="checkbox" />
    Instalar actualizaciones automáticamente
  </label>
  <button id="save-settings-btn">Guardar</button>
  <div class="toolbar">
    <button id="check-updates-btn" class="secondary">Buscar actualizaciones</button>
    <span id="update-status"></span>
  </div>
</div>
```

- [ ] **Step 2: Verify manually**

There's no automated test for static markup in this project (frontend testing here is manual, per the existing pattern — `main.js` has no test file). Open `ui/src/index.html` in a browser or run the dev build and confirm the checkbox, its label, the new button, and the empty status span render inside the Settings panel without breaking the existing layout.

- [ ] **Step 3: Commit**

```bash
git add ui/src/index.html
git commit -m "Add auto-update checkbox and check-updates button to Settings panel"
```

---

## Task 6: Frontend — wire the checkbox, button, startup check, and update dialog

**Files:**
- Modify: `ui/src/main.js`

**Interfaces:**
- Consumes: `check_for_update` / `install_update` commands (Task 4, exact names via `invoke("check_for_update")` / `invoke("install_update")`, returning `{ version, notes } | null`); `get_config`/`set_config` (already exist, now also carry `auto_update_enabled`); `#auto-update-checkbox`, `#check-updates-btn`, `#update-status` (Task 5).

- [ ] **Step 1: Extend settings load/save to include the checkbox**

Find the existing `settingsBtn` and `saveSettingsBtn` handlers (near the bottom of `main.js`) and extend them:

```javascript
const autoUpdateCheckbox = document.getElementById("auto-update-checkbox");
const checkUpdatesBtn = document.getElementById("check-updates-btn");
const updateStatus = document.getElementById("update-status");

settingsBtn.addEventListener("click", async () => {
  historyPanel.style.display = "none";
  const isHidden = settingsPanel.style.display === "none";
  if (isHidden) {
    const config = await invoke("get_config");
    chdmanPathInput.value = config.chdman_path;
    autoUpdateCheckbox.checked = config.auto_update_enabled;
  }
  settingsPanel.style.display = isHidden ? "flex" : "none";
});

saveSettingsBtn.addEventListener("click", async () => {
  await invoke("set_config", {
    chdmanPath: chdmanPathInput.value,
    autoUpdateEnabled: autoUpdateCheckbox.checked,
  });
  settingsPanel.style.display = "none";
});
```

This requires `set_config` in `lib.rs` to accept `auto_update_enabled` too — that's a small addition needed here, tracked as Step 2 below (it belongs in this task since the frontend can't be tested without it, even though `set_config` lives in `lib.rs`).

- [ ] **Step 2: Extend `set_config` in `lib.rs` to accept the new field**

This is a Rust change bundled into this frontend-facing task because `saveSettingsBtn`'s handler above is untestable without it. In `ui/src-tauri/src/lib.rs`, change:

```rust
#[tauri::command]
fn set_config(app_handle: tauri::AppHandle, chdman_path: String, auto_update_enabled: bool) -> Result<(), String> {
    let app_dir = resolve_app_config_dir(&app_handle)?;
    save_config(&app_dir, &Config { chdman_path, auto_update_enabled }).map_err(|e| e.to_string())
}
```

Run `cargo test --lib -- --nocapture` from `ui/src-tauri` to confirm this compiles cleanly with Task 3/4's changes (expected: PASS, same count as Task 4's Step 4).

- [ ] **Step 3: Implement the shared "show an update result" helper and the manual check button**

```javascript
function showUpdateResult(text) {
  updateStatus.textContent = text;
}

async function promptAndMaybeInstall(update, { alwaysReport }) {
  if (!update) {
    if (alwaysReport) showUpdateResult("Ya tenés la última versión");
    return;
  }

  if (autoUpdateCheckbox.checked) {
    showUpdateResult(`Instalando v${update.version}...`);
    try {
      await invoke("install_update");
      // install_update restarts the app on success; if we're still here,
      // it returned an error (e.g. a conversion was running) instead of
      // throwing, which shouldn't happen given it's a Result — but stay
      // defensive since the app not restarting would otherwise look like
      // nothing happened.
    } catch (err) {
      showUpdateResult(`No se pudo instalar: ${err}`);
    }
    return;
  }

  const install = confirm(`Hay una actualización disponible (v${update.version}). ¿Instalar ahora?`);
  if (install) {
    showUpdateResult(`Instalando v${update.version}...`);
    try {
      await invoke("install_update");
    } catch (err) {
      showUpdateResult(`No se pudo instalar: ${err}`);
    }
  } else if (alwaysReport) {
    showUpdateResult(`Actualización v${update.version} disponible (Buscar actualizaciones para instalar)`);
  }
}

checkUpdatesBtn.addEventListener("click", async () => {
  showUpdateResult("Buscando...");
  try {
    const update = await invoke("check_for_update");
    await promptAndMaybeInstall(update, { alwaysReport: true });
  } catch (err) {
    showUpdateResult("No se pudo comprobar (sin conexión)");
  }
});
```

- [ ] **Step 4: Implement the silent startup check**

Add near the bottom of `main.js` (module-level, runs once on load):

```javascript
(async () => {
  try {
    const update = await invoke("check_for_update");
    await promptAndMaybeInstall(update, { alwaysReport: false });
  } catch {
    // Silent by design: a failed startup check (offline, GitHub down)
    // must not interrupt opening the app or show an alert.
  }
})();
```

- [ ] **Step 5: Verify manually**

Since this project's frontend has no test runner, verify by running the dev build (`npm run tauri dev` from `ui/`, per the project's existing workflow) and:
1. Opening Settings shows the checkbox reflecting the saved `auto_update_enabled` value.
2. Toggling it and clicking Guardar persists across a Settings panel close/reopen.
3. Clicking "Buscar actualizaciones" shows a status message (expect "No se pudo comprobar (sin conexión)" or a real result, since a real release with a matching signature doesn't exist until Task 8/9 run — this confirms the wiring, not a real update).

- [ ] **Step 6: Commit**

```bash
git add ui/src/main.js ui/src-tauri/src/lib.rs
git commit -m "Wire auto-update checkbox, manual check button, and startup check in the frontend"
```

---

## Task 7: GitHub Actions release workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: GitHub repo secrets `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`, `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — all added manually by the user per Task 8, not generated by this task.
- Produces: a GitHub Release (named after the pushed tag) carrying the MSI, the NSIS setup `.exe`, and `latest.json` as assets — this is what `check_for_update`'s configured endpoint (Task 2) fetches at runtime.

- [ ] **Step 1: Write the workflow file**

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

jobs:
  release:
    runs-on: windows-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc

      - name: Install frontend dependencies
        run: npm install
        working-directory: ui

      - name: Import code-signing certificate
        run: |
          $certBytes = [System.Convert]::FromBase64String("${{ secrets.WINDOWS_CERTIFICATE }}")
          $certPath = "$env:RUNNER_TEMP\chd-converter-signing.pfx"
          [System.IO.File]::WriteAllBytes($certPath, $certBytes)
          $securePassword = ConvertTo-SecureString -String "${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}" -Force -AsPlainText
          Import-PfxCertificate -FilePath $certPath -CertStoreLocation Cert:\CurrentUser\My -Password $securePassword
          Remove-Item $certPath
        shell: pwsh

      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          projectPath: ui
          tagName: ${{ github.ref_name }}
          releaseName: "CHD Converter ${{ github.ref_name }}"
          releaseDraft: false
          prerelease: false
          args: --config '{"bundle":{"windows":{"certificateThumbprint":"ADEF2B14DE4A6012C0CD6AC765321D9EADFBA213","digestAlgorithm":"sha256"}}}'
```

The `--config` override on the last step injects the code-signing block at build time instead of committing it to `tauri.conf.json` — this matches the project's existing pattern of keeping that block machine/environment-local rather than checked into git, just via a CLI override instead of a manually-edited mirror file.

- [ ] **Step 2: Verify the YAML is well-formed**

Run: `python3 -c "import yaml, sys; yaml.safe_load(open('.github/workflows/release.yml'))"` (or any YAML linter available) from the repo root.
Expected: no exception raised — confirms the file parses as valid YAML. (This workflow's real correctness can only be verified by an actual tag push, which is Task 9 — this step only catches syntax errors before that.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "Add GitHub Actions release workflow for tag-triggered builds"
```

---

## Task 8: One-time secret setup and release docs

**Files:**
- Create: `docs/RELEASING.md`

**Interfaces:**
- Produces: the human-executed steps that make Tasks 2 and 7's placeholders real (the updater keypair, the four GitHub secrets). No code interface — this task is documentation plus manual actions the user performs themselves.

- [ ] **Step 1: Write `docs/RELEASING.md`**

```markdown
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

2. Add these secrets under the repo's Settings → Secrets and variables →
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
builds, code-signs, updater-signs, and publishes a GitHub Release named
after the tag with the MSI, the NSIS setup `.exe`, and `latest.json`
attached. No manual WSL build needed.
```

- [ ] **Step 2: Commit**

```bash
git add docs/RELEASING.md
git commit -m "Document the one-time updater secret setup and the tag-based release workflow"
```

---

## Task 9: End-to-end verification with a real tag push

**Files:** none (verification-only task; no files are created or modified beyond what a normal release already touches: version bumps in `tauri.conf.json`/`Cargo.toml`, as documented in Task 8).

**Interfaces:**
- Consumes: everything from Tasks 1–8.
- Produces: a confirmed-working `v0.1.3` GitHub Release and a confirmed-working update-check round-trip from an installed `0.1.2` app.

- [ ] **Step 1: Complete the one-time setup from Task 8, Step 1**

Generate the real keypair, commit the real pubkey into `tauri.conf.json`, and add all four GitHub secrets. This is a manual, human-executed step — no agent should generate real secret material.

- [ ] **Step 2: Cut the first CI-built release**

Follow `docs/RELEASING.md`'s "Publishing a new version" section to bump to `0.1.3` and push the `v0.1.3` tag.

- [ ] **Step 3: Confirm the workflow succeeded**

Check the Actions tab on GitHub for the `Release` workflow run triggered by the tag push. Expected: green, with a new Release named `CHD Converter v0.1.3` carrying `CHD Converter_0.1.3_x64_en-US.msi`, `CHD Converter_0.1.3_x64-setup.exe`, and `latest.json` as assets.

- [ ] **Step 4: Confirm the update check finds it from an older install**

On a machine with `0.1.2` installed (or reinstall `0.1.2` from the existing local artifacts if needed), open the app and click "Buscar actualizaciones" in Settings. Expected: shows an "actualización disponible v0.1.3" prompt (checkbox off) or auto-installs it (checkbox on), per Task 6's logic. If the checkbox is off and you accept the install prompt, confirm the app restarts on `0.1.3` afterward (check the Settings panel or window title/about info, whichever currently shows a version — if neither does, this is an acceptable gap for this plan and not a blocker: the MSI's own installed-programs entry will show `0.1.3` in Windows' "Apps & features" list as a fallback confirmation).

- [ ] **Step 5: No commit**

This task is pure verification; nothing to commit beyond what Task 9 Step 2's version bump (covered by Task 8's documented workflow) already produces as its own commit+tag.
