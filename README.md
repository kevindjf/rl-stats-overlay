# 🎮 RL Stats Overlay

**🇫🇷 Français** · [🇬🇧 English](README.en.md)

> **Overlay Rocket League pour OBS et HUD in-game.** Wins, losses et streak de session, en temps réel. Compatible Easy Anti-Cheat — utilise uniquement la **Stats API officielle Psyonix**, aucune injection.

<p align="center">
  <a href="https://github.com/kevindjf/rl-stats-overlay/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/kevindjf/rl-stats-overlay?style=for-the-badge"></a>
  <img alt="Platform Windows" src="https://img.shields.io/badge/platform-Windows-blue?style=for-the-badge">
  <img alt="EAC safe" src="https://img.shields.io/badge/EAC-safe-success?style=for-the-badge">
  <img alt="License MIT" src="https://img.shields.io/badge/license-MIT-lightgrey?style=for-the-badge">
</p>

<p align="center">
  <a href="https://kevindjf.github.io/rl-stats-overlay/"><strong>📖 Documentation complète</strong></a> ·
  <a href="https://kevindjf.github.io/rl-stats-overlay/install/">Installation</a> ·
  <a href="https://kevindjf.github.io/rl-stats-overlay/hud/">HUD in-game</a> ·
  <a href="https://kevindjf.github.io/rl-stats-overlay/obs/">OBS</a> ·
  <a href="https://kevindjf.github.io/rl-stats-overlay/themes/">Thèmes</a>
</p>

---

## ✨ Ce que ça fait

<p align="center">
  <img alt="Aperçu de l'overlay" src="docs/images/preview.png" width="640">
  <br><em>Aperçu : ta session en direct (wins, losses, streak) affichée par-dessus Rocket League.</em>
</p>

- **Un overlay en direct de ta session** : nombre de **wins**, de **losses**, et la **streak** en cours (🔥 série de victoires, ❄️ série de défaites). À chaque match terminé, les chiffres bougent tout seuls. **Affiché par défaut au lancement** — désactivable d'un clic si tu veux uniquement la source OBS.
- **Stats live du match en cours** : buts, arrêts, tirs, passes décisives — extraits en direct de la Stats API et affichés dans le dashboard et les thèmes compatibles.
- **Récap post-match détaillé** *(opt-in, désactivé par défaut)* : à la fin de chaque match, une fenêtre dédiée affiche tes stats complètes — boost moyen, BPM, temps à 0/100, distribution du boost, vitesse moyenne, distance parcourue, temps au sol/aérien/mur, powerslides, démos infligées/subies. Disponible aussi en source navigateur OBS. Tant que tu ne l'actives pas dans les réglages, aucun match n'est enregistré et aucune fenêtre ne s'ouvre.
  - **Toggle Match / Session** : bascule entre le résumé du dernier match et l'agrégat de la session entière (mêmes métriques, moyennées sur tous les matchs joués depuis le dernier reset).
  - **Tauri ↔ OBS toujours synchronisés** : la fenêtre Tauri pilote la source OBS. Fermer la croix, basculer Match/Session, désactiver l'affichage post-match dans les réglages → la source OBS reflète la même chose en moins de 2 s. La case "Source OBS" ne sert qu'à muter le navigateur seul (la fenêtre Tauri reste ta vue maîtresse).
  - **Pas d'apparition à contre-temps** : le récap n'apparaît jamais au démarrage de l'app (plus de flash du match d'hier au boot), et il s'efface dès que le match suivant commence — y compris pendant le countdown, sans bloquer la vue du jeu.
