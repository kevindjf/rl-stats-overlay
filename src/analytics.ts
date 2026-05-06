// Analytics views (Historique / Session / All-time) for the settings window.
// Talks to the Rust backend via Tauri commands declared in lib.rs:
//   - get_recent_matches
//   - get_match_detail
//   - get_session_aggregate
//   - get_lifetime_aggregate
//   - list_profiles
//   - start_new_session
//   - delete_match

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { t } from "./i18n";

// ----- Types matching the Rust serde structs (camelCase) -------------------

export interface SpectatorStats {
  boostAvg: number;
  boostTimeAt0S: number;
  boostTimeAt100S: number;
  boostPct025: number;
  boostPct2550: number;
  boostPct5075: number;
  boostPct75100: number;
  boostPctBoosting: number;
  bpm: number;
  boostUsedSupersonic: number;
  speedAvgPct: number;
  totalDistance: number;
  pctTimeSlow: number;
  pctTimeBoostSpeed: number;
  pctTimeSupersonic: number;
  pctTimeGround: number;
  pctTimeAerial: number;
  pctTimeWall: number;
  powerslideTotalS: number;
  powerslideCount: number;
  powerslideAvgS: number;
  demosTaken: number;
}

export interface MatchPlayerRecord {
  playerName: string;
  primaryId: string | null;
  teamNum: number;
  isLocalTeam: boolean;
  isLocalPlayer: boolean;
  goals: number;
  shots: number;
  saves: number;
  assists: number;
  score: number;
  demos: number;
  spectator: SpectatorStats | null;
}

export interface GoalRecord {
  ord: number;
  scoredAtSecondsRemaining: number | null;
  scorerName: string | null;
  scorerTeamNum: number | null;
  assisterName: string | null;
  lastTouchName: string | null;
  goalSpeed: number | null;
  impactX: number | null;
  impactY: number | null;
  impactZ: number | null;
}

export interface StatfeedRecord {
  ord: number;
  atSecondsRemaining: number | null;
  eventName: string;
  typeLabel: string | null;
  mainPlayer: string | null;
  mainTeamNum: number | null;
  secondaryPlayer: string | null;
  secondaryTeamNum: number | null;
}

export interface MatchSummary {
  matchGuid: string;
  primaryId: string;
  endedAtMs: number;
  startedAtMs: number;
  arena: string | null;
  teamSize: number;
  blueScore: number;
  orangeScore: number;
  localTeamNum: number;
  isWin: boolean;
  overtime: boolean;
  durationSeconds: number;
}

export interface MatchDetail {
  matchGuid: string;
  primaryId: string;
  startedAtMs: number;
  endedAtMs: number;
  arena: string | null;
  teamSize: number;
  localTeamNum: number;
  winnerTeamNum: number | null;
  isWin: boolean;
  blueScore: number;
  orangeScore: number;
  overtime: boolean;
  durationSeconds: number;
  ballHitsBlue: number;
  ballHitsOrange: number;
  crossbarHits: number;
  players: MatchPlayerRecord[];
  goals: GoalRecord[];
  statfeed: StatfeedRecord[];
}

export interface ProfileInfo {
  primaryId: string;
  displayName: string;
  firstSeenAtMs: number;
  lastSeenAtMs: number;
  matchCount: number;
}

export interface PlaylistBreakdown {
  teamSize: number;
  wins: number;
  losses: number;
}

export interface AggregateStats {
  matches: number;
  wins: number;
  losses: number;
  currentStreak: number;
  bestWinStreak: number;
  bestLossStreak: number;
  avgGoals: number;
  avgShots: number;
  avgSaves: number;
  avgAssists: number;
  avgScore: number;
  avgDemos: number;
  avgBoost: number | null;
  avgBpm: number | null;
  avgSupersonicPct: number | null;
  avgAerialPct: number | null;
  avgPowerslideCount: number | null;
  // Extended aggregates for the post-match HUD's "Session" toggle.
  avgDemosTaken: number | null;
  avgBoostingPct: number | null;
  avgBoostTimeAt0S: number | null;
  avgBoostTimeAt100S: number | null;
  avgBoostPct025: number | null;
  avgBoostPct2550: number | null;
  avgBoostPct5075: number | null;
  avgBoostPct75100: number | null;
  avgSpeedAvgPct: number | null;
  avgPctTimeSlow: number | null;
  avgPctTimeBoostSpeed: number | null;
  avgPctTimeGround: number | null;
  avgPctTimeWall: number | null;
  totalDistance: number | null;
  totalPowerslideS: number | null;
  totalDurationS: number;
  byTeamSize: PlaylistBreakdown[];
  recentOutcomes: boolean[];
  bestMatchScore: number | null;
  bestMatchGuid: string | null;
  worstMatchGuid: string | null;
  startedAtMs: number | null;
}

