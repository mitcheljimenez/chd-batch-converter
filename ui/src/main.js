import {
  t,
  setLanguage,
  setPlatform,
  chdmanInstallCommand,
  translateError,
  formatBytes,
  formatPercentChange,
} from "./i18n.js";

const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;
const { openPath } = window.__TAURI__.opener;

let currentFolder = null;
let organizeDestination = null;
let moveChdDestination = null;
let discs = []; // [{ name, folder, kind, status: "pending"|"ok"|"skip"|"fail", message: "" }]
let formatOverrides = new Set(); // full paths (see joinPath) the user forced to CD format

const pickFolderBtn = document.getElementById("pick-folder-btn");
const rescanBtn = document.getElementById("rescan-btn");
const convertOpenFolderBtn = document.getElementById("convert-open-folder-btn");
const folderLabel = document.getElementById("folder-label");
const convertBtn = document.getElementById("convert-btn");
const cancelBtn = document.getElementById("cancel-btn");
const progressTrack = document.getElementById("progress-bar-track");
const progressFill = document.getElementById("progress-bar-fill");
const discsPendingLabel = document.getElementById("discs-pending-label");
const discTable = document.getElementById("disc-table");

const navConvert = document.getElementById("nav-convert");
const navOrganize = document.getElementById("nav-organize");
const navMoveChd = document.getElementById("nav-move-chd");
const navFlatten = document.getElementById("nav-flatten");
const navHistory = document.getElementById("nav-history");
const navSettings = document.getElementById("nav-settings");
const convertView = document.getElementById("convert-view");
const organizeView = document.getElementById("organize-view");
const moveChdView = document.getElementById("move-chd-view");
const flattenView = document.getElementById("flatten-view");
const historyView = document.getElementById("history-view");
const settingsView = document.getElementById("settings-view");

const chdmanPathLabel = document.getElementById("chdman-path-label");
const chdmanPathInput = document.getElementById("chdman-path-input");
const chdmanInstallHint = document.getElementById("chdman-install-hint");
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
const organizeOpenDestBtn = document.getElementById("organize-open-dest-btn");
const organizePickDestBtn = document.getElementById("organize-pick-dest-btn");
const organizeDestLabel = document.getElementById("organize-dest-label");
const moveChdExplanation = document.getElementById("move-chd-explanation");
const moveChdNoFolderHint = document.getElementById("move-chd-no-folder-hint");
const moveChdPickDestBtn = document.getElementById("move-chd-pick-dest-btn");
const moveChdDestLabel = document.getElementById("move-chd-dest-label");
const runMoveChdBtn = document.getElementById("run-move-chd-btn");
const moveChdOpenDestBtn = document.getElementById("move-chd-open-dest-btn");
const flattenExplanation = document.getElementById("flatten-explanation");
const flattenNoFolderHint = document.getElementById("flatten-no-folder-hint");
const runFlattenBtn = document.getElementById("run-flatten-btn");
const navExtract = document.getElementById("nav-extract");
const extractView = document.getElementById("extract-view");
const extractExplanation = document.getElementById("extract-explanation");
const extractNoFolderHint = document.getElementById("extract-no-folder-hint");
const extractAllBtn = document.getElementById("extract-all-btn");
const extractProgressTrack = document.getElementById("extract-progress-track");
const extractProgressFill = document.getElementById("extract-progress-fill");
const extractTable = document.getElementById("extract-table");
const autoUpdateCheckbox = document.getElementById("auto-update-checkbox");
const autoUpdateLabelText = document.getElementById("auto-update-label-text");
const parallelConversionCheckbox = document.getElementById("parallel-conversion-checkbox");
const trashOriginalsCheckbox = document.getElementById("trash-originals-checkbox");
const savingsLabel = document.getElementById("savings-label");
const trashOriginalsLabelText = document.getElementById("trash-originals-label-text");
const parallelConversionLabelText = document.getElementById("parallel-conversion-label-text");
const parallelConversionHint = document.getElementById("parallel-conversion-hint");
const languageLabelText = document.getElementById("language-label-text");
const languageSelect = document.getElementById("language-select");
const checkUpdatesBtn = document.getElementById("check-updates-btn");
const updateStatus = document.getElementById("update-status");
const appVersionLabel = document.getElementById("app-version-label");

const updateOverlay = document.getElementById("update-overlay");
const updateOverlayText = document.getElementById("update-overlay-text");
const updateOverlayProgressTrack = document.getElementById("update-overlay-progress-track");
const updateOverlayProgressFill = document.getElementById("update-overlay-progress-fill");
const updateOverlayActionBtn = document.getElementById("update-overlay-action-btn");
const updateOverlayNotes = document.getElementById("update-overlay-notes");
const updateOverlayNotesLabel = document.getElementById("update-overlay-notes-label");
const updateOverlayNotesBody = document.getElementById("update-overlay-notes-body");

