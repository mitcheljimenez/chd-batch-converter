import { t, setLanguage, translateError } from "./i18n.js";

const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;

let currentFolder = null;
let discs = []; // [{ name, folder, kind, status: "pending"|"ok"|"skip"|"fail", message: "" }]

const pickFolderBtn = document.getElementById("pick-folder-btn");
const folderLabel = document.getElementById("folder-label");
const convertBtn = document.getElementById("convert-btn");
const cancelBtn = document.getElementById("cancel-btn");
const progressTrack = document.getElementById("progress-bar-track");
const progressFill = document.getElementById("progress-bar-fill");
const discTable = document.getElementById("disc-table");

const navConvert = document.getElementById("nav-convert");
const navOrganize = document.getElementById("nav-organize");
const navHistory = document.getElementById("nav-history");
const navSettings = document.getElementById("nav-settings");
const convertView = document.getElementById("convert-view");
const organizeView = document.getElementById("organize-view");
const historyView = document.getElementById("history-view");
const settingsView = document.getElementById("settings-view");

const chdmanPathLabel = document.getElementById("chdman-path-label");
const chdmanPathInput = document.getElementById("chdman-path-input");
const saveSettingsBtn = document.getElementById("save-settings-btn");
const historyPanel = document.getElementById("history-panel");
const organizeExplanation = document.getElementById("organize-explanation");
const organizeNoFolderHint = document.getElementById("organize-no-folder-hint");
const storageModePcLabel = document.getElementById("storage-mode-pc-label");
const storageModeInternalLabel = document.getElementById("storage-mode-internal-label");
const storageModeExternalLabel = document.getElementById("storage-mode-external-label");
const storageModeExternalTooltip = document.getElementById("storage-mode-external-tooltip");
const storageModeExternalRadio = document.getElementById("storage-mode-external");
const sdCardIdInput = document.getElementById("sd-card-id-input");
const runOrganizeBtn = document.getElementById("run-organize-btn");
const autoUpdateCheckbox = document.getElementById("auto-update-checkbox");
const autoUpdateLabelText = document.getElementById("auto-update-label-text");
const languageLabelText = document.getElementById("language-label-text");
const languageSelect = document.getElementById("language-select");
const checkUpdatesBtn = document.getElementById("check-updates-btn");
const updateStatus = document.getElementById("update-status");

const views = { convert: convertView, organize: organizeView, history: historyView, settings: settingsView };
const navButtons = { convert: navConvert, organize: navOrganize, history: navHistory, settings: navSettings };

// Switches the visible section and highlights its sidebar entry. History
// and Settings need to refresh from backend state every time they're
// opened (a run may have finished, or config may have changed), so this
// runs their existing "on open" side effects here instead of tying them to
// a per-panel toggle button.
async function showView(name) {
  for (const key of Object.keys(views)) {
    views[key].style.display = key === name ? "flex" : "none";
    navButtons[key].classList.toggle("active", key === name);
  }
  if (name === "settings") {
    const config = await invoke("get_config");
    chdmanPathInput.value = config.chdman_path;
    autoUpdateCheckbox.checked = config.auto_update_enabled;
    languageSelect.value = config.language;
  } else if (name === "history") {
    await renderHistory();
  } else if (name === "organize") {
    updateOrganizeAvailability();
  }
}

navConvert.addEventListener("click", () => showView("convert"));
navOrganize.addEventListener("click", () => showView("organize"));
navHistory.addEventListener("click", () => showView("history"));
navSettings.addEventListener("click", () => showView("settings"));

// Re-applies every static piece of UI text in the current language. Called
// once at startup (after the persisted language loads) and again whenever
// the user switches languages from the selector.
function applyTranslations() {
  pickFolderBtn.textContent = t("pickFolder");
  folderLabel.textContent = currentFolder ?? t("noFolderSelected");
  convertBtn.textContent = t("convertAll");
  cancelBtn.textContent = t("cancel");
  navConvert.textContent = t("navConvert");
  navOrganize.textContent = t("organizeMultidisc");
  navHistory.textContent = t("history");
  navSettings.textContent = t("settings");
  chdmanPathLabel.textContent = t("chdmanPathLabel");
  autoUpdateLabelText.textContent = t("autoUpdateLabel");
  languageLabelText.textContent = t("languageLabel");
  saveSettingsBtn.textContent = t("save");
  checkUpdatesBtn.textContent = t("checkUpdates");
  organizeExplanation.textContent = t("organizeExplanation");
  storageModePcLabel.textContent = t("storageModePc");
  storageModeInternalLabel.textContent = t("storageModeInternal");
  storageModeExternalLabel.textContent = t("storageModeExternal");
  storageModeExternalTooltip.title = t("storageExternalTooltip");
  sdCardIdInput.placeholder = t("sdCardIdPlaceholder");
  runOrganizeBtn.textContent = t("runOrganize");
}

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

function updateOrganizeAvailability() {
  const hasFolder = Boolean(currentFolder);
  organizeNoFolderHint.style.display = hasFolder ? "none" : "block";
  runOrganizeBtn.disabled = !hasFolder;
}

pickFolderBtn.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;

  currentFolder = selected;
  folderLabel.textContent = selected;
  updateOrganizeAvailability();

  const scanned = await invoke("prescan", { root: selected });
  discs = scanned.map((d) => ({ ...d, status: "pending", message: "" }));
  renderTable();
  updateProgress();
  convertBtn.disabled = discs.length === 0;
});