// ----- Module-scoped state -------------------------------------------------

export type AnalyticsTab = "history" | "session" | "alltime";
const DEFAULT_TAB: AnalyticsTab = "history";

let activeTab: AnalyticsTab = readTab();
let activeProfile: string | null = readProfile();
let activeMatchDetail: string | null = null; // guid; null = list view

let matchRecordedUnlisten: null | (() => void) = null;

function readTab(): AnalyticsTab {
  try {
    const v = localStorage.getItem("analytics:tab");
    if (v === "history" || v === "session" || v === "alltime") return v;
  } catch (_) { /* ignore */ }
  return DEFAULT_TAB;
}
function writeTab(v: AnalyticsTab) {
  try { localStorage.setItem("analytics:tab", v); } catch (_) { /* ignore */ }
}
function readProfile(): string | null {
  try { return localStorage.getItem("analytics:profile"); } catch (_) { return null; }
}
function writeProfile(v: string | null) {
  try {
    if (v) localStorage.setItem("analytics:profile", v);
    else localStorage.removeItem("analytics:profile");
  } catch (_) { /* ignore */ }
}

export function getActiveTab(): AnalyticsTab { return activeTab; }
export function setActiveTab(v: AnalyticsTab) {
  activeTab = v;
  activeMatchDetail = null;
  writeTab(v);
}
export function setActiveProfile(v: string | null) {
  activeProfile = v;
  writeProfile(v);
}
export function getActiveProfile(): string | null { return activeProfile; }

/// Wire the `rlstats://match-recorded` Tauri event so analytics views auto
/// refresh when a new match is persisted. Idempotent — safe to call repeatedly.
export async function ensureMatchRecordedListener(onRecorded: () => void) {
  if (matchRecordedUnlisten) return;
  matchRecordedUnlisten = await listen<string>("rlstats://match-recorded", onRecorded);
}

// ----- Tab nav rendering ---------------------------------------------------

export function renderTabNav(): string {
  const tab = (id: AnalyticsTab, label: string) =>
    `<button class="analytics-tab ${activeTab === id ? "active" : ""}" data-analytics-tab="${id}">${label}</button>`;
  return /* html */ `
    <nav class="analytics-tabs" aria-label="${esc(t("analytics.tabs.label"))}">
      ${tab("history", t("analytics.tabs.history"))}
      ${tab("session", t("analytics.tabs.session"))}
      ${tab("alltime", t("analytics.tabs.alltime"))}
    </nav>
  `;
}

export function bindTabNav(onSwitch: () => void) {
  document.querySelectorAll<HTMLButtonElement>("button.analytics-tab[data-analytics-tab]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const v = btn.dataset.analyticsTab as AnalyticsTab | undefined;
      if (!v) return;
      setActiveTab(v);
      onSwitch();
    });
  });
}

// ----- Profile selector ----------------------------------------------------

export async function renderProfileSelector(currentPid: string): Promise<string> {
  const profiles = await invoke<ProfileInfo[]>("list_profiles").catch(() => [] as ProfileInfo[]);
  if (profiles.length === 0) {
    return /* html */ `
      <span class="muted analytics-profile-empty">${t("analytics.profile.none")}</span>
    `;
  }
  const selected = activeProfile || currentPid || profiles[0]?.primaryId || "";
  if (!activeProfile) activeProfile = selected;
  const opts = profiles
    .map((p) => {
      const platform = p.primaryId.split("|")[0] || "?";
      const label = `${p.displayName || p.primaryId} (${platform} · ${p.matchCount})`;
      return `<option value="${esc(p.primaryId)}" ${selected === p.primaryId ? "selected" : ""}>${esc(label)}</option>`;
    })
    .join("");
  return /* html */ `
    <label class="analytics-profile-label" for="analytics-profile-select">
      ${t("analytics.profile.label")}
    </label>
    <select id="analytics-profile-select" class="analytics-profile-select">
      ${opts}
    </select>
  `;
}

