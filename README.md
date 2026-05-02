# 🎮 RL Stats Overlay

**🇫🇷 Français** · [🇬🇧 English](README.en.md)

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
- **Stats live du match en cours** : buts, arrêts, tirs, passes décisives — extraits en direct de la Stats API et affichés dans le dashboard et les thèmes compatibles.
- **Deux modes d'affichage au choix** (ou les deux en même temps) :
  - 🎮 **HUD in-game** — une petite fenêtre transparente posée par-dessus ton Rocket League. Drag à la souris pour la repositionner, clic-droit pour le menu contextuel (Reset session / Verrouiller / Quitter), verrouillage 1-clic une fois bien placée. Auto-scale selon le DPI de ton écran au premier lancement (1080p / 1440p / 4K / 5K+).
  - 📺 **Source navigateur OBS** — URL à coller dans une Browser Source pour ton stream.
- **Bouton flottant rapide** : un petit rond cliquable sur le bord gauche de ton écran ouvre la fenêtre des réglages d'un clic. Auto-masqué pendant un match.
- **Plusieurs thèmes** prêts à l'emploi (et tu peux créer le tien — voir [le guide designer](docs/themes-fr.md))
- **Session intelligente** : tes wins/losses sont sauvegardés et persistent entre redémarrages. La session se réinitialise toute seule après 6h d'inactivité (nouvelle journée de jeu = compteurs propres).
- **Auto-hide du HUD quand RL est fermé** (option) : le HUD apparaît tout seul à l'ouverture de Rocket League et disparaît à sa fermeture.
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
3. **Glisse-la à la souris** où tu veux. Pour un placement au pixel près, utilise aussi les champs **X / Y / W / H** dans la section *HUD* (pas réglable de 1 à 50 px). Toutes les valeurs sont persistées.
4. **Clic-droit sur le HUD** : menu rapide pour reset la session, verrouiller la position (clic-through), ou quitter l'app.
5. Une fois bien placé, coche **"Verrouiller la position"** dans la section *Apparence* — le HUD redevient cliquable à travers (les inputs souris passent au jeu).

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

Voir [docs/development.md](docs/development.md) pour :

- Lancer l'app en mode dev (Windows ou macOS)
- Tester les overlays sans Rocket League grâce au mock server (`bun run dev/mock-server.ts`)
- Compiler localement
- Architecture du projet

## 🙏 Crédits

Inspiré par [**RocketStats** de Lyliya](https://github.com/Lyliya/RocketStats), une référence historique pour les overlays Rocket League côté streamers. RL Stats Overlay est une réécriture indépendante (Tauri + Rust + TypeScript) centrée sur la **Stats API officielle Psyonix** et la compatibilité Easy Anti-Cheat.

## 📜 Licence

[MIT](./LICENSE) — projet non affilié à Psyonix ni à Epic Games. *Rocket League* est une marque déposée de Psyonix LLC.
