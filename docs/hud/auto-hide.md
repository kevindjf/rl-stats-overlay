# Auto-masquage du HUD

Option : afficher le HUD automatiquement quand Rocket League est lancé, le masquer quand le jeu se ferme.

## Comportement

- **À la connexion à la Stats API** (RL ouvert + Stats API active) → le HUD s'affiche tout seul.
- **À la déconnexion** (RL fermé ou crash) → le HUD se masque.

L'option est **off par défaut** — opt-in pour ne pas surprendre les utilisateurs existants qui pilotent le HUD manuellement avec le bouton "Afficher le HUD".

## Activer l'option

Dans la section **Apparence** : coche **"Masquer auto quand RL est fermé"**.

<!-- IMAGE: images/auto-hide-toggle.png — option checkbox in settings (800×300) -->

## Détection de Rocket League

L'app détecte la présence de Rocket League par **deux mécanismes complémentaires** :

1. **Connexion WebSocket** à `ws://localhost:49123` (la Stats API). Présent = RL est lancé et la Stats API est active.
2. **Polling de la liste des processus Windows** (`RocketLeague.exe`) — collapse le délai de détection de "RL a crashé" à <1 s, contre ~8 s sans (la Stats API ne fait pas de FIN propre sur un Quit).

Pas d'injection. Pas de hook. Juste de la lecture passive.