export function bindProfileSelector(onChange: () => void) {
  const sel = document.getElementById("analytics-profile-select") as HTMLSelectElement | null;
  if (!sel) return;
  sel.addEventListener("change", () => {
    setActiveProfile(sel.value || null);
    onChange();
  });
}

// ----- Tab content renderers ----------------------------------------------

export async function renderActiveTab(): Promise<string> {
  if (activeTab === "history") {
    if (activeMatchDetail) {
      return await renderMatchDetailView(activeMatchDetail);
    }
    return await renderHistoryList();
  }
  if (activeTab === "session") return await renderAggregateView("session");
  return await renderAggregateView("alltime");
}

async function renderHistoryList(): Promise<string> {
  const pid = activeProfile || "";
  const matches = await invoke<MatchSummary[]>("get_recent_matches", {
    primaryId: pid || null,
    limit: 100,
    offset: 0,
  }).catch(() => [] as MatchSummary[]);

  if (matches.length === 0) {
    return /* html */ `
      <div class="analytics-empty">
        <p>${t("analytics.history.empty")}</p>
        <p class="muted" style="font-size: 12px;">${t("analytics.history.emptyHint")}</p>
      </div>
    `;
  }

  const rows = matches
    .map((m) => {
      const localScore = m.localTeamNum === 0 ? m.blueScore : m.orangeScore;
      const oppScore = m.localTeamNum === 0 ? m.orangeScore : m.blueScore;
      const wlBadge = m.isWin
        ? `<span class="wl-badge win">W</span>`
        : `<span class="wl-badge loss">L</span>`;
      const otTag = m.overtime ? `<span class="ot-tag">⚡OT</span>` : "";
      const dur = formatDuration(m.durationSeconds);
      const arena = m.arena ? formatArena(m.arena) : "—";
      const ago = formatTimeAgo(m.endedAtMs);
      return /* html */ `
        <button class="match-row" data-match-guid="${esc(m.matchGuid)}">
          ${wlBadge}
          <span class="match-score">${localScore}–${oppScore}</span>
          <span class="match-tag">${m.teamSize}v${m.teamSize}</span>
          <span class="match-arena">${esc(arena)}</span>
          <span class="match-duration">${dur}</span>
          ${otTag}
          <span class="match-when">${esc(ago)}</span>
          <span class="match-arrow">›</span>
        </button>
      `;
    })
    .join("");

  return /* html */ `
    <div class="analytics-history-list">
      ${rows}
    </div>
  `;
}

export function bindHistoryList(rerender: () => void) {
  document.querySelectorAll<HTMLElement>("button.match-row[data-match-guid]").forEach((row) => {
    row.addEventListener("click", () => {
      activeMatchDetail = row.dataset.matchGuid || null;
      rerender();
    });
  });
}

async function renderMatchDetailView(guid: string): Promise<string> {
  const detail = await invoke<MatchDetail | null>("get_match_detail", { matchGuid: guid })
    .catch(() => null);
  if (!detail) {
    return /* html */ `
      <div class="analytics-empty">
        <p>${t("analytics.match.notFound")}</p>
        <button class="ghost" id="btn-back-history">${t("analytics.match.back")}</button>
      </div>
    `;
  }
  const localTeam = detail.localTeamNum;
  const winnerLabel = detail.isWin
    ? `<span class="result-badge win">${t("analytics.match.win")}</span>`
    : `<span class="result-badge loss">${t("analytics.match.loss")}</span>`;

  const team0 = detail.players.filter((p) => p.teamNum === 0);
  const team1 = detail.players.filter((p) => p.teamNum === 1);
  const localTeamPlayers = localTeam === 0 ? team0 : team1;
  const oppTeamPlayers = localTeam === 0 ? team1 : team0;

  return /* html */ `
    <div class="match-detail">
      <div class="match-detail-header">
        <button class="ghost" id="btn-back-history">← ${t("analytics.match.back")}</button>
        <span class="muted">${esc(formatTimeAgo(detail.endedAtMs))} · ${esc(formatArena(detail.arena || ""))}</span>
      </div>

      <div class="match-detail-score">
        <div class="team-score ${localTeam === 0 ? "is-local" : ""}">
          <span class="team-name">${t("analytics.team.blue")}</span>
          <span class="score-num">${detail.blueScore}</span>
        </div>
        <div class="match-meta">
          ${winnerLabel}
          <span class="muted">${detail.teamSize}v${detail.teamSize} · ${formatDuration(detail.durationSeconds)}${detail.overtime ? " · ⚡OT" : ""}</span>
        </div>
        <div class="team-score ${localTeam === 1 ? "is-local" : ""}">
          <span class="team-name">${t("analytics.team.orange")}</span>
          <span class="score-num">${detail.orangeScore}</span>
        </div>
      </div>

      ${detailsPanel("match.players",   t("analytics.match.players"),   true,  renderPlayersTable(localTeamPlayers, oppTeamPlayers, localTeam))}
      ${detailsPanel("match.goals",      t("analytics.match.goals"),     true,  renderGoalsTimeline(detail.goals, detail.durationSeconds))}
      ${detailsPanel("match.advanced",   `${t("analytics.match.advanced")} ⓘ`, true,
        `${renderAdvancedTable(localTeamPlayers, oppTeamPlayers, localTeam)}
         <p class="muted" style="font-size: 11px; margin-top: 8px;">${t("analytics.match.advancedHint")}</p>`)}
      ${detailsPanel("match.histograms", t("analytics.match.histograms"), true,  renderHistograms(localTeamPlayers))}
      ${detailsPanel("match.possession", t("analytics.match.possession"), false, renderPossession(detail))}
      ${detailsPanel("match.statfeed",   t("analytics.match.statfeed"),   false, renderStatfeed(detail.statfeed, detail.durationSeconds))}

      <div class="match-detail-actions">
        <button class="danger" id="btn-delete-match" data-match-guid="${esc(detail.matchGuid)}">
          ${t("analytics.match.delete")}
        </button>
      </div>
    </div>
  `;
}

