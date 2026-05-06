// Post-match HUD page — shown both as a Tauri window and as an OBS Browser
// Source. Focus is on the LOCAL PLAYER's tryhard-relevant metrics for the
// match that just ended: boost management, movement, demo balance.
//
// Two display modes share the same DOM:
//   - "match"   → /api/match-summary/latest (last finalized match)
//   - "session" → /api/session-summary (aggregated since session start)
// The active mode is stored server-side (settings.post_match_hud_mode) so the
// Tauri window and every OBS browser source always reflect the same toggle.
// A click POSTs to /hud/post-match-mode; the polling loop picks up changes
// from /api/state.postMatchHudMode within ~2s.

const card = document.getElementById("card");
const noSpec = document.getElementById("no-spectator");

const $ = (id) => document.getElementById(id);

const VALID_MODES = new Set(["match", "session"]);
let mode = "match"; // optimistic default until first /api/state poll lands.

function showCard(visible) {
  card.style.display = visible ? "" : "none";
}

// ===== Tooltips ============================================================
// `data-tip` lives either on the element itself (header, histograms) or on
// its parent <span> (wrapped value cells). Map each tip-bearing id to the
// right place; `applyTooltips(mode)` then overwrites the text in bulk.
const TIP_TARGETS = {
  "result-badge": "self", "score": "self", "meta": "self",
  "hist-boost": "self", "hist-speed": "self", "hist-air": "self",
  "goals": "parent", "shots": "parent", "saves": "parent",
  "assists": "parent", "demos": "parent", "demos-taken": "parent",
  "score-pts": "parent",
  "avg-boost": "parent", "boosting-pct": "parent", "bpm": "parent",
  "time-zero": "parent", "time-full": "parent",
  "speed-avg": "parent", "dist": "parent",
  "ps-count": "parent", "ps-tot": "parent",
};

const MATCH_TIPS = {
  "result-badge": "Issue du match — déterminée par MatchEnded.",
  "score": "Score final : ton équipe — équipe adverse.",
  "meta": "Format · durée gameplay (chrono RL, hors replays/podium) · arène.",
  "goals": "Buts marqués pendant ce match.",
  "shots": "Tirs cadrés pendant ce match (qui auraient été des buts sans intervention adverse). Inclut les buts.",
  "saves": "Arrêts effectués pendant ce match : interventions sur un tir adverse cadré vers ton but.",
  "assists": "Passes décisives ce match : dernière touche d'un coéquipier avant ton but.",
  "demos": "Démolitions infligées ce match : nombre de fois où tu as détruit un adversaire en supersonique.",
  "demos-taken": "Démolitions subies ce match : nombre de fois où un adversaire t'a détruit.",
  "score-pts": "Score total RL ce match : combine buts, arrêts, passes, démos, sauvetages d'épopée, etc.",
  "avg-boost": "Moyenne de ton réservoir de boost (sur 100) tout au long du match. 50 ≈ équilibré, <40 tu manques souvent, >65 tu sur-collectes.",
  "boosting-pct": "% du match où tu maintenais le bouton boost. À comparer à ta BPM : haut % + BPM faible = tu actionnes le boost mais sans en consommer (réservoir vide).",
  "bpm": "Boost consommé par minute ce match. Joueurs SSL : 350-450 BPM. Approximé via décréments du compteur Boost — ~5-10 % d'erreur typique vs replay.",
  "time-zero": "Secondes passées avec le réservoir VIDE ce match. Critique : à 0 boost tu es sur-jouable.",
  "time-full": "Secondes passées avec le réservoir PLEIN ce match. Au-delà de ~10 s, signe que tu sur-collectes.",
  "speed-avg": "% de la vitesse maximale (2300 UU/s = 100 %) moyennée sur tout le match. SSL ≈ 65-72 %.",
  "dist": "Distance totale parcourue par ta voiture pendant le match (1 km ≈ 100 000 UU).",
  "ps-count": "Nombre de fois où tu as déclenché un powerslide (frein à main) ce match.",
  "ps-tot": "Durée cumulée de tous tes powerslides ce match.",
  "hist-boost": "Distribution du temps passé dans chaque tranche de réservoir. Idéal : minimiser 0-25 et 75-100, maximiser 25-75 (zone de confort).",
  "hist-speed": "Distribution du temps passé dans chaque tranche de vitesse. Beaucoup de Lent = positionnement attentiste ; beaucoup de Supersonique = jeu rapide.",
  "hist-air": "Répartition du temps au sol, en l'air et au mur. Plus d'aérien = jeu vertical ; plus de mur = jeu de mur.",
};

