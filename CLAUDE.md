# RL Stats Overlay — Project Notes for Claude

## What this app is

Tauri 2 desktop app (Rust backend + Vite/TypeScript frontend) that turns
Rocket League's official Stats API into a session overlay (wins / losses /
streak), with two consumption surfaces:

- **In-game HUD** — a transparent always-on-top Tauri window placed over the
  game while the user plays.
- **OBS Browser Source** — an embedded HTTP server (axum) on `localhost:49124`
  exposes the same overlay as a webpage for OBS streamers.

The Rust backend connects to the **official Psyonix Stats API** at
`ws://localhost:49123` (no injection, EAC-safe) and rebroadcasts session
state to both surfaces.

## Stats API reference (read this before touching match logic)

A clean, offline-readable copy of the official Rocket League Stats API
documentation lives at:

- **[`docs/stats-api-reference.md`](docs/stats-api-reference.md)**

Use that file in priority — do not WebFetch the online doc on every call.
It is fetched from `https://www.rocketleague.com/en/developer/stats-api`
and includes connection details, configuration, the `UpdateState` tick
payload, and the full schema for every event (BallHit, MatchCreated,
MatchEnded, GoalScored, StatfeedEvent, etc.).

If you suspect Psyonix changed the API since the cached version, re-fetch
the page and regenerate the markdown — the source URL and fetch date are
in the file's header.

## Repo layout cheat-sheet

- `src-tauri/src/lib.rs` — Tauri entry point, plugin registration, command handlers, log setup
- `src-tauri/src/settings.rs` — JSON settings persistence (`%APPDATA%\RLStatsOverlay\settings.json`)
- `src-tauri/src/state.rs` — in-memory app state, session counters
- `src-tauri/src/ini_patcher.rs` — Steam/Epic install detection + `DefaultStatsAPI.ini` patching
- `src/main.ts` — settings UI (Tauri window)
- `src/i18n.ts` — flat-catalog FR/EN i18n with `{var}` interpolation
- `overlays/themes/<name>/` — overlay HTML/CSS/theme.json bundles served by the embedded HTTP server
- `docs/development.md` — dev workflow (Bun + Tauri + mock server)

## Tooling

- **Bun** is the package manager and runtime. Use `bun install`, `bun run tauri dev`.
- **Rust** is built via `cargo` through `tauri-cli`. The frontend runs on Vite.
- Releases are produced by `.github/workflows/release.yml` using `tauri-apps/tauri-action@v0`,
  triggered by tags matching `v*`.
