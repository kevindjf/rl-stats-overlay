// Most themes don't need their own JS — just import the shared loop.
// It reads from /api/state every second, writes #v-streak / #v-wins /
// #v-losses, toggles `.ok` on #conn when the WS link is alive, and
// pulses each value with a `.bump` class on change.
import { startSessionOverlay } from "/overlays/shared/session-overlay.js";

startSessionOverlay();