export function bindMatchDetail(rerender: () => void) {
  const back = document.getElementById("btn-back-history");
  if (back) back.addEventListener("click", () => { activeMatchDetail = null; rerender(); });
  const del = document.getElementById("btn-delete-match");
  if (del) del.addEventListener("click", async () => {
    const guid = (del as HTMLButtonElement).dataset.matchGuid;
    if (!guid) return;
    if (!confirm(t("analytics.match.deleteConfirm"))) return;
    await invoke("delete_match", { matchGuid: guid }).catch((e) => console.error(e));
    activeMatchDetail = null;
    rerender();
  });
}

function renderPlayersTable(local: MatchPlayerRecord[], opp: MatchPlayerRecord[], localTeam: number): string {
  const renderRow = (p: MatchPlayerRecord) => /* html */ `
    <tr class="${p.isLocalTeam ? "team-local" : "team-opp"}">
      <td>${p.isLocalPlayer ? "★" : ""}</td>
      <td class="player-name">${esc(p.playerName)}</td>
      <td class="num">${p.goals}</td>
      <td class="num">${p.shots}</td>
      <td class="num">${p.saves}</td>
      <td class="num">${p.assists}</td>
      <td class="num">${p.score}</td>
      <td class="num">${p.demos}</td>
    </tr>
  `;
  return /* html */ `
    <table class="match-table">
      <thead>
        <tr>
          <th></th>
          <th>${t("analytics.player.name")}</th>
          <th>G</th><th>S</th><th>Sa</th><th>A</th><th>${t("analytics.player.score")}</th><th>D</th>
        </tr>
      </thead>
      <tbody>
        <tr class="team-divider"><td colspan="8">${localTeam === 0 ? t("analytics.team.blue") : t("analytics.team.orange")}</td></tr>
        ${local.map(renderRow).join("")}
        <tr class="team-divider"><td colspan="8">${localTeam === 0 ? t("analytics.team.orange") : t("analytics.team.blue")}</td></tr>
        ${opp.map(renderRow).join("")}
      </tbody>
    </table>
  `;
}

