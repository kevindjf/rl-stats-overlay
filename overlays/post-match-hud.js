// Post-match HUD page — shown both as a Tauri window and as an OBS Browser
// Source. Focus is on the LOCAL PLAYER's tryhard-relevant metrics for the
// match that just ended: boost management, movement, demo balance.

const card = document.getElementById("card");
const empty = document.getElementById("empty-state");
const noSpec = document.getElementById("no-spectator");

const $ = (id) => document.getElementById(id);

function showCard(visible) {
  card.style.display = visible ? "" : "none";
  empty.classList.toggle("hidden", visible);
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
  const localTeam = detail.localTeamNum;
  const localScore = localTeam === 0 ? detail.blueScore : detail.orangeScore;
  const oppScore = localTeam === 0 ? detail.orangeScore : detail.blueScore;
  const badge = $("result-badge");
  badge.textContent = isWin ? "VICTOIRE" : "DÉFAITE";
  badge.className = `result-badge ${isWin ? "win" : "loss"}`;
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

// On page load, render whatever match is the latest (covers cold-boot
// re-display after the app was relaunched between two matches).
fetchLatest().then(applyMatch);

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
