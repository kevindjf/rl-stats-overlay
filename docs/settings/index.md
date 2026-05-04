# Fenêtre de réglages

Tour de la fenêtre des réglages — ce qui s'y trouve, comment naviguer.

## Vue d'ensemble

La fenêtre est organisée en **6 sections repliables** :

1. **Session en cours** — wins / losses / streak + filtre par taille d'équipe.
2. **Joueur** — pseudo en jeu + identifiant stable capturé.
3. **Apparence** — bouton flottant, auto-masquage du HUD.
4. **HUD en jeu** — afficher/masquer le HUD, mode édition, scale, X/Y/W/H, stats du match en cours.
5. **OBS Browser Source** — URL à coller dans OBS, copier/aperçu.
6. **Theme** — thème actif + tous les overrides du thème.

<!-- IMAGE: images/settings-overview.png — full settings window, all sections expanded (900×1200) -->

## Sections repliables

Chaque section est un `<details>` HTML — clique sur le **titre** pour la plier ou la déplier. Un **chevron** à gauche du titre tourne sur 90° quand la section est ouverte.

## Mini-résumés (collapsed)

Quand une section est repliée, un **mini-résumé** reste visible à droite du titre. Il te donne l'info clé sans avoir à déplier :

| Section | Résumé replié |
|---|---|
| Session en cours | `12W · 5L · +3` (couleurs vert/rouge sur W/L) |
| Joueur | Ton pseudo en blanc |
| Apparence | (aucun) |
| HUD en jeu | `120% · ● Afficher le HUD` |
| OBS Browser Source | `localhost:49124` |
| Theme | Nom du thème actif (ex: `Circle`) |

Quand la section est ouverte, le résumé s'efface (l'info est déjà visible dans le contenu) — pas de doublon.

<!-- IMAGE: images/settings-collapsed-summaries.png — sections collapsed showing summaries on the right (900×900) -->

## Persistance de l'état

L'état **ouvert / fermé** de chaque section est sauvegardé dans le `localStorage` du WebView2 (clé `panel:<id>`). À chaque redémarrage de l'app, les sections retrouvent leur état précédent.

## Détails par section

### Session en cours

- **Wins / Losses / Streak** affichés en gros, avec couleurs vert/rouge selon la streak.
- Bouton **✏️ Éditer** pour corriger manuellement (utile en cas de crash mid-match ou d'événement raté).
- Bouton **Reset** pour repartir à zéro.
- Filtre **Compter les matchs en** : 1v1 / 2v2 / 3v3 / 4v4 — ne distingue pas Ranked vs Casual (la Stats API n'expose pas le mode), mais permet au moins de scoper la session à une taille d'équipe.

### Joueur

- **Pseudo en jeu** + identifiant stable capturé au premier match (format `Steam|123|0` ou `Epic|abc|0`).
- Si Steam/Epic est auto-détecté, aucune saisie nécessaire.
- L'identifiant survit aux changements de pseudo en jeu — capté une fois, conservé.

### Apparence

- **Afficher le bouton flottant** — voir [Bouton flottant](../hud/floating-launcher.md).
- **Masquer auto quand RL est fermé** — voir [Auto-masquage](../hud/auto-hide.md).

### HUD en jeu

- Voir [HUD in-game](../hud/index.md) et [Mode édition](../hud/edit-mode.md).

### OBS Browser Source

- Voir [OBS Browser Source](../obs/index.md).

### Theme

- Voir [Thèmes](../themes/index.md).
