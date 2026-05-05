// Mock Rocket League Stats API server.
// - Raw TCP on port 49123 (matches the real RL Stats API protocol),
//   broadcasting JSON envelopes back-to-back with no framing — what
//   the Tauri app's `ws_client.rs` reads.
// - HTTP+WebSocket on port 49125 for the control panel and the browser
//   overlays in dev mode (cannot share 49123 because Bun.listen and
//   Bun.serve can't bind the same port).
//
// Run with:  bun run mock-server.ts

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const TCP_PORT = 49123;
const HTTP_PORT = 49125;
const TICK_HZ = 30;
const TICK_DT = 1 / TICK_HZ;

type Stats = {
  Score: number;
  Goals: number;
  Shots: number;
  Assists: number;
  Saves: number;
  Demos: number;
};

type PlayerSim = {
  Boost: number;
  bBoosting: boolean;
  bSupersonic: boolean;
  bOnGround: boolean;
  bOnWall: boolean;
  bPowersliding: boolean;
  bDemolished: boolean;
  Speed: number;
  // internal phase counters (ticks)
  boostPhaseTicks: number;
  powerslideTicks: number;
  pickupCooldownTicks: number;
  demolishedTicks: number;
  airborneTicks: number;
};

const blank = (): Stats => ({ Score: 0, Goals: 0, Shots: 0, Assists: 0, Saves: 0, Demos: 0 });

const blankSim = (): PlayerSim => ({
  Boost: 50,
  bBoosting: false,
  bSupersonic: false,
  bOnGround: true,
  bOnWall: false,
  bPowersliding: false,
  bDemolished: false,
  Speed: 800,
  boostPhaseTicks: 0,
  powerslideTicks: 0,
  pickupCooldownTicks: 0,
  demolishedTicks: 0,
  airborneTicks: 0,
});

const state = {
  myName: "TestPlayer",
  myTeam: 0 as 0 | 1,
  matchGuid: "",
  inMatch: false,
  player: blank(),
  opponent: blank(),
  playerSim: blankSim(),
  opponentSim: blankSim(),
  teamScore: [0, 0],
  timeSeconds: 300,
  ballSpeed: 0,
  ballLastTeam: 255,
  /** Active overlay theme (folder name under overlays/themes/). */
  theme: "circle",
  /** Theme var overrides applied on top of the active theme's CSS defaults. */
  themeVars: {} as Record<string, string | number | boolean>,
  tickTimer: null as ReturnType<typeof setInterval> | null,
  clockTimer: null as ReturnType<typeof setInterval> | null,
  scriptedTimer: null as ReturnType<typeof setTimeout> | null,
};

// WebSocket clients (browser overlays, control panel). On port HTTP_PORT.
const wsClients = new Set<any>();
// Raw-TCP clients (the Tauri ws_client.rs treats RL Stats API as raw TCP).
// On port TCP_PORT — matches the real RL game protocol.
const tcpClients = new Set<any>();

const broadcast = (Event: string, Data: Record<string, any> = {}) => {
  const msg = JSON.stringify({ Event, Data: { MatchGuid: state.matchGuid, ...Data } });
  for (const c of wsClients) {
    try { c.send(msg); } catch (_) {}
  }
  for (const sock of tcpClients) {
    try { sock.write(msg); } catch (_) {}
  }
};

const totalClients = () => wsClients.size + tcpClients.size;

// --- Realistic per-tick simulation of SPECTATOR fields (boost/speed/aerial/powerslide).
// The recorder will sample these to compute BPM, % time supersonic, etc.

