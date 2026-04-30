// Mock Rocket League Stats API server.
// - WebSocket on port 49123 (same as the real game), broadcasting UpdateState ticks
//   and lifecycle/match events the overlays consume.
// - HTTP control panel on the same port at /control to drive the simulation manually.
//
// Run with:  bun run mock-server.ts

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const PORT = 49123;

type Stats = {
  Score: number;
  Goals: number;
  Shots: number;
  Assists: number;
  Saves: number;
  Demos: number;
};

const blank = (): Stats => ({ Score: 0, Goals: 0, Shots: 0, Assists: 0, Saves: 0, Demos: 0 });

const state = {
  myName: "TestPlayer",
  myTeam: 0 as 0 | 1,
  matchGuid: "",
  inMatch: false,
  player: blank(),
  opponent: blank(),
  teamScore: [0, 0],
  timeSeconds: 300,
  ballSpeed: 0,
  /** Active overlay theme (folder name under overlays/themes/). */
  theme: "circle",
  /** Theme var overrides applied on top of the active theme's CSS defaults. */
  themeVars: {} as Record<string, string | number | boolean>,
  tickTimer: null as ReturnType<typeof setInterval> | null,
  clockTimer: null as ReturnType<typeof setInterval> | null,
};

const clients = new Set<any>();

const broadcast = (Event: string, Data: Record<string, any> = {}) => {
  const msg = JSON.stringify({ Event, Data: { MatchGuid: state.matchGuid, ...Data } });
  for (const c of clients) {
    try { c.send(msg); } catch (_) {}
  }
};

const buildUpdateState = () => ({
  Players: [
    {
      Name: state.myName,
      PrimaryId: "Mock|1|0",
      Shortcut: 1,
      TeamNum: state.myTeam,
      Score: state.player.Score,
      Goals: state.player.Goals,
      Shots: state.player.Shots,
      Assists: state.player.Assists,
      Saves: state.player.Saves,
      Touches: 0,
      CarTouches: 0,
      Demos: state.player.Demos,
      bHasCar: true,
      Speed: 1200,
      Boost: 60,
      bBoosting: false,
      bOnGround: true,
      bOnWall: false,
      bPowersliding: false,
      bDemolished: false,
      bSupersonic: false,
    },
    {
      Name: "Opponent",
      PrimaryId: "Mock|2|0",
      Shortcut: 2,
      TeamNum: (1 - state.myTeam) as 0 | 1,
      Score: state.opponent.Score,
      Goals: state.opponent.Goals,
      Shots: state.opponent.Shots,
      Assists: state.opponent.Assists,
      Saves: state.opponent.Saves,
      Touches: 0,
      CarTouches: 0,
      Demos: state.opponent.Demos,
      bHasCar: true,
      Speed: 800,
      Boost: 40,
      bBoosting: false,
      bOnGround: true,
      bOnWall: false,
      bPowersliding: false,
      bDemolished: false,
      bSupersonic: false,
    },
  ],
  Game: {
    Teams: [
      { Name: "Blue",   TeamNum: 0, Score: state.teamScore[0], ColorPrimary: "0000FF", ColorSecondary: "0000AA" },
      { Name: "Orange", TeamNum: 1, Score: state.teamScore[1], ColorPrimary: "FF8800", ColorSecondary: "FF4400" },
    ],
    TimeSeconds: state.timeSeconds,
    bOvertime: false,
    Ball: { Speed: state.ballSpeed, TeamNum: 255 },
    bReplay: false,
    bHasWinner: false,
    Winner: "",
    Arena: "Stadium_P",
    bHasTarget: false,
    Target: { Name: "", Shortcut: 0, TeamNum: 0 },
  },
});

const startTicks = () => {
  if (state.tickTimer) return;
  // Match the default PacketSendRate of the real game (30 Hz).
  state.tickTimer = setInterval(() => {
    if (clients.size === 0) return;
    broadcast("UpdateState", buildUpdateState());
  }, 1000 / 30);
};

const startClock = () => {
  if (state.clockTimer) return;
  state.clockTimer = setInterval(() => {
    if (!state.inMatch) return;
    if (state.timeSeconds > 0) {
      state.timeSeconds -= 1;
      broadcast("ClockUpdatedSeconds", { TimeSeconds: state.timeSeconds, bOvertime: false });
    }
  }, 1000);
};