- **Onglets Analytics** dans la fenêtre des réglages : Historique (liste paginée + détail par match), Session (agrégats W/L, moyennes, records), All-time (sur toute ta carrière). Sélecteur multi-profils si tu jouais sur plusieurs comptes. Apparaissent uniquement si tu actives le récap post-match.
- **Deux modes d'affichage au choix** (ou les deux en même temps) :
  - 🎮 **HUD in-game** — une petite fenêtre transparente posée par-dessus ton Rocket League. **Mode édition** pour la déplacer/redimensionner à la souris (drag au milieu pour bouger, poignée bas-droit pour scale), désactivé pendant le jeu pour que les clics passent au jeu. **Slider d'échelle** dans les réglages pour un scale au pourcentage près. Clic-droit pour le menu contextuel (Reset session / Verrouiller / Quitter). Auto-scale selon le DPI de ton écran au premier lancement (1080p / 1440p / 4K / 5K+).
  - 📺 **Source navigateur OBS** — URL à coller dans une Browser Source pour ton stream.
- **Bouton flottant rapide** : un petit rond cliquable sur le bord gauche de ton écran ouvre la fenêtre des réglages d'un clic. Auto-masqué pendant un match.
- **Plusieurs thèmes** prêts à l'emploi (et tu peux créer le tien — voir [le guide designer](docs/themes/designer-guide.md))
- **Session intelligente** : tes wins/losses sont sauvegardés et persistent entre redémarrages. La session se réinitialise toute seule après 6h d'inactivité (nouvelle journée de jeu = compteurs propres). Cliquer sur **Reset** sur le HUD ou dans l'onglet Session démarre proprement une nouvelle session — la session DB et le compteur in-memory restent toujours synchros.
- **Aucun résultat raté** : forfaits, mate qui quitte, abandons, déconnexions mid-match — chaque résultat est tracé en plusieurs filets de sécurité. Si l'API du jeu n'envoie pas le gagnant, on déduit depuis le score de la dernière trame. Si la fin de match arrive sous une autre forme (ex : sortie du lobby avant le coup de sifflet), le compteur est rattrapé à ce moment-là. Et au démarrage suivant, la session est réconciliée avec l'historique disque pour récupérer ce qui aurait pu manquer.
- **Auto-hide du HUD quand RL est fermé** (option) : le HUD apparaît tout seul à l'ouverture de Rocket League et disparaît à sa fermeture.
- **Fenêtre de réglages repliable** : chaque section (Session, Joueur, Apparence, HUD, OBS, Thème) se plie/déplie d'un clic sur son titre. Quand elle est repliée, un mini-résumé reste affiché à droite (ex: `12W · 5L · +3` pour la session, `Circle` pour le thème actif) et l'état ouvert/fermé est mémorisé entre redémarrages.
- **Setup guidé** : pas besoin d'aller chercher tes fichiers à la main — l'app retrouve toute seule où ton Rocket League est installé (Steam ou Epic), active pour toi la fonction "stats en direct" déjà intégrée au jeu (mais désactivée par défaut), et **détecte automatiquement ton compte Steam/Epic local**. Aucune saisie de pseudo nécessaire dans la majorité des cas — l'overlay s'associe à toi tout seul dès le premier match (et suit même les changements de compte).

## 🚀 Installation (3 minutes, zéro ligne de commande)

