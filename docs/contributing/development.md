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

## Commandes utiles

| Commande | Effet |
|----------|-------|
| `bun run tauri:dev` | Lance l'app Tauri en mode dev (hot reload UI) |
| `bun run tauri:build` | Build .exe + .msi (Windows) ou .app + .dmg (macOS) |
| `bun run mock` | Lance le mock Stats API |
| `bun run dev` | Lance Vite seul (settings UI uniquement) |
| `bun run build` | Build le frontend Vite |

> 📐 Pour la **structure du repo** + le **diagramme d'architecture**, voir [Architecture](architecture.md).

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