// --- Actions exposed to the control panel ---

const actions = {
  startMatch() {
    if (state.inMatch) return { ok: false, reason: "already in match" };
    state.matchGuid = crypto.randomUUID().replace(/-/g, "").toUpperCase().slice(0, 32);
    state.player = blank();
    state.opponent = blank();
    state.teamScore = [0, 0];
    state.timeSeconds = 300;
    state.inMatch = true;

    broadcast("MatchCreated");
    setTimeout(() => broadcast("MatchInitialized"), 200);
    setTimeout(() => broadcast("CountdownBegin"), 400);
    setTimeout(() => broadcast("RoundStarted"), 3500);
    return { ok: true };
  },
  endMatch(won: boolean) {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    const winnerTeam = won ? state.myTeam : (1 - state.myTeam);
    if (state.teamScore[0] === state.teamScore[1]) {
      // Force a score gap so the result looks coherent.
      state.teamScore[winnerTeam] += 1;
    }
    broadcast("MatchEnded", { WinnerTeamNum: winnerTeam });
    setTimeout(() => broadcast("PodiumStart"), 1000);
    setTimeout(() => broadcast("MatchDestroyed"), 4000);
    state.inMatch = false;
    return { ok: true };
  },
  addStat(stat: keyof Stats, amount = 1) {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    state.player[stat] += amount;
    if (stat === "Goals") {
      state.player.Score += 100 * amount;
      state.teamScore[state.myTeam] += amount;
      broadcast("GoalScored", {
        GoalSpeed: 90,
        GoalTime: 30,
        ImpactLocation: { X: 0, Y: 5120, Z: 320 },
        Scorer: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
        BallLastTouch: {
          Player: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
          Speed: 90,
        },
      });
    } else if (stat === "Saves") {
      state.player.Score += 75 * amount;
    } else if (stat === "Shots") {
      state.player.Score += 25 * amount;
    } else if (stat === "Assists") {
      state.player.Score += 50 * amount;
    } else if (stat === "Demos") {
      state.player.Score += 25 * amount;
    }
    return { ok: true };
  },
  oppGoal() {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    state.opponent.Goals += 1;
    state.opponent.Score += 100;
    state.teamScore[1 - state.myTeam] += 1;
    broadcast("GoalScored", {
      GoalSpeed: 85,
      GoalTime: 30,
      ImpactLocation: { X: 0, Y: -5120, Z: 320 },
      Scorer: { Name: "Opponent", Shortcut: 2, TeamNum: 1 - state.myTeam },
      BallLastTouch: {
        Player: { Name: "Opponent", Shortcut: 2, TeamNum: 1 - state.myTeam },
        Speed: 85,
      },
    });
    return { ok: true };
  },
  setName(name: string) { state.myName = (name || "TestPlayer").slice(0, 32); return { ok: true }; },
  setTeam(team: number)  { state.myTeam = (team === 1 ? 1 : 0); return { ok: true }; },
  setTheme(theme: string) { state.theme = (theme || "circle").replace(/[^a-z0-9-]/gi, ""); return { ok: true }; },
  setThemeVar(key: string, value: string | number | boolean | null) {
    if (!key) return { ok: false, reason: "key required" };
    if (value === null) {
      delete state.themeVars[key];
    } else {
      state.themeVars[key] = value;
    }
    return { ok: true };
  },
  resetThemeVars() {
    state.themeVars = {};
    return { ok: true };
  },
  status() {
    return {
      ok: true,
      clients: clients.size,
      myName: state.myName,
      myTeam: state.myTeam,
      inMatch: state.inMatch,
      teamScore: state.teamScore,
      player: state.player,
    };
  },
};

// --- Static assets ---
// In the new repo layout, this file lives in dev/ and overlays in overlays/,
// so we resolve relative to the repo root (parent of HERE).
const REPO_ROOT = join(HERE, "..");

const tryReadAbs = (abs: string): Buffer | null => {
  try { return readFileSync(abs); } catch (_) { return null; }
};

