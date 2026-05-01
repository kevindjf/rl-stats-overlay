// Shared session bootstrap reused by every theme.
// Each theme provides the DOM (label text, layout, styling) and just calls
// startSessionOverlay(). The logic — loading config, applying theme vars,
// reading W/L/streak from the host — lives here so themes stay tiny and
// consistent.
//
// The Rust client owns the connection to the RL Stats API (which is a raw
// TCP stream of JSON envelopes, not a WebSocket — see ws_client.rs). The
// overlay reads the rolled-up session state from the embedded HTTP server's
// /api/state endpoint.

import { loadOverlayConfig, applyThemeVars } from "/overlays/shared/ws-client.js";

const POLL_INTERVAL_MS = 1000;

/**
 * @param {object} [opts]
 * @param {{streak: string, wins: string, losses: string, conn: string}} [opts.selectors]
 *   CSS selectors for the four DOM elements the loop updates. Defaults match
 *   the conventional ids used by the bundled themes.
 */
export async function startSessionOverlay(opts = {}) {
  const sel = {
    streak: "#v-streak",
    wins: "#v-wins",
    losses: "#v-losses",
    conn: "#conn",
    // Optional — themes that don't display per-match stats simply leave
    // these elements out and the loop skips the readout silently.
    match: "#v-match",
    matchRow: "#row-match",
    ...(opts.selectors || {}),
  };

  const elStreak = document.querySelector(sel.streak);
  const elWins = document.querySelector(sel.wins);
  const elLosses = document.querySelector(sel.losses);
  const elConn = document.querySelector(sel.conn);
  const elMatch = document.querySelector(sel.match);
  const elMatchRow = document.querySelector(sel.matchRow);
  if (!elStreak || !elWins || !elLosses) {
    console.error("startSessionOverlay: missing DOM elements", sel);
    return;
  }

  function bump(el) {
    // Restart the CSS keyframe by toggling the class across two animation
    // frames. The previous `void el.offsetWidth` trick worked but forced a
    // synchronous layout — wasteful when 3 cards bump at once on a goal.
    el.classList.remove("bump");
    requestAnimationFrame(() => {
      requestAnimationFrame(() => el.classList.add("bump"));
    });
  }

  let prev = { wins: 0, losses: 0, streak: 0 };

  function render(session, animate = false) {
    const streakSign = session.streak > 0 ? "+" : session.streak < 0 ? "" : "+";
    elStreak.textContent = `${streakSign}${session.streak}`;
    elStreak.classList.toggle("win", session.streak > 0);
    elStreak.classList.toggle("loss", session.streak < 0);

    elWins.textContent = String(session.wins);
    elLosses.textContent = String(session.losses);

    if (animate) {
      if (session.wins !== prev.wins) bump(elWins);
      if (session.losses !== prev.losses) bump(elLosses);
      if (session.streak !== prev.streak) bump(elStreak);
    }
    prev = { wins: session.wins, losses: session.losses, streak: session.streak };
  }

  // Initial paint with zeros so themes show up before the first poll lands.
  render({ wins: 0, losses: 0, streak: 0 });

  const cfg = await loadOverlayConfig();
  applyThemeVars(cfg.themeVars);

  /**
   * Update the optional per-match readout. Themes wire it by including
   * a `#v-match` element (and optionally a `#row-match` wrapper to hide
   * between matches). Looks up the local player by PrimaryId — when we
   * can't find them, the row is simply hidden.
   */
  function renderMatch(matchStats) {
    if (!elMatch) return;
    const players = Array.isArray(matchStats?.players) ? matchStats.players : [];
    const me = cfg.primaryId
      ? players.find((p) => (p.primaryId || "") === cfg.primaryId)
      : null;
    if (!me) {
      if (elMatchRow) elMatchRow.hidden = true;
      return;
    }
    if (elMatchRow) elMatchRow.hidden = false;
    elMatch.textContent = `${me.goals || 0}/${me.saves || 0}/${me.shots || 0}/${me.assists || 0}`;
  }

  async function poll() {
    try {
      const res = await fetch("/api/state", { cache: "no-store" });
      if (!res.ok) {
        elConn?.classList.remove("ok");
        return;
      }
      const data = await res.json();
      elConn?.classList.toggle("ok", !!data.connected);
      const s = data.session || {};
      render(
        {
          wins: s.wins || 0,
          losses: s.losses || 0,
          streak: s.streak || 0,
        },
        true,
      );
      renderMatch(data.matchStats);
    } catch (_) {
      elConn?.classList.remove("ok");
    }
  }

  // Run the polling loop only while the page is actually being rendered.
  // OBS keeps browser sources alive in the background, and the in-game HUD
  // gets hidden frequently — there is no point hammering /api/state at 1 Hz
  // when nobody can see the result. We resume immediately on visibility
  // change so the streak updates on the next frame after toggling back.
  let timer = null;
  function startPolling() {
    if (timer != null) return;
    poll();
    timer = setInterval(poll, POLL_INTERVAL_MS);
  }
  function stopPolling() {
    if (timer != null) {
      clearInterval(timer);
      timer = null;
    }
  }
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) stopPolling();
    else startPolling();
  });
  if (!document.hidden) startPolling();
}