const stepSim = (sim: PlayerSim) => {
  // Demolished state: lock for ~3s then respawn full boost
  if (sim.bDemolished) {
    sim.demolishedTicks += 1;
    sim.Boost = 33;
    sim.Speed = 0;
    sim.bBoosting = false;
    sim.bSupersonic = false;
    sim.bOnGround = false;
    sim.bOnWall = false;
    sim.bPowersliding = false;
    if (sim.demolishedTicks > 3 * TICK_HZ) {
      sim.bDemolished = false;
      sim.demolishedTicks = 0;
      sim.bOnGround = true;
    }
    return;
  }

  // Boost dynamics: alternate boosting / cruising phases
  sim.boostPhaseTicks += 1;
  if (sim.bBoosting) {
    // Consume ~33 boost/s while boosting
    sim.Boost = Math.max(0, sim.Boost - 33 * TICK_DT);
    if (sim.Boost === 0 || sim.boostPhaseTicks > Math.random() * 2 * TICK_HZ + 0.3 * TICK_HZ) {
      sim.bBoosting = false;
      sim.boostPhaseTicks = 0;
    }
  } else {
    // Random bursts of boosting roughly every 1-3s if boost available
    if (sim.Boost > 5 && sim.boostPhaseTicks > Math.random() * 2 * TICK_HZ + TICK_HZ) {
      sim.bBoosting = true;
      sim.boostPhaseTicks = 0;
    }
  }

  // Pickups: simulate "hitting a big pad" every ~6-10s, full refill
  sim.pickupCooldownTicks += 1;
  if (sim.pickupCooldownTicks > (6 + Math.random() * 4) * TICK_HZ) {
    sim.Boost = Math.min(100, sim.Boost + (Math.random() < 0.4 ? 100 : 12));
    sim.pickupCooldownTicks = 0;
  }

  // Speed: tied to boosting state with some noise
  let target = 1000 + Math.random() * 400;
  if (sim.bBoosting) target = 1700 + Math.random() * 600;
  if (sim.bBoosting && sim.Boost > 30 && Math.random() < 0.35) target = 2300; // supersonic
  sim.Speed = sim.Speed + (target - sim.Speed) * 0.4;
  sim.bSupersonic = sim.Speed >= 2200;

  // Aerial / wall toggling: ~15% of time off ground
  if (sim.bOnGround && Math.random() < 0.01) {
    sim.bOnGround = false;
    sim.airborneTicks = 0;
    sim.bOnWall = Math.random() < 0.3;
  } else if (!sim.bOnGround) {
    sim.airborneTicks += 1;
    if (sim.airborneTicks > Math.random() * 3 * TICK_HZ + 0.5 * TICK_HZ) {
      sim.bOnGround = true;
      sim.bOnWall = false;
      sim.airborneTicks = 0;
    }
  }

  // Powerslide: short bursts (~150-400ms) every ~3-5s, only on ground
  if (sim.bPowersliding) {
    sim.powerslideTicks += 1;
    if (sim.powerslideTicks > 0.15 * TICK_HZ + Math.random() * 0.3 * TICK_HZ) {
      sim.bPowersliding = false;
      sim.powerslideTicks = 0;
    }
  } else if (sim.bOnGround && Math.random() < 0.012) {
    sim.bPowersliding = true;
    sim.powerslideTicks = 0;
  }
};

const tickSim = () => {
  if (!state.inMatch) return;
  stepSim(state.playerSim);
  stepSim(state.opponentSim);
  // Decay ball speed gradually
  state.ballSpeed = Math.max(0, state.ballSpeed * 0.95);
};

const buildUpdateState = () => {
  // Local team gets full SPECTATOR fields. The "opponent" team's SPECTATOR
  // fields are intentionally zeroed to mimic the real API behavior — only the
  // local team / spectated player has these populated.
  const ps = state.playerSim;
  const me: any = {
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
    bHasCar: !ps.bDemolished,
    Speed: Math.round(ps.Speed),
    Boost: Math.round(ps.Boost),
    bBoosting: ps.bBoosting,
    bOnGround: ps.bOnGround,
    bOnWall: ps.bOnWall,
    bPowersliding: ps.bPowersliding,
    bDemolished: ps.bDemolished,
    bSupersonic: ps.bSupersonic,
  };
  const opp: any = {
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
    // SPECTATOR fields not present in the real API for opposing players —
    // we leave them as defaults / zero to validate the UI handling.
    bHasCar: true,
    Speed: 0,
    Boost: 0,
    bBoosting: false,
    bOnGround: true,
    bOnWall: false,
    bPowersliding: false,
    bDemolished: false,
    bSupersonic: false,
  };
  return {
    Players: [me, opp],
    Game: {
      Teams: [
        { Name: "Blue",   TeamNum: 0, Score: state.teamScore[0], ColorPrimary: "0000FF", ColorSecondary: "0000AA" },
        { Name: "Orange", TeamNum: 1, Score: state.teamScore[1], ColorPrimary: "FF8800", ColorSecondary: "FF4400" },
      ],
      TimeSeconds: state.timeSeconds,
      bOvertime: false,
      Ball: { Speed: Math.round(state.ballSpeed), TeamNum: state.ballLastTeam },
      bReplay: false,
      bHasWinner: false,
      Winner: "",
      Arena: "Stadium_P",
      bHasTarget: false,
      Target: { Name: "", Shortcut: 0, TeamNum: 0 },
    },
  };
};

