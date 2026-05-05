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
  "session.recordsLeading": "Records — meilleure série de wins :",
  "session.recordsMid": "· pire série de losses :",
  "session.filter.label": "Compter les matchs en :",
  "session.filter.note": "Ne distingue pas Ranked et Casual : l'API officielle de Rocket League n'expose pas le mode de matchmaking. Le filtre ci-dessus se base uniquement sur la <strong>taille des équipes</strong> détectée en début de match.",
  "session.resetConfirm": "Réinitialiser la session ? (les wins/losses repartent à zéro)",
  "session.streakEmpty": "—",
  "session.editStart": "✏️ Éditer",
  "session.editSave": "💾 Enregistrer",
  "session.editCancel": "Annuler",

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
  "hud.scaleLabel": "Échelle",
  "hud.scaleHint": "Redimensionne le HUD et son contenu en même temps. Astuce : tu peux aussi attraper le coin bas-droit du HUD à la souris (active le mode édition pour mieux le voir).",
  "hud.editMode": "Mode édition",
  "hud.editModeHint": "Pour déplacer/redimensionner le HUD à la souris. À couper avant de jouer.",
  "hud.lock": "Verrouiller la position",
  "hud.lockHint": "Verrouillé : le HUD redevient transparent au clic (les inputs passent au jeu) et le drag/clic-droit est désactivé.",
  "hud.autoHide": "Masquer auto quand RL est fermé",
  "hud.matchTitle": "Match en cours",
  "hud.matchHint": "G/S/Sh/A — buts, arrêts, tirs, passes décisives.",
  "hud.matchEmpty": "En attente du prochain match…",
  "hud.snap.title": "Position rapide",
  "hud.snap.hint": "Aligne la fenêtre HUD sur un bord de l'écran courant. Idéal avec le thème <em>Inline</em>.",
  "hud.snap.placeholder": "Choisir une position…",
  "hud.snap.topLeft": "Coin haut-gauche",
  "hud.snap.topCenter": "Centré en haut",
  "hud.snap.topRight": "Coin haut-droit",
  "hud.snap.middleLeft": "Bord gauche",
  "hud.snap.center": "Centre de l'écran",
  "hud.snap.middleRight": "Bord droit",
  "hud.snap.bottomLeft": "Coin bas-gauche",
  "hud.snap.bottomCenter": "Centré en bas",
  "hud.snap.bottomRight": "Coin bas-droit",

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

  // Top tabs (settings / history / session / alltime)
  "topTabs.settings": "Réglages",
  "topTabs.history": "Historique",
  "topTabs.session": "Session",
  "topTabs.alltime": "All-time",

  // Analytics — common
  "analytics.tabs.label": "Vues d'analyse",
  "analytics.tabs.history": "Historique",
  "analytics.tabs.session": "Session",
  "analytics.tabs.alltime": "All-time",
  "analytics.profile.label": "Profil :",
  "analytics.profile.none": "Aucun profil enregistré pour l'instant — joue un match pour le créer.",

  // History list
  "analytics.history.empty": "Aucun match enregistré pour ce profil.",
  "analytics.history.emptyHint": "L'historique se remplira automatiquement à la fin de chaque match.",

  // Match detail
  "analytics.match.back": "Retour à l'historique",
  "analytics.match.notFound": "Match introuvable.",
  "analytics.match.win": "VICTOIRE",
  "analytics.match.loss": "DÉFAITE",
  "analytics.match.players": "Joueurs",
  "analytics.match.goals": "Timeline des goals",
  "analytics.match.advanced": "Boost & mouvement (équipe locale uniquement)",
  "analytics.match.advancedHint": "L'API live ne fournit pas les stats avancées de l'équipe adverse — limitation Psyonix. Les valeurs marquées BPM⚠ et distance sont approximées (~95 % de précision vs replay).",
  "analytics.match.histograms": "Histogrammes",
  "analytics.match.possession": "Possession & crossbars",
  "analytics.match.statfeed": "Statfeed",
  "analytics.match.crossbars": "Crossbars",
  "analytics.match.noGoals": "Aucun goal pendant ce match.",
  "analytics.match.noStatfeed": "Aucun event Statfeed enregistré.",
  "analytics.match.assistedBy": "passe de",
  "analytics.match.delete": "🗑 Supprimer ce match",
  "analytics.match.deleteConfirm": "Supprimer ce match de l'historique ? Cette action est irréversible.",

  // Players / advanced columns
  "analytics.player.name": "Joueur",
  "analytics.player.score": "Pts",
  "analytics.team.blue": "Bleu",
  "analytics.team.orange": "Orange",
  "analytics.adv.avgBoost": "Avg",
  "analytics.adv.t0": "T0",
  "analytics.adv.t100": "T100",
  "analytics.adv.avgSpeed": "Speed%",
  "analytics.adv.dist": "Dist.",
  "analytics.adv.slow": "<1400",
  "analytics.adv.boostSpeed": "Boost",
  "analytics.adv.super": "Super",
  "analytics.adv.ground": "Sol",
  "analytics.adv.aerial": "Aerial",
  "analytics.adv.psTot": "PS tot",
  "analytics.adv.psCount": "PS nb",
  "analytics.adv.psAvg": "PS moy",
  "analytics.adv.demosTaken": "Démos subies",

  // Histograms
  "analytics.hist.boostDist": "Distribution boost",
  "analytics.hist.speedDist": "Distribution vitesse",
  "analytics.hist.airGround": "Sol vs aérien",
  "analytics.hist.slow": "Lent",
  "analytics.hist.boost": "Boost",
  "analytics.hist.super": "Supersonic",
  "analytics.hist.ground": "Sol",
  "analytics.hist.aerial": "Aérien",
  "analytics.hist.wall": "Mur",
  "analytics.hist.empty": "Pas de données SPECTATOR pour ce match.",

  // Aggregate (session + lifetime)
  "analytics.aggregate.empty": "Pas encore de match dans cette vue.",
  "analytics.aggregate.matches": "Matchs",
  "analytics.aggregate.winRate": "Win rate",
  "analytics.aggregate.streak": "Streak",
  "analytics.aggregate.best": "Records",
  "analytics.aggregate.trend": "Tendance W/L (récent)",
  "analytics.aggregate.averages": "Moyennes par match",
  "analytics.aggregate.byPlaylist": "Par playlist (taille d'équipe)",
  "analytics.aggregate.records": "Records",
  "analytics.aggregate.bestMatch": "Meilleur match",
  "analytics.aggregate.worstMatch": "Pire match",
  "analytics.aggregate.openMatch": "Ouvrir →",
  "analytics.aggregate.goals": "Goals",
  "analytics.aggregate.shots": "Shots",
  "analytics.aggregate.saves": "Saves",
  "analytics.aggregate.assists": "Assists",
  "analytics.aggregate.score": "Score",
  "analytics.aggregate.demos": "Démos",
  "analytics.aggregate.avgBoost": "Boost moyen",
  "analytics.aggregate.noPlaylist": "Pas de répartition disponible.",
  "analytics.aggregate.resetSession": "✕ Démarrer une nouvelle session",
  "analytics.aggregate.resetConfirm": "Démarrer une nouvelle session ? Les matchs précédents restent dans l'historique mais la vue Session repart de zéro.",
  "analytics.aggregate.sessionStart": "Session démarrée {ago}",
  "analytics.aggregate.lifetimeStart": "Depuis le {date}",

  // Time formatting
  "analytics.time.justNow": "à l'instant",
  "analytics.time.minutesAgo": "il y a {n} min",
  "analytics.time.hoursAgo": "il y a {n} h",
  "analytics.time.daysAgo": "il y a {n} j",

  // Post-match settings panel
  "postMatch.title": "Analyses post-match",
  "postMatch.intro": "Stocke les stats détaillées de chaque match pour l'historique, la session et les stats lifetime.",
  "postMatch.enable": "Activer l'enregistrement des matchs",
  "postMatch.enableHint": "Active la capture, le stockage SQLite et les onglets Historique/Session/All-time.",
  "postMatch.enableWarn": "⚠ Si tu désactives, les matchs joués pendant cette période ne pourront pas être récupérés plus tard.",
  "postMatch.disableConfirm": "Désactiver les analyses post-match ? Les matchs joués pendant la désactivation seront perdus.",
  "postMatch.showHud": "Afficher la fenêtre récap après chaque match",
  "postMatch.showHudHint": "Petite fenêtre transparente toujours-au-dessus avec le résumé du dernier match. Reste affichée jusqu'au prochain match.",
  "postMatch.showObs": "Activer la source OBS récap post-match",
  "postMatch.showObsHint": "Expose le récap comme Browser Source à ajouter dans OBS. Si désactivé, l'URL renvoie une page vide.",
  "postMatch.openFolder": "📂 Ouvrir le dossier de données",
  "postMatch.clearHistory": "🗑 Effacer tout l'historique…",
  "postMatch.clearConfirm": "Effacer tout l'historique des matchs ? Cette action est irréversible.",
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
  "session.recordsLeading": "Records — best win streak:",
  "session.recordsMid": "· worst loss streak:",
  "session.filter.label": "Count matches in:",
  "session.filter.note": "Doesn't distinguish Ranked from Casual: the official Rocket League API doesn't expose the matchmaking mode. The filter above is based solely on the <strong>team size</strong> detected at match start.",
  "session.resetConfirm": "Reset the session? (wins/losses go back to zero)",
  "session.streakEmpty": "—",
  "session.editStart": "✏️ Edit",
  "session.editSave": "💾 Save",
  "session.editCancel": "Cancel",

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
  "hud.scaleLabel": "Scale",
  "hud.scaleHint": "Resizes the HUD and its content together. Tip: you can also grab the bottom-right corner of the HUD with your mouse (turn edit mode on to see it).",
  "hud.editMode": "Edit mode",
  "hud.editModeHint": "Lets you move/resize the HUD with your mouse. Turn off before playing.",
  "hud.lock": "Lock position",
  "hud.lockHint": "Locked: the HUD becomes click-through again (cursor events pass to the game) and drag / right-click are disabled.",
  "hud.autoHide": "Auto-hide when RL is offline",
  "hud.matchTitle": "Current match",
  "hud.matchHint": "G/S/Sh/A — goals, saves, shots, assists.",
  "hud.matchEmpty": "Waiting for the next match…",
  "hud.snap.title": "Quick position",
  "hud.snap.hint": "Snaps the HUD window to an edge of the current monitor. Pairs well with the <em>Inline</em> theme.",
  "hud.snap.placeholder": "Pick a position…",
  "hud.snap.topLeft": "Top-left corner",
  "hud.snap.topCenter": "Top center",
  "hud.snap.topRight": "Top-right corner",
  "hud.snap.middleLeft": "Left edge",
  "hud.snap.center": "Screen center",
  "hud.snap.middleRight": "Right edge",
  "hud.snap.bottomLeft": "Bottom-left corner",
  "hud.snap.bottomCenter": "Bottom center",
  "hud.snap.bottomRight": "Bottom-right corner",

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

  // Top tabs
  "topTabs.settings": "Settings",
  "topTabs.history": "History",
  "topTabs.session": "Session",
  "topTabs.alltime": "All-time",

  // Analytics — common
  "analytics.tabs.label": "Analytics views",
  "analytics.tabs.history": "History",
  "analytics.tabs.session": "Session",
  "analytics.tabs.alltime": "All-time",
  "analytics.profile.label": "Profile:",
  "analytics.profile.none": "No profile recorded yet — play one match to capture it.",

  // History list
  "analytics.history.empty": "No matches recorded for this profile yet.",
  "analytics.history.emptyHint": "The list will populate automatically at the end of each match.",

  // Match detail
  "analytics.match.back": "Back to history",
  "analytics.match.notFound": "Match not found.",
  "analytics.match.win": "WIN",
  "analytics.match.loss": "LOSS",
  "analytics.match.players": "Players",
  "analytics.match.goals": "Goal timeline",
  "analytics.match.advanced": "Boost & movement (local team only)",
  "analytics.match.advancedHint": "The live API does not expose advanced stats for the opposing team — Psyonix limitation. BPM⚠ and total distance values are approximated (~95% accuracy vs replay).",
  "analytics.match.histograms": "Histograms",
  "analytics.match.possession": "Possession & crossbars",
  "analytics.match.statfeed": "Statfeed",
  "analytics.match.crossbars": "Crossbars",
  "analytics.match.noGoals": "No goals scored in this match.",
  "analytics.match.noStatfeed": "No statfeed events recorded.",
  "analytics.match.assistedBy": "assist by",
  "analytics.match.delete": "🗑 Delete this match",
  "analytics.match.deleteConfirm": "Delete this match from history? This cannot be undone.",

  // Players / advanced columns
  "analytics.player.name": "Player",
  "analytics.player.score": "Pts",
  "analytics.team.blue": "Blue",
  "analytics.team.orange": "Orange",
  "analytics.adv.avgBoost": "Avg",
  "analytics.adv.t0": "T0",
  "analytics.adv.t100": "T100",
  "analytics.adv.avgSpeed": "Speed%",
  "analytics.adv.dist": "Dist.",
  "analytics.adv.slow": "<1400",
  "analytics.adv.boostSpeed": "Boost",
  "analytics.adv.super": "Super",
  "analytics.adv.ground": "Ground",
  "analytics.adv.aerial": "Aerial",
  "analytics.adv.psTot": "PS tot",
  "analytics.adv.psCount": "PS #",
  "analytics.adv.psAvg": "PS avg",
  "analytics.adv.demosTaken": "Demos taken",

  // Histograms
  "analytics.hist.boostDist": "Boost distribution",
  "analytics.hist.speedDist": "Speed distribution",
  "analytics.hist.airGround": "Ground vs air",
  "analytics.hist.slow": "Slow",
  "analytics.hist.boost": "Boost",
  "analytics.hist.super": "Supersonic",
  "analytics.hist.ground": "Ground",
  "analytics.hist.aerial": "Aerial",
  "analytics.hist.wall": "Wall",
  "analytics.hist.empty": "No SPECTATOR data for this match.",

  // Aggregate (session + lifetime)
  "analytics.aggregate.empty": "No matches in this view yet.",
  "analytics.aggregate.matches": "Matches",
  "analytics.aggregate.winRate": "Win rate",
  "analytics.aggregate.streak": "Streak",
  "analytics.aggregate.best": "Best",
  "analytics.aggregate.trend": "W/L trend (recent)",
  "analytics.aggregate.averages": "Per-match averages",
  "analytics.aggregate.byPlaylist": "By playlist (team size)",
  "analytics.aggregate.records": "Records",
  "analytics.aggregate.bestMatch": "Best match",
  "analytics.aggregate.worstMatch": "Worst match",
  "analytics.aggregate.openMatch": "Open →",
  "analytics.aggregate.goals": "Goals",
  "analytics.aggregate.shots": "Shots",
  "analytics.aggregate.saves": "Saves",
  "analytics.aggregate.assists": "Assists",
  "analytics.aggregate.score": "Score",
  "analytics.aggregate.demos": "Demos",
  "analytics.aggregate.avgBoost": "Avg boost",
  "analytics.aggregate.noPlaylist": "No breakdown available.",
  "analytics.aggregate.resetSession": "✕ Start a new session",
  "analytics.aggregate.resetConfirm": "Start a new session? Previous matches stay in history but the Session view resets.",
  "analytics.aggregate.sessionStart": "Session started {ago}",
  "analytics.aggregate.lifetimeStart": "Since {date}",

  // Time formatting
  "analytics.time.justNow": "just now",
  "analytics.time.minutesAgo": "{n} min ago",
  "analytics.time.hoursAgo": "{n} h ago",
  "analytics.time.daysAgo": "{n} d ago",

  // Post-match settings panel
  "postMatch.title": "Post-match analytics",
  "postMatch.intro": "Stores detailed stats for every match — powers the History, Session and Lifetime tabs.",
  "postMatch.enable": "Enable match recording",
  "postMatch.enableHint": "Activates capture, SQLite storage and the History/Session/All-time tabs.",
  "postMatch.enableWarn": "⚠ Matches played while this is off won't be recoverable later, even if you re-enable.",
  "postMatch.disableConfirm": "Disable post-match analytics? Matches played while it's off will be lost.",
  "postMatch.showHud": "Show recap window after each match",
  "postMatch.showHudHint": "Small always-on-top transparent window summarizing the last match. Stays visible until the next match starts.",
  "postMatch.showObs": "Enable OBS post-match browser source",
  "postMatch.showObsHint": "Exposes the recap as a Browser Source URL for OBS. When disabled, the URL serves a blank page.",
  "postMatch.openFolder": "📂 Open data folder",
  "postMatch.clearHistory": "🗑 Clear all history…",
  "postMatch.clearConfirm": "Clear the entire match history? This cannot be undone.",
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
