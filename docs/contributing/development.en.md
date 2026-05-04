# Development

## Stack

- **Tauri 2** (Rust + WebView) for the desktop app
- **Vanilla TS + Vite** for the settings window
- **Vanilla JS** for the overlays (served statically)
- **Bun** for dev scripts and the mock server

## Prerequisites

| Tool | Version | Why |
|------|---------|-----|
| **Rust** | 1.77+ stable | Tauri backend |
| **Bun** | 1.3+ | Frontend tooling, mock server |
| **Tauri CLI** | 2.x | Run / build |
| **WebView2** | latest | Windows webview (already on W11) |

On Windows:

```powershell
winget install -e --id Rustlang.Rustup
winget install -e --id Oven-sh.Bun
```

On macOS (dev only, no Stats API support):

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

## Run in dev

### Full mode (with mock Stats API)

Two terminals:

```bash
# 1. Mock Rocket League Stats API (port 49123)
bun run dev/mock-server.ts

# 2. Tauri app (settings UI + overlay HTTP server + WS client)
bun run tauri dev
```

In a browser, open <http://localhost:49123/control> to drive a fake match (start, +1 goal, finish Win/Loss). The Tauri app receives events as if they came from the game.

### Overlay-only mode (quick debug)

If you just want to iterate on the boost overlay:

```bash
bun run dev/mock-server.ts
```

Then open <http://localhost:49123/overlays/boost.html> directly in Chrome. No Tauri needed.

## Useful commands

| Command | Effect |
|---------|--------|
| `bun run tauri:dev` | Run the Tauri app in dev mode (UI hot reload) |
| `bun run tauri:build` | Build .exe + .msi (Windows) or .app + .dmg (macOS) |
| `bun run mock` | Run the mock Stats API |
| `bun run dev` | Run Vite alone (settings UI only) |
| `bun run build` | Build the Vite frontend |

> 📐 For the **repo structure** + **architecture diagram**, see [Architecture](architecture.md).

## Tests

For now, validation is manual:

1. Run the mock + the app
2. Open the app, set name "TestPlayer", finish the wizard
3. In the mock control, start a match → the app turns green
4. Finish Win → wins=1, streak=W1
5. Finish Loss → losses=1, streak=L1
6. Reset session → everything zeroes out

For Rust unit tests (coming), `cargo test --manifest-path src-tauri/Cargo.toml`.

## Contributing

PRs welcome! Conventions:

- **Commits** in English, conventional commits (`feat:`, `fix:`, `docs:`, `chore:`)
- **Code** in English (comments, identifiers)
- **User-facing docs** in French *and* English (the README + `/docs` site are bilingual)
- Run `cargo fmt --manifest-path src-tauri/Cargo.toml` and `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` before pushing
