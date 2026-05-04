# Building a custom theme

> French version: see [`themes-fr.md`](themes-fr.md).

This guide walks you through making your own RL Stats Overlay theme, testing it, and sharing it. You don't need to be a developer — basic HTML/CSS knowledge and the ability to edit a text file are enough.

**You do NOT need to**: build the project, install Rust, touch GitHub, or read any TypeScript. Drop a folder, hit a button, your theme appears.

---

## 5-minute walkthrough

1. **Launch** the RL Stats Overlay app.
2. Open the **Theme** section in settings.
3. Click **📁 Dossier des thèmes**. Windows Explorer opens at `%APPDATA%/RLStatsOverlay/themes/`.
4. From the project's GitHub repo, copy the [`overlays/themes/_template/`](../overlays/themes/_template/) folder and paste it where Explorer just opened.
5. Rename it (e.g. `my-cool-theme`). **Important**: no spaces, no accented chars, no special chars — only letters, digits, dashes or underscores.
6. Switch back to the app and click **🔄 Rafraîchir**. Your theme shows up in the dropdown.
7. Pick it. The HUD updates immediately.

That's it. Now you just edit the files to make it look the way you want.

---

## What's inside a theme

A theme is a folder with **4 required files**:

```
my-cool-theme/
├── theme.json   ← theme metadata + the colors/options the user can edit in-app
├── boost.html   ← markup (the boxes, the text slots)
├── boost.css    ← styling (colors, fonts, sizes, layout)
└── boost.js     ← one line, don't worry about it
```

You can also add (optional):
- `screenshot.png` — a preview image for your theme
- `fonts/` — a folder with custom fonts (`.ttf`, `.otf`, `.woff2`)
- `images/` — a folder with custom images

You then reference them from your CSS via a relative path (e.g. `url("./images/logo.png")`).

---

## Step 1 — `theme.json`

This is your theme's **identity card**. The app reads it to know your theme's display name, and which knobs to expose under the "Theme" section in the settings.

Minimal example:

```json
{
  "manifestVersion": 1,
  "id": "my-cool-theme",
  "label": "My Cool Theme",
  "description": "A short sentence describing the look.",
  "author": "your-handle",
  "vars": [
    { "key": "colorPanel", "label": "Background colour", "group": "Colours",
      "spec": { "kind": "color", "default": "#16181d" } },

    { "key": "colorWin", "label": "Wins colour", "group": "Colours",
      "spec": { "kind": "color", "default": "#5dd16f" } },

    { "key": "colorLoss", "label": "Losses colour", "group": "Colours",
      "spec": { "kind": "color", "default": "#ff5c5c" } }
  ]
}
```

**What it means**:

- `id`: must match the folder name exactly (`my-cool-theme`).
- `label`: what the user will see in the dropdown (free text — spaces, accents, emoji all OK).
- `description`: a one-liner explaining the style.
- `vars`: the list of **knobs** the user can tweak from the app. Each entry produces a colour picker (or a switch, or a slider). Whenever the user edits one, your CSS receives the new value live.

### The 3 knob kinds

```json
// A colour (opens a colour picker in the app)
{ "kind": "color", "default": "#ff5c5c" }

// An on/off switch
{ "kind": "boolean", "default": true }

// A number with a slider
{ "kind": "number", "default": 80, "min": 50, "max": 200, "step": 1, "unit": "px" }
```

### The magic link between `theme.json` and `boost.css`

Each `key` in `theme.json` becomes a **CSS custom property** you can use from `boost.css`. The naming rule is dead simple: `colorPanel` (camelCase in JSON) becomes `--color-panel` (kebab-case in CSS).

Examples:

| `theme.json` (key)   | CSS (variable)        |
|----------------------|-----------------------|
| `colorPanel`         | `var(--color-panel)`  |
| `colorWin`           | `var(--color-win)`    |
| `showIcons`          | `var(--show-icons)`   |
| `borderRadius`       | `var(--border-radius)`|

---

## Step 2 — `boost.html`

The **skeleton** of your overlay. You can put whatever you want inside — **except 4 elements that are required**: those are the slots the app fills with the actual player stats.

