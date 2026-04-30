// "Circle" theme — defers all logic to the shared session overlay loop.
// The HTML / CSS in this folder is what makes this theme look like Circle;
// the behavior is identical across themes.

import { startSessionOverlay } from "/overlays/shared/session-overlay.js";

startSessionOverlay();
