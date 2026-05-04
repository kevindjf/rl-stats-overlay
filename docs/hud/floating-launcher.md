# Bouton flottant

Le **bouton flottant** est un petit rond cliquable épinglé sur le bord gauche de ton écran principal. Un clic ouvre la fenêtre des réglages d'**RL Stats Overlay**.

## À quoi ça sert

Pendant ta session de jeu, tu n'as ni la fenêtre de réglages ouverte, ni l'icône de la barre des tâches en évidence. Le bouton flottant te donne un **accès en un clic** :

- pour reset une session
- pour activer le mode édition et repositionner le HUD
- pour changer de thème
- pour quitter l'app proprement

## Apparence

- Petit cercle ~56 px (en pixels logiques, ajusté au DPI de ton écran).
- Toujours au-dessus, ne capture pas le focus.
- Vertical-center sur le bord gauche du moniteur principal.

<!-- IMAGE: images/floating-launcher-on-edge.png — circle on left edge of desktop (800×600) -->

## Activer / désactiver

Dans la section **Apparence** : coche / décoche **"Afficher le bouton flottant"**. Persisté entre redémarrages.

<!-- IMAGE: images/floating-launcher-toggle.png — settings toggle (800×300) -->

## Auto-masquage en match

Le bouton se masque tout seul **dès qu'un match commence** dans Rocket League (événement `MatchInitialized` / `MatchCreated` de la Stats API) et **réapparaît à la fin du match** (`MatchDestroyed`).

Ce comportement est non-configurable par design — un cercle flottant pendant un match en stream serait visuellement intrusif.
