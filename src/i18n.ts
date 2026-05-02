// Lightweight UI i18n. Two-language catalog (fr / en) with simple
// `{var}` interpolation. Backend persists the user's choice as
// "auto" | "fr" | "en"; "auto" resolves to navigator.language at boot.

export type Lang = "fr" | "en";
export type LangPref = Lang | "auto";

type Catalog = Record<string, string>;

const fr: Catalog = {
  // Header
  "header.subtitle": "Powered by the official Rocket League Stats API. Fully EAC-safe.",
  "header.connected": "Connecté au jeu",
  "header.waiting": "En attente du jeu",

  // Session panel
  "session.title": "Session en cours",
  "session.reset": "Reset",
  "session.wins": "Wins",
  "session.losses": "Losses",
  "session.streak": "Streak",
  "session.records": "Records de la session — meilleure série de wins : <strong>{best_win}</strong> · pire série de losses : <strong>{best_loss}</strong>",
  "session.filter.label": "Compter les matchs en :",
  "session.filter.note": "Ne distingue pas Ranked et Casual : l'API officielle de Rocket League n'expose pas le mode de matchmaking. Le filtre ci-dessus se base uniquement sur la <strong>taille des équipes</strong> détectée en début de match.",
  "session.resetConfirm": "Réinitialiser la session ? (les wins/losses repartent à zéro)",
  "session.streakEmpty": "—",

  // Player panel
  "player.title": "Joueur",
  "player.label": "Pseudo en jeu",
  "player.placeholder": "Ton pseudo Rocket League",
  "player.save": "Enregistrer",
  "player.idCaptured": "Identifiant stable capturé : <code>{id}</code>",
  "player.idPending": "L'identifiant stable sera capturé automatiquement au prochain match.",
  "player.savePrompt": "Renseigne un pseudo avant d'enregistrer.",
  "player.detectedWaiting": "En attente du premier match…",
  "player.detectedNote": "Pseudo détecté automatiquement à partir de ton compte Steam/Epic. Aucune saisie requise.",

  // Apparence panel — groups the launcher visibility + HUD lock + auto-hide
  // toggles that used to live in the HUD panel. Keeps "look & feel" knobs
  // in one place so the HUD panel stays focused on geometry / show-hide.
  "appearance.title": "Apparence",
  "launcher.enable": "Afficher le bouton flottant",
  "launcher.enableHint": "Petit rond cliquable sur le bord gauche de l'écran. Ouvre cette fenêtre. Auto-masqué pendant un match.",

  // HUD panel
  "hud.title": "HUD en jeu",
  "hud.note": "Affiche l'overlay en fenêtre transparente par-dessus Rocket League. Fonctionne uniquement en <strong>plein écran fenêtré (borderless)</strong>.",
  "hud.show": "▶ Afficher le HUD",
  "hud.hide": "🟢 HUD activé — masquer",
  "hud.reload": "🔄 Recharger",
  "hud.reloadTitle": "Recharge le HUD pour forcer un fetch frais des assets",
  "hud.reloaded": "✓ Rechargé",
  "hud.step": "Pas",
  "hud.lock": "Verrouiller la position",
  "hud.lockHint": "Verrouillé : le HUD redevient transparent au clic (les inputs passent au jeu) et le drag/clic-droit est désactivé.",
  "hud.autoHide": "Masquer auto quand RL est fermé",
  "hud.matchTitle": "Match en cours",
  "hud.matchHint": "G/S/Sh/A — buts, arrêts, tirs, passes décisives.",
  "hud.matchEmpty": "En attente du prochain match…",

  // OBS panel
  "obs.title": "OBS Browser Source",
  "obs.note": "Colle cette URL dans une <em>Browser Source</em> OBS, dimensions <code>400 × 300</code>.",
  "obs.copy": "📋 Copier l'URL",
  "obs.preview": "👁 Aperçu navigateur",
  "obs.copied": "✓ Copié",

  // Theme panel
  "theme.title": "Theme",
  "theme.openFolder": "📁 Dossier des thèmes",
  "theme.openFolderTitle": "Ouvre le dossier où déposer un thème custom",
  "theme.refresh": "🔄 Rafraîchir",
  "theme.refreshTitle": "Rescanne le dossier après un drag-drop",
  "theme.refreshed": "✓ Rafraîchi",
  "theme.resetAll": "Reset all overrides",
  "theme.resetConfirm": "Réinitialiser tous les réglages de ce thème ?",
  "theme.openFolderError": "Impossible d'ouvrir le dossier des thèmes : {err}",
  "theme.activeLabel": "Active theme",
  "theme.varOverride": "Override active",
  "theme.varReset": "Reset to default",

  // Footer
  "footer.settingsAt": "Settings stockés dans <code>{path}</code>",
  "footer.openLogs": "📂 Ouvrir les logs",
  "footer.openLogsTitle": "Ouvre le dossier des logs (utile pour signaler un bug)",
  "footer.trayHint": "La croix de la fenêtre envoie l'app dans la zone de notification — utilise le bouton ci-dessous (ou clic droit sur l'icône système) pour quitter complètement.",
  "footer.quit": "⏻ Quitter l'application",
  "footer.language": "Langue",
  "footer.langAuto": "Auto",
  "footer.langFr": "Français",
  "footer.langEn": "English",

  // Wizard
  "wizard.welcome": "🎮 Bienvenue dans RL Stats Overlay",
  "wizard.welcomeSub": "Configurons ton installation Rocket League. Ça prend 30 secondes.",
  "wizard.installTitle": "1. Installation détectée",
  "wizard.notDetected": "Aucune installation détectée automatiquement. Tu peux indiquer le dossier manuellement.",
  "wizard.installLabel": "Rocket League — {platform}",
  "wizard.browse": "📂 Indiquer un dossier manuellement",
  "wizard.browseTitle": "Sélectionne le dossier d'installation de Rocket League",
  "wizard.apiTitle": "2. Activation de la Stats API",
  "wizard.apiAlreadyOk": "La Stats API était déjà activée correctement. Aucune modification nécessaire.",
  "wizard.apiApplied": "Configuration appliquée à <code>{path}</code>",
  "wizard.apiBackup": "Sauvegarde de l'ancien fichier dans <code>{path}</code>.",
  "wizard.apiNote1": "La Stats API est une fonctionnalité <strong>officielle Psyonix</strong>, compatible Easy Anti-Cheat. Aucune injection dans le jeu, uniquement la lecture de l'API qu'il expose lui-même.",
  "wizard.apiNote2": "⚠️ <strong>Redémarre Rocket League</strong> si le jeu était lancé pour que le changement prenne effet.",
  "wizard.playerTitle": "3. Ton pseudo en jeu",
  "wizard.playerLabel": "Tape exactement le pseudo affiché en match (sensible aux espaces)",
  "wizard.playerPlaceholder": "ex: Pooley",
  "wizard.finish": "Terminer ▶",
  "wizard.finishPrompt": "Renseigne ton pseudo en jeu avant de continuer.",
  "wizard.patchError": "Impossible de modifier la configuration de la Stats API :\n{err}\n\nVérifie que tu as les droits d'écriture sur le dossier d'installation.",
  "wizard.autoDetectedTitle": "Identifiant détecté automatiquement",
  "wizard.autoDetectedNote": "Ton compte Steam/Epic a été détecté. L'overlay s'associera tout seul à ton joueur dès le premier match.",

  // Updater banner
  "update.banner": "🔔 Nouvelle version <strong>{version}</strong> disponible",
  "update.install": "Installer",
  "update.dismiss": "Plus tard",
  "update.downloading": "Téléchargement…",
  "update.retry": "Réessayer",
};