function renderAdvancedTable(local: MatchPlayerRecord[], opp: MatchPlayerRecord[], localTeam: number): string {
  const cell = (v: number | null | undefined, fmt: (n: number) => string) =>
    v == null ? `<td class="num placeholder">—</td>` : `<td class="num">${fmt(v)}</td>`;
  const oneDec = (n: number) => n.toFixed(1);
  const intStr = (n: number) => Math.round(n).toString();
  const pct = (n: number) => `${n.toFixed(0)}%`;
  const sec = (n: number) => `${n.toFixed(1)}s`;
  const km = (n: number) => `${(n / 1000).toFixed(1)}k`;

  const renderRow = (p: MatchPlayerRecord) => {
    const s = p.spectator;
    return /* html */ `
      <tr class="${p.isLocalTeam ? "team-local" : "team-opp"}">
        <td>${p.isLocalPlayer ? "★" : ""}</td>
        <td class="player-name">${esc(p.playerName)}</td>
        ${cell(s?.bpm, intStr)}
        ${cell(s?.boostAvg, intStr)}
        ${cell(s?.boostTimeAt0S, sec)}
        ${cell(s?.boostTimeAt100S, sec)}
        ${cell(s?.speedAvgPct, pct)}
        ${cell(s?.totalDistance, km)}
        ${cell(s?.pctTimeSlow, pct)}
        ${cell(s?.pctTimeBoostSpeed, pct)}
        ${cell(s?.pctTimeSupersonic, pct)}
        ${cell(s?.pctTimeGround, pct)}
        ${cell(s?.pctTimeAerial, pct)}
        ${cell(s?.powerslideTotalS, sec)}
        ${cell(s?.powerslideCount, intStr)}
        ${cell(s?.powerslideAvgS, oneDec)}
        ${cell(s?.demosTaken, intStr)}
      </tr>
    `;
  };
  return /* html */ `
    <table class="match-table dense">
      <thead>
        <tr>
          <th></th><th>${t("analytics.player.name")}</th>
          <th>BPM</th><th>${t("analytics.adv.avgBoost")}</th>
          <th>${t("analytics.adv.t0")}</th><th>${t("analytics.adv.t100")}</th>
          <th>${t("analytics.adv.avgSpeed")}</th><th>${t("analytics.adv.dist")}</th>
          <th>${t("analytics.adv.slow")}</th><th>${t("analytics.adv.boostSpeed")}</th><th>${t("analytics.adv.super")}</th>
          <th>${t("analytics.adv.ground")}</th><th>${t("analytics.adv.aerial")}</th>
          <th>${t("analytics.adv.psTot")}</th><th>${t("analytics.adv.psCount")}</th><th>${t("analytics.adv.psAvg")}</th>
          <th>${t("analytics.adv.demosTaken")}</th>
        </tr>
      </thead>
      <tbody>
        <tr class="team-divider"><td colspan="17">${localTeam === 0 ? t("analytics.team.blue") : t("analytics.team.orange")}</td></tr>
        ${local.map(renderRow).join("")}
        <tr class="team-divider"><td colspan="17">${localTeam === 0 ? t("analytics.team.orange") : t("analytics.team.blue")}</td></tr>
        ${opp.map(renderRow).join("")}
      </tbody>
    </table>
  `;
}

function renderHistograms(local: MatchPlayerRecord[]): string {
  const blocks = local
    .filter((p) => !!p.spectator)
    .map((p) => {
      const s = p.spectator!;
      const boostBars = [
        { label: "0-25", v: s.boostPct025 },
        { label: "25-50", v: s.boostPct2550 },
        { label: "50-75", v: s.boostPct5075 },
        { label: "75-100", v: s.boostPct75100 },
      ];
      const speedBars = [
        { label: t("analytics.hist.slow"), v: s.pctTimeSlow },
        { label: t("analytics.hist.boost"), v: s.pctTimeBoostSpeed },
        { label: t("analytics.hist.super"), v: s.pctTimeSupersonic },
      ];
      const aerialBars = [
        { label: t("analytics.hist.ground"), v: s.pctTimeGround },
        { label: t("analytics.hist.aerial"), v: s.pctTimeAerial },
        { label: t("analytics.hist.wall"), v: s.pctTimeWall },
      ];
      return /* html */ `
        <div class="histogram-block">
          <div class="histogram-title">${esc(p.playerName)} ${p.isLocalPlayer ? "★" : ""}</div>
          <div class="histogram-grid">
            <div class="hist-col">
              <h4>${t("analytics.hist.boostDist")}</h4>
              ${boostBars.map((b) => bar(b.label, b.v)).join("")}
            </div>
            <div class="hist-col">
              <h4>${t("analytics.hist.speedDist")}</h4>
              ${speedBars.map((b) => bar(b.label, b.v)).join("")}
            </div>
            <div class="hist-col">
              <h4>${t("analytics.hist.airGround")}</h4>
              ${aerialBars.map((b) => bar(b.label, b.v)).join("")}
            </div>
          </div>
        </div>
      `;
    });
  if (blocks.length === 0) {
    return `<p class="muted">${t("analytics.hist.empty")}</p>`;
  }
  return blocks.join("");
}

