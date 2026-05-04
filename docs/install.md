# Installation

Trois minutes, zéro ligne de commande.

## Télécharger

1. Va sur la page [**Releases**](https://github.com/kevindjf/rl-stats-overlay/releases/latest).
2. Télécharge `RL Stats Overlay_x.y.z_x64-setup.exe` (Windows 64 bits).

> macOS / Linux ne sont pas supportés en build officiel — l'app est testée sur Windows 10 / 11. Pour les environnements de dev, voir [Contribuer → Développement](contributing/development.md).

<!-- IMAGE: images/install-releases-page.png — GitHub Releases page with the .exe asset highlighted (1280×720) -->

## Installer

Double-clique le `.exe` téléchargé. L'installeur NSIS te propose le mode "current user" — pas besoin de droits admin.

## Premier lancement (wizard)

À la première ouverture, le wizard te guide :

1. **Installation détectée** — l'app trouve toute seule où Rocket League est installé (Steam ou Epic). Si la détection échoue, utilise le bouton "📂 Indiquer un dossier manuellement".
2. **Activation de la Stats API** — l'app modifie pour toi `DefaultStatsAPI.ini` (avec backup automatique de l'ancien fichier). C'est une fonctionnalité officielle Psyonix, simplement désactivée par défaut.
3. **Pseudo en jeu** — *seulement si* la détection auto de ton compte Steam/Epic a échoué. Sinon, cette étape est sautée et l'overlay s'associera à toi tout seul dès le premier match.

À la fin du wizard, **redémarre Rocket League** si le jeu était lancé pendant la modif.

<!-- IMAGE CHECKLIST
  - images/install-wizard-step1.png     900×600  RL install detected
  - images/install-wizard-step2.png     900×600  Stats API activation
  - images/install-wizard-step3.png     900×600  Name fallback step
-->

## Windows SmartScreen

À la première ouverture du `.exe`, Windows affiche **"Windows a protégé votre ordinateur"**.

C'est normal et attendu : l'app n'est pas encore signée avec un certificat de code-signing payant. Windows met cet avertissement par défaut sur tout binaire d'un éditeur qu'il ne connaît pas, indépendamment du contenu.

**Pour passer l'écran (deux étapes)** :

1. Sur le premier dialog, clique sur le lien **"Informations complémentaires"** (souligné, sous le texte rouge). Le bouton "Ne pas exécuter" est seul à ce stade — c'est volontaire.

    ![Premier dialog SmartScreen — uniquement Ne pas exécuter](images/install-smartscreen-1.png){ loading=lazy }

2. Le dialog se met à jour : tu vois maintenant le nom du fichier, "Éditeur inconnu", et **un nouveau bouton "Exécuter quand même"** apparaît en bas. Clique dessus.

    ![Deuxième dialog SmartScreen — bouton Exécuter quand même apparu](images/install-smartscreen-2.png){ loading=lazy }

Le code source est entièrement public dans [le dépôt GitHub](https://github.com/kevindjf/rl-stats-overlay) — tu peux soumettre le `.exe` sur [VirusTotal](https://www.virustotal.com) si tu veux une analyse indépendante. Voir aussi [Dépannage](troubleshooting.md).

## Désinstaller

- Via Windows : **Paramètres → Applications → RL Stats Overlay → Désinstaller**.
- Pour effacer aussi les réglages persistés : supprime le dossier `%APPDATA%\RLStatsOverlay`.
