# 🎮 RL Stats Overlay

> **Overlay Rocket League pour OBS et HUD in-game.** Wins, losses et streak de session, en temps réel. Compatible Easy Anti-Cheat — utilise uniquement la **Stats API officielle Psyonix**, aucune injection.

<p align="center">
  <a href="https://github.com/kevindjf/rl-stats-overlay/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/kevindjf/rl-stats-overlay?style=for-the-badge"></a>
  <img alt="Platform Windows" src="https://img.shields.io/badge/platform-Windows-blue?style=for-the-badge">
  <img alt="EAC safe" src="https://img.shields.io/badge/EAC-safe-success?style=for-the-badge">
  <img alt="License MIT" src="https://img.shields.io/badge/license-MIT-lightgrey?style=for-the-badge">
</p>

---

## ✨ Ce que ça fait

<p align="center">
  <img alt="Aperçu de l'overlay" src="docs/images/preview.png" width="640">
  <br><em>Aperçu : ta session en direct (wins, losses, streak) affichée par-dessus Rocket League.</em>
</p>

- **Un overlay en direct de ta session** : nombre de **wins**, de **losses**, et la **streak** en cours (🔥 série de victoires, ❄️ série de défaites). À chaque match terminé, les chiffres bougent tout seuls.
- **Deux modes d'affichage au choix** (ou les deux en même temps) :
  - 🎮 **HUD in-game** — une petite fenêtre transparente posée par-dessus ton Rocket League pendant que tu joues
  - 📺 **Source navigateur OBS** — pour l'afficher sur ton stream
- **Plusieurs thèmes** prêts à l'emploi (et tu peux créer le tien — voir [le guide designer](docs/themes-fr.md))
- **Session intelligente** : tes wins/losses sont sauvegardés et persistent entre redémarrages. La session se réinitialise toute seule après 6h d'inactivité (nouvelle journée de jeu = compteurs propres).
- **Setup guidé** : pas besoin d'aller chercher tes fichiers à la main — l'app retrouve toute seule où ton Rocket League est installé (Steam ou Epic) et active pour toi la fonction "stats en direct" déjà intégrée au jeu (mais désactivée par défaut). Il te reste juste à taper ton pseudo en jeu, pour que l'overlay sache lequel des joueurs du match c'est toi.

## 🚀 Installation (3 minutes, zéro ligne de commande)

1. Va sur la page [**Releases**](https://github.com/kevindjf/rl-stats-overlay/releases/latest)
2. Télécharge `RL Stats Overlay_x.y.z_x64-setup.exe`
3. Double-clique pour lancer l'installation
4. À la première ouverture, suis le **wizard de configuration** :
   - Sélectionne ton installation Rocket League (détectée automatiquement)
   - Tape ton pseudo en jeu (exactement comme il s'affiche en match)
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
3. Clique **📐 Repositionner** (ou raccourci global <kbd>Ctrl + Shift + L</kbd>) pour sortir du mode click-through et déplacer la fenêtre par-dessus la jauge de boost en jeu
4. Re-clique <kbd>Ctrl + Shift + L</kbd> pour figer la position

> ⚠️ **Important** : Rocket League doit tourner en **plein écran fenêtré (borderless)** pour que la fenêtre transparente s'affiche par-dessus. Dans RL : *Settings → Video → Window Mode → **Borderless**.*

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

Voir [docs/development.md](docs/development.md) pour :

- Lancer l'app en mode dev (Windows ou macOS)
- Tester les overlays sans Rocket League grâce au mock server (`bun run dev/mock-server.ts`)
- Compiler localement
- Architecture du projet

## 📜 Licence

[MIT](./LICENSE) — projet non affilié à Psyonix ni à Epic Games. *Rocket League* est une marque déposée de Psyonix LLC.