const mime = (ext: string): string => {
  switch (ext) {
    case ".html": return "text/html; charset=utf-8";
    case ".css":  return "text/css; charset=utf-8";
    case ".js":   return "application/javascript; charset=utf-8";
    case ".json": return "application/json; charset=utf-8";
    case ".svg":  return "image/svg+xml";
    case ".png":  return "image/png";
    default:      return "application/octet-stream";
  }
};

const serveFile = (relFromRoot: string): Response => {
  const abs = join(REPO_ROOT, relFromRoot);
  const buf = tryReadAbs(abs);
  if (!buf) return new Response(`Missing ${relFromRoot}`, { status: 404 });
  const ext = abs.slice(abs.lastIndexOf("."));
  return new Response(buf, { headers: { "Content-Type": mime(ext) } });
};

const serveDevFile = (file: string): Response => {
  const abs = join(HERE, file);
  const buf = tryReadAbs(abs);
  if (!buf) return new Response(`Missing dev/${file}`, { status: 404 });
  const ext = abs.slice(abs.lastIndexOf("."));
  return new Response(buf, { headers: { "Content-Type": mime(ext) } });
};

// --- HTTP + WebSocket server ---

const server = Bun.serve({
  port: PORT,
  hostname: "0.0.0.0",
  async fetch(req, server) {
    const url = new URL(req.url);

    // Upgrade WebSocket clients (overlays connect here, same port as the real game).
    if (req.headers.get("upgrade")?.toLowerCase() === "websocket") {
      if (server.upgrade(req)) return;
      return new Response("Upgrade failed", { status: 400 });
    }

    // Control panel (dev-only).
    if (url.pathname === "/" || url.pathname === "/control") {
      return serveDevFile("mock-control.html");
    }

    // Mock-provided minimal /api/config so overlays can fetch the configured
    // player, active theme, and any theme variable overrides without going
    // through the Tauri app in dev mode.
    if (url.pathname === "/api/config") {
      return Response.json({
        playerName: state.myName,
        primaryId: "Mock|1|0",
        theme: state.theme,
        themeVars: state.themeVars,
      });
    }

    // Stable URL for the boost overlay — resolves the active theme on the
    // fly and rewrites the HTML with a <base href> so relative paths resolve
    // to the right theme folder. This is the URL users put in OBS so they
    // don't need to change it when switching themes.
    if (url.pathname === "/overlays/boost.html") {
      const themed = join(REPO_ROOT, "overlays", "themes", state.theme, "boost.html");
      const buf = tryReadAbs(themed);
      if (!buf) return new Response(`Theme '${state.theme}' missing boost.html`, { status: 404 });
      const baseHref = `/overlays/themes/${state.theme}/`;
      const rewritten = buf
        .toString("utf8")
        .replace(/<head>/i, `<head>\n  <base href="${baseHref}">`);
      return new Response(rewritten, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      });
    }

    // Overlays + their assets, served straight from the overlays/ folder.
    if (url.pathname.startsWith("/overlays/")) {
      const safe = url.pathname.replace(/\.\.+/g, "").slice(1); // strip leading /
      return serveFile(safe);
    }

    if (url.pathname === "/action" && req.method === "POST") {
      const body = await req.json().catch(() => ({}));
      const fn = (actions as any)[body.action];
      if (typeof fn !== "function") {
        return Response.json({ ok: false, reason: "unknown action" }, { status: 400 });
      }
      const result = fn(...(body.args || []));
      return Response.json(result);
    }

    return new Response("Not found", { status: 404 });
  },
  websocket: {
    open(ws) {
      clients.add(ws);
      console.log(`✓ client connected (${clients.size} total)`);
    },
    close(ws) {
      clients.delete(ws);
      console.log(`× client disconnected (${clients.size} total)`);
    },
    message() {},
  },
});

startTicks();
startClock();

console.log(`
╭──────────────────────────────────────────────────────────╮
│  RL Mock Stats API — dev server                          │
│                                                          │
│  WebSocket    : ws://localhost:${PORT}                   │
│  Control      : http://localhost:${PORT}/control         │
│  Boost overlay: http://localhost:${PORT}/overlays/boost.html │
│                                                          │
│  Use the control panel to drive a fake match without     │
│  having Rocket League running.                           │
╰──────────────────────────────────────────────────────────╯
`);
