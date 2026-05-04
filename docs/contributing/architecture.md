# Architecture

Vue d'ensemble du dépôt et du flux de données pour les contributeurs.

## Structure du repo

```
rl-stats-overlay/
├── README.md                 # Doc utilisateur final (FR)
├── README.en.md              # Doc utilisateur final (EN)
├── LICENSE                   # MIT
├── mkdocs.yml                # Config du site doc (MkDocs Material)
├── docs/                     # Doc utilisateur, hostée sur GH Pages
│   ├── index.md              # Landing
│   ├── install.md            # Install + SmartScreen + wizard
│   ├── hud/                  # HUD in-game, mode édition, …
│   ├── obs/                  # Browser Source OBS
│   ├── settings/             # Tour de la fenêtre Settings
│   ├── themes/               # Thèmes inclus + designer guide
│   ├── troubleshooting.md
│   ├── reference/            # Stats API offline mirror
│   └── contributing/         # Dev + architecture
├── src-tauri/                # Backend Rust + config Tauri
│   ├── Cargo.toml            # Dépendances Rust
│   ├── tauri.conf.json       # Config app + fenêtres + bundle
│   ├── capabilities/         # Permissions Tauri 2
│   ├── icons/
│   └── src/
│       ├── main.rs           # Entry point
│       ├── lib.rs            # Tauri commands + bootstrap
│       ├── state.rs          # AppState (Arc<Mutex>)
│       ├── settings.rs       # Persistance JSON dans %APPDATA%
│       ├── session.rs        # Logique W/L/streak
│       ├── ini_patcher.rs    # Détection RL + patch DefaultStatsAPI.ini
│       ├── ws_client.rs      # Client WebSocket vers ws://localhost:49123
│       └── http_server.rs    # axum, sert overlays + /api/config + /hud/*
├── src/                      # Frontend settings UI (Tauri webview principal)
│   ├── index.html
│   ├── main.ts               # Vanilla TS, render dashboard ou wizard
│   ├── i18n.ts               # Catalogue FR/EN
│   └── style.css
├── overlays/                 # Servis statiquement par axum, embarqués via rust-embed
│   ├── themes/
│   │   ├── _shared/          # menu.js (drag/resize/scale) + menu.css
│   │   ├── circle/           # Theme Circle
│   │   ├── default/          # Theme Default
│   │   ├── minimal/          # Theme Minimal
│   │   ├── redesigned/       # Theme Redesigned
│   │   └── _template/        # Squelette pour custom themes
│   ├── shared/
│   │   └── ws-client.js      # Logique reconnect WS partagée par tous les thèmes
│   └── launcher/             # Bouton flottant
├── dev/                      # Outils dev — exclus du build prod
│   ├── mock-server.ts        # Mock Stats API (Bun)
│   ├── mock-control.html     # Panneau de pilotage faux match
│   └── README.md
├── .github/workflows/
│   ├── build.yml             # CI Windows + macOS sur push/PR
│   ├── release.yml           # GitHub Release sur tag v*
│   └── docs.yml              # Build + deploy MkDocs sur gh-pages
├── package.json
├── tsconfig.json
├── vite.config.ts
└── .gitignore
```

## Diagramme

```
┌────────────────────────────────────────────────────────────┐
│ Tauri app (Rust binary)                                    │
│                                                            │
│   ┌──────────────┐   ┌─────────────────┐                   │
│   │ Settings UI  │   │ In-game HUD     │                   │
│   │ (Vite webview│   │ (Webview, loads │                   │
│   │  sur /)      │   │  http://…)      │                   │
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
                  En dev :
                  Bun dev/mock-server.ts
```

## Notes clés

- **Pas de port en écoute pour la Stats API** : l'app est *cliente* du WebSocket `ws://localhost:49123` (Rocket League est le serveur, exposé via la Stats API officielle).
- **`http_server.rs` (axum) sur `:49124`** sert :
  - les assets des overlays (HTML/CSS/JS embarqués via `rust-embed`),
  - `/api/config` (pseudo, primary_id, theme actif),
  - les routes HUD (`/hud/start-drag`, `/hud/start-resize`, `/hud/toggle-lock`, `/session/reset`, `/app/quit`) appelées en POST par le JS de l'overlay (qui n'a pas accès à `window.__TAURI__` parce qu'il est chargé en plain HTTP).
- Le **HUD in-game** est une fenêtre Tauri secondaire qui charge `http://localhost:49124/overlays/boost.html` — la même URL que celle copiée pour OBS. Les deux surfaces affichent la même chose.
- Le **`localStorage`** côté navigateur sert au cache UI (état des sections repliables) ; la **source de vérité** reste côté Rust (`settings.json`).
- **Mode debug** : `rust-embed` lit les overlays depuis le disque, donc tu peux éditer `overlays/themes/<name>/*.css` et reload le HUD pour voir le résultat sans rebuild Rust.