const SESSION_TIPS = {
  "result-badge": "Bilan W/L de la session en cours (depuis le dernier reset ou un gap > 6h).",
  "score": "Victoires et défaites cumulées sur la session.",
  "meta": "Nombre de matchs joués · durée gameplay cumulée sur la session.",
  "goals": "Moyenne de buts par match sur la session.",
  "shots": "Moyenne de tirs cadrés par match sur la session (inclut les buts).",
  "saves": "Moyenne d'arrêts par match sur la session.",
  "assists": "Moyenne de passes décisives par match sur la session.",
  "demos": "Moyenne de démolitions infligées par match sur la session.",
  "demos-taken": "Moyenne de démolitions subies par match sur la session.",
  "score-pts": "Score RL moyen par match sur la session.",
  "avg-boost": "Boost moyen (sur 100) moyenné sur tous les matchs de la session.",
  "boosting-pct": "% du temps où tu maintiens le bouton boost, moyenné sur la session.",
  "bpm": "Boost-per-minute moyen sur la session. Approximé via décréments — ~5-10 % d'erreur.",
  "time-zero": "Temps moyen par match passé à 0 boost sur la session.",
  "time-full": "Temps moyen par match passé à 100 boost sur la session.",
  "speed-avg": "% vitesse maximale moyenne sur tous les matchs de la session.",
  "dist": "Distance TOTALE parcourue sur l'ensemble de la session (somme de tous les matchs).",
  "ps-count": "Nombre TOTAL de powerslides sur l'ensemble de la session.",
  "ps-tot": "Durée TOTALE cumulée des powerslides sur la session.",
  "hist-boost": "Distribution moyenne du boost agrégée sur la session (moyenne par match, chaque match poids égal).",
  "hist-speed": "Distribution moyenne de la vitesse agrégée sur la session (moyenne par match).",
  "hist-air": "Répartition moyenne sol/aérien/mur agrégée sur la session (moyenne par match).",
};

function applyTooltips(mode) {
  const dict = mode === "session" ? SESSION_TIPS : MATCH_TIPS;
  Object.keys(TIP_TARGETS).forEach((id) => {
    const el = $(id);
    if (!el) return;
    const target = TIP_TARGETS[id] === "self" ? el : el.parentElement;
    if (target && dict[id]) target.dataset.tip = dict[id];
  });
}

function fmtArena(arena) {
  if (!arena) return "—";
  return arena.replace(/_P$/i, "").replace(/_/g, " ");
}
function fmtDuration(s) {
  const m = Math.floor(s / 60);
  const sec = Math.max(0, s - m * 60);
  return `${m}:${String(sec).padStart(2, "0")}`;
}
function fmtSec(s) {
  if (s == null) return "—";
  if (s < 10) return `${s.toFixed(1)}s`;
  return `${Math.round(s)}s`;
}
function fmtPct(v) {
  if (v == null) return "—";
  return `${Math.round(v)}%`;
}
function fmtKm(uu) {
  if (uu == null) return "—";
  // Rocket League convention: 100 Unreal Units = 1 m, so 1 km = 100 000 UU.
  // Convert to a human-readable km value (1 decimal place).
  const km = uu / 100000;
  if (km >= 10) return `${km.toFixed(1)} km`;
  return `${km.toFixed(2)} km`;
}

function clearChildrenAfter(el, selector) {
  // Keep the <h4> and remove any previously injected bar rows.
  el.querySelectorAll(selector).forEach((n) => n.remove());
}

function renderBars(container, bars) {
  clearChildrenAfter(container, ".pm-hist-row");
  for (const b of bars) {
    const v = Math.max(0, Math.min(100, b.pct || 0));
    const row = document.createElement("div");
    row.className = "pm-hist-row";
    if (b.title) row.title = b.title;
    row.innerHTML = `
      <span class="pm-bar-label">${b.label}</span>
      <div class="pm-bar-track"><div class="pm-bar-fill ${b.tone || ""}" style="width:${v}%"></div></div>
      <span class="pm-bar-value">${v.toFixed(0)}%</span>
    `;
    container.appendChild(row);
  }
}