function bar(label: string, pctValue: number): string {
  const v = Math.max(0, Math.min(100, pctValue || 0));
  return /* html */ `
    <div class="bar-row">
      <div class="bar-track"><div class="bar-fill" style="width: ${v}%"></div></div>
      <div class="bar-label"><span>${esc(label)}</span><span class="bar-value">${v.toFixed(0)}%</span></div>
    </div>
  `;
}

function renderPossession(d: MatchDetail): string {
  const total = d.ballHitsBlue + d.ballHitsOrange;
  const blue = total > 0 ? (d.ballHitsBlue / total) * 100 : 0;
  const orange = total > 0 ? (d.ballHitsOrange / total) * 100 : 0;
  return /* html */ `
    <div class="possession-rows">
      <div class="bar-row">
        <div class="bar-track"><div class="bar-fill blue" style="width: ${blue}%"></div></div>
        <div class="bar-label"><span>${t("analytics.team.blue")}</span><span class="bar-value">${blue.toFixed(0)}%</span></div>
      </div>
      <div class="bar-row">
        <div class="bar-track"><div class="bar-fill orange" style="width: ${orange}%"></div></div>
        <div class="bar-label"><span>${t("analytics.team.orange")}</span><span class="bar-value">${orange.toFixed(0)}%</span></div>
      </div>
    </div>
    <p class="muted" style="margin-top: 10px; font-size: 12px;">
      ${t("analytics.match.crossbars")}: ${d.crossbarHits}
    </p>
  `;
}

function renderGoalsTimeline(goals: GoalRecord[], duration: number): string {
  if (goals.length === 0) return `<p class="muted">${t("analytics.match.noGoals")}</p>`;
  return /* html */ `
    <ul class="goals-timeline">
      ${goals.map((g) => {
        const elapsed = (g.scoredAtSecondsRemaining != null && duration > 0)
          ? Math.max(0, duration - g.scoredAtSecondsRemaining)
          : null;
        const time = elapsed != null ? formatClock(elapsed) : "—";
        const speed = g.goalSpeed != null ? ` · ${Math.round(g.goalSpeed)} km/h` : "";
        const assist = g.assisterName ? ` (${t("analytics.match.assistedBy")} ${esc(g.assisterName)})` : "";
        const teamCls = g.scorerTeamNum === 0 ? "blue" : "orange";
        return `
          <li class="goal-row ${teamCls}">
            <span class="goal-time">${time}</span>
            <span class="goal-icon">⚽</span>
            <span class="goal-scorer">${esc(g.scorerName || "?")}</span>
            <span class="goal-assist muted">${assist}</span>
            <span class="goal-speed muted">${speed}</span>
          </li>
        `;
      }).join("")}
    </ul>
  `;
}

function renderStatfeed(events: StatfeedRecord[], duration: number): string {
  if (events.length === 0) return `<p class="muted">${t("analytics.match.noStatfeed")}</p>`;
  return /* html */ `
    <ul class="statfeed-list">
      ${events.map((e) => {
        const elapsed = (e.atSecondsRemaining != null && duration > 0)
          ? Math.max(0, duration - e.atSecondsRemaining)
          : null;
        const time = elapsed != null ? formatClock(elapsed) : "—";
        const subj = e.mainPlayer || "?";
        const obj = e.secondaryPlayer ? ` → ${esc(e.secondaryPlayer)}` : "";
        return `
          <li>
            <span class="sf-time">${time}</span>
            <span class="sf-name">${esc(e.typeLabel || e.eventName)}</span>
            <span class="muted">${esc(subj)}${obj}</span>
          </li>
        `;
      }).join("")}
    </ul>
  `;
}

