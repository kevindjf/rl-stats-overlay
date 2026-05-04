# RL Stats Overlay

> **Rocket League overlay for OBS and in-game HUD.** Live session wins, losses and streak in real time. Easy Anti-Cheat compatible — uses only the **official Psyonix Stats API**, no injection.

![Overlay preview](images/preview.png){ loading=lazy }

## At a glance

RL Stats Overlay shows your live **wins / losses / streak** on top of Rocket League or inside an OBS scene. Counters update on their own at the end of every match — no manual input.

## Why RL Stats Overlay

- **EAC-safe**: no injection, no memory reads. The app only reads the official Stats API the game exposes on `ws://localhost:49123` — the same one RLCS broadcasters use.
- **Two surfaces, one app**: transparent in-game HUD **and** OBS Browser Source, driven by the same session.
- **Guided setup**: auto-detection of Rocket League (Steam/Epic), auto-activation of the Stats API, auto-detection of your account. No command line.
- **Live in-match stats**: goals, saves, shots, assists, updated on every tick.
- **Multiple themes** ready to use (and you can author your own — see [Designer guide](themes/designer-guide.md)).

## Get started in 3 minutes

<div class="grid cards" markdown>

- :material-download:{ .lg } **[Install](install.md)**

    Download, install, follow the first-launch wizard.

- :material-monitor:{ .lg } **[In-game HUD](hud/index.md)**

    Show the transparent HUD over Rocket League.

- :material-broadcast:{ .lg } **[OBS Browser Source](obs/index.md)**

    Paste the URL into an OBS scene for streaming.

</div>

## Useful links

- 💾 [Releases (download)](https://github.com/kevindjf/rl-stats-overlay/releases/latest)
- 🐛 [Report a bug / request a feature](https://github.com/kevindjf/rl-stats-overlay/issues)
- 🛠 [Source code](https://github.com/kevindjf/rl-stats-overlay)
- 📜 [MIT License](https://github.com/kevindjf/rl-stats-overlay/blob/main/LICENSE)