storageModeExternalRadio.addEventListener("change", () => {
  sdCardIdInput.style.display = storageModeExternalRadio.checked ? "block" : "none";
});

runOrganizeBtn.addEventListener("click", async () => {
  if (!currentFolder) return;

  const mode = document.querySelector('input[name="storage-mode"]:checked')?.value ?? "pc";
  let androidBase = null;
  if (mode === "internal") {
    androidBase = "/storage/emulated/0/ROMs";
  } else if (mode === "external" && sdCardIdInput.value.trim()) {
    androidBase = `/storage/${sdCardIdInput.value.trim()}/ROMs`;
  }

  try {
    const summary = await invoke("organize_multidisc", { root: currentFolder, androidBase });
    alert(t("organizeSummary", summary));
  } catch (err) {
    alert(t("organizeFailed", translateError(err)));
  }
});

convertBtn.addEventListener("click", async () => {
  // Disable synchronously, BEFORE the await: hiding the button only after
  // start_conversion resolves leaves a window in which a fast double-click
  // fires two IPC calls, which the backend then rejects with
  // CONVERSION_IN_PROGRESS.
  convertBtn.disabled = true;
  try {
    await invoke("start_conversion", { root: currentFolder });
    convertBtn.style.display = "none";
    cancelBtn.style.display = "inline-block";
    progressTrack.style.display = "block";
  } catch (err) {
    // A real failure (bad chdman path, spawn error) must not lock the button
    // forever — re-enable so the user can fix the setting and retry.
    convertBtn.disabled = false;
    alert(t("conversionStartError", translateError(err)));
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

saveSettingsBtn.addEventListener("click", async () => {
  await invoke("set_config", {
    chdmanPath: chdmanPathInput.value,
    autoUpdateEnabled: autoUpdateCheckbox.checked,
    language: languageSelect.value,
  });
});

// Saved on its own, independent of the Guardar button: a checkbox toggle
// that silently required a separate "Guardar" click to take effect was
// confusing (it looked applied immediately since the checkbox visually
// stayed checked, but the persisted config still held the old value).
autoUpdateCheckbox.addEventListener("change", async () => {
  await invoke("set_config", {
    chdmanPath: chdmanPathInput.value,
    autoUpdateEnabled: autoUpdateCheckbox.checked,
    language: languageSelect.value,
  });
});

// Also saved immediately, and re-renders every static label right away —
// leaving stale text until the next Guardar click would be as confusing as
// the auto-update checkbox was before it got the same treatment.
languageSelect.addEventListener("change", async () => {
  setLanguage(languageSelect.value);
  applyTranslations();
  await invoke("set_config", {
    chdmanPath: chdmanPathInput.value,
    autoUpdateEnabled: autoUpdateCheckbox.checked,
    language: languageSelect.value,
  });
});

async function renderHistory() {
  const history = await invoke("get_history");
  historyPanel.innerHTML = "";
  for (const run of history.slice().reverse()) {
    const date = new Date(Number(run.timestamp) * 1000).toLocaleString();
    const status = run.cancelled
      ? t("cancelled")
      : t("historySummary", { converted: run.converted, skipped: run.skipped, failed: run.failed });
    const row = document.createElement("div");
    row.className = "history-row";
    row.append(mk("history-folder", run.folder), mk("", date), mk("", status));
    historyPanel.appendChild(row);
  }
}

function showUpdateResult(text) {
  updateStatus.textContent = text;
}

// Runs install_update, and on success shows the release notes (falling back
// to a generic line when the release has none) before restarting — restart
// tears the process down immediately, so the notes must be shown and
// dismissed first, never after.
async function installAndShowNotes(update) {
  showUpdateResult(t("installing", update.version));
  try {
    await invoke("install_update");
    alert(t("updateInstalled", update.version, update.notes));
    await invoke("restart_app");
  } catch (err) {
    showUpdateResult(t("installFailed", translateError(err)));
  }
}

async function promptAndMaybeInstall(update, { alwaysReport }) {
  if (!update) {
    if (alwaysReport) showUpdateResult(t("upToDate"));
    return;
  }

  const config = await invoke("get_config");
  const autoUpdateEnabled = config.auto_update_enabled;

  if (autoUpdateEnabled) {
    await installAndShowNotes(update);
    return;
  }

  const install = confirm(t("confirmInstall", update.version));
  if (install) {
    await installAndShowNotes(update);
  } else if (alwaysReport) {
    showUpdateResult(t("updateAvailableDeferred", update.version));
  }
}

checkUpdatesBtn.addEventListener("click", async () => {
  showUpdateResult(t("checking"));
  try {
    const update = await invoke("check_for_update", { silent: false });
    await promptAndMaybeInstall(update, { alwaysReport: true });
  } catch (err) {
    showUpdateResult(t("checkFailed"));
  }
});

(async () => {
  try {
    const update = await invoke("check_for_update", { silent: true });
    await promptAndMaybeInstall(update, { alwaysReport: false });
  } catch {
    // Silent by design: a failed startup check (offline, GitHub down)
    // must not interrupt opening the app or show an alert.
  }
})();

// Runs once at startup so every visible label reflects the persisted
// language from the first frame, instead of only updating once Settings
// happens to be opened.
(async () => {
  const config = await invoke("get_config");
  setLanguage(config.language);
  applyTranslations();
  languageSelect.value = config.language;
  autoUpdateCheckbox.checked = config.auto_update_enabled;
  chdmanPathInput.value = config.chdman_path;
  updateOrganizeAvailability();
})();
