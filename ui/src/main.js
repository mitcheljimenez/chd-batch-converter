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
    const icon = { pending: "•", ok: "✅", skip: "⏭️", fail: "❌" }[disc.status];
    row.innerHTML = "";
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
  try {
    await invoke("start_conversion", { root: currentFolder });
    convertBtn.style.display = "none";
    cancelBtn.style.display = "inline-block";
  } catch (err) {
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

listen("run-finished", () => {
  convertBtn.style.display = "inline-block";
  cancelBtn.style.display = "none";
});
