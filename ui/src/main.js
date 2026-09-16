const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;

let currentFolder = null;
let discs = []; // [{ name, folder, kind, status: "pending"|"ok"|"skip"|"fail", message: "" }]

const pickFolderBtn = document.getElementById("pick-folder-btn");
const folderLabel = document.getElementById("folder-label");
const convertBtn = document.getElementById("convert-btn");
const cancelBtn = document.getElementById("cancel-btn");
const progressFill = document.getElementById("progress-bar-fill");
const discTable = document.getElementById("disc-table");

function mk(cls, text) {
  const s = document.createElement("span");
  s.className = cls;
  s.textContent = text ?? "";
  return s;
}

function renderTable() {
  discTable.innerHTML = "";
  for (const disc of discs) {
    const row = document.createElement("div");
    row.className = "disc-row";
    // "cancel" is a frontend-only status: the backend never emits it, it's
    // applied locally to discs left pending when a run ends cancelled.
    const icon = { pending: "•", ok: "✅", skip: "⏭️", fail: "❌", cancel: "⏹️" }[disc.status];
    row.append(
      // disc.status is backend-controlled (not a filesystem/user value), so
      // it's safe in the className, but disc.name and disc.message come
      // from real filenames and the .bat's log text — build them via
      // textContent, never innerHTML, so HTML-like characters in a
      // filename (e.g. "<img src=x onerror=...>.cue") render as literal
      // text instead of executing as markup.
      mk(`disc-status-icon status-${disc.status}`, icon),
      mk("disc-name", disc.name),
      mk("disc-message", disc.message ?? "")
    );
    discTable.appendChild(row);
  }
}

function updateProgress() {
  const done = discs.filter((d) => d.status !== "pending").length;
  const pct = discs.length === 0 ? 0 : Math.round((done / discs.length) * 100);
  progressFill.style.width = `${pct}%`;
}

pickFolderBtn.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;

  currentFolder = selected;
  folderLabel.textContent = selected;

  const scanned = await invoke("prescan", { root: selected });
  discs = scanned.map((d) => ({ ...d, status: "pending", message: "" }));
  renderTable();
  updateProgress();
  convertBtn.disabled = discs.length === 0;
});

convertBtn.addEventListener("click", async () => {
  // Disable synchronously, BEFORE the await: hiding the button only after
  // start_conversion resolves leaves a window in which a fast double-click
  // fires two IPC calls, which the backend then rejects with "Ya hay una
  // conversión en curso".
  convertBtn.disabled = true;
  try {
    await invoke("start_conversion", { root: currentFolder });
    convertBtn.style.display = "none";
    cancelBtn.style.display = "inline-block";
  } catch (err) {
    // A real failure (bad chdman path, spawn error) must not lock the button
    // forever — re-enable so the user can fix the setting and retry.
    convertBtn.disabled = false;
    alert(`No se pudo iniciar la conversion: ${err}`);
  }
});

cancelBtn.addEventListener("click", async () => {
  await invoke("cancel_conversion");
});

// Windows paths use backslashes; the backend reports the source path exactly
// as chdman/cmd.exe see it (an absolute path like "C:\Games\GameA\Track.cue"),
// while a ScannedDisc only carries { name, folder, kind } from the pre-scan.
// Match by comparing the disc's folder+name against the tail of the reported
// path, case-insensitively and with backslashes normalized to forward
// slashes, so drive-letter casing or slash-style differences between the
// scan and the log don't break the match. This assumes folder+name is
// unique per run; two identically-named discs in different folders are
// still disambiguated correctly since the folder is part of the comparison,
// but a disc that appears twice under the *same* folder+name (not possible
// from a single filesystem scan) would be ambiguous.
function matchDiscByPath(path) {
  const normalizedPath = path.toLowerCase().replace(/\\/g, "/");
  return discs.find((d) => {
    const discFull = `${d.folder}/${d.name}`.toLowerCase().replace(/\\/g, "/").replace(/\/+/g, "/");
    return normalizedPath === discFull || normalizedPath.endsWith(`/${discFull}`.replace(/\/+/g, "/")) || normalizedPath.endsWith(discFull);
  });
}

listen("disc-updated", (event) => {
  const { status, path, message } = event.payload;
  const disc = matchDiscByPath(path);
  if (disc) {
    disc.status = status.toLowerCase(); // Rust enum serializes as "Ok" | "Skip" | "Fail"
    disc.message = message;
    renderTable();
    updateProgress();
  }
});

listen("run-finished", (event) => {
  // A cancelled run leaves discs that were never reached stuck on the pending
  // "•" forever, which reads as "still working". Mark them as cancelled.
  if (event?.payload?.cancelled) {
    for (const disc of discs) {
      if (disc.status === "pending") disc.status = "cancel";
    }
    renderTable();
    updateProgress();
  }
  convertBtn.style.display = "inline-block";
  // Re-enable: the click handler disabled it synchronously at run start.
  convertBtn.disabled = discs.length === 0;
  cancelBtn.style.display = "none";
});

const settingsBtn = document.getElementById("settings-btn");
const settingsPanel = document.getElementById("settings-panel");
const chdmanPathInput = document.getElementById("chdman-path-input");
const saveSettingsBtn = document.getElementById("save-settings-btn");
const historyBtn = document.getElementById("history-btn");
const historyPanel = document.getElementById("history-panel");
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

historyBtn.addEventListener("click", async () => {
  settingsPanel.style.display = "none";
  const isHidden = historyPanel.style.display === "none";
  if (isHidden) {
    const history = await invoke("get_history");
    historyPanel.innerHTML = "";
    for (const run of history.slice().reverse()) {
      const date = new Date(Number(run.timestamp) * 1000).toLocaleString();
      const status = run.cancelled
        ? "Cancelado"
        : `${run.converted} convertidos, ${run.skipped} saltados, ${run.failed} fallidos`;
      const row = document.createElement("div");
      row.className = "history-row";
      row.append(mk("history-folder", run.folder), mk("", date), mk("", status));
      historyPanel.appendChild(row);
    }
  }
  historyPanel.style.display = isHidden ? "flex" : "none";
});

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

(async () => {
  try {
    const update = await invoke("check_for_update");
    await promptAndMaybeInstall(update, { alwaysReport: false });
  } catch {
    // Silent by design: a failed startup check (offline, GitHub down)
    // must not interrupt opening the app or show an alert.
  }
})();