const views = {
  convert: convertView,
  organize: organizeView,
  moveChd: moveChdView,
  flatten: flattenView,
  extract: extractView,
  history: historyView,
  settings: settingsView,
};
const navButtons = {
  convert: navConvert,
  organize: navOrganize,
  moveChd: navMoveChd,
  flatten: navFlatten,
  extract: navExtract,
  history: navHistory,
  settings: navSettings,
};

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
    parallelConversionCheckbox.checked = config.parallel_conversion;
    languageSelect.value = config.language;
  } else if (name === "history") {
    await renderHistory();
  } else if (name === "organize") {
    updateOrganizeAvailability();
  } else if (name === "moveChd") {
    updateMoveChdAvailability();
  } else if (name === "flatten") {
    updateFlattenAvailability();
  } else if (name === "extract") {
    // Only re-scan the first time this folder's Extract tab is opened, not
    // on every visit -- otherwise navigating away and back (or just
    // switching tabs mid-run) would wipe out-of-progress/finished rows and
    // replace them with a fresh "pending" scan, discarding real state for
    // no reason.
    extractNoFolderHint.style.display = currentFolder ? "none" : "block";
    if (currentFolder && chdsScannedForFolder !== currentFolder && !extractRunning) {
      await rescanChds();
    } else {
      renderExtractTable();
      updateExtractAllAvailability();
    }
  }
}

navConvert.addEventListener("click", () => showView("convert"));
navOrganize.addEventListener("click", () => showView("organize"));
navMoveChd.addEventListener("click", () => showView("moveChd"));
navFlatten.addEventListener("click", () => showView("flatten"));
navExtract.addEventListener("click", () => showView("extract"));
navHistory.addEventListener("click", () => showView("history"));
navSettings.addEventListener("click", () => showView("settings"));

// Re-applies every static piece of UI text in the current language. Called
// once at startup (after the persisted language loads) and again whenever
// Fetched once at startup from Tauri's app API; kept in module state so
// applyTranslations() can re-render the label's text in the new language
// without re-invoking the backend every time.
let appVersion = null;
// "windows" | "linux" | "macos", also fetched once at startup.
let platform = "windows";

// Joins a folder and file name the way the backend's Path::join does, so a
// path built here compares equal to (and can be handed back as) one built
// in Rust: "\\" on Windows, "/" on Linux/macOS.
function joinPath(folder, name) {
  return `${folder}${platform === "windows" ? "\\" : "/"}${name}`;
}

function updateVersionLabel() {
  if (appVersion) appVersionLabel.textContent = t("versionLabel", appVersion);
}

