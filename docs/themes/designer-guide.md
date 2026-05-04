# Créer un thème custom

> Pour la version anglaise : voir [`themes-en.md`](themes-en.md).

Ce guide t'explique comment créer ton propre thème pour l'overlay, le tester, et le partager. Pas besoin de savoir programmer — il suffit de connaître un peu de HTML / CSS (le langage des pages web) et de pouvoir éditer des fichiers texte.

**Tu n'as pas besoin de** : compiler le projet, installer Rust, toucher à GitHub, ou comprendre du code TypeScript. Tu déposes un dossier, tu cliques sur un bouton, ton thème apparaît.

---

## En 5 minutes

1. **Lance l'app** RL Stats Overlay.
2. Va dans la section **Theme** des paramètres.
3. Clique sur **📁 Dossier des thèmes**. L'Explorateur Windows s'ouvre sur `%APPDATA%/RLStatsOverlay/themes/`.
4. Dans le dépôt GitHub du projet, copie le dossier [`overlays/themes/_template/`](../overlays/themes/_template/) et colle-le à l'emplacement ouvert.
5. Renomme-le (par exemple : `mon-super-theme`). **Important** : pas d'espaces, pas d'accents, pas de caractères spéciaux — uniquement lettres, chiffres, tirets ou underscores.
6. Reviens dans l'app et clique sur **🔄 Rafraîchir**. Ton thème apparaît dans la liste déroulante.
7. Sélectionne-le. Il s'affiche immédiatement dans le HUD.

C'est tout. Il te reste juste à l'éditer pour qu'il ait l'apparence que tu veux.

---

## Ce qu'il y a dans un thème

Un thème = un dossier avec **4 fichiers obligatoires** :

```
mon-super-theme/
├── theme.json   ← description du thème + couleurs/options éditables dans l'app
├── boost.html   ← la structure (les blocs, les zones de texte)
├── boost.css    ← l'apparence (couleurs, polices, tailles, position)
└── boost.js     ← une seule ligne, ne touche à rien
```

Tu peux aussi ajouter (optionnel) :
- `screenshot.png` — un aperçu pour ton thème
- `fonts/` — un dossier avec tes polices custom (`.ttf`, `.otf`, `.woff2`)
- `images/` — un dossier avec tes images

Tu y accèdes ensuite depuis ton CSS avec un chemin relatif (par exemple `url("./images/logo.png")`).

---

## Étape 1 — `theme.json`

C'est la **carte d'identité** de ton thème. L'app lit ce fichier pour savoir comment t'appeler ton thème dans la liste, et quelles options proposer à l'utilisateur dans la section "Theme".

Exemple minimal :

```json
{
  "manifestVersion": 1,
  "id": "mon-super-theme",
  "label": "Mon super thème",
  "description": "Une petite phrase qui décrit l'ambiance.",
  "author": "ton-pseudo",
  "vars": [
    { "key": "colorPanel", "label": "Couleur de fond", "group": "Couleurs",
      "spec": { "kind": "color", "default": "#16181d" } },

    { "key": "colorWin", "label": "Couleur des victoires", "group": "Couleurs",
      "spec": { "kind": "color", "default": "#5dd16f" } },

    { "key": "colorLoss", "label": "Couleur des défaites", "group": "Couleurs",
      "spec": { "kind": "color", "default": "#ff5c5c" } }
  ]
}
```

**Ce que ça veut dire** :

- `id` : doit être identique au nom du dossier (`mon-super-theme`).
- `label` : ce que verra l'utilisateur dans le sélecteur (peut contenir des espaces, des accents, etc.).
- `description` : une phrase pour expliquer le style.
- `vars` : la liste des **réglages** que l'utilisateur pourra modifier depuis l'app. Chaque entrée crée un sélecteur de couleur (ou un interrupteur, ou un slider). À chaque changement, ton CSS reçoit la nouvelle valeur en temps réel.

### Les 3 types de réglages possibles

```json
// Une couleur (ouvre un color picker dans l'app)
{ "kind": "color", "default": "#ff5c5c" }

// Un interrupteur on/off
{ "kind": "boolean", "default": true }

// Un nombre avec un slider
{ "kind": "number", "default": 80, "min": 50, "max": 200, "step": 1, "unit": "px" }
```

### Le lien magique entre `theme.json` et `boost.css`

Chaque `key` dans `theme.json` devient automatiquement une **variable CSS** que tu utilises dans ton fichier `boost.css`. La règle est simple : `colorPanel` (en camelCase dans le JSON) devient `--color-panel` (en kebab-case dans le CSS).

Exemple :

| `theme.json` (key)   | CSS (variable)        |
|----------------------|-----------------------|
| `colorPanel`         | `var(--color-panel)`  |
| `colorWin`           | `var(--color-win)`    |
| `showIcons`          | `var(--show-icons)`   |
| `borderRadius`       | `var(--border-radius)`|

---

## Étape 2 — `boost.html`

Le **squelette** de ton overlay. Tu peux mettre ce que tu veux à l'intérieur, **sauf que 4 éléments sont obligatoires** : ce sont eux que l'app va remplir avec les vraies stats du joueur.

| ID HTML       | Ce que l'app y écrit                                                  |
|---------------|----------------------------------------------------------------------|
| `#v-streak`   | Le streak (par exemple `+3` ou `-2`)                                  |
| `#v-wins`     | Le nombre de victoires                                                |
| `#v-losses`   | Le nombre de défaites                                                 |
| `#conn`       | Un point qui devient vert (`.ok`) quand RL est connecté à l'overlay   |

Exemple super simple :