function applyMatch(detail) {
  if (!detail) {
    showCard(false);
    return;
  }
  showCard(true);

  // Header
  const isWin = detail.isWin;
  const winnerKnown = detail.winnerTeamNum !== null && detail.winnerTeamNum !== undefined;
  const localTeam = detail.localTeamNum;
  const localScore = localTeam === 0 ? detail.blueScore : detail.orangeScore;
  const oppScore = localTeam === 0 ? detail.orangeScore : detail.blueScore;
  const badge = $("result-badge");
  if (!winnerKnown) {
    badge.textContent = "TERMINÉ";
    badge.className = "result-badge unknown";
  } else {
    badge.textContent = isWin ? "VICTOIRE" : "DÉFAITE";
    badge.className = `result-badge ${isWin ? "win" : "loss"}`;
  }
  $("score").textContent = `${localScore} — ${oppScore}`;
  $("meta").textContent =
    `${detail.teamSize}v${detail.teamSize} · ${fmtDuration(detail.durationSeconds)}` +
    (detail.overtime ? " · ⚡OT" : "") +
    ` · ${fmtArena(detail.arena)}`;

  // Local player
  const local = (detail.players || []).find((p) => p.isLocalPlayer);
  if (!local) {
    noSpec.classList.remove("hidden");
    // Still show core team-level stats as best we can: blank locals.
    $("goals").textContent = "—";
    $("shots").textContent = "—";
    $("saves").textContent = "—";
    $("assists").textContent = "—";
    $("demos").textContent = "—";
    $("score-pts").textContent = "—";
    $("demos-taken").textContent = "—";
    return;
  }
  noSpec.classList.add("hidden");

  // Core stats
  $("goals").textContent = local.goals;
  $("shots").textContent = local.shots;
  $("saves").textContent = local.saves;
  $("assists").textContent = local.assists;
  $("demos").textContent = local.demos;
  $("score-pts").textContent = local.score;
  const s = local.spectator;
  $("demos-taken").textContent = s ? s.demosTaken : "—";

  // Boost management
  $("avg-boost").textContent = s ? Math.round(s.boostAvg) : "—";
  $("boosting-pct").textContent = s ? fmtPct(s.boostPctBoosting) : "—";
  $("bpm").textContent = s ? Math.round(s.bpm) : "—";
  $("time-zero").textContent = s ? fmtSec(s.boostTimeAt0S) : "—";
  $("time-full").textContent = s ? fmtSec(s.boostTimeAt100S) : "—";

  // Movement
  $("speed-avg").textContent = s ? fmtPct(s.speedAvgPct) : "—";
  $("dist").textContent = s ? fmtKm(s.totalDistance) : "—";
  $("ps-count").textContent = s ? s.powerslideCount : "—";
  $("ps-tot").textContent = s ? fmtSec(s.powerslideTotalS) : "—";

  // Histograms
  const histBoost = $("hist-boost");
  const histSpeed = $("hist-speed");
  const histAir = $("hist-air");
  if (s) {
    renderBars(histBoost, [
      { label: "0 – 24",   pct: s.boostPct025,   tone: "danger",
        title: "% du match avec ton réservoir entre 0 et 24 inclus. Élevé = tu manques souvent de boost dans des moments critiques. Objectif : minimiser." },
      { label: "25 – 49",  pct: s.boostPct2550,  tone: "warn",
        title: "% du match avec ton réservoir entre 25 et 49 inclus. Marge de manœuvre limitée — peu d'aérien, pas de soutien rapide possible." },
      { label: "50 – 74",  pct: s.boostPct5075,  tone: "",
        title: "% du match avec ton réservoir entre 50 et 74 inclus. Zone de confort tactique — assez pour réagir, pas si plein que tu sur-collectes." },
      { label: "75 – 100", pct: s.boostPct75100, tone: "blue",
        title: "% du match avec ton réservoir entre 75 et 100 inclus. Trop élevé = tu sur-collectes, tu rates des opportunités d'aérien ou de défense rapide." },
    ]);
    renderBars(histSpeed, [
      { label: "Lent",          pct: s.pctTimeSlow,        tone: "",
        title: "Vitesse < 1400 UU/s. Phases de positionnement, attente, contrôle de balle au sol." },
      { label: "Boost",         pct: s.pctTimeBoostSpeed,  tone: "warn",
        title: "Vitesse 1400-2200 UU/s. Vitesse boost classique, déplacements actifs." },
      { label: "Supersonique",  pct: s.pctTimeSupersonic,  tone: "blue",
        title: "Vitesse ≥ 2200 UU/s. Vitesse max RL, agressivité, démos possibles. Beaucoup de % = jeu rapide." },
    ]);
    renderBars(histAir, [
      { label: "Sol",    pct: s.pctTimeGround, tone: "",
        title: "Au moins 3 roues touchent le sol. Phase de positionnement et conduite normale." },
      { label: "Aérien", pct: s.pctTimeAerial, tone: "blue",
        title: "En l'air sans toucher mur ni sol. Aerials, double-jumps, mécaniques verticales." },
      { label: "Mur",    pct: s.pctTimeWall,   tone: "warn",
        title: "Roues collées à un mur. Wall reads, dribbling vertical, contrôle au mur." },
    ]);
    histBoost.style.display = "";
    histSpeed.style.display = "";
    histAir.style.display = "";
  } else {
    histBoost.style.display = "none";
    histSpeed.style.display = "none";
    histAir.style.display = "none";
  }
}