// the user switches languages from the selector.
function applyTranslations() {
  pickFolderBtn.textContent = t("pickFolder");
  rescanBtn.textContent = t("rescan");
  convertOpenFolderBtn.textContent = t("openFolder");
  folderLabel.textContent = currentFolder ?? t("noFolderSelected");
  convertBtn.textContent = t("convertAll");
  cancelBtn.textContent = t("cancel");
  navConvert.textContent = t("navConvert");
  navOrganize.textContent = t("organizeMultidisc");
  navHistory.textContent = t("history");
  navSettings.textContent = t("settings");
  chdmanPathLabel.textContent = t("chdmanPathLabel");
  const installCommand = chdmanInstallCommand();
  chdmanInstallHint.style.display = installCommand ? "" : "none";
  if (installCommand) chdmanInstallHint.textContent = t("chdmanInstallHint", installCommand);
  autoUpdateLabelText.textContent = t("autoUpdateLabel");
  parallelConversionLabelText.textContent = t("parallelConversionLabel");
  parallelConversionHint.textContent = t("parallelConversionHint");
  trashOriginalsLabelText.textContent = t("trashOriginalsLabel");
  languageLabelText.textContent = t("languageLabel");
  saveSettingsBtn.textContent = t("save");
  checkUpdatesBtn.textContent = t("checkUpdates");
  updateVersionLabel();
  updatePendingLabel();
  organizeExplanation.textContent = t("organizeExplanation");
  storageModePcLabel.textContent = t("storageModePc");
  storageModeInternalLabel.textContent = t("storageModeInternal");
  storageModeExternalLabel.textContent = t("storageModeExternal");
  storageModeExternalTooltip.title = t("storageExternalTooltip");
  sdCardIdInput.placeholder = t("sdCardIdPlaceholder");
  runOrganizeBtn.textContent = t("runOrganize");
  organizePickDestBtn.textContent = t("pickDestination");
  organizeDestLabel.textContent = organizeDestination ?? t("noDestinationSelected");
  organizeOpenDestBtn.textContent = t("openDestFolder");
  navMoveChd.textContent = t("navMoveChd");
  moveChdExplanation.textContent = t("moveChdExplanation");
  moveChdNoFolderHint.textContent = t("moveChdNoFolderHint");
  moveChdPickDestBtn.textContent = t("pickDestination");
  moveChdDestLabel.textContent = moveChdDestination ?? t("noDestinationSelected");
  runMoveChdBtn.textContent = t("runMoveChd");
  moveChdOpenDestBtn.textContent = t("openDestFolder");
  navFlatten.textContent = t("navFlatten");
  flattenExplanation.textContent = t("flattenExplanation");
  flattenNoFolderHint.textContent = t("flattenNoFolderHint");
  runFlattenBtn.textContent = t("runFlatten");
  navExtract.textContent = t("navExtract");
  extractExplanation.textContent = t("extractExplanation");
  extractAllBtn.textContent = t("extractAllBtn");
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
    const main = document.createElement("div");
    main.className = "disc-row-main";
    // disc.status is backend-controlled (not a filesystem/user value), so
    // it's safe in the className, but disc.name and disc.message come
    // from real filenames and the .bat's log text — build them via
    // textContent, never innerHTML, so HTML-like characters in a
    // filename (e.g. "<img src=x onerror=...>.cue") render as literal
    // text instead of executing as markup.
    main.append(
      mk(`disc-status-icon status-${disc.status}`, icon),
      mk("disc-name", disc.name),
      mk("disc-message", disc.status === "pending" && disc.progressPercent !== undefined
        ? t(disc.progressPhase === "verifying" ? "phaseVerifying" : "phaseCompressing", Math.round(disc.progressPercent))
        : disc.message ?? "")
    );

    if (disc.kind === "iso" && disc.status === "pending") {
      const select = document.createElement("select");
      select.className = "disc-format-override";
      const dvdOption = document.createElement("option");
      dvdOption.value = "dvd";
      dvdOption.textContent = t("overrideFormatDvd");
      const cdOption = document.createElement("option");
      cdOption.value = "cd";
      cdOption.textContent = t("overrideFormatCd");
      select.append(dvdOption, cdOption);
      select.value = formatOverrides.has(joinPath(disc.folder, disc.name)) ? "cd" : "dvd";
      select.addEventListener("change", () => {
        const fullPath = joinPath(disc.folder, disc.name);
        if (select.value === "cd") {
          formatOverrides.add(fullPath);
        } else {
          formatOverrides.delete(fullPath);
        }
      });
      main.appendChild(select);
    }

    row.appendChild(main);

    if (disc.status === "pending" && disc.progressPercent !== undefined) {
      const track = document.createElement("div");
      track.className = "disc-progress-track";
      const fill = document.createElement("div");
      fill.className = "disc-progress-fill";
      fill.style.width = `${Math.round(disc.progressPercent)}%`;
      track.appendChild(fill);
      row.appendChild(track);
    }

    discTable.appendChild(row);
  }
}

// How many discs are still pending conversion, i.e. haven't reached a
// terminal status ("ok"/"skip"/"fail"/"cancel") yet -- unlike `discs.length`
// (the total scanned), this drops as each disc finishes so the "N files to
// convert" label stays accurate mid-run instead of just showing the count
// from the initial scan forever.
// Discs still without a .chd: queued ones plus any a cancelled run never
// finished (its partial .chd is removed, so they still need converting).
function pendingDiscCount() {
  return discs.filter((d) => d.status === "pending" || d.status === "cancel").length;
}

function updatePendingLabel() {
  discsPendingLabel.textContent = t("discsPendingLabel", pendingDiscCount());
}

function updateProgress() {
  if (discs.length === 0) {
    progressFill.style.width = "0%";
    return;
  }
  // Smooth overall progress by counting the active file's own live percent
  // as a fraction of one unit, not just whole finished/pending steps.
  // Compression is weighted as 70% of a file's work and verification as the
  // remaining 30% (compression is typically the slower half).
  let doneUnits = 0;
  for (const disc of discs) {
    if (disc.status !== "pending") {
      doneUnits += 1;
    } else if (disc.progressPercent !== undefined) {
      doneUnits +=
        disc.progressPhase === "verifying"
          ? 0.7 + (disc.progressPercent / 100) * 0.3
          : (disc.progressPercent / 100) * 0.7;
    }
  }
  const pct = Math.min(100, Math.round((doneUnits / discs.length) * 100));
  progressFill.style.width = `${pct}%`;
}

function updateOrganizeAvailability() {
  const hasFolder = Boolean(currentFolder) && Boolean(organizeDestination);
  organizeNoFolderHint.style.display = hasFolder ? "none" : "block";
  runOrganizeBtn.disabled = !hasFolder;
}

function updateMoveChdAvailability() {
  const hasFolders = Boolean(currentFolder) && Boolean(moveChdDestination);
  moveChdNoFolderHint.style.display = hasFolders ? "none" : "block";
  runMoveChdBtn.disabled = !hasFolders;
}

function updateFlattenAvailability() {
  const hasFolder = Boolean(currentFolder);
  flattenNoFolderHint.style.display = hasFolder ? "none" : "block";
  runFlattenBtn.disabled = !hasFolder;
}

organizePickDestBtn.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;
  organizeDestination = selected;
  organizeDestLabel.textContent = selected;
  organizeOpenDestBtn.style.display = "none";
  updateOrganizeAvailability();
});

