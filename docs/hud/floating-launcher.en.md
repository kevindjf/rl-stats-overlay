# Floating launcher

The **floating launcher** is a small clickable circle pinned to the left edge of your primary monitor. One click opens the **RL Stats Overlay** settings window.

## What it's for

During gameplay, neither the settings window nor a taskbar icon is visible. The floating launcher gives you a **one-click access** to:

- reset a session
- turn on edit mode to reposition the HUD
- switch themes
- cleanly quit the app

## Appearance

- Small ~56 px circle (in logical pixels, adjusted for your monitor's DPI).
- Always on top, doesn't capture focus.
- Vertically centered on the left edge of the primary monitor.

## Enable / disable

In the **Appearance** section: tick / untick **"Show floating launcher"**. Persisted across restarts.

## Auto-hide during a match

The launcher hides itself **as soon as a match starts** in Rocket League (Stats API `MatchInitialized` / `MatchCreated` event) and **reappears at match end** (`MatchDestroyed`).

This behavior is non-configurable by design — a floating circle on stream during a match would be visually intrusive.