| HTML id       | What the app writes there                                                |
|---------------|--------------------------------------------------------------------------|
| `#v-streak`   | The streak (e.g. `+3` or `-2`)                                           |
| `#v-wins`     | The wins count                                                           |
| `#v-losses`   | The losses count                                                         |
| `#conn`       | A dot that gets `.ok` class added when RL is talking to the overlay      |

Dead-simple example:

```html
<!doctype html>
<html lang="en">
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

**Everything else is yours**: as many `<div>`s as you want, SVGs, CSS animations, web fonts, background images — anything modern browsers support.

---

## Step 3 — `boost.css`

The look. All standard CSS works. To use the colours/options you declared in `theme.json`, write `var(--your-variable-name)`.

Example:

```css
:root {
  /* Defaults in case theme.json is broken. Harmless — the app overrides
     them at runtime with whatever the user picked. */
  --color-panel: #16181d;
  --color-win: #5dd16f;
  --color-loss: #ff5c5c;
}

html, body {
  margin: 0;
  background: transparent; /* IMPORTANT: transparent, otherwise it hides the game */
  font-family: "Segoe UI", sans-serif;
  color: white;
}

.panel {
  position: absolute;
  inset: 0;                         /* centred in the window */
  margin: auto;
  width: 220px;
  height: 150px;
  background: var(--color-panel);   /* uses your theme.json variable */
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
.conn-dot.ok { opacity: 0; }        /* hidden when connected */
```

**Animation tip**: when a number changes, the app adds a `.bump` class on the element for 240 ms. Hook a CSS animation onto that class to make values pulse/scale/flash on every update:

```css
@keyframes bump {
  0%, 100% { transform: scale(1); }
  40%      { transform: scale(1.18); }
}
.bump { animation: bump 240ms ease-out; }
```

---

## Step 4 — `boost.js`

A single line in 99% of cases. Just copy this:

```js
import { startSessionOverlay } from "/overlays/shared/session-overlay.js";
startSessionOverlay();
```

This boots the loop that polls player stats every second and fills your 4 elements `#v-streak`, `#v-wins`, `#v-losses`, `#conn`.

Only touch this file if you want to do something fancy (e.g. play a sound on every win).

---

## Testing your theme

While the app is running, open in your browser:

```
http://localhost:49124/overlays/themes/my-cool-theme/boost.html
```

You'll see your theme full-size in Chrome/Firefox/Edge and you can use the **dev tools** (`F12`) to debug your CSS like any web page. No rebuild, edit the CSS, refresh, done.

To see your changes in the actual HUD: hit **🔄 Rafraîchir** in the app, then switch the active theme away and back to force a reload.

---

## Sharing

When your theme is ready, just:

1. **Zip the whole folder** (right-click → Send to → Compressed folder).
2. **Share the `.zip`** wherever you want — Discord, GitHub, your own site...

To **install** a theme someone shared: same flow as creating one — extract the zip → drop the folder into `%APPDATA%/RLStatsOverlay/themes/` → click **🔄 Rafraîchir** in the app.

---

## Common issues

- **"My theme isn't showing up"**: make sure your folder is directly in `%APPDATA%/RLStatsOverlay/themes/` (not a sub-folder), and that the file is exactly named `theme.json`. Then click **🔄 Rafraîchir**.

- **"The HUD is fully transparent / empty"**: probably a typo in `boost.html` or `boost.css`. Open `http://localhost:49124/overlays/themes/<your-id>/boost.html` in Chrome, then `F12` → Console to see errors.

- **"My in-app colour picks don't show up"**: check that your CSS reads `var(--color-panel)` (not a hard-coded `#16181d`). The naming rule: `colorPanel` in JSON ↔ `--color-panel` in CSS.

- **"My overlay background isn't transparent"**: add `background: transparent;` on `body` (or use a colour with alpha, e.g. `rgba(0,0,0,0.5)`).

- **"My custom images don't show"**: use a **relative** path from your CSS, e.g. `url("./images/logo.png")`, not an absolute one.

---

## Going further

Read the code of the bundled themes in the repo:

- [`overlays/themes/circle/`](../overlays/themes/circle/) — the default theme, with an SVG arc hugging the boost gauge
- [`overlays/themes/minimal/`](../overlays/themes/minimal/) — a stripped-down, super-clean theme
- [`overlays/themes/redesigned/`](../overlays/themes/redesigned/) — a panel-less theme with big bold numbers

They're MIT-licensed; remix or fork them freely.