moveChdPickDestBtn.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;
  moveChdDestination = selected;
  moveChdDestLabel.textContent = selected;
  moveChdOpenDestBtn.style.display = "none";
  updateMoveChdAvailability();
});

runMoveChdBtn.addEventListener("click", async () => {
  if (!currentFolder || !moveChdDestination) return;
  try {
    const summary = await invoke("move_chd_files", { root: currentFolder, destination: moveChdDestination });
    invalidateChdScan();
    alert(t("moveChdSummary", summary));
    moveChdOpenDestBtn.style.display = summary.files_moved > 0 ? "inline-block" : "none";
  } catch (err) {
    alert(t("moveChdFailed", translateError(err)));
  }
});

moveChdOpenDestBtn.addEventListener("click", async () => {
  if (!moveChdDestination) return;
  try {
    await openPath(moveChdDestination);
  } catch (err) {
    alert(translateError(err));
  }
});

runFlattenBtn.addEventListener("click", async () => {
  if (!currentFolder) return;
  try {
    const summary = await invoke("flatten_folders", { root: currentFolder });
    invalidateChdScan();
    alert(t("flattenSummary", summary));
  } catch (err) {
    alert(t("flattenFailed", translateError(err)));
  }
});

async function rescan(root) {
  const scanned = await invoke("prescan", { root });
  formatOverrides = new Set();
  discs = scanned.map((d) => ({ ...d, status: "pending", message: "" }));
  renderTable();
  updateProgress();
  updatePendingLabel();
  convertBtn.disabled = discs.length === 0;
}

let chds = []; // [{ name, folder, kind, status: "pending"|"ok"|"fail", progressPhase, progressPercent, result, error }]
let extractRunning = false;
// Which folder `chds` currently reflects, so showView("extract") can tell a
// genuinely new folder (needs a fresh scan) apart from just re-opening the
// tab on the same one (must NOT wipe existing rows/results).
let chdsScannedForFolder = null;

// Converting, moving, flattening or organizing changes which .chd files
// exist, so the Extract tab's list must be rebuilt on its next visit
// instead of showing the pre-change scan.
function invalidateChdScan() {
  chdsScannedForFolder = null;
}
// The items the in-flight run() was started with -- NOT always all of
// `chds` (a single row's "Extraer" button runs just that one item). The
// overall progress bar must be computed against this subset, not the full
// table, or a single-file extraction finishes at 1/N% instead of 100%.
let currentRunChds = [];

function fullChdPath(chd) {
  return joinPath(chd.folder, chd.name);
}

async function rescanChds() {
  extractNoFolderHint.style.display = currentFolder ? "none" : "block";
  if (!currentFolder) {
    chds = [];
    chdsScannedForFolder = null;
    renderExtractTable();
    updateExtractAllAvailability();
    return;
  }
  const scanned = await invoke("prescan_chds", { root: currentFolder });
  chds = scanned.map((c) => ({ ...c, status: "pending", result: null, error: null }));
  chdsScannedForFolder = currentFolder;
  renderExtractTable();
  updateExtractAllAvailability();
}

function updateExtractAllAvailability() {
  const hasExtractable = chds.some((c) => c.kind !== "unknown" && c.status !== "ok");
  extractAllBtn.disabled = extractRunning || !hasExtractable;
}

// Mirrors updateProgress() for the Convert tab: weighs the active item's own
// live percent as a fraction of one unit instead of only counting whole
// finished/pending steps, so the overall bar moves smoothly instead of
// jumping in steps of 1/N. Extraction is weighted as 70% of an item's work
// and verification as the remaining 30%, matching Convert's
// compressing/verifying split. Computed against `currentRunChds` (the
// items the in-flight run actually includes), NOT the full `chds` table --
// a single row's "Extraer" button only ever touches one item, and dividing
// by every scanned .chd would strand the bar far short of 100% when it
// finishes.
function updateExtractProgress() {
  if (currentRunChds.length === 0) {
    extractProgressFill.style.width = "0%";
    return;
  }
  let doneUnits = 0;
  for (const chd of currentRunChds) {
    if (chd.status !== "pending") {
      doneUnits += 1;
    } else if (chd.progressPercent !== undefined) {
      doneUnits +=
        chd.progressPhase === "verifying"
          ? 0.7 + (chd.progressPercent / 100) * 0.3
          : (chd.progressPercent / 100) * 0.7;
    }
  }
  const pct = Math.min(100, Math.round((doneUnits / currentRunChds.length) * 100));
  extractProgressFill.style.width = `${pct}%`;
}