```html
<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8" />
  <link rel="stylesheet" href="./boost.css" />
</head>
<body>
  <div class="panel">
    <span class="conn-dot" id="conn"></span>

    <div>STRK <span id="v-streak">+0</span></div>
    <div>WINS <span id="v-wins">0</span></div>
    <div>LOSS <span id="v-losses">0</span></div>
  </div>

  <script type="module" src="./boost.js"></script>
</body>
</html>
```

**Tout le reste est libre** : autant de `<div>` que tu veux, des SVG, des animations CSS, des polices, des images de fond...

---

## Étape 3 — `boost.css`

L'apparence. Tout le langage CSS standard fonctionne. Pour utiliser les couleurs/options définies dans `theme.json`, tu écris `var(--nom-de-ta-variable)`.

Exemple :

```css
:root {
  /* Valeurs par défaut au cas où theme.json est cassé. Sans danger,
     l'app les écrase au runtime avec celles que tu as choisies. */
  --color-panel: #16181d;
  --color-win: #5dd16f;
  --color-loss: #ff5c5c;
}

html, body {
  margin: 0;
  background: transparent; /* IMPORTANT : transparent, sinon ça cache RL */
  font-family: "Segoe UI", sans-serif;
  color: white;
}

.panel {
  position: absolute;
  inset: 0;                         /* centré dans la fenêtre */
  margin: auto;
  width: 220px;
  height: 150px;
  background: var(--color-panel);   /* utilise la variable du theme.json */
  border-radius: 12px;
  padding: 16px;
}

#v-wins   { color: var(--color-win); }
#v-losses { color: var(--color-loss); }

.conn-dot {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--color-loss);
}
.conn-dot.ok { opacity: 0; }        /* caché quand connecté */
```

**Astuce animations** : quand un chiffre change, l'app ajoute la classe `.bump` sur l'élément pendant 240 ms. Tu peux écrire une animation dans ton CSS pour faire pulser/scaler/clignoter le chiffre :

```css
@keyframes bump {
  0%, 100% { transform: scale(1); }
  40%      { transform: scale(1.18); }
}
.bump { animation: bump 240ms ease-out; }
```

---

## Étape 4 — `boost.js`

Une seule ligne dans 99 % des cas. Copie-colle :

```js
import { startSessionOverlay } from "/overlays/shared/session-overlay.js";
startSessionOverlay();
```

Cette ligne lance la boucle qui va lire les stats du joueur toutes les secondes et remplir tes 4 éléments `#v-streak`, `#v-wins`, `#v-losses`, `#conn`.

Ne touche à ce fichier que si tu veux faire des choses très avancées (par exemple jouer un son à chaque victoire).

---

## Tester ton thème

Pendant que l'app tourne, ouvre dans ton navigateur :

```
http://localhost:49124/overlays/themes/mon-super-theme/boost.html
```

Tu vois ton thème en grand, dans Chrome/Firefox/Edge, et tu peux utiliser les **outils de développement** (touche `F12`) pour debugger ton CSS comme une page web normale. Aucune recompilation, modifie le CSS, recharge la page, c'est tout.

Pour voir tes changements dans le HUD réel : clique sur **🔄 Rafraîchir** dans l'app, puis change le thème et reviens dessus pour forcer le rechargement.

---

## Distribution

Une fois ton thème fini, il suffit de :

1. **Zipper le dossier** entier (clic droit → Envoyer vers → Dossier compressé).
2. **Partager le `.zip`** à qui tu veux (Discord, GitHub, site perso...).

Pour **installer** un thème reçu, c'est exactement le même flow que pour créer le tien : extraire le zip → glisser le dossier dans `%APPDATA%/RLStatsOverlay/themes/` → cliquer sur **🔄 Rafraîchir** dans l'app.

---

## Problèmes fréquents

- **"Mon thème n'apparaît pas dans la liste"** : vérifie que ton dossier est bien dans `%APPDATA%/RLStatsOverlay/themes/` (pas un sous-dossier), et que le fichier s'appelle exactement `theme.json`. Clique ensuite sur **🔄 Rafraîchir**.

- **"Le HUD est complètement transparent / vide"** : il y a sans doute une erreur dans ton `boost.html` ou `boost.css`. Ouvre `http://localhost:49124/overlays/themes/<ton-id>/boost.html` dans Chrome, puis F12 → onglet Console pour voir les erreurs.

- **"Mes couleurs choisies dans l'app ne se voient pas"** : vérifie que ton CSS utilise bien `var(--color-panel)` (et pas une couleur en dur comme `#16181d`). La règle de nommage : `colorPanel` dans le JSON ↔ `--color-panel` dans le CSS.

- **"Le fond de l'overlay n'est pas transparent"** : ajoute `background: transparent;` sur `body` (ou utilise une couleur avec un alpha, par exemple `rgba(0,0,0,0.5)`).

- **"Mes images custom n'apparaissent pas"** : utilise un chemin **relatif** depuis ton CSS, par exemple `url("./images/logo.png")`, pas un chemin absolu.

---

## Pour aller plus loin

Regarde le code des thèmes intégrés dans le repo GitHub :

- [`overlays/themes/circle/`](../overlays/themes/circle/) — le thème par défaut, avec un arc SVG qui épouse la jauge de boost
- [`overlays/themes/minimal/`](../overlays/themes/minimal/) — un thème dépouillé, super simple
- [`overlays/themes/redesigned/`](../overlays/themes/redesigned/) — un thème "panel-less" avec de gros chiffres

Ils sont sous licence MIT, tu peux t'en inspirer ou les remixer librement.