function fmtNum(v, decimals = 1) {
  if (v == null || Number.isNaN(v)) return "—";
  return v.toFixed(decimals);
}

function applySession(agg) {
  if (!agg || !agg.matches) {
    showCard(false);
    return;
  }
  showCard(true);
  noSpec.classList.add("hidden");

  // Header
  const badge = $("result-badge");
  badge.textContent = "SESSION";
  badge.className = "result-badge unknown";
  $("score").textContent = `${agg.wins}W — ${agg.losses}L`;
  const dur = agg.totalDurationS || 0;
  const h = Math.floor(dur / 3600);
  const m = Math.floor((dur % 3600) / 60);
  const durStr = h > 0 ? `${h}h${String(m).padStart(2, "0")}` : `${m} min`;
  $("meta").textContent = `${agg.matches} match${agg.matches > 1 ? "s" : ""} · ${durStr}`;

  // Core averages — same DOM, decimal values signal "per-match avg".
  $("goals").textContent      = fmtNum(agg.avgGoals);
  $("shots").textContent      = fmtNum(agg.avgShots);
  $("saves").textContent      = fmtNum(agg.avgSaves);
  $("assists").textContent    = fmtNum(agg.avgAssists);
  $("demos").textContent      = fmtNum(agg.avgDemos);
  $("score-pts").textContent  = agg.avgScore != null ? Math.round(agg.avgScore) : "—";
  $("demos-taken").textContent = fmtNum(agg.avgDemosTaken);

  // Boost (averaged across matches)
  $("avg-boost").textContent    = agg.avgBoost != null ? Math.round(agg.avgBoost) : "—";
  $("boosting-pct").textContent = fmtPct(agg.avgBoostingPct);
  $("bpm").textContent          = agg.avgBpm != null ? Math.round(agg.avgBpm) : "—";
  $("time-zero").textContent    = fmtSec(agg.avgBoostTimeAt0S);
  $("time-full").textContent    = fmtSec(agg.avgBoostTimeAt100S);

  // Movement — speed avg averaged, distance & powerslides SUMMED over session.
  $("speed-avg").textContent = fmtPct(agg.avgSpeedAvgPct);
  $("dist").textContent      = fmtKm(agg.totalDistance);
  // Total powerslide count is matches × avg → derive from sum of total_s
  // is not the count. Display the sum-of-counts approximation: there is no
  // explicit total count column, but matches × avgPowerslideCount is good.
  $("ps-count").textContent  = agg.avgPowerslideCount != null
    ? Math.round(agg.avgPowerslideCount * agg.matches)
    : "—";
  $("ps-tot").textContent    = fmtSec(agg.totalPowerslideS);

  const histBoost = $("hist-boost");
  const histSpeed = $("hist-speed");
  const histAir = $("hist-air");
  const haveBoostHist = agg.avgBoostPct025 != null;
  if (haveBoostHist) {
    renderBars(histBoost, [
      { label: "0 – 24",   pct: agg.avgBoostPct025,  tone: "danger" },
      { label: "25 – 49",  pct: agg.avgBoostPct2550, tone: "warn" },
      { label: "50 – 74",  pct: agg.avgBoostPct5075, tone: "" },
      { label: "75 – 100", pct: agg.avgBoostPct75100, tone: "blue" },
    ]);
    renderBars(histSpeed, [
      { label: "Lent",         pct: agg.avgPctTimeSlow,        tone: "" },
      { label: "Boost",        pct: agg.avgPctTimeBoostSpeed,  tone: "warn" },
      { label: "Supersonique", pct: agg.avgSupersonicPct,      tone: "blue" },
    ]);
    renderBars(histAir, [
      { label: "Sol",    pct: agg.avgPctTimeGround, tone: "" },
      { label: "Aérien", pct: agg.avgAerialPct,     tone: "blue" },
      { label: "Mur",    pct: agg.avgPctTimeWall,   tone: "warn" },
    ]);
    histBoost.style.display = "";
    histSpeed.style.display = "";
    histAir.style.display = "";
  } else {
    histBoost.style.display = "none";
    histSpeed.style.display = "none";
    histAir.style.display = "none";
  }
}

