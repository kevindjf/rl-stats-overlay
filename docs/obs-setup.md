# OBS — guide complet

## Installation rapide

1. Lance **RL Stats Overlay**
2. Clique sur **📋 Copier l'URL** dans la section *OBS Browser Source*
3. Dans OBS : **Sources** → **+** → **Browser**
4. Décoche **Local file**
5. Colle l'URL dans le champ **URL** (`http://localhost:49124/overlays/boost.html` ou similaire)
6. Width : `320` · Height : `360`
7. Coche éventuellement **Refresh browser when scene becomes active**
8. ✓

## Positionnement

Le boost overlay est conçu pour s'aligner **autour de la jauge de boost de Rocket League**, qui se trouve dans le coin inférieur droit de l'écran.

- En 1080p, la jauge fait ~200 px de diamètre
- L'overlay (320 × 360) la chevauche avec un trou central de 200 px (la jauge reste visible)
- Active le mode édition de la source dans OBS, place le centre de l'overlay sur le centre de la jauge

> 💡 Pour des rendus en 1440p ou 4K, tu peux scaler la source dans OBS plutôt que d'agrandir le HTML.

## Plusieurs overlays empilés

Tu peux ajouter plusieurs Browser Sources avec la même URL — elles se synchronisent toutes (même `localStorage` côté navigateur OBS, donc les wins/losses s'incrémentent au même endroit).

Si tu veux des overlays séparés (par exemple un overlay de stream et un autre pour ton PC de capture), chacun affichera la même session car ils interrogent le même backend local.

## Compatibilité

- **OBS Studio** ≥ 28 : ✅ testé
- **Streamlabs** : ✅ même mécanisme
- **XSplit** : *non testé*, devrait fonctionner (Browser Source standard)

## Problèmes courants

**L'overlay est blanc / vide**
→ L'app `RL Stats Overlay` doit tourner sur la même machine que OBS. L'URL utilise `localhost` qui ne sort pas de la machine.

**L'overlay est trop petit / pixellisé**
→ Augmente Width/Height dans la Browser Source (640 × 720 par exemple) plutôt que de scaler la source dans OBS.

**Le compteur saute**
→ Si tu refresh la Browser Source en cours de stream, la session reprend de zéro côté OBS. C'est volontaire — le compteur "vrai" est dans l'app desktop, qui est la source de vérité. Tu peux re-synchroniser en re-cliquant **Refresh** ou en redémarrant la source.
