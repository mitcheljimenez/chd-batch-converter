const translations = {
  es: {
    navConvert: "Convertir",
    pickFolder: "Elegir carpeta",
    noFolderSelected: "Ninguna carpeta seleccionada",
    convertAll: "Convertir todo",
    rescan: "Rescanear",
    openFolder: "Abrir carpeta",
    cancel: "Cancelar",
    settings: "Configuración",
    history: "Historial",
    chdmanPathLabel: "Ruta de chdman",
    chdmanInstallHint: (command) =>
      `Déjalo vacío para usar el chdman instalado en el sistema. Si no lo tienes: ${command}`,
    autoUpdateLabel: "Instalar actualizaciones automáticamente",
    parallelConversionLabel: "Convertir varios discos a la vez",
    trashOriginalsLabel: "Mover los archivos originales a la papelera tras convertir y verificar",
    sizeChange: (before, after, change) => `${before} → ${after} (${change})`,
    runSavings: (saved, percent) => `Espacio ahorrado: ${saved} (${percent} menos)`,
    historySaved: (saved) => `ahorró ${saved}`,
    noteORIGINALS_TRASHED: "originales enviados a la papelera",
    noteORIGINALS_TRASH_FAILED: "no se pudieron mover los originales a la papelera (siguen en su sitio)",
    parallelConversionHint:
      "Activado: un disco por núcleo del procesador, más rápido con muchos juegos. Desactivado: un disco a la vez usando todos los núcleos; deja el PC más libre mientras convierte y avanza en orden.",
    languageLabel: "Idioma",
    save: "Guardar",
    checkUpdates: "Buscar actualizaciones",
    cancelled: "Cancelado",
    historySummary: ({ converted, skipped, failed }) =>
      `${converted} convertidos, ${skipped} saltados, ${failed} fallidos`,
    conversionStartError: (err) => `No se pudo iniciar la conversión: ${err}`,
    discsPendingLabel: (n) =>
      n === 0
        ? "No hay archivos pendientes de convertir (todos ya tienen su .chd)"
        : `${n} archivo(s) por convertir`,
    phaseCompressing: (pct) => `Comprimiendo ${pct}%`,
    phaseVerifying: (pct) => `Verificando ${pct}%`,
    upToDate: "Ya tienes la última versión",
    confirmInstall: (version) =>
      `Hay una actualización disponible (v${version}). ¿Instalar ahora?`,
    updateAvailableDeferred: (version) =>
      `Actualización v${version} disponible (Buscar actualizaciones para instalar)`,
    checking: "Buscando...",
    checkFailed: "No se pudo comprobar (sin conexión)",
    organizeMultidisc: "Organizar multi-disco",
    organizeExplanation:
      "Esto va a mover los archivos de juegos multi-disco (Disc 1, Disc 2, etc.) a carpetas separadas y crear un .m3u para cada uno.",
    storageModePc: "Solo PC (rutas relativas)",
    storageModeInternal: "Almacenamiento interno de Android",
    storageModeExternal: "Tarjeta SD externa",
    storageExternalTooltip:
      "Usa esto si vas a copiar las carpetas organizadas a la tarjeta SD de tu celular o handheld Android. Necesitas el ID de esa tarjeta: en el dispositivo, abre una app de administrador de archivos, entra al almacenamiento externo/SD, y fíjate en la ruta que muestra — algo como /storage/1234-5678/. Esa parte \"1234-5678\" es el ID que tienes que escribir aquí.",
    sdCardIdPlaceholder: "ID de la tarjeta SD, ej. 1234-5678",
    runOrganize: "Organizar",
    organizeSummary: ({ games_organized, files_moved, skipped_already_organized }) =>
      `Listo: ${games_organized} juego(s) organizado(s), ${files_moved} archivo(s) movido(s)` +
      (skipped_already_organized > 0
        ? `. ${skipped_already_organized} ya estaban organizados y se dejaron como estaban.`
        : "."),
    organizeFailed: (err) => `No se pudo organizar: ${err}`,
    pickDestination: "Elegir carpeta destino",
    noDestinationSelected: "Ninguna carpeta destino seleccionada",
    navMoveChd: "Mover .chd",
    moveChdExplanation:
      "Esto va a buscar todos los archivos .chd, incluso en subcarpetas anidadas, y moverlos a la carpeta destino, separándolos de sus .bin/.cue/.iso originales (que quedan donde estaban).",
    moveChdNoFolderHint: "Elige una carpeta de origen y una de destino primero.",
    runMoveChd: "Mover",
    moveChdSummary: ({ files_moved, renamed_due_to_collision }) =>
      `Listo: ${files_moved} archivo(s) .chd movido(s)` +
      (renamed_due_to_collision > 0
        ? `. ${renamed_due_to_collision} se renombraron para no sobrescribir un archivo existente con el mismo nombre.`
        : "."),
    moveChdFailed: (err) => `No se pudo mover: ${err}`,
    openDestFolder: "Abrir carpeta destino",
    navFlatten: "Aplanar carpetas",
    flattenExplanation:
      "Esto va a sacar los archivos de ROMs que están dentro de carpetas y ponerlos directamente en la carpeta elegida, eliminando las carpetas que queden vacías. Se ignoran las carpetas cuyo nombre contiene \".m3u\" (juegos multi-disco), que quedan intactas.",
    flattenNoFolderHint: "Elige una carpeta primero.",
    runFlatten: "Aplanar",
    flattenSummary: ({ files_moved, folders_removed, renamed_due_to_collision }) =>
      `Listo: ${files_moved} archivo(s) movido(s), ${folders_removed} carpeta(s) vacía(s) eliminada(s)` +
      (renamed_due_to_collision > 0
        ? `. ${renamed_due_to_collision} se renombraron para no sobrescribir un archivo existente con el mismo nombre.`
        : "."),
    flattenFailed: (err) => `No se pudo aplanar: ${err}`,
    navExtract: "Extraer .chd",
    extractExplanation:
      "Esto busca archivos .chd en la carpeta elegida y permite extraerlos de vuelta a .iso, .bin/.cue o .gdi (Dreamcast), detectando el formato automáticamente y verificando el .chd original tras cada extracción. El .chd original no se toca ni se borra.",
    extractNoFolderHint: "Elige una carpeta primero.",
    extractNoFilesFound: "No se encontraron archivos .chd en esta carpeta.",
    extractAllBtn: "Extraer todos",
    extractBtn: "Extraer",
    extractKindCd: "CD",
    extractKindDvd: "DVD",
    extractKindGd: "GD-ROM (Dreamcast)",
    extractKindUnknown: "Desconocido",
    extractDone: (path) => `Extraído: ${path}`,
    extractFailed: (err) => `No se pudo extraer: ${err}`,
    phaseExtracting: (pct) => `Extrayendo ${pct}%`,
    phaseExtractVerifying: (pct) => `Verificando ${pct}%`,
    overrideFormatDvd: "DVD (zlib)",
    overrideFormatCd: "CD",
    updateNoticeAuto: (version) =>
      `Hay una actualización disponible (v${version}). Se va a instalar automáticamente.`,
    updateDownloading: (version, pct) => `Descargando actualización v${version}... ${pct}%`,
    updateInstallingOverlay: "Instalando...",
    updateReleaseNotesLabel: "Novedades de esta versión:",
    updateDoneOverlay: (version) => `¡Listo! Se actualizó a la versión ${version}.`,
    updateRestartNow: "Reiniciar ahora",
    updateErrorOverlay: (err) => `No se pudo actualizar: ${err}`,
    updateDismiss: "Cerrar",
    versionLabel: (version) => `Versión instalada: ${version}`,
    errors: {
      CHDMAN_NOT_CONFIGURED: () =>
        chdmanInstallCommand()
          ? `No se encontró chdman en el sistema. Instálalo con: ${chdmanInstallCommand()}. También puedes indicar su ruta en Configuración.`
          : "No se configuró la ruta de chdman.exe",
      CHDMAN_NOT_FOUND: (path) =>
        `chdman no encontrado en la ruta configurada: ${path}`,
      SCRIPT_NOT_FOUND: "No se encontró el script de conversión incluido",
      CONVERSION_IN_PROGRESS: "Ya hay una conversión en curso",
      UPDATE_DEFERRED_CONVERSION_IN_PROGRESS:
        "Hay una conversión en curso; se reintentará luego",
      NO_UPDATE_AVAILABLE: "No hay actualización disponible",
      EXTRACT_UNKNOWN_FORMAT: "No se pudo determinar si este .chd es de CD o DVD",
      EXTRACT_DEST_EXISTS: (path) => `Ya existe el archivo de destino: ${path}`,
      EXTRACT_FAILED: "chdman no pudo extraer este archivo",
      EXTRACT_VERIFY_FAILED: "Se extrajo el archivo, pero chdman no pudo verificar el .chd original",
      EXTRACT_IN_PROGRESS: "Ya hay una extracción en curso",
    },
  },
  en: {
    navConvert: "Convert",
    pickFolder: "Choose folder",
    noFolderSelected: "No folder selected",
    convertAll: "Convert all",
    rescan: "Rescan",
    openFolder: "Open folder",
    cancel: "Cancel",
    settings: "Settings",
    history: "History",
    chdmanPathLabel: "Path to chdman",
    chdmanInstallHint: (command) =>
      `Leave empty to use the chdman installed on your system. If you don't have it: ${command}`,
    autoUpdateLabel: "Install updates automatically",
    parallelConversionLabel: "Convert several discs at once",
    trashOriginalsLabel: "Move the original files to the trash after converting and verifying",
    sizeChange: (before, after, change) => `${before} → ${after} (${change})`,
    runSavings: (saved, percent) => `Space saved: ${saved} (${percent} smaller)`,
    historySaved: (saved) => `saved ${saved}`,
    noteORIGINALS_TRASHED: "originals moved to the trash",
    noteORIGINALS_TRASH_FAILED: "couldn't move the originals to the trash (they're still in place)",
    parallelConversionHint:
      "On: one disc per CPU core, faster for large libraries. Off: one disc at a time using every core; keeps the PC more responsive while converting and goes in order.",
    languageLabel: "Language",
    save: "Save",
    checkUpdates: "Check for updates",
    cancelled: "Cancelled",
    historySummary: ({ converted, skipped, failed }) =>
      `${converted} converted, ${skipped} skipped, ${failed} failed`,
    conversionStartError: (err) => `Could not start the conversion: ${err}`,
    discsPendingLabel: (n) =>
      n === 0
        ? "No files pending conversion (all already have a .chd)"
        : `${n} file(s) to convert`,
    phaseCompressing: (pct) => `Compressing ${pct}%`,
    phaseVerifying: (pct) => `Verifying ${pct}%`,
    upToDate: "You already have the latest version",
    confirmInstall: (version) =>
      `An update is available (v${version}). Install now?`,
    updateAvailableDeferred: (version) =>
      `Update v${version} available (use "Check for updates" to install)`,
    checking: "Checking...",
    checkFailed: "Could not check (no connection)",
    organizeMultidisc: "Organize multi-disc games",
    organizeExplanation:
      "This will move multi-disc game files (Disc 1, Disc 2, etc.) into separate folders and create a .m3u playlist for each one.",
    storageModePc: "PC only (relative paths)",
    storageModeInternal: "Android internal storage",
    storageModeExternal: "External SD card",
    storageExternalTooltip:
      "Use this if you're going to copy the organized folders onto your phone or handheld's SD card. You need that card's ID: on the device, open a file manager app, go into external/SD storage, and look at the path it shows — something like /storage/1234-5678/. That \"1234-5678\" part is the ID to type here.",
    sdCardIdPlaceholder: "SD card ID, e.g. 1234-5678",
    runOrganize: "Organize",
    organizeSummary: ({ games_organized, files_moved, skipped_already_organized }) =>
      `Done: ${games_organized} game(s) organized, ${files_moved} file(s) moved` +
      (skipped_already_organized > 0
        ? `. ${skipped_already_organized} were already organized and left as-is.`
        : "."),
    organizeFailed: (err) => `Could not organize: ${err}`,
    pickDestination: "Choose destination folder",
    noDestinationSelected: "No destination folder selected",
    navMoveChd: "Move .chd files",
    moveChdExplanation:
      "This will find every .chd file, even in nested subfolders, and move it into the destination folder, separating it from its original .bin/.cue/.iso (which stay where they were).",
    moveChdNoFolderHint: "Choose a source folder and a destination folder first.",
    runMoveChd: "Move",
    moveChdSummary: ({ files_moved, renamed_due_to_collision }) =>
      `Done: ${files_moved} .chd file(s) moved` +
      (renamed_due_to_collision > 0
        ? `. ${renamed_due_to_collision} were renamed to avoid overwriting an existing file with the same name.`
        : "."),
    moveChdFailed: (err) => `Could not move: ${err}`,
    openDestFolder: "Open destination folder",
    navFlatten: "Flatten folders",
    flattenExplanation:
      "This will take ROM files out of whatever folders they're nested in and put them directly in the chosen folder, then remove any folders that end up empty. Folders whose name contains \".m3u\" (multi-disc games) are skipped and left untouched.",
    flattenNoFolderHint: "Choose a folder first.",
    runFlatten: "Flatten",
    flattenSummary: ({ files_moved, folders_removed, renamed_due_to_collision }) =>
      `Done: ${files_moved} file(s) moved, ${folders_removed} empty folder(s) removed` +
      (renamed_due_to_collision > 0
        ? `. ${renamed_due_to_collision} were renamed to avoid overwriting an existing file with the same name.`
        : "."),
    flattenFailed: (err) => `Could not flatten: ${err}`,
    navExtract: "Extract .chd",
    extractExplanation:
      "This looks for .chd files in the chosen folder and lets you extract them back to .iso, .bin/.cue or .gdi (Dreamcast), auto-detecting the format and verifying the original .chd after each extraction. The original .chd is never touched or deleted.",
    extractNoFolderHint: "Choose a folder first.",
    extractNoFilesFound: "No .chd files found in this folder.",
    extractAllBtn: "Extract all",
    extractBtn: "Extract",
    extractKindCd: "CD",
    extractKindDvd: "DVD",
    extractKindGd: "GD-ROM (Dreamcast)",
    extractKindUnknown: "Unknown",
    extractDone: (path) => `Extracted: ${path}`,
    extractFailed: (err) => `Could not extract: ${err}`,
    phaseExtracting: (pct) => `Extracting ${pct}%`,
    phaseExtractVerifying: (pct) => `Verifying ${pct}%`,
    overrideFormatDvd: "DVD (zlib)",
    overrideFormatCd: "CD",
    updateNoticeAuto: (version) =>
      `An update is available (v${version}). It will be installed automatically.`,
    updateDownloading: (version, pct) => `Downloading update v${version}... ${pct}%`,
    updateInstallingOverlay: "Installing...",
    updateReleaseNotesLabel: "What's new in this version:",
    updateDoneOverlay: (version) => `Done! Updated to version ${version}.`,
    updateRestartNow: "Restart now",
    updateErrorOverlay: (err) => `Could not update: ${err}`,
    updateDismiss: "Close",
    versionLabel: (version) => `Installed version: ${version}`,
    errors: {
      CHDMAN_NOT_CONFIGURED: () =>
        chdmanInstallCommand()
          ? `chdman was not found on this system. Install it with: ${chdmanInstallCommand()}. You can also set its path in Settings.`
          : "chdman.exe path is not configured",
      CHDMAN_NOT_FOUND: (path) =>
        `chdman not found at the configured path: ${path}`,
      SCRIPT_NOT_FOUND: "The bundled conversion script was not found",
      CONVERSION_IN_PROGRESS: "A conversion is already in progress",
      UPDATE_DEFERRED_CONVERSION_IN_PROGRESS:
        "A conversion is in progress; this will be retried later",
      NO_UPDATE_AVAILABLE: "No update available",
      EXTRACT_UNKNOWN_FORMAT: "Could not determine whether this .chd is CD or DVD format",
      EXTRACT_DEST_EXISTS: (path) => `Destination file already exists: ${path}`,
      EXTRACT_FAILED: "chdman failed to extract this file",
      EXTRACT_VERIFY_FAILED: "The file was extracted, but chdman could not verify the original .chd",
      EXTRACT_IN_PROGRESS: "An extraction is already in progress",
    },
  },
};

