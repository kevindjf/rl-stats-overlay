# Custom themes

The app supports community-made themes. They live alongside the built-in ones, no rebuild required.

## Step-by-step guides

- 🇫🇷 **[Créer un thème — guide complet en français](docs/themes-fr.md)**
- 🇬🇧 **[Building a theme — full English guide](docs/themes-en.md)**

The rest of this page is a quick technical reference. For a friendly walkthrough start with one of the guides above.

## For users — installing a third-party theme

1. Get a theme zip from a creator.
2. Open the app → **Settings → Theme → 📁 Dossier des thèmes** (Explorer opens).
3. Drop the unzipped folder there.
4. Click **🔄 Rafraîchir** in the same panel. The theme appears in the dropdown.

That's it. Bundled themes shipped with the app stay available; user themes can override a bundled id by sharing the same folder name.

## For designers — making a theme

Start from the [`overlays/themes/_template/`](overlays/themes/_template/) folder. The README inside walks through the 4 files (`theme.json`, `boost.html`, `boost.css`, `boost.js`) and the contract:

- 4 element ids the shared session loop targets (`#v-streak`, `#v-wins`, `#v-losses`, `#conn`)
- `vars[]` in the manifest declare which knobs appear in the settings UI (colors, booleans, numbers); each becomes a `--kebab-case` CSS custom property at runtime
- everything else is your call: any HTML structure, CSS animations, SVG, web fonts, transforms

Pure web standards — test in any browser at `http://localhost:49124/overlays/themes/<your-id>/boost.html` while the app is running.

## Manifest schema (v1)

```json
{
  "manifestVersion": 1,
  "id": "my-theme",                        // matches the folder name
  "label": "My Theme",                     // shown in the dropdown
  "description": "One-liner.",
  "author": "your-name",                   // optional
  "vars": [
    { "key": "colorPanel", "label": "Background", "group": "Colors",
      "spec": { "kind": "color", "default": "#16181d" } }
  ]
}
```

When the manifest version evolves, older themes keep working — the loader is forward-compatible by design.
