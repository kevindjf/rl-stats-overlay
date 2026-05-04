# Architecture

Repo overview + data flow for contributors.

## Repo structure

```
rl-stats-overlay/
├── README.md                 # End-user docs (FR)
├── README.en.md              # End-user docs (EN)
├── LICENSE                   # MIT
├── mkdocs.yml                # Doc site config (MkDocs Material)
├── docs/                     # User docs, hosted on GH Pages
│   ├── index.md              # Landing
│   ├── install.md            # Install + SmartScreen + wizard
│   ├── hud/                  # In-game HUD, edit mode, …
│   ├── obs/                  # OBS Browser Source
│   ├── settings/             # Settings window tour
│   ├── themes/               # Bundled themes + designer guide
│   ├── troubleshooting.md
│   ├── reference/            # Stats API offline mirror
│   └── contributing/         # Dev + architecture
├── src-tauri/                # Rust backend + Tauri config
│   ├── Cargo.toml            # Rust deps
│   ├── tauri.conf.json       # App + windows + bundle config
│   ├── capabilities/         # Tauri 2 permissions
│   ├── icons/
│   └── src/
│       ├── main.rs           # Entry point
│       ├── lib.rs            # Tauri commands + bootstrap
│       ├── state.rs          # AppState (Arc<Mutex>)
│       ├── settings.rs       # JSON persistence in %APPDATA%
│       ├── session.rs        # W/L/streak logic
│       ├── ini_patcher.rs    # RL detection + DefaultStatsAPI.ini patch
│       ├── ws_client.rs      # WebSocket client to ws://localhost:49123
│       └── http_server.rs    # axum, serves overlays + /api/config + /hud/*
├── src/                      # Settings UI (main Tauri webview)
│   ├── index.html
│   ├── main.ts               # Vanilla TS, renders dashboard or wizard
│   ├── i18n.ts               # FR/EN catalog
│   └── style.css
├── overlays/                 # Served statically by axum, embedded via rust-embed
│   ├── themes/
│   │   ├── _shared/          # menu.js (drag/resize/scale) + menu.css
│   │   ├── circle/           # Circle theme
│   │   ├── default/          # Default theme
│   │   ├── minimal/          # Minimal theme
│   │   ├── redesigned/       # Redesigned theme
│   │   └── _template/        # Skeleton for custom themes
│   ├── shared/
│   │   └── ws-client.js      # Shared WS reconnect logic for all themes
│   └── launcher/             # Floating launcher
├── dev/                      # Dev tooling — excluded from prod build
│   ├── mock-server.ts        # Mock Stats API (Bun)
│   ├── mock-control.html     # Fake-match control panel
│   └── README.md
├── .github/workflows/
│   ├── build.yml             # CI Windows + macOS on push/PR
│   ├── release.yml           # GitHub Release on tag v*
│   └── docs.yml              # Build + deploy MkDocs to gh-pages
├── package.json
├── tsconfig.json
├── vite.config.ts
└── .gitignore
```

## Diagram

```
┌────────────────────────────────────────────────────────────┐
│ Tauri app (Rust binary)                                    │
│                                                            │
│   ┌──────────────┐   ┌─────────────────┐                   │
│   │ Settings UI  │   │ In-game HUD     │                   │
│   │ (Vite webview│   │ (webview, loads │                   │
│   │  on /)       │   │  http://…)      │                   │
│   └──────┬───────┘   └────────┬────────┘                   │
│          │ invoke()           │ HTTP POST /hud/*           │
│   ┌──────▼────────────────────▼──────┐                     │
│   │      lib.rs — Tauri commands     │                     │
│   │      + state.rs (AppState)       │                     │
│   └──────┬──────────────────────┬────┘                     │
│          │                      │                          │
│   ┌──────▼─────────┐   ┌────────▼──────────┐               │
│   │ ws_client.rs   │   │ http_server.rs    │               │
│   │ → ws://:49123  │   │ axum on :49124    │               │
│   │   (Stats API)  │   │ - /overlays/*     │               │
│   │                │   │ - /api/config     │               │
│   │                │   │ - /hud/start-drag │               │
│   │                │   │ - /hud/start-resize                │
│   └────────────────┘   └───────────────────┘               │
│                                                            │
│   Persistence: %APPDATA%/RLStatsOverlay/settings.json      │
└────────────────────────────────────────────────────────────┘
                          │
            ws://:49123   │   http://:49124
                          ▼
                  Rocket League (Stats API)
                          ┊
                  In dev:
                  Bun dev/mock-server.ts
```

## Key notes

- **No listening port for the Stats API**: the app is a *client* of the `ws://localhost:49123` WebSocket (Rocket League is the server, exposed via the official Stats API).
- **`http_server.rs` (axum) on `:49124`** serves:
  - overlay assets (HTML/CSS/JS embedded via `rust-embed`),
  - `/api/config` (name, primary_id, active theme),
  - HUD routes (`/hud/start-drag`, `/hud/start-resize`, `/hud/toggle-lock`, `/session/reset`, `/app/quit`) called via POST by the overlay JS (which doesn't have access to `window.__TAURI__` because it's loaded over plain HTTP).
- The **in-game HUD** is a secondary Tauri window that loads `http://localhost:49124/overlays/boost.html` — the same URL copied for OBS. Both surfaces display the same thing.
- The browser-side **`localStorage`** is used for UI cache (collapsible-section state); the **source of truth** stays on the Rust side (`settings.json`).
- **Debug mode**: `rust-embed` reads overlays from disk, so you can edit `overlays/themes/<name>/*.css` and reload the HUD to see the result without rebuilding Rust.
