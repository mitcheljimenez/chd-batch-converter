const translations = {
  es: {
    navConvert: "Convertir",
    pickFolder: "Elegir carpeta",
    noFolderSelected: "Ninguna carpeta seleccionada",
    convertAll: "Convertir todo",
    cancel: "Cancelar",
    settings: "Configuración",
    history: "Historial",
    chdmanPathLabel: "Ruta de chdman.exe",
    autoUpdateLabel: "Instalar actualizaciones automáticamente",
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
      "Usá esto si vas a copiar las carpetas organizadas a la tarjeta SD de tu celular o handheld Android. Necesitás el ID de esa tarjeta: en el dispositivo, abrí una app de administrador de archivos, entrá al almacenamiento externo/SD, y fijate la ruta que muestra — algo como /storage/1234-5678/. Esa parte \"1234-5678\" es el ID que tenés que escribir acá.",
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
    moveChdNoFolderHint: "Elegí una carpeta de origen y una de destino primero.",
    runMoveChd: "Mover",
    moveChdSummary: ({ files_moved, renamed_due_to_collision }) =>
      `Listo: ${files_moved} archivo(s) .chd movido(s)` +
      (renamed_due_to_collision > 0
        ? `. ${renamed_due_to_collision} se renombraron para no sobrescribir un archivo existente con el mismo nombre.`
        : "."),
    moveChdFailed: (err) => `No se pudo mover: ${err}`,
    updateNoticeAuto: (version) =>
      `Hay una actualización disponible (v${version}). Se va a instalar automáticamente.`,
    updateDownloading: (version, pct) => `Descargando actualización v${version}... ${pct}%`,
    updateInstallingOverlay: "Instalando...",
    updateDoneOverlay: (version) => `¡Listo! Se actualizó a la versión ${version}.`,
    updateRestartNow: "Reiniciar ahora",
    updateErrorOverlay: (err) => `No se pudo actualizar: ${err}`,
    updateDismiss: "Cerrar",
    errors: {
      CHDMAN_NOT_CONFIGURED: "No se configuró la ruta de chdman.exe",
      CHDMAN_NOT_FOUND: (path) =>
        `chdman.exe no encontrado en la ruta configurada: ${path}`,
      SCRIPT_NOT_FOUND: "No se encontró el script de conversión incluido",
      CONVERSION_IN_PROGRESS: "Ya hay una conversión en curso",
      UPDATE_DEFERRED_CONVERSION_IN_PROGRESS:
        "Hay una conversión en curso; se reintentará luego",
      NO_UPDATE_AVAILABLE: "No hay actualización disponible",
    },
  },
  en: {
    navConvert: "Convert",
    pickFolder: "Choose folder",
    noFolderSelected: "No folder selected",
    convertAll: "Convert all",
    cancel: "Cancel",
    settings: "Settings",
    history: "History",
    chdmanPathLabel: "Path to chdman.exe",
    autoUpdateLabel: "Install updates automatically",
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
    updateNoticeAuto: (version) =>
      `An update is available (v${version}). It will be installed automatically.`,
    updateDownloading: (version, pct) => `Downloading update v${version}... ${pct}%`,
    updateInstallingOverlay: "Installing...",
    updateDoneOverlay: (version) => `Done! Updated to version ${version}.`,
    updateRestartNow: "Restart now",
    updateErrorOverlay: (err) => `Could not update: ${err}`,
    updateDismiss: "Close",
    errors: {
      CHDMAN_NOT_CONFIGURED: "chdman.exe path is not configured",
      CHDMAN_NOT_FOUND: (path) =>
        `chdman.exe not found at the configured path: ${path}`,
      SCRIPT_NOT_FOUND: "The bundled conversion script was not found",
      CONVERSION_IN_PROGRESS: "A conversion is already in progress",
      UPDATE_DEFERRED_CONVERSION_IN_PROGRESS:
        "A conversion is in progress; this will be retried later",
      NO_UPDATE_AVAILABLE: "No update available",
    },
  },
};

let currentLanguage = "es";

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