async function renderAggregateView(scope: "session" | "alltime"): Promise<string> {
  const cmd = scope === "session" ? "get_session_aggregate" : "get_lifetime_aggregate";
  const agg = await invoke<AggregateStats>(cmd, { primaryId: activeProfile || null }).catch(
    () => ({
      matches: 0, wins: 0, losses: 0, currentStreak: 0,
      bestWinStreak: 0, bestLossStreak: 0,
      avgGoals: 0, avgShots: 0, avgSaves: 0, avgAssists: 0, avgScore: 0, avgDemos: 0,
      avgBoost: null, avgBpm: null, avgSupersonicPct: null, avgAerialPct: null, avgPowerslideCount: null,
      byTeamSize: [], recentOutcomes: [],
      bestMatchScore: null, bestMatchGuid: null, worstMatchGuid: null, startedAtMs: null,
    } as AggregateStats),
  );

  if (agg.matches === 0) {
    return /* html */ `
      <div class="analytics-empty">
        <p>${t("analytics.aggregate.empty")}</p>
      </div>
    `;
  }

  const winRate = agg.matches > 0 ? (agg.wins / agg.matches) * 100 : 0;
  const streakStr = agg.currentStreak === 0
    ? "—"
    : `${agg.currentStreak > 0 ? "+" : ""}${agg.currentStreak} ${agg.currentStreak > 0 ? "W" : "L"}`;
  const bestStr = `${agg.bestWinStreak}W / ${agg.bestLossStreak}L`;
  const sinceLine = scope === "session" && agg.startedAtMs
    ? t("analytics.aggregate.sessionStart", { ago: formatTimeAgo(agg.startedAtMs) })
    : (agg.startedAtMs ? t("analytics.aggregate.lifetimeStart", { date: formatDate(agg.startedAtMs) }) : "");

  const trendEmojis = agg.recentOutcomes.map((w) => (w ? "✅" : "❌")).join("");
  const playlist = agg.byTeamSize.length === 0
    ? `<p class="muted">${t("analytics.aggregate.noPlaylist")}</p>`
    : agg.byTeamSize.map((b) => {
        const total = b.wins + b.losses;
        const wr = total > 0 ? Math.round((b.wins / total) * 100) : 0;
        return `<span class="playlist-pill">${b.teamSize}v${b.teamSize} · W ${b.wins} / L ${b.losses} · ${wr}%</span>`;
      }).join(" ");

  const resetButton = scope === "session"
    ? `<button class="ghost" id="btn-start-new-session">${t("analytics.aggregate.resetSession")}</button>`
    : "";

  return /* html */ `
    <div class="analytics-aggregate">
      <header class="agg-header">
        <p class="muted">${esc(sinceLine)}</p>
        ${resetButton}
      </header>

      <div class="agg-cards">
        <div class="agg-card">
          <div class="agg-card-num">${agg.matches}</div>
          <div class="agg-card-lbl">${t("analytics.aggregate.matches")}</div>
        </div>
        <div class="agg-card">
          <div class="agg-card-num">${winRate.toFixed(1)}%</div>
          <div class="agg-card-lbl">${t("analytics.aggregate.winRate")}</div>
        </div>
        <div class="agg-card">
          <div class="agg-card-num">${esc(streakStr)}</div>
          <div class="agg-card-lbl">${t("analytics.aggregate.streak")}</div>
        </div>
        <div class="agg-card">
          <div class="agg-card-num">${esc(bestStr)}</div>
          <div class="agg-card-lbl">${t("analytics.aggregate.best")}</div>
        </div>
      </div>

      <details class="panel" open>
        <summary><span class="panel-chevron">▸</span><h2>${t("analytics.aggregate.trend")}</h2></summary>
        <div class="panel-body">
          <p class="trend-emojis">${trendEmojis}</p>
        </div>
      </details>

      <details class="panel" open>
        <summary><span class="panel-chevron">▸</span><h2>${t("analytics.aggregate.averages")}</h2></summary>
        <div class="panel-body">
          <p>
            ${t("analytics.aggregate.goals")} ${agg.avgGoals.toFixed(1)}
            · ${t("analytics.aggregate.shots")} ${agg.avgShots.toFixed(1)}
            · ${t("analytics.aggregate.saves")} ${agg.avgSaves.toFixed(1)}
            · ${t("analytics.aggregate.assists")} ${agg.avgAssists.toFixed(1)}
            · ${t("analytics.aggregate.score")} ${Math.round(agg.avgScore)}
            · ${t("analytics.aggregate.demos")} ${agg.avgDemos.toFixed(1)}
          </p>
          ${agg.avgBoost != null ? `<p class="muted">
            ${t("analytics.aggregate.avgBoost")} ${Math.round(agg.avgBoost)}
            ${agg.avgBpm != null ? ` · BPM ${Math.round(agg.avgBpm)}⚠` : ""}
            ${agg.avgSupersonicPct != null ? ` · ${t("analytics.adv.super")} ${agg.avgSupersonicPct.toFixed(0)}%` : ""}
            ${agg.avgAerialPct != null ? ` · ${t("analytics.adv.aerial")} ${agg.avgAerialPct.toFixed(0)}%` : ""}
          </p>` : ""}
        </div>
      </details>

      <details class="panel">
        <summary><span class="panel-chevron">▸</span><h2>${t("analytics.aggregate.byPlaylist")}</h2></summary>
        <div class="panel-body">${playlist}</div>
      </details>

      <details class="panel">
        <summary><span class="panel-chevron">▸</span><h2>${t("analytics.aggregate.records")}</h2></summary>
        <div class="panel-body">
          ${agg.bestMatchGuid ? `<p>🏆 ${t("analytics.aggregate.bestMatch")}: ${agg.bestMatchScore} pts <button class="link" data-go-match="${esc(agg.bestMatchGuid)}">${t("analytics.aggregate.openMatch")}</button></p>` : ""}
          ${agg.worstMatchGuid ? `<p>💀 ${t("analytics.aggregate.worstMatch")}: <button class="link" data-go-match="${esc(agg.worstMatchGuid)}">${t("analytics.aggregate.openMatch")}</button></p>` : ""}
        </div>
      </details>
    </div>
  `;
}

