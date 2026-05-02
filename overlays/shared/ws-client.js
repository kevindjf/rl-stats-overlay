// Shared WebSocket client used by every overlay.
// Connects to the Rocket League Stats API on ws://localhost:49123 and dispatches
// each event to subscribers. Reconnects automatically with exponential backoff.

/**
 * @typedef {(data: any) => void} EventHandler
 */

export class StatsApiClient {
  /**
   * @param {object} [opts]
   * @param {string} [opts.url]            WebSocket URL (defaults to ws://localhost:49123)
   * @param {(connected: boolean) => void} [opts.onConnectionChange]
   */
  constructor(opts = {}) {
    this.url = opts.url || `ws://${location.hostname || "localhost"}:49123`;
    this.onConnectionChange = opts.onConnectionChange || (() => {});
    /** @type {Map<string, Set<EventHandler>>} */
    this.handlers = new Map();
    this.ws = null;
    this.reconnectDelay = 1000;
    this.connected = false;
  }

  /**
   * Subscribe to an event by name (e.g. "UpdateState", "MatchEnded").
   * Returns an unsubscribe function.
   * @param {string} event
   * @param {EventHandler} fn
   */
  on(event, fn) {
    let set = this.handlers.get(event);
    if (!set) {
      set = new Set();
      this.handlers.set(event, set);
    }
    set.add(fn);
    return () => set.delete(fn);
  }

  start() {
    this._connect();
  }

  _connect() {
    try {
      this.ws = new WebSocket(this.url);
    } catch (_) {
      return this._scheduleReconnect();
    }
    this.ws.onopen = () => {
      this.connected = true;
      this.reconnectDelay = 1000;
      this.onConnectionChange(true);
    };
    this.ws.onclose = () => {
      this.connected = false;
      this.onConnectionChange(false);
      this._scheduleReconnect();
    };
    this.ws.onerror = () => {
      try { this.ws?.close(); } catch (_) {}
    };
    this.ws.onmessage = (msg) => {
      let payload;
      try { payload = JSON.parse(msg.data); } catch (_) { return; }
      const handlers = this.handlers.get(payload?.Event);
      if (!handlers) return;
      for (const fn of handlers) {
        try { fn(payload.Data); } catch (e) { console.error(e); }
      }
    };
  }

  _scheduleReconnect() {
    setTimeout(() => this._connect(), this.reconnectDelay);
    this.reconnectDelay = Math.min(this.reconnectDelay * 1.5, 10000);
  }
}

/**
 * Fetch the overlay configuration from the host server. The Tauri HTTP
 * server (or the dev mock) exposes this at /api/config and returns the
 * configured player, the active theme name and any theme-specific
 * variables (colors, sizes, toggles) the user has overridden in settings.
 *
 * Falls back to URL query params for player/primaryId if the endpoint is
 * unreachable, so direct dev URLs still work.
 *
 * @returns {Promise<{
 *   playerName: string,
 *   primaryId: string,
 *   theme: string,
 *   themeVars: Record<string, string|number|boolean>,
 * }>}
 */
export async function loadOverlayConfig() {
  const fallback = () => {
    const params = new URLSearchParams(location.search);
    return {
      playerName: (params.get("player") || "").trim(),
      primaryId:  (params.get("id") || "").trim(),
      theme: (params.get("theme") || "circle").trim(),
      themeVars: {},
    };
  };
  try {
    const res = await fetch("/api/config", { cache: "no-store" });
    if (!res.ok) return fallback();
    const data = await res.json();
    return {
      playerName: (data.playerName || "").trim(),
      primaryId:  (data.primaryId  || "").trim(),
      theme: (data.theme || "circle").trim(),
      themeVars: data.themeVars && typeof data.themeVars === "object" ? data.themeVars : {},
    };
  } catch (_) {
    return fallback();
  }
}

/**
 * Apply a `themeVars` map (as returned by /api/config) to the document
 * root as CSS custom properties. Boolean values toggle a `data-*` attribute
 * on :root so themes can wire show/hide flags through CSS attribute
 * selectors instead of inline display rules.
 *
 * Mapping rules:
 *   - "color"   value  → css var, raw value (e.g. "#5dd16f")
 *   - "number"  value  → css var, suffixed with "px" by default
 *   - "boolean" value  → data attribute and a 1/0 css var for opacity-style use
 *
 * The keys in `vars` are camelCase; this helper converts them to kebab-case
 * for the matching --css-var.
 *
 * Diff-aware: keys we set on a previous call but that disappear from the new
 * map are explicitly removed from `:root` and `dataset`. Without this, a
 * user "Reset" leaves the previous inline override winning over the theme
 * default until a full page reload. Module-scoped state — fine because
 * each overlay HTML loads this module exactly once per webview lifetime.
 *
 * @param {Record<string, string|number|boolean>} vars
 */
const _appliedThemeKeys = new Set();
export function applyThemeVars(vars) {
  const root = document.documentElement;
  const next = new Set();
  for (const [key, raw] of Object.entries(vars || {})) {
    const cssName = "--" + key.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
    if (typeof raw === "boolean") {
      root.dataset[key] = String(raw);
      root.style.setProperty(cssName, raw ? "1" : "0");
    } else if (typeof raw === "number") {
      root.style.setProperty(cssName, `${raw}px`);
    } else {
      root.style.setProperty(cssName, String(raw));
    }
    next.add(key);
  }
  // Drop keys that vanished from `vars` since the previous call — this is the
  // "Reset" path. Without it, a removed override leaves a stale inline style
  // on :root that keeps winning over the theme's CSS default.
  for (const key of _appliedThemeKeys) {
    if (next.has(key)) continue;
    const cssName = "--" + key.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
    root.style.removeProperty(cssName);
    delete root.dataset[key];
  }
  _appliedThemeKeys.clear();
  for (const k of next) _appliedThemeKeys.add(k);
}

/**
 * Identify the local player inside an UpdateState payload, preferring a stable
 * PrimaryId match over a name match.
 * @param {any[]} players
 * @param {{playerName: string, primaryId: string}} cfg
 */
export function findLocalPlayer(players, cfg) {
  if (!Array.isArray(players)) return null;
  if (cfg.primaryId) {
    const byId = players.find((p) => (p.PrimaryId || "") === cfg.primaryId);
    if (byId) return byId;
  }
  const name = cfg.playerName.toLowerCase();
  if (name) {
    const byName = players.find((p) => (p.Name || "").toLowerCase() === name);
    if (byName) return byName;
  }
  return null;
}
