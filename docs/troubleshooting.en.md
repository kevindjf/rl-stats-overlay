# Troubleshooting

## Windows SmartScreen shows "Windows protected your PC"

That's expected for an unsigned open-source project. Microsoft requires a paid code-signing certificate (~€150/year) to make the dialog go away. For v1, we live without one.

**Workaround**:

1. On the SmartScreen dialog, click **More info** (the link under the title)
2. A **Run anyway** button appears → click it
3. The app's reputation improves over time as more downloads happen; the dialog will eventually stop appearing

If you want to verify the binary is clean, submit it to [VirusTotal](https://www.virustotal.com) — the app is small and open-source, full source available in this repo.

---

## The overlay stays empty / "Waiting for game" even after Rocket League is launched

Check in this order:

### 1. Is the Stats API enabled?

Open `<RL folder>\TAGame\Config\DefaultStatsAPI.ini` and check it contains:

```ini
[TAGame.MatchStatsExporter_TA]
PacketSendRate=30
Port=49123
```

If not, re-open the app's wizard (you can delete `%APPDATA%\RLStatsOverlay\settings.json` to force it).

### 2. Did you **restart** Rocket League after the patch?

The game reads the `.ini` file at startup. Any change requires a full restart (not just back-to-menu).

### 3. Is port 49123 already in use?

Another app may be squatting the port (an old BakkesMod, another overlay…).

```powershell
# PowerShell — who's listening on 49123?
Get-NetTCPConnection -LocalPort 49123
```

If a PID shows up, another process is preventing RL from using that port. Terminate it, or change the port in the `.ini` (and open an issue, we'll adapt the app).

---

## The in-game HUD doesn't show on top of Rocket League

**Most common cause**: Rocket League is running in **exclusive fullscreen**. The OS doesn't render always-on-top windows over apps in exclusive fullscreen.

**Fix**: switch to **borderless fullscreen** in Rocket League.

1. In RL: **Settings → Video → Window Mode**
2. Pick **Borderless**
3. Apply → the HUD will appear on top of the game

> 💡 Borderless mode has no perceptible perf impact on most modern machines and gives you the huge benefit of being able to switch between windows instantly (Discord, OBS, etc.) without crashes.

---

## The in-game HUD is click-through, I can't move it

That's intentional — the HUD is **click-through** by default so it doesn't get in the way during gameplay.

To reposition it:

1. Open the settings window (system tray icon or the floating launcher).
2. Expand the **In-game HUD** section.
3. Tick **Edit mode** — a cyan dashed border appears around the HUD with a chunky grip in the bottom-right.
4. Drag it with the mouse (drag the middle) or resize via the grip.
5. Untick **Edit mode** once it's well-placed so the HUD becomes click-through again.

See [HUD → Edit mode](hud/edit-mode.md) for details.

---

## My wins/losses aren't being counted

Check that your **in-game name** in the app exactly matches the one shown in match (case-insensitive but space-sensitive). You can edit it in the main window → **Player** section.

The app also captures a **stable identifier** on the first match — visible under the name field. Once captured, the app finds your account even if you change your in-game name.

If nothing is counted, open an issue with:
- Your in-game name (screenshot of the relevant match)
- The contents of `%APPDATA%\RLStatsOverlay\settings.json`

---

## Port 49124 (local HTTP) is taken

The app auto-scans 49124 to 49133 to find a free port. The actual port is shown in the **OBS Browser Source** section (the URL contains the chosen port).

If your OBS still has the URL with an old port (49124) but the app bound 49125, just **re-click 📋 Copy URL** and paste it into OBS.

---

## The antivirus flags the binary

Since it isn't signed and embeds a local HTTP server + a WebSocket client, some heuristic antivirus engines flag it as suspicious (false positive).

- Check on [VirusTotal](https://www.virustotal.com) — the vast majority of engines don't flag it
- Full source is in this repo, audit possible
- You can also compile from source yourself (see [Contributing → Development](contributing/development.md))