async function fetchLatest() {
  try {
    const r = await fetch("/api/match-summary/latest");
    if (r.status === 204 || r.status === 404) return null;
    if (!r.ok) return null;
    return await r.json();
  } catch (_) {
    return null;
  }
}

async function fetchSessionAgg() {
  try {
    const r = await fetch("/api/session-summary");
    if (r.status === 204 || r.status === 404) return null;
    if (!r.ok) return null;
    return await r.json();
  } catch (_) {
    return null;
  }
}

async function fetchState() {
  try {
    const r = await fetch("/api/state");
    if (!r.ok) return null;
    return await r.json();
  } catch (_) {
    return null;
  }
}

// Polling loop. Persistent display: the page keeps showing the last
// recorded match (or session aggregate) at all times — including during the
// next match's gameplay. Hidden ONLY when (a) the user clicks the X (server
// gates both endpoints to 204), or (b) no data has been recorded yet. The
// card content is replaced when a new MatchEnded lands a fresher matchGuid
// or session aggregate.
let lastShownGuid = null;
let lastShownSessionKey = null;
async function refresh() {
  // Sync mode from server first — picks up toggle clicks made on other
  // surfaces (Tauri ↔ OBS) within one polling tick.
  const state = await fetchState();
  if (state && VALID_MODES.has(state.postMatchHudMode) && state.postMatchHudMode !== mode) {
    applyModeChange(state.postMatchHudMode);
  }
  if (mode === "session") {
    const agg = await fetchSessionAgg();
    if (agg && agg.matches > 0) {
      // Cheap change detector — re-render when match count or last outcome moves.
      const key = `${agg.matches}|${agg.wins}|${agg.losses}|${agg.totalDurationS}`;
      if (key !== lastShownSessionKey) {
        lastShownSessionKey = key;
        applySession(agg);
      }
    } else {
      if (lastShownSessionKey !== null || !card.style.display) applySession(null);
      lastShownSessionKey = null;
    }
    return;
  }
  // mode === "match"
  const detail = await fetchLatest();
  if (detail) {
    if (detail.matchGuid !== lastShownGuid) {
      lastShownGuid = detail.matchGuid;
      applyMatch(detail);
    }
  } else {
    if (lastShownGuid !== null || !card.style.display) applyMatch(null);
    lastShownGuid = null;
  }
}

// Apply a mode change locally (visual state + change-detector reset). Used
// both by user clicks (after the server POST returns) AND by polling that
// detects a mode change made on another surface.
function applyModeChange(next) {
  if (!VALID_MODES.has(next) || next === mode) return;
  mode = next;
  // Reset change detectors so the new view paints on the next tick.
  lastShownGuid = null;
  lastShownSessionKey = null;
  document.querySelectorAll(".pm-mode").forEach((el) => {
    el.classList.toggle("active", el.dataset.mode === mode);
  });
  applyTooltips(mode);
}

async function setMode(next) {
  if (!VALID_MODES.has(next) || next === mode) return;
  // Optimistic local update so the click feels instant; server propagates
  // to the other surfaces via /api/state.
  applyModeChange(next);
  refresh();
  try {
    await fetch(`/hud/post-match-mode?mode=${encodeURIComponent(next)}`, { method: "POST" });
  } catch (_) { /* offline / standalone — local mode still applies */ }
}

// Wire up toggle buttons. Initial active state is set by applyModeChange()
// after the first /api/state poll lands; until then the HTML default
// (data-mode="match" has class "active") wins.
document.querySelectorAll(".pm-mode").forEach((el) => {
  el.addEventListener("click", () => setMode(el.dataset.mode));
});

refresh();
setInterval(refresh, 2000);

// Close button — asks the Rust side to hide the post-match HUD window.
// We POST to /hud/post-match-close because the page is loaded over plain
// HTTP, not tauri://, so window.__TAURI__ isn't available here. The
// endpoint is a no-op when the page is opened in a regular browser tab
// (the Rust side just returns 404 for "no post_match_hud window").
const btnClose = document.getElementById("btn-close");
if (btnClose) {
  btnClose.addEventListener("click", async () => {
    try {
      await fetch("/hud/post-match-close", { method: "POST" });
    } catch (_) { /* offline / standalone — ignore */ }
  });
}
