# Troubleshooting

## Windows SmartScreen affiche "Windows a protégé votre PC"

C'est normal pour un projet open-source non signé. Microsoft demande un certificat de code-signing payant (~150 €/an) pour faire disparaître cet écran. Pour la v1, on s'en passe.

**Procédure** :

1. Sur l'écran SmartScreen, clique sur **Plus d'infos** (le lien est sous le titre)
2. Un bouton **Exécuter quand même** apparaît → clique dessus
3. La réputation de l'app s'améliore avec les téléchargements ; cet écran finira par ne plus apparaître

Si tu veux vérifier que le binaire est sain, soumets-le sur [VirusTotal](https://www.virustotal.com) — l'app étant petite et open-source, le code source est entièrement auditable dans ce dépôt.

---

## L'overlay reste vide / "En attente du jeu" même après lancement de Rocket League

Vérifie dans cet ordre :

### 1. La Stats API est-elle activée ?

Ouvre `<dossier RL>\TAGame\Config\DefaultStatsAPI.ini` et vérifie qu'il contient :

```ini
[/Script/TAGame.StatsAPIClient]
PacketSendRate=30
Port=49123
```

Si non, ré-ouvre le wizard de l'app (tu peux supprimer `%APPDATA%\RLStatsOverlay\settings.json` pour le forcer).

### 2. As-tu **redémarré** Rocket League après le patch ?

Le jeu lit le fichier `.ini` au démarrage. Toute modif nécessite un redémarrage complet (pas juste retour au menu).

### 3. Le port 49123 est-il déjà utilisé ?

Une autre application peut squatter le port (ancien BakkesMod, autre overlay…).

```powershell
# PowerShell — qui écoute sur 49123 ?
Get-NetTCPConnection -LocalPort 49123
```

Si un PID s'affiche, c'est qu'un autre process bloque RL d'utiliser ce port. Termine-le ou change le port dans le `.ini` (et ouvre une issue, on adaptera l'app).

---

## Le HUD in-game ne s'affiche pas par-dessus Rocket League

**Cause la plus fréquente** : Rocket League est lancé en **plein écran exclusif** (Fullscreen). Les fenêtres always-on-top du système d'exploitation ne s'affichent **pas** par-dessus une application en plein écran exclusif.

**Solution** : passer en **plein écran fenêtré (borderless)** dans Rocket League.

1. Dans RL : **Settings → Video → Window Mode**
2. Choisis **Borderless**
3. Applique → le HUD apparaîtra par-dessus le jeu

> 💡 Le mode Borderless n'a aucun impact perceptible sur les performances pour la plupart des configs récentes, et offre l'énorme avantage de pouvoir basculer entre fenêtres instantanément (Discord, OBS, etc.) sans crash.

---

## Le HUD in-game est click-through, je ne peux pas le déplacer

C'est volontaire — le HUD est click-through par défaut pour ne pas gêner le jeu.

Pour le repositionner :

- Clique **📐 Repositionner** dans la fenêtre principale, OU
- Appuie sur le raccourci global <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>L</kbd>

Tu peux maintenant déplacer/redimensionner la fenêtre normalement. Une fois positionnée, ré-appuie sur <kbd>Ctrl + Shift + L</kbd> pour réactiver le click-through.

---

## Mes wins/losses ne sont pas comptés

Vérifie que ton **pseudo en jeu** dans l'app correspond exactement à celui affiché en match (sensible aux espaces, majuscules/minuscules ignorées). Tu peux le modifier dans la fenêtre principale → section **Joueur**.

L'app capture aussi un **identifiant stable** au premier match — visible sous le champ pseudo. Une fois capturé, l'app retrouve ton compte même si tu changes de pseudo en jeu.

Si rien n'est compté, ouvre une issue avec :
- Ton pseudo en jeu (capture du match en question)
- Le contenu de `%APPDATA%\RLStatsOverlay\settings.json`

---

## Le port 49124 (HTTP local) est occupé

L'app scanne automatiquement de 49124 à 49133 pour trouver un port libre. Le port effectif est affiché dans la section **OBS Browser Source** (l'URL contient le port choisi).

Si ton OBS a une URL avec un ancien port (49124) mais que l'app a bind 49125, il suffit de **re-cliquer sur 📋 Copier l'URL** et la coller dans OBS.

---

## L'antivirus signale le binaire

Comme il n'est pas signé et embarque un serveur HTTP local + un client WebSocket, certains antivirus heuristiques le marquent comme suspect (faux positif).

- Vérifie sur [VirusTotal](https://www.virustotal.com) — la grande majorité des moteurs ne flag pas
- Le code source complet est dans ce dépôt, audit possible
- Tu peux aussi compiler toi-même depuis les sources (voir [docs/development.md](development.md))