const en: Catalog = {
  "header.subtitle": "Powered by the official Rocket League Stats API. Fully EAC-safe.",
  "header.connected": "Connected to game",
  "header.waiting": "Waiting for game",

  "session.title": "Current session",
  "session.reset": "Reset",
  "session.wins": "Wins",
  "session.losses": "Losses",
  "session.streak": "Streak",
  "session.records": "Session records — best win streak: <strong>{best_win}</strong> · worst loss streak: <strong>{best_loss}</strong>",
  "session.filter.label": "Count matches in:",
  "session.filter.note": "Doesn't distinguish Ranked from Casual: the official Rocket League API doesn't expose the matchmaking mode. The filter above is based solely on the <strong>team size</strong> detected at match start.",
  "session.resetConfirm": "Reset the session? (wins/losses go back to zero)",
  "session.streakEmpty": "—",

  "player.title": "Player",
  "player.label": "In-game name",
  "player.placeholder": "Your Rocket League name",
  "player.save": "Save",
  "player.idCaptured": "Stable identifier captured: <code>{id}</code>",
  "player.idPending": "The stable identifier will be captured automatically on the next match.",
  "player.savePrompt": "Enter a name before saving.",
  "player.detectedWaiting": "Waiting for the first match…",
  "player.detectedNote": "Auto-detected from your Steam/Epic account. No input needed.",

  // Apparence (Appearance) panel — see French catalog above.
  "appearance.title": "Appearance",
  "launcher.enable": "Show floating launcher",
  "launcher.enableHint": "Small clickable circle on the left edge of the screen. Opens this window. Auto-hidden during a match.",

  "hud.title": "In-game HUD",
  "hud.note": "Displays the overlay as a transparent window over Rocket League. Works only in <strong>borderless fullscreen</strong>.",
  "hud.show": "▶ Show HUD",
  "hud.hide": "🟢 HUD active — hide",
  "hud.reload": "🔄 Reload",
  "hud.reloadTitle": "Reload the HUD to force a fresh asset fetch",
  "hud.reloaded": "✓ Reloaded",
  "hud.step": "Step",
  "hud.lock": "Lock position",
  "hud.lockHint": "Locked: the HUD becomes click-through again (cursor events pass to the game) and drag / right-click are disabled.",
  "hud.autoHide": "Auto-hide when RL is offline",
  "hud.matchTitle": "Current match",
  "hud.matchHint": "G/S/Sh/A — goals, saves, shots, assists.",
  "hud.matchEmpty": "Waiting for the next match…",

  "obs.title": "OBS Browser Source",
  "obs.note": "Paste this URL into an OBS <em>Browser Source</em>, dimensions <code>400 × 300</code>.",
  "obs.copy": "📋 Copy URL",
  "obs.preview": "👁 Browser preview",
  "obs.copied": "✓ Copied",

  "theme.title": "Theme",
  "theme.openFolder": "📁 Themes folder",
  "theme.openFolderTitle": "Open the folder where you can drop a custom theme",
  "theme.refresh": "🔄 Refresh",
  "theme.refreshTitle": "Rescan the folder after a drag-drop",
  "theme.refreshed": "✓ Refreshed",
  "theme.resetAll": "Reset all overrides",
  "theme.resetConfirm": "Reset all settings for this theme?",
  "theme.openFolderError": "Could not open the themes folder: {err}",
  "theme.activeLabel": "Active theme",
  "theme.varOverride": "Override active",
  "theme.varReset": "Reset to default",

  "footer.settingsAt": "Settings stored in <code>{path}</code>",
  "footer.openLogs": "📂 Open logs",
  "footer.openLogsTitle": "Open the logs folder (useful when filing a bug)",
  "footer.trayHint": "The window's close button sends the app to the system tray — use the button below (or right-click the tray icon) to fully quit.",
  "footer.quit": "⏻ Quit application",
  "footer.language": "Language",
  "footer.langAuto": "Auto",
  "footer.langFr": "Français",
  "footer.langEn": "English",

  "wizard.welcome": "🎮 Welcome to RL Stats Overlay",
  "wizard.welcomeSub": "Let's configure your Rocket League install. Takes 30 seconds.",
  "wizard.installTitle": "1. Detected install",
  "wizard.notDetected": "No install detected automatically. You can pick the folder manually.",
  "wizard.installLabel": "Rocket League — {platform}",
  "wizard.browse": "📂 Pick a folder manually",
  "wizard.browseTitle": "Select your Rocket League install folder",
  "wizard.apiTitle": "2. Stats API activation",
  "wizard.apiAlreadyOk": "The Stats API was already enabled correctly. No change needed.",
  "wizard.apiApplied": "Configuration applied to <code>{path}</code>",
  "wizard.apiBackup": "Old file backed up to <code>{path}</code>.",
  "wizard.apiNote1": "The Stats API is an <strong>official Psyonix feature</strong>, fully Easy Anti-Cheat compatible. No injection into the game — we only read the API the game itself exposes.",
  "wizard.apiNote2": "⚠️ <strong>Restart Rocket League</strong> if it was running so the change takes effect.",
  "wizard.playerTitle": "3. Your in-game name",
  "wizard.playerLabel": "Type your in-game name exactly as it appears in match (case + spaces matter)",
  "wizard.playerPlaceholder": "e.g. Pooley",
  "wizard.finish": "Finish ▶",
  "wizard.finishPrompt": "Enter your in-game name before continuing.",
  "wizard.patchError": "Could not modify the Stats API config:\n{err}\n\nCheck that you have write permissions on the install folder.",
  "wizard.autoDetectedTitle": "Identifier auto-detected",
  "wizard.autoDetectedNote": "Your Steam/Epic account was detected. The overlay will identify you on the first match.",

  "update.banner": "🔔 New version <strong>{version}</strong> available",
  "update.install": "Install",
  "update.dismiss": "Later",
  "update.downloading": "Downloading…",
  "update.retry": "Retry",
};

const catalogs: Record<Lang, Catalog> = { fr, en };

let currentLang: Lang = "fr";

export function resolveLang(pref: LangPref): Lang {
  if (pref === "fr" || pref === "en") return pref;
  // "auto" — pick by browser locale, defaulting to French.
  const nav = (typeof navigator !== "undefined" ? navigator.language : "fr") || "fr";
  return nav.toLowerCase().startsWith("en") ? "en" : "fr";
}

export function setLanguage(pref: LangPref): Lang {
  currentLang = resolveLang(pref);
  document.documentElement.lang = currentLang;
  return currentLang;
}

export function getLang(): Lang {
  return currentLang;
}

export function t(key: string, vars?: Record<string, string | number>): string {
  const raw = catalogs[currentLang][key] ?? catalogs.fr[key] ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (_, k) =>
    vars[k] !== undefined ? String(vars[k]) : `{${k}}`,
  );
}