function renderExtractTable() {
  extractTable.innerHTML = "";
  if (currentFolder && chds.length === 0) {
    extractTable.appendChild(mk("disc-message", t("extractNoFilesFound")));
    return;
  }
  for (const chd of chds) {
    const row = document.createElement("div");
    row.className = "disc-row";
    const kindLabel = { cd: t("extractKindCd"), dvd: t("extractKindDvd"), gd: t("extractKindGd"), unknown: t("extractKindUnknown") }[chd.kind];
    const icon = { pending: "•", ok: "✅", fail: "❌" }[chd.status];
    const main = document.createElement("div");
    main.className = "disc-row-main";

    let messageText = kindLabel;
    if (chd.status === "pending" && chd.progressPercent !== undefined) {
      messageText = t(
        chd.progressPhase === "verifying" ? "phaseExtractVerifying" : "phaseExtracting",
        Math.round(chd.progressPercent)
      );
    } else if (chd.status === "ok") {
      messageText = t("extractDone", chd.result);
    } else if (chd.status === "fail") {
      messageText = chd.error;
    }

    main.append(
      mk(`disc-status-icon status-${chd.status}`, icon),
      mk("disc-name", chd.name),
      mk("disc-message", messageText)
    );

    const btn = document.createElement("button");
    btn.className = "secondary";
    btn.textContent = t("extractBtn");
    btn.disabled = chd.kind === "unknown" || chd.status !== "pending" || extractRunning;
    btn.addEventListener("click", () => runExtraction([chd]));
    main.appendChild(btn);
    row.appendChild(main);

    if (chd.status === "pending" && chd.progressPercent !== undefined) {
      const track = document.createElement("div");
      track.className = "disc-progress-track";
      const fill = document.createElement("div");
      fill.className = "disc-progress-fill";
      fill.style.width = `${Math.round(chd.progressPercent)}%`;
      track.appendChild(fill);
      row.appendChild(track);
    }

    extractTable.appendChild(row);
  }
}

// Drives both the per-row "Extraer" button (a single-item list) and
// "Extraer todos" (every pending, non-"unknown" item) through the same
// streaming backend command, so a single-file extraction gets a live
// progress bar too instead of the UI going silent for however long chdman
// takes.
async function runExtraction(items) {
  if (items.length === 0 || extractRunning) return;
  extractRunning = true;
  currentRunChds = items;
  extractAllBtn.disabled = true;
  extractProgressTrack.style.display = "block";
  extractProgressFill.style.width = "0%";
  renderExtractTable();
  try {
    await invoke("start_extract_all", {
      items: items.map((c) => ({ chd_path: fullChdPath(c), kind: c.kind })),
    });
  } catch (err) {
    // A real failure (bad chdman path, or another run already in flight)
    // must not leave the UI stuck in "extracting" state forever.
    extractRunning = false;
    currentRunChds = [];
    extractProgressTrack.style.display = "none";
    alert(t("extractFailed", translateError(err)));
    renderExtractTable();
    updateExtractAllAvailability();
  }
}

extractAllBtn.addEventListener("click", () => {
  const items = chds.filter((c) => c.kind !== "unknown" && c.status !== "ok");
  runExtraction(items);
});

function matchChdByPath(chdPath) {
  return chds.find((c) => fullChdPath(c) === chdPath);
}

listen("extract-item-progress", (event) => {
  const { chd_path, phase, percent } = event.payload;
  const chd = matchChdByPath(chd_path);
  if (chd && chd.status === "pending") {
    chd.progressPhase = phase;
    chd.progressPercent = percent;
    renderExtractTable();
    updateExtractProgress();
  }
});

listen("extract-item-done", (event) => {
  const { chd_path, output, error } = event.payload;
  const chd = matchChdByPath(chd_path);
  if (chd) {
    chd.progressPercent = undefined;
    chd.progressPhase = undefined;
    if (error) {
      chd.status = "fail";
      chd.error = translateError(error);
    } else {
      chd.status = "ok";
      chd.result = output;
    }
    renderExtractTable();
    updateExtractProgress();
  }
});

listen("extract-all-finished", (event) => {
  const { done = 0, failed = 0 } = event?.payload ?? {};
  notifyIfAway(t("notifyExtractDone"), t("notifySummary", done, failed));
  extractRunning = false;
  currentRunChds = [];
  extractProgressTrack.style.display = "none";
  updateExtractAllAvailability();
});

// Makes `folder` the working folder and scans it. Shared by the picker,
// restoring the last folder at startup, and dropping a folder on the window.
async function selectFolder(folder) {
  currentFolder = folder;
  folderLabel.textContent = folder;
  updateOrganizeAvailability();
  updateMoveChdAvailability();
  updateFlattenAvailability();
  rescanBtn.disabled = false;
  convertOpenFolderBtn.disabled = false;
  // Remembered so the next launch opens straight into it.
  saveConfig();

  await rescan(folder);
  await rescanChds();
}

// Dropping a folder (or any file inside one) on the window opens it, the
// same as picking it. Ignored while converting or extracting, so a stray
// drop can't switch folders out from under a running batch.
function isBusy() {
  return cancelBtn.style.display !== "none" || extractRunning;
}

