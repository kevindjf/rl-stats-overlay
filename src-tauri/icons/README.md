# Icons

Placeholder icons generated from `icon.svg` (W/L logo, basic style). Replace with proper artwork before public release.

## Required formats

Tauri's bundle expects these files at build time, listed in `tauri.conf.json` :

- `32x32.png`
- `128x128.png`
- `128x128@2x.png`
- `icon.png` (≥ 512×512, source for the others)
- `icon.icns` (macOS)
- `icon.ico` (Windows)

## Regenerate everything from a single source PNG

The Tauri CLI generates all sizes/formats from one master PNG (≥ 1024×1024 recommended) :

```bash
bun install
bun run tauri icon path/to/master.png
```

This (re)writes every file in this folder, including the platform-specific `.icns` and `.ico` files which macOS `sips` and Linux `convert` cannot produce reliably.

## Current state

Generated from `icon.svg` (W/L logo) via `bun run tauri icon icon.png`. Replace `icon.png` with proper artwork (≥ 1024×1024, transparent background) and re-run the command to refresh every size/format at once.

iOS/Android variants are intentionally omitted — this project ships desktop only.
