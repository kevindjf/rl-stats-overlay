# Theme template

Starting point for a custom RL Stats Overlay theme. Copy this folder and rename it.

## TL;DR

1. Click **📁 Dossier des thèmes** in the app's settings → Theme section. Explorer opens at `%APPDATA%/RLStatsOverlay/themes/`.
2. Drop a copy of this `_template/` folder there. Rename it (the folder name becomes the theme id; avoid spaces).
3. Edit the 4 files inside.
4. Click **🔄 Rafraîchir** in the same Theme section. Your theme appears in the dropdown.

No rebuild, no Rust, no TypeScript.

## Files

```
my-theme/
├── theme.json   ← required: manifest (label, description, vars editable in the UI)
├── boost.html   ← required: markup (must include the 4 ids documented below)
├── boost.css    ← required: styling
└── boost.js     ← required: usually one line — `import …/session-overlay.js; startSessionOverlay();`
```

Optional: `screenshot.png`, custom fonts in `fonts/`, images in `images/` — referenced via relative paths from `boost.html` / `boost.css`.

## DOM contract

`session-overlay.js` (the shared loop) targets these ids — your `boost.html` MUST include them:

| id          | Purpose                                                  |
|-------------|----------------------------------------------------------|
| `#v-streak` | Streak text (`+3`, `-2`, etc.)                           |
| `#v-wins`   | Wins integer                                             |
| `#v-losses` | Losses integer                                           |
| `#conn`     | Connection dot — gains a `.ok` class while RL is connected |

Anything else (extra elements, classes, structure) is yours.

## Theme variables

Each entry in `theme.json` `vars[]` becomes:
- a control in the settings UI (color picker / slider / toggle)
- a CSS custom property on `:root` at runtime

Naming: `camelCase` keys map to `--kebab-case` CSS vars. `colorPanel` → `--color-panel`.

Spec kinds:

```json
{ "kind": "color",   "default": "#16181d" }
{ "kind": "boolean", "default": true }
{ "kind": "number",  "default": 80, "min": 50, "max": 200, "step": 1, "unit": "px" }
```

## Testing locally

While the app is running you can open `http://localhost:49124/overlays/themes/<your-id>/boost.html` directly in a browser to see your theme — no need to set it active in the app first.

## Distribution

Zip the folder and share the link. Anyone drops it in `%APPDATA%/RLStatsOverlay/themes/`, hits Refresh, done.