window.__TAURI__.webview.getCurrentWebview().onDragDropEvent(async (event) => {
  const { type, paths } = event.payload;
  if (type === "enter" || type === "over") {
    document.body.classList.toggle("drag-over", !isBusy());
    return;
  }
  document.body.classList.remove("drag-over");
  if (type !== "drop" || isBusy() || !paths?.length) return;
  const folder = await invoke("folder_for_drop", { path: paths[0] });
  if (folder) await selectFolder(folder);
});

pickFolderBtn.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;
  await selectFolder(selected);
});

rescanBtn.addEventListener("click", async () => {
  if (!currentFolder) return;
  await rescan(currentFolder);
});

convertOpenFolderBtn.addEventListener("click", async () => {
  if (!currentFolder) return;
  try {
    await openPath(currentFolder);
  } catch (err) {
    alert(translateError(err));
  }
});

function updateSdCardIdVisibility() {
  sdCardIdInput.style.display = storageModeExternalRadio.checked ? "block" : "none";
}

for (const radio of document.querySelectorAll('input[name="storage-mode"]')) {
  radio.addEventListener("change", updateSdCardIdVisibility);
}

runOrganizeBtn.addEventListener("click", async () => {
  if (!currentFolder || !organizeDestination) return;

  const mode = document.querySelector('input[name="storage-mode"]:checked')?.value ?? "pc";
  let androidBase = null;
  if (mode === "internal") {
    androidBase = "/storage/emulated/0/ROMs";
  } else if (mode === "external" && sdCardIdInput.value.trim()) {
    androidBase = `/storage/${sdCardIdInput.value.trim()}/ROMs`;
  }

  try {
    invalidateChdScan();
    const summary = await invoke("organize_multidisc", {
      root: currentFolder,
      destination: organizeDestination,
      androidBase,
    });
    alert(t("organizeSummary", summary));
    organizeOpenDestBtn.style.display = summary.games_organized > 0 ? "inline-block" : "none";
  } catch (err) {
    alert(t("organizeFailed", translateError(err)));
  }
});

organizeOpenDestBtn.addEventListener("click", async () => {
  if (!organizeDestination) return;
  try {
    await openPath(organizeDestination);
  } catch (err) {
    alert(translateError(err));
  }
});

convertBtn.addEventListener("click", async () => {
  // Switch to "in progress" UI synchronously, BEFORE the await: hiding the
  // button only after start_conversion resolves leaves a window in which a
  // fast double-click fires two IPC calls (which the backend then rejects
  // with CONVERSION_IN_PROGRESS), and also leaves the click with no visible
  // feedback until the backend confirms — this gives instant feedback and
  // real per-file progress fills the bar in as chdman reports it.
  convertBtn.disabled = true;
  // The backend reads this once when the run starts; locking it makes clear
  // a mid-run change wouldn't apply until the next run.
  trashOriginalsCheckbox.disabled = true;
  savingsLabel.style.display = "none";
  convertBtn.style.display = "none";
  cancelBtn.style.display = "inline-block";
  rescanBtn.disabled = true;
  progressTrack.style.display = "block";
  progressFill.style.width = "0%";
  try {
    await invoke("start_conversion", {
      root: currentFolder,
      formatOverrides: Array.from(formatOverrides),
      // Passed explicitly (not read from the saved config) so a click right
      // after toggling can't race the checkbox's own save.
      trashOriginals: trashOriginalsCheckbox.checked,
    });
  } catch (err) {
    // A real failure (bad chdman path, spawn error) must not leave the UI
    // stuck in "converting" state forever — revert so the user can fix the
    // setting and retry.
    convertBtn.disabled = false;
    trashOriginalsCheckbox.disabled = false;
    convertBtn.style.display = "inline-block";
    cancelBtn.style.display = "none";
    rescanBtn.disabled = false;
    progressTrack.style.display = "none";
    alert(t("conversionStartError", translateError(err)));
  }
});

cancelBtn.addEventListener("click", async () => {
  await invoke("cancel_conversion");
});

// Windows paths use backslashes; the backend reports the source path exactly
// as it handed it to chdman (an absolute path like "C:\Games\GameA\Track.cue"),
// while a ScannedDisc only carries { name, folder, kind } from the pre-scan.
// Match by comparing the disc's folder+name against the tail of the reported
// path with backslashes normalized to forward slashes -- and, on Windows
// only, case-insensitively, so drive-letter casing differences between the
// scan and the event don't break the match. Linux/macOS compare exactly,
// since there "Game.cue" and "game.cue" can be two different files. This
// assumes folder+name is unique per run; two identically-named discs in
// different folders are still disambiguated correctly since the folder is
// part of the comparison, but a disc that appears twice under the *same* folder+name (not possible
// from a single filesystem scan) would be ambiguous.
function matchDiscByPath(path) {
  const fold = (s) => (platform === "windows" ? s.toLowerCase() : s);
  const normalizedPath = fold(path).replace(/\\/g, "/");
  return discs.find((d) => {
    const discFull = fold(`${d.folder}/${d.name}`).replace(/\\/g, "/").replace(/\/+/g, "/");
    return normalizedPath === discFull || normalizedPath.endsWith(`/${discFull}`.replace(/\/+/g, "/")) || normalizedPath.endsWith(discFull);
  });
}