const startTicks = () => {
  if (state.tickTimer) return;
  state.tickTimer = setInterval(() => {
    tickSim();
    if (totalClients() === 0) return;
    broadcast("UpdateState", buildUpdateState());
  }, 1000 / TICK_HZ);
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
    state.playerSim = blankSim();
    state.opponentSim = blankSim();
    state.teamScore = [0, 0];
    state.timeSeconds = 300;
    state.ballLastTeam = 255;
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
      state.ballLastTeam = state.myTeam;
      state.ballSpeed = 1500;
      broadcast("GoalScored", {
        GoalSpeed: 90,
        GoalTime: 30,
        ImpactLocation: { X: (Math.random() - 0.5) * 1600, Y: 5120, Z: 320 },
        Scorer: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
        BallLastTouch: {
          Player: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
          Speed: 90,
        },
      });
    } else if (stat === "Saves") {
      state.player.Score += 75 * amount;
      // Statfeed entry too
      broadcast("StatfeedEvent", {
        EventName: "Save",
        Type: "Save",
        MainTarget: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
      });
    } else if (stat === "Shots") {
      state.player.Score += 25 * amount;
    } else if (stat === "Assists") {
      state.player.Score += 50 * amount;
    } else if (stat === "Demos") {
      state.player.Score += 25 * amount;
      broadcast("StatfeedEvent", {
        EventName: "Demolish",
        Type: "Demolition",
        MainTarget: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
        SecondaryTarget: { Name: "Opponent", Shortcut: 2, TeamNum: 1 - state.myTeam },
      });
    }
    return { ok: true };
  },
  oppGoal() {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    state.opponent.Goals += 1;
    state.opponent.Score += 100;
    state.teamScore[1 - state.myTeam] += 1;
    state.ballLastTeam = (1 - state.myTeam) as 0 | 1;
    state.ballSpeed = 1500;
    broadcast("GoalScored", {
      GoalSpeed: 85,
      GoalTime: 30,
      ImpactLocation: { X: (Math.random() - 0.5) * 1600, Y: -5120, Z: 320 },
      Scorer: { Name: "Opponent", Shortcut: 2, TeamNum: 1 - state.myTeam },
      BallLastTouch: {
        Player: { Name: "Opponent", Shortcut: 2, TeamNum: 1 - state.myTeam },
        Speed: 85,
      },
    });
    return { ok: true };
  },
  ballHit(team: number) {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    const t = (team === 1 ? 1 : 0) as 0 | 1;
    state.ballLastTeam = t;
    state.ballSpeed = 1200 + Math.random() * 800;
    const playerName = t === state.myTeam ? state.myName : "Opponent";
    const shortcut = t === state.myTeam ? 1 : 2;
    broadcast("BallHit", {
      Players: [{ Name: playerName, Shortcut: shortcut, TeamNum: t }],
      Ball: {
        PreHitSpeed: state.ballSpeed * 0.6,
        PostHitSpeed: state.ballSpeed,
        Location: {
          X: (Math.random() - 0.5) * 4000,
          Y: (Math.random() - 0.5) * 5000,
          Z: 100 + Math.random() * 500,
        },
      },
    });
    return { ok: true };
  },
  crossbar(team: number) {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    const t = (team === 1 ? 1 : 0) as 0 | 1;
    const playerName = t === state.myTeam ? state.myName : "Opponent";
    const shortcut = t === state.myTeam ? 1 : 2;
    const speed = 800 + Math.random() * 600;
    broadcast("CrossbarHit", {
      BallLocation: { X: 120, Y: t === 0 ? -2944 : 2944, Z: 320 },
      BallSpeed: speed,
      ImpactForce: 100 + Math.random() * 80,
      BallLastTouch: {
        Player: { Name: playerName, Shortcut: shortcut, TeamNum: t },
        Speed: speed,
      },
    });
    return { ok: true };
  },
  statfeed(eventName: string, type?: string) {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    broadcast("StatfeedEvent", {
      EventName: eventName,
      Type: type || eventName,
      MainTarget: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
    });
    return { ok: true };
  },
  demolishMe() {
    if (!state.inMatch) return { ok: false, reason: "no match running" };
    state.playerSim.bDemolished = true;
    state.playerSim.demolishedTicks = 0;
    broadcast("StatfeedEvent", {
      EventName: "Demolish",
      Type: "Demolition",
      MainTarget: { Name: "Opponent", Shortcut: 2, TeamNum: 1 - state.myTeam },
      SecondaryTarget: { Name: state.myName, Shortcut: 1, TeamNum: state.myTeam },
    });
    return { ok: true };
  },
  scriptedMatch() {
    if (state.inMatch) return { ok: false, reason: "already in match" };
    if (state.scriptedTimer) clearTimeout(state.scriptedTimer);
    actions.startMatch();
    // Schedule a series of events over ~30s of wall clock to simulate a match.
    const schedule: Array<[number, () => void]> = [
      [2000, () => actions.ballHit(state.myTeam)],
      [3500, () => actions.crossbar(state.myTeam)],
      [5000, () => actions.ballHit(1 - state.myTeam)],
      [7000, () => actions.addStat("Shots")],
      [8500, () => actions.ballHit(state.myTeam)],
      [10000, () => actions.addStat("Goals")],
      [12000, () => actions.statfeed("Save", "Save")],
      [13500, () => actions.ballHit(1 - state.myTeam)],
      [15000, () => actions.oppGoal()],
      [17000, () => actions.demolishMe()],
      [19000, () => actions.addStat("Saves")],
      [21000, () => actions.ballHit(state.myTeam)],
      [22500, () => actions.crossbar(1 - state.myTeam)],
      [24000, () => actions.addStat("Goals")],
      [26000, () => actions.addStat("Shots")],
      [28000, () => actions.statfeed("EpicSave", "Epic Save")],
      [30000, () => actions.endMatch(true)],
    ];
    for (const [ms, fn] of schedule) {
      setTimeout(fn, ms);
    }
    return { ok: true, eventsScheduled: schedule.length };
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
      clients: totalClients(),
      wsClients: wsClients.size,
      tcpClients: tcpClients.size,
      myName: state.myName,
      myTeam: state.myTeam,
      inMatch: state.inMatch,
      teamScore: state.teamScore,
      player: state.player,
      sim: {
        boost: Math.round(state.playerSim.Boost),
        speed: Math.round(state.playerSim.Speed),
        boosting: state.playerSim.bBoosting,
        supersonic: state.playerSim.bSupersonic,
        ground: state.playerSim.bOnGround,
        wall: state.playerSim.bOnWall,
        powerslide: state.playerSim.bPowersliding,
        demolished: state.playerSim.bDemolished,
      },
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

// --- Raw TCP server on TCP_PORT (49123) — emulates the real RL Stats API ---
// The Tauri app's `ws_client.rs` does plain TCP and reads brace-delimited JSON
// objects back-to-back. We send the SAME envelopes used over WebSocket, just
// without the WS framing.
Bun.listen({
  port: TCP_PORT,
  hostname: "127.0.0.1",
  socket: {
    open(socket) {
      tcpClients.add(socket);
      console.log(`✓ TCP client connected (${tcpClients.size} on :${TCP_PORT})`);
    },
    close(socket) {
      tcpClients.delete(socket);
      console.log(`× TCP client disconnected (${tcpClients.size} on :${TCP_PORT})`);
    },
    error(socket) {
      tcpClients.delete(socket);
    },
    data() {
      // The real RL Stats API only emits — clients never send. Ignore inbound.
    },
  },
});

// --- HTTP + WebSocket server on HTTP_PORT (49125) ---

const server = Bun.serve({
  port: HTTP_PORT,
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
      wsClients.add(ws);
      console.log(`✓ WS client connected (${wsClients.size} on :${HTTP_PORT})`);
    },
    close(ws) {
      wsClients.delete(ws);
      console.log(`× WS client disconnected (${wsClients.size} on :${HTTP_PORT})`);
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
│  Stats API TCP: tcp://127.0.0.1:${TCP_PORT} (real RL protocol)    │
│  Control panel: http://localhost:${HTTP_PORT}/control            │
│  Boost overlay: http://localhost:${HTTP_PORT}/overlays/boost.html │
│                                                          │
│  Use the control panel to drive a fake match without     │
│  having Rocket League running.                           │
╰──────────────────────────────────────────────────────────╯
`);
