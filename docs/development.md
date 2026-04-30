# Development

## Stack

- **Tauri 2** (Rust + Webview) pour l'app desktop
- **Vanilla TS + Vite** pour la fenêtre settings
- **Vanilla JS** pour les overlays (servis statiquement)
- **Bun** pour les scripts dev et le mock server

## Prérequis

| Outil | Version | Pourquoi |
|-------|---------|----------|
| **Rust** | 1.77+ stable | Backend Tauri |
| **Bun** | 1.3+ | Frontend tooling, mock server |
| **Tauri CLI** | 2.x | Lancement / build |
| **WebView2** | dernière | Webview Windows (déjà installé sur W11) |

Sous Windows :

```powershell
winget install -e --id Rustlang.Rustup
winget install -e --id Oven-sh.Bun
```

Sous macOS (dev seulement, pas de support Stats API) :

```bash
brew install rustup-init bun
rustup-init -y
```

## Setup

```bash
git clone https://github.com/kevindjf/rl-stats-overlay.git
cd rl-stats-overlay
bun install
```

## Lancer en dev

### Mode complet (avec mock Stats API)

Trois terminaux :

```bash
# 1. Mock Rocket League Stats API (port 49123)
bun run dev/mock-server.ts

# 2. App Tauri (settings UI + HTTP server overlays + WS client)
bun run tauri dev
```

Dans un navigateur, ouvre <http://localhost:49123/control> pour piloter un faux match (start, +1 goal, finir Win/Loss). L'app Tauri reçoit les events comme s'ils venaient du jeu.

### Mode overlay-only (debug rapide)

Si tu veux juste itérer sur le boost overlay :

```bash
bun run dev/mock-server.ts
```

Puis ouvre <http://localhost:49123/overlays/boost.html> directement dans Chrome. Pas besoin de Tauri.

## Structure du repo

```
rl-stats-overlay/
├── README.md                 # Doc utilisateur final
├── LICENSE                   # MIT
├── docs/                     # Docs supplémentaires
├── src-tauri/                # Backend Rust + config Tauri
│   ├── Cargo.toml            # Dépendances Rust
│   ├── tauri.conf.json       # Config app + fenêtres + bundle
│   ├── capabilities/         # Permissions Tauri 2
│   ├── icons/                # Icônes (placeholder)
│   └── src/
│       ├── main.rs           # Entry point
│       ├── lib.rs            # Tauri commands + bootstrap
│       ├── state.rs          # AppState (Arc<Mutex>)
│       ├── settings.rs       # Persistance JSON dans %APPDATA%
│       ├── session.rs        # Logique W/L/streak
│       ├── ini_patcher.rs    # Détection RL + patch DefaultStatsAPI.ini
│       ├── ws_client.rs      # Client WebSocket vers ws://localhost:49123
│       └── http_server.rs    # axum, sert overlays + /api/config
├── src/                      # Frontend settings UI
│   ├── index.html
│   ├── main.ts               # Vanilla TS, render dashboard ou wizard
│   └── style.css
├── overlays/                 # Overlays HTML/CSS/JS bundle dans le binaire
│   ├── boost.html
│   ├── boost.css
│   ├── boost.js
│   └── shared/
│       └── ws-client.js      # Logique reconnect WS partagée
├── dev/                      # Outils dev — exclus du build prod
│   ├── mock-server.ts        # Mock Stats API (Bun)
│   ├── mock-control.html     # Panneau de pilotage faux match
│   └── README.md
├── .github/workflows/
│   ├── build.yml             # CI Windows + macOS sur push/PR
│   └── release.yml           # GitHub Release sur tag v*
├── package.json
├── tsconfig.json
├── vite.config.ts
└── .gitignore
```

## Commandes utiles

| Commande | Effet |
|----------|-------|
| `bun run tauri:dev` | Lance l'app Tauri en mode dev (hot reload UI) |
| `bun run tauri:build` | Build .exe + .msi (Windows) ou .app + .dmg (macOS) |
| `bun run mock` | Lance le mock Stats API |
| `bun run dev` | Lance Vite seul (settings UI uniquement) |
| `bun run build` | Build le frontend Vite |

## Architecture en quelques mots

```
┌────────────────────────────────────────────────────────────┐
│ Tauri app (Rust binary)                                    │
│                                                            │
│   ┌──────────────┐   ┌─────────────────┐                   │
│   │ Settings UI  │   │ In-game HUD     │                   │
│   │ (Vite Webview│   │ (Webview, loads │                   │
│   │  on /)       │   │  http://...)    │                   │
│   └──────┬───────┘   └────────┬────────┘                   │
│          │ invoke()           │ HTTP                       │
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
│   └────────────────┘   └───────────────────┘               │
│                                                            │
│   Persistence: %APPDATA%/RLStatsOverlay/settings.json      │
└────────────────────────────────────────────────────────────┘
                          │
            ws://:49123   │   http://:49124
                          ▼
                  Rocket League (Stats API)
                          ┊
                  Or, in dev:
                  Bun mock-server.ts
```

- L'app **n'écoute pas** les ports — elle est *cliente* du WebSocket Stats API et *serveuse* HTTP pour les overlays.
- Le **HUD in-game** est une fenêtre Tauri secondaire qui charge `http://localhost:49124/overlays/boost.html` — la même URL que celle copiée pour OBS.
- L'**OBS browser source** charge la même URL — donc les deux affichages sont identiques.
- Le **localStorage** côté navigateur sert à éviter de perdre la session si l'app crash, mais la **vérité** reste côté Rust (`settings.json`).

## Tests

Pour l'instant, la validation est manuelle :

1. Lance le mock + l'app
2. Ouvre l'app, configure un pseudo "TestPlayer", coche bien le wizard
3. Dans le mock control, démarre un match → l'app passe au vert
4. Finis Win → wins=1, streak=W1
5. Finis Loss → losses=1, streak=L1
6. Reset session → tout repart à zéro

Pour les tests Rust unitaires (à venir), `cargo test --manifest-path src-tauri/Cargo.toml`.

## Contribuer

PRs bienvenues ! Conventions :

- **Commits** en anglais, conventional commits (`feat:`, `fix:`, `docs:`, `chore:`)
- **Code** en anglais (commentaires, identifiants)
- **Doc utilisateur** en français
- Lance `cargo fmt --manifest-path src-tauri/Cargo.toml` et `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` avant de push