listen("disc-progress", (event) => {
  const { path, phase, percent } = event.payload;
  const disc = matchDiscByPath(path);
  if (disc && disc.status === "pending") {
    disc.progressPhase = phase;
    disc.progressPercent = percent;
    renderTable();
    updateProgress();
  }
});

listen("disc-updated", (event) => {
  const { status, path, message, note, original_bytes, chd_bytes } = event.payload;
  const disc = matchDiscByPath(path);
  if (disc) {
    disc.status = status.toLowerCase(); // Rust enum serializes as "Ok" | "Skip" | "Fail"
    const parts = [message];
    if (original_bytes != null && chd_bytes != null) {
      parts.push(
        t("sizeChange", formatBytes(original_bytes), formatBytes(chd_bytes), formatPercentChange(original_bytes, chd_bytes)),
      );
    }
    if (note) parts.push(t(`note${note}`));
    disc.message = parts.join(" · ");
    renderTable();
    updateProgress();
    updatePendingLabel();
  }
});

listen("run-finished", (event) => {
  invalidateChdScan();
  showRunSavings(event?.payload);
  const record = event?.payload;
  // A cancelled run was ended by the user, who's clearly at the app.
  if (record && !record.cancelled) {
    const summary = t("notifySummary", record.converted, record.failed);
    const saved = savedText(record);
    notifyIfAway(t("notifyConvertDone"), summary + saved);
  }
  // A cancelled run leaves discs that were never reached stuck on the pending
  // "•" forever, which reads as "still working". Mark them as cancelled.
  if (event?.payload?.cancelled) {
    for (const disc of discs) {
      if (disc.status === "pending") disc.status = "cancel";
    }
    renderTable();
    updateProgress();
    updatePendingLabel();
  }
  convertBtn.style.display = "inline-block";
  // Re-enable: the click handler disabled it synchronously at run start.
  convertBtn.disabled = discs.length === 0;
  trashOriginalsCheckbox.disabled = false;
  cancelBtn.style.display = "none";
  rescanBtn.disabled = false;
  // The fill has a permanent shimmer animation while it's visible (it reads
  // as "still working" otherwise) — hide the whole track once the run is
  // done instead of just leaving it parked near 100%, or it visibly keeps
  // flickering after conversion has actually finished.
  progressTrack.style.display = "none";
});

// Persists every Settings field at once (set_config replaces the whole
// config), so each control that saves itself can't drop another's value.
async function saveConfig() {
  await invoke("set_config", {
    config: {
      chdman_path: chdmanPathInput.value,
      auto_update_enabled: autoUpdateCheckbox.checked,
      language: languageSelect.value,
      parallel_conversion: parallelConversionCheckbox.checked,
      trash_originals: trashOriginalsCheckbox.checked,
      last_folder: currentFolder ?? "",
    },
  });
}

saveSettingsBtn.addEventListener("click", saveConfig);

// Saved on its own, independent of the Guardar button: a checkbox toggle
// that silently required a separate "Guardar" click to take effect was
// confusing (it looked applied immediately since the checkbox visually
// stayed checked, but the persisted config still held the old value).
autoUpdateCheckbox.addEventListener("change", saveConfig);

// Same immediate-save treatment; read by the backend at the start of each
// conversion run, so it applies from the next "Convertir todo" on.
parallelConversionCheckbox.addEventListener("change", saveConfig);

// Lives on the Convertir view (next to the action it affects) rather than
// in Settings, but persists the same way.
trashOriginalsCheckbox.addEventListener("change", saveConfig);

// Also saved immediately, and re-renders every static label right away —
// leaving stale text until the next Guardar click would be as confusing as
// the auto-update checkbox was before it got the same treatment.
languageSelect.addEventListener("change", async () => {
  setLanguage(languageSelect.value);
  applyTranslations();
  await saveConfig();
});

// A desktop notification for a long batch finishing while the user is in
// another app. Skipped when the window has focus: they can already see the
// result, and a notification on top of it would just be noise. Permission
// is asked the first time one is actually needed (macOS requires it).
async function notifyIfAway(title, body) {
  if (document.hasFocus()) return;
  const notification = window.__TAURI__.notification;
  if (!notification) return;
  try {
    let granted = await notification.isPermissionGranted();
    if (!granted) granted = (await notification.requestPermission()) === "granted";
    if (granted) notification.sendNotification({ title, body });
  } catch {
    // Notifications are a nicety; never let one break the UI.
  }
}

// " · ahorró X" for a run record, or "" when it saved nothing.
function savedText(record) {
  const saved = (record.original_bytes ?? 0) - (record.chd_bytes ?? 0);
  return record.original_bytes > 0 && saved > 0 ? ` · ${t("historySaved", formatBytes(saved))}` : "";
}

