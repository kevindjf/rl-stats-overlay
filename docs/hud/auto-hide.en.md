# HUD auto-hide

Option: show the HUD automatically when Rocket League is running, hide it when the game closes.

## Behavior

- **On Stats API connection** (RL open + Stats API active) → the HUD shows itself.
- **On disconnect** (RL closed or crashed) → the HUD hides.

The option is **off by default** — opt-in so existing users who drive the HUD manually with the "Show HUD" button aren't surprised.

## Turn it on

In the **Appearance** section: tick **"Auto-hide when RL is offline"**.

## Rocket League detection

The app detects Rocket League's presence with **two complementary signals**:

1. **WebSocket connection** to `ws://localhost:49123` (the Stats API). Present = RL is running and the Stats API is active.
2. **Windows process list polling** (`RocketLeague.exe`) — collapses the "RL crashed" detection delay to <1 s, vs. ~8 s without (the Stats API doesn't send a clean FIN on a Quit).

No injection. No hooks. Just passive reads.