1. Va sur la page [**Releases**](https://github.com/kevindjf/rl-stats-overlay/releases/latest)
2. Télécharge `RL Stats Overlay_x.y.z_x64-setup.exe`
3. Double-clique pour lancer l'installation
4. À la première ouverture, suis le **wizard de configuration** (2 étapes si ton compte Steam/Epic est détecté, 3 sinon) :
   - Sélectionne ton installation Rocket League (détectée automatiquement)
   - Confirme l'activation de la Stats API
   - *(seulement si auto-détection impossible)* Tape ton pseudo en jeu
   - C'est fini — **redémarre Rocket League** pour activer la Stats API

> ### ⚠️ Windows affiche "Microsoft Defender SmartScreen empêché le démarrage"
>
> **C'est normal et attendu.** L'app n'est pas (encore) signée avec un certificat
> de code-signing payant — Windows met cet avertissement par défaut sur tout
> binaire d'un éditeur qu'il ne connaît pas, indépendamment du contenu.
>
> **Pour la passer** : sur l'écran SmartScreen, clique sur **Plus d'infos**, puis
> sur le bouton **Exécuter quand même** qui apparaît.
>
> Le code source est entièrement public dans ce dépôt et tu peux soumettre
> le `.exe` sur [VirusTotal](https://www.virustotal.com) si tu veux une analyse
> indépendante. Voir [Troubleshooting](docs/troubleshooting.md#windows-smartscreen-affiche-windows-a-protégé-votre-pc) pour plus de détails.

## 🟢 Utilisation HUD in-game

1. Ouvre RL Stats Overlay
2. Clique **▶ Afficher le HUD** → une fenêtre transparente apparaît
3. Active le **Mode édition** dans la section *HUD en jeu* — une bordure pointillée cyan apparaît autour du HUD avec une grosse poignée dans le coin bas-droit.
4. **Place-le où tu veux** :
   - **Glisser à la souris** au milieu du HUD pour le déplacer.
   - **Glisser la poignée bas-droit** pour le redimensionner — le contenu (cards, icônes, texte) suit l'échelle automatiquement.
   - Ou utilise le **slider Échelle** (50 % – 250 %) et les champs **X / Y / W / H** dans le panneau pour un placement au pixel près.
5. **Coupe le Mode édition** une fois bien placé — le HUD redevient cliquable à travers (les inputs souris passent au jeu) et la bordure cyan disparaît.
6. **Clic-droit sur le HUD** (toujours dispo) : menu rapide pour reset la session, basculer le mode édition, ou quitter l'app.

> ⚠️ **Important** : Rocket League doit tourner en **plein écran fenêtré (borderless)** pour que la fenêtre transparente s'affiche par-dessus. Dans RL : *Settings → Video → Window Mode → **Borderless**.* (Le mode "Fullscreen" exclusif n'est pas supporté par Windows pour les overlays — c'est une limite du système, pas de l'app.)

## 📺 Utilisation OBS (streamers)

1. Dans RL Stats Overlay, clique **📋 Copier l'URL**
2. Dans OBS : **Sources → + → Browser Source**
3. Coche **Local file** : décoché
4. Colle l'URL dans le champ **URL**
5. Width : `320` · Height : `360`
6. ✓

L'overlay tourne tant que l'app `RL Stats Overlay` est ouverte sur ta machine. Tu peux la fermer une fois la stream terminée.

## 🛡 Compatible Easy Anti-Cheat

L'app **n'injecte rien** dans Rocket League. Elle se contente de lire la **Stats API officielle de Psyonix**, exposée nativement par le jeu via un WebSocket local (`ws://localhost:49123`). C'est la même API utilisée par les broadcasters pro pour les RLCS.

Contrairement à BakkesMod / SOS, aucune DLL injectée, aucune lecture mémoire, aucune action côté serveur de matchmaking. Le seul changement effectué est l'activation d'une fonctionnalité **dormante mais officielle** dans `DefaultStatsAPI.ini`.

## 📂 Configuration

Toute la config tient dans un seul fichier JSON :

- **Windows** : `%APPDATA%\RLStatsOverlay\settings.json`
- **macOS (dev)** : `~/Library/Application Support/RLStatsOverlay/settings.json`

Tu peux le supprimer pour repartir à zéro (le wizard se relancera).

## 🧰 Pour les développeurs / contributeurs

Voir [docs/contributing/development.md](docs/contributing/development.md) pour :

- Lancer l'app en mode dev (Windows ou macOS)
- Tester les overlays sans Rocket League grâce au mock server (`bun run dev/mock-server.ts`)
- Compiler localement
- Architecture du projet

## 🙏 Crédits

Inspiré par [**RocketStats** de Lyliya](https://github.com/Lyliya/RocketStats), une référence historique pour les overlays Rocket League côté streamers. RL Stats Overlay est une réécriture indépendante (Tauri + Rust + TypeScript) centrée sur la **Stats API officielle Psyonix** et la compatibilité Easy Anti-Cheat.

## 📜 Licence

[MIT](./LICENSE) — projet non affilié à Psyonix ni à Epic Games. *Rocket League* est une marque déposée de Psyonix LLC.