// Total space freed by the run that just finished (over the discs it
// converted). Hidden when nothing was converted.
function showRunSavings(record) {
  const before = record?.original_bytes ?? 0;
  const after = record?.chd_bytes ?? 0;
  // Nothing converted, or (only with data that doesn't compress) the .chd
  // files came out no smaller -- there's no saving to announce.
  if (before <= 0 || after >= before) {
    savingsLabel.style.display = "none";
    return;
  }
  savingsLabel.textContent = t(
    "runSavings",
    formatBytes(before - after),
    formatPercentChange(before, after).replace(/^[-+−]/, ""),
  );
  savingsLabel.style.display = "block";
}

async function renderHistory() {
  const history = await invoke("get_history");
  historyPanel.innerHTML = "";
  for (const run of history.slice().reverse()) {
    const date = new Date(Number(run.timestamp) * 1000).toLocaleString();
    const status = run.cancelled
      ? t("cancelled")
      : t("historySummary", { converted: run.converted, skipped: run.skipped, failed: run.failed }) +
        savedText(run);
    const row = document.createElement("div");
    row.className = "history-row";
    row.append(mk("history-folder", run.folder), mk("", date), mk("", status));
    historyPanel.appendChild(row);
  }
}

function showUpdateResult(text) {
  updateStatus.textContent = text;
}

// Set while the update overlay is driving an actual install, so the
// download-progress listener (which fires for the whole app's lifetime)
// knows whether to touch the overlay at all, and what version's text to show.
let overlayInstallVersion = null;

function showOverlayAction(text, onClick) {
  updateOverlayActionBtn.style.display = "inline-block";
  updateOverlayActionBtn.textContent = text;
  updateOverlayActionBtn.onclick = onClick;
}

// Real download-progress events from the backend (chunk counts as
// download_and_install streams the update) drive the bar here instead of it
// being an indeterminate spinner for however many seconds the download takes.
listen("update-download-progress", (event) => {
  if (!overlayInstallVersion) return;
  const { downloaded, total } = event.payload;
  if (!total) return;
  const pct = Math.min(100, Math.round((downloaded / total) * 100));
  updateOverlayProgressTrack.style.display = "block";
  updateOverlayProgressFill.style.width = `${pct}%`;
  updateOverlayText.textContent = t("updateDownloading", overlayInstallVersion, pct);
});

// Covers the whole window with a small "installing" card (logo, spinner,
// progress bar) for the entire download+install, instead of the previous
// silent-then-alert flow — the underlying app is never visible or usable
// while an install is in flight, since a successful one ends in an
// immediate restart.
async function installWithOverlay(update) {
  overlayInstallVersion = update.version;
  updateOverlay.style.display = "flex";
  updateOverlayActionBtn.style.display = "none";
  updateOverlayProgressTrack.style.display = "none";
  updateOverlayProgressFill.style.width = "0%";
  updateOverlayText.textContent = t("updateNoticeAuto", update.version);

  if (update.notes && update.notes.trim()) {
    updateOverlayNotesLabel.textContent = t("updateReleaseNotesLabel");
    updateOverlayNotesBody.textContent = update.notes;
    updateOverlayNotes.style.display = "block";
  } else {
    updateOverlayNotes.style.display = "none";
  }

  try {
    await invoke("install_update");
    updateOverlayProgressTrack.style.display = "none";
    updateOverlayText.textContent = t("updateDoneOverlay", update.version);
    showOverlayAction(t("updateRestartNow"), async () => {
      await invoke("restart_app");
    });
  } catch (err) {
    updateOverlayProgressTrack.style.display = "none";
    updateOverlayText.textContent = t("updateErrorOverlay", translateError(err));
    showOverlayAction(t("updateDismiss"), () => {
      overlayInstallVersion = null;
      updateOverlay.style.display = "none";
    });
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
    await installWithOverlay(update);
    return;
  }

  const install = confirm(t("confirmInstall", update.version));
  if (install) {
    await installWithOverlay(update);
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
    showUpdateResult(translateError(err));
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
  platform = await invoke("get_platform");
  setPlatform(platform);
  if (platform !== "windows") chdmanPathInput.placeholder = "/usr/bin/chdman";
  appVersion = await window.__TAURI__.app.getVersion();
  applyTranslations();
  languageSelect.value = config.language;
  autoUpdateCheckbox.checked = config.auto_update_enabled;
  parallelConversionCheckbox.checked = config.parallel_conversion;
  trashOriginalsCheckbox.checked = config.trash_originals;
  chdmanPathInput.value = config.chdman_path;
  updateOrganizeAvailability();
  updateMoveChdAvailability();
  // Reopen the folder from last time, unless it's gone (unplugged drive,
  // renamed, deleted) -- then just start empty, as on a first launch.
  if (config.last_folder && (await invoke("folder_exists", { path: config.last_folder }))) {
    await selectFolder(config.last_folder);
  }
})();