let currentLanguage = "es";
// "windows" | "linux" | "macos" (from the backend's get_platform). Windows
// bundles chdman.exe with the installer; elsewhere it comes from the
// system's package manager, so errors/settings show how to install it.
let currentPlatform = "windows";

export function setPlatform(platform) {
  currentPlatform = platform;
}

export function chdmanInstallCommand() {
  switch (currentPlatform) {
    case "macos":
      return "brew install rom-tools";
    case "linux":
      return "sudo apt install mame-tools (Debian/Ubuntu) · sudo pacman -S mame-tools (Arch) · sudo dnf install mame-tools (Fedora)";
    default:
      return null;
  }
}

// Human-readable size in the current language's number format, using
// 1024-based units like Windows Explorer and most file managers.
export function formatBytes(bytes) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = Math.abs(bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const number = new Intl.NumberFormat(currentLanguage === "es" ? "es-ES" : "en-US", {
    maximumFractionDigits: unit >= 3 ? 2 : 1,
  }).format(value);
  return `${bytes < 0 ? "-" : ""}${number} ${units[unit]}`;
}

// Percent change from `before` to `after`, signed ("-36 %" when smaller).
export function formatPercentChange(before, after) {
  if (before <= 0) return "";
  const change = ((after - before) / before) * 100;
  const number = new Intl.NumberFormat(currentLanguage === "es" ? "es-ES" : "en-US", {
    maximumFractionDigits: 0,
    signDisplay: "exceptZero",
  }).format(change);
  return `${number} %`;
}

export function setLanguage(language) {
  currentLanguage = translations[language] ? language : "es";
}

export function t(key, ...args) {
  const value = translations[currentLanguage][key];
  return typeof value === "function" ? value(...args) : value;
}

// Backend commands fail with a stable code (e.g. "CHDMAN_NOT_FOUND:C:\...")
// instead of a hardcoded-language message, so this is where that code gets
// turned into user-facing text. Errors Tauri/the OS/reqwest raise directly
// (not one of ours) have no code to look up — those pass through untranslated
// rather than being silently dropped.
export function translateError(err) {
  const message = String(err);
  const [code, ...rest] = message.split(":");
  const entry = translations[currentLanguage].errors[code];
  if (!entry) return message;
  return typeof entry === "function" ? entry(rest.join(":")) : entry;
}