export function bindAggregateView(rerender: () => void) {
  const reset = document.getElementById("btn-start-new-session");
  if (reset) reset.addEventListener("click", async () => {
    if (!confirm(t("analytics.aggregate.resetConfirm"))) return;
    await invoke("start_new_session").catch((e) => console.error(e));
    rerender();
  });
  document.querySelectorAll<HTMLButtonElement>("button[data-go-match]").forEach((b) => {
    b.addEventListener("click", () => {
      const guid = b.dataset.goMatch || null;
      if (!guid) return;
      activeMatchDetail = guid;
      activeTab = "history";
      writeTab(activeTab);
      rerender();
    });
  });
}

// ----- Helpers -------------------------------------------------------------

/// Render a collapsible panel whose open/closed state is persisted in
/// localStorage under `panel:<key>`. Mirrors the pattern from main.ts so the
/// existing `bindPanelCollapse()` (called from main.ts) handles toggle events.
function detailsPanel(key: string, title: string, defaultOpen: boolean, body: string): string {
  const isOpen = panelOpen(key, defaultOpen);
  return /* html */ `
    <details class="panel" data-panel-key="${esc(key)}" ${isOpen ? "open" : ""}>
      <summary>
        <span class="panel-chevron">▸</span>
        <h2>${title}</h2>
      </summary>
      <div class="panel-body">${body}</div>
    </details>
  `;
}

function panelOpen(key: string, defaultOpen: boolean): boolean {
  try {
    const v = localStorage.getItem(`panel:${key}`);
    if (v === "open") return true;
    if (v === "closed") return false;
  } catch (_) { /* ignore */ }
  return defaultOpen;
}

/// Persist `<details>` open/closed state for any panel that has a
/// `data-panel-key` attribute. Idempotent — adding twice is a no-op.
export function bindAnalyticsPanelToggles() {
  document.querySelectorAll<HTMLDetailsElement>("details.panel[data-panel-key]").forEach((el) => {
    if ((el as any)._toggleBound) return;
    (el as any)._toggleBound = true;
    el.addEventListener("toggle", () => {
      try {
        localStorage.setItem(`panel:${el.dataset.panelKey!}`, el.open ? "open" : "closed");
      } catch (_) { /* ignore */ }
    });
  });
}

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));
}
function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.max(0, seconds - m * 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}
function formatClock(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.max(0, Math.floor(seconds - m * 60));
  return `${m}:${s.toString().padStart(2, "0")}`;
}
function formatArena(arena: string): string {
  return arena.replace(/_P$/i, "").replace(/_/g, " ");
}
function formatTimeAgo(ms: number): string {
  const diff = (Date.now() - ms) / 1000;
  if (diff < 60) return t("analytics.time.justNow");
  if (diff < 3600) return t("analytics.time.minutesAgo", { n: Math.round(diff / 60).toString() });
  if (diff < 86400) return t("analytics.time.hoursAgo", { n: Math.round(diff / 3600).toString() });
  if (diff < 7 * 86400) return t("analytics.time.daysAgo", { n: Math.round(diff / 86400).toString() });
  return new Date(ms).toLocaleDateString();
}
function formatDate(ms: number): string {
  return new Date(ms).toLocaleDateString();
}
