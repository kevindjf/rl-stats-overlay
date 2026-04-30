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

- **Boost overlay** : un anneau autour de la jauge de boost qui affiche `W (wins)`, `L (losses)`, et la **streak actuelle** (🔥 séries de wins / ❄️ séries de losses)
- **HUD in-game** : fenêtre transparente always-on-top qui s'affiche par-dessus Rocket League en mode plein écran fenêtré (borderless)
- **Browser Source OBS** : la même chose en source navigateur pour les streamers
- **Session intelligente** : la session se réinitialise après 6h d'inactivité, persiste entre les redémarrages
- **Configuration automatique** : détection auto de Rocket League (Steam et Epic), patch de la config en un clic

## 🚀 Installation (3 minutes, zéro ligne de commande)

1. Va sur la page [**Releases**](https://github.com/kevindjf/rl-stats-overlay/releases/latest)
2. Télécharge `RLStatsOverlay-Setup-x.y.z-x64-setup.exe`
3. Double-clique pour lancer l'installation
4. ⚠️ Si Windows affiche un avertissement SmartScreen : clique sur **Plus d'infos** → **Exécuter quand même** *(normal, l'app n'est pas encore code-signée — voir [Troubleshooting](docs/troubleshooting.md))*
5. À la première ouverture, suis le **wizard de configuration** :
   - Sélectionne ton installation Rocket League (détectée automatiquement)
   - Tape ton pseudo en jeu (exactement comme il s'affiche en match)
   - C'est fini — **redémarre Rocket League** pour activer la Stats API

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

## 🗺 Roadmap

- [x] **v1.0** : boost overlay (W/L/streak)
- [ ] **v1.1** : barre stats live (goals/assists/saves/shots/demos)
- [ ] **v1.2** : événements visuels (CrossbarHit, GoalScored avec speed)
- [ ] **v1.3** : récap post-match (delta session)
- [ ] **v2.0** : code-signing (suppression du SmartScreen warning), localisation EN/FR
- [ ] **v2.x** : éditeur visuel d'overlays (couleurs, position, taille via UI)

## 📜 Licence

[MIT](./LICENSE) — projet non affilié à Psyonix ni à Epic Games. *Rocket League* est une marque déposée de Psyonix LLC.
