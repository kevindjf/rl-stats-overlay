import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { relaunch } from "@tauri-apps/plugin-process";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { check as checkForUpdate } from "@tauri-apps/plugin-updater";

import { type LangPref, setLanguage, t } from "./i18n";
import { runWizard } from "./wizard";
import * as analytics from "./analytics";

// ----- Types ----------------------------------------------------------------

interface Session {
  wins: number;
  losses: number;
  streak: number;
  best_win_streak: number;
  best_loss_streak: number;
  last_update: number;
}

// Camel-cased to mirror the `serde(rename_all = "camelCase")` attribute on
// the Rust side — these come straight off the wire as-is.
interface PlayerStats {
  primaryId: string;
  name: string;
  teamNum: number;
  goals: number;
  saves: number;
  shots: number;
  assists: number;
  score: number;
}

interface MatchStats {
  players: PlayerStats[];
  teamScores: [number, number];
  timeSeconds: number;
  overtime: boolean;
}

interface StateSnapshot {
  connected: boolean;
  player_name: string;
  primary_id: string;
  setup_done: boolean;
  hud_visible: boolean;
  http_port: number;
  session: Session;
  settings_path: string;
  overlay_url: string;
  theme: string;
  theme_vars: Record<string, string | number | boolean>;
  hud_x: number;
  hud_y: number;
  hud_w: number;
  hud_h: number;
  hud_scale_base_w: number;
  hud_scale_base_h: number;
  count_team_sizes: number[];
  language: LangPref;
  has_local_platform_candidates: boolean;
  hud_position_locked: boolean;
  hud_edit_mode: boolean;
  auto_hide_hud_when_offline: boolean;
  match_stats: MatchStats;
  no_auto_install: boolean;
  launcher_enabled: boolean;
  match_in_progress: boolean;
  analytics_enabled: boolean;
  show_post_match_hud: boolean;
  show_post_match_obs: boolean;
}

// ----- Theme schema ---------------------------------------------------------
// Each entry describes one user-editable knob for a theme. The backend stores
// raw values; the schema only tells the UI what type of input to render and
// what default to use as a hint when the user hasn't overridden anything yet.

type VarType =
  | { kind: "color";   default: string }
  | { kind: "number";  default: number; min: number; max: number; step?: number; unit?: string }
  | { kind: "boolean"; default: boolean };

interface ThemeVarDef {
  key: string;
  label: string;
  group?: string;
  spec: VarType;
}

interface ThemeDef {
  id: string;
  label: string;
  description: string;
  vars: ThemeVarDef[];
}

// Themes are now discovered dynamically — bundled themes ship a
// `theme.json` next to their CSS/HTML, and user-installed themes drop a
// folder in `%APPDATA%/RLStatsOverlay/themes/<id>/`. The Rust side scans
// both locations and returns the merged manifest list via the
// `list_themes` Tauri command. See THEMES.md for the contract.
//
// `THEMES` below is a cache populated on first call to `loadThemes()`.
let THEMES: ThemeDef[] = [];

interface RawManifest {
  manifestVersion?: number;
  id: string;
  label: string;
  description?: string;
  author?: string;
  vars: ThemeVarDef[];
  user_installed?: boolean;
}

async function loadThemes(): Promise<ThemeDef[]> {
  const list = await invoke<RawManifest[]>("list_themes").catch(() => []);
  THEMES = list.length
    ? list.map((m) => ({
        id: m.id,
        label: m.label,
        description: m.description ?? "",
        vars: m.vars ?? [],
      }))
    : _FALLBACK_THEMES;
  return THEMES;
}

// Hardcoded fallback so the UI doesn't go blank if `list_themes` ever
// fails (manifest parse errors, fs glitch). Empty list is a degraded
// state but the app stays usable — user can still toggle/copy URL etc.
const _FALLBACK_THEMES: ThemeDef[] = [
  {
    id: "circle",
    label: "Circle",
    description: "3 cards arching around the boost gauge — RocketStats classic.",
    vars: [
      { key: "colorCard",       label: "Card background",      group: "Colors", spec: { kind: "color", default: "#16181d" } },
      { key: "colorWin",        label: "Wins / win-streak",    group: "Colors", spec: { kind: "color", default: "#5dd16f" } },
      { key: "colorLoss",       label: "Losses / loss-streak", group: "Colors", spec: { kind: "color", default: "#ff5c5c" } },
      { key: "colorStreakIcon", label: "Streak icon (flame)",  group: "Colors", spec: { kind: "color", default: "#ffb13a" } },

      { key: "showIcons",       label: "Show icons",           group: "Layout", spec: { kind: "boolean", default: true } },
    ],
  },
  {
    id: "default",
    label: "Default",
    description: "RocketStats classic — small black slab, three text rows. Lightest possible look.",
    vars: [
      { key: "colorPanelBg",     label: "Panel background",  group: "Colors", spec: { kind: "color", default: "#000000" } },
      { key: "colorPanelBorder", label: "Panel border",      group: "Colors", spec: { kind: "color", default: "#c8c8c8" } },
      { key: "colorWin",         label: "Wins value",        group: "Colors", spec: { kind: "color", default: "#5dd16f" } },
      { key: "colorLoss",        label: "Losses value",      group: "Colors", spec: { kind: "color", default: "#ff5c5c" } },
      { key: "colorStreakIcon",  label: "Streak value",      group: "Colors", spec: { kind: "color", default: "#ffb13a" } },
    ],
  },
  {
    id: "redesigned",
    label: "Redesigned",
    description: "Big floating icons + bold values, no panel — RocketStats v2 style.",
    vars: [
      { key: "colorWin",         label: "Wins / win-streak",    group: "Colors", spec: { kind: "color", default: "#5dd16f" } },
      { key: "colorLoss",        label: "Losses / loss-streak", group: "Colors", spec: { kind: "color", default: "#ff5c5c" } },
      { key: "colorStreakIcon",  label: "Streak icon (flame)",  group: "Colors", spec: { kind: "color", default: "#ffb13a" } },
    ],
  },
  {
    id: "minimal",
    label: "Minimal",
    description: "Single dark glass panel with 3 rows — discreet, drops anywhere on screen.",
    vars: [
      { key: "colorPanelBg",     label: "Panel background",     group: "Colors", spec: { kind: "color", default: "#0f121a" } },
      { key: "colorPanelBorder", label: "Panel border",         group: "Colors", spec: { kind: "color", default: "#1e2230" } },
      { key: "colorWin",         label: "Wins / win-streak",    group: "Colors", spec: { kind: "color", default: "#5dd16f" } },
      { key: "colorLoss",        label: "Losses / loss-streak", group: "Colors", spec: { kind: "color", default: "#ff5c5c" } },
      { key: "colorStreakIcon",  label: "Streak icon (flame)",  group: "Colors", spec: { kind: "color", default: "#ffb13a" } },

      { key: "showIcons",        label: "Show icons",           group: "Layout", spec: { kind: "boolean", default: true } },
    ],
  },
  {
    id: "inline",
    label: "Inline",
    description: "Single horizontal line — fits along the top or bottom edge of the screen. Snap to 9 anchor presets from the dashboard.",
    vars: [
      { key: "colorPanelBg",     label: "Panel background", group: "Colors", spec: { kind: "color", default: "#0f121aCC" } },
      { key: "colorPanelBorder", label: "Panel border",     group: "Colors", spec: { kind: "color", default: "#1e2230" } },
      { key: "colorWin",         label: "Wins value",       group: "Colors", spec: { kind: "color", default: "#5dd16f" } },
      { key: "colorLoss",        label: "Losses value",     group: "Colors", spec: { kind: "color", default: "#ff5c5c" } },
      { key: "colorStreakIcon",  label: "Streak value",     group: "Colors", spec: { kind: "color", default: "#ffb13a" } },
    ],
  },
];

function themeById(id: string): ThemeDef | undefined {
  return THEMES.find((t) => t.id === id);
}

// ----- Bootstrap ------------------------------------------------------------

const root = document.getElementById("app")!;
let currentState: StateSnapshot | null = null;
// Whether the Session panel is in edit mode (inputs enabled + Save/Cancel
// shown). Module-scoped so the flag survives across renderDashboard()
// calls. Reset to false after Save / Cancel.
let isEditingSession = false;

// ----- Top-level tabs ------------------------------------------------------
type TopTab = "settings" | "history" | "session" | "alltime";

/// Feature flag: the History / Session / All-time analytics views are
/// fully implemented end-to-end (capture, storage, queries, UI) but
/// hidden in the v1 release so we ship a focused product around the
/// post-match HUD first. Flip to `true` for the v2 release that exposes
/// the analytics tabs. Backend keeps recording matches in either mode.
const FEATURE_ANALYTICS_TABS = false;

function readTopTab(): TopTab {
  if (!FEATURE_ANALYTICS_TABS) return "settings";
  try {
    const v = localStorage.getItem("topTab");
    if (v === "settings" || v === "history" || v === "session" || v === "alltime") return v;
  } catch (_) { /* ignore */ }
  return "settings";
}
function writeTopTab(v: TopTab) {
  try { localStorage.setItem("topTab", v); } catch (_) { /* ignore */ }
}
let activeTopTab: TopTab = readTopTab();
// Sync analytics' internal tab when starting in an analytics view.
if (FEATURE_ANALYTICS_TABS && activeTopTab !== "settings") {
  analytics.setActiveTab(activeTopTab);
}

// ----- Collapsible panels ---------------------------------------------------

/// Lookup whether a collapsible panel was last left open. We use the native
/// `<details>` element for the toggle, but its `open` state isn't persisted
/// across reloads on its own — we mirror it to localStorage keyed by the
/// panel's stable `data-panel-key` so the dashboard remembers each section's
/// preferred state across app restarts.
function panelOpen(key: string, defaultOpen: boolean): boolean {
  try {
    const v = localStorage.getItem(`panel:${key}`);
    if (v === "open") return true;
    if (v === "closed") return false;
  } catch (_) { /* private mode / quota — fall through to default */ }
  return defaultOpen;
}

/// Wrap a section's body in a `<details>` whose summary acts as the
/// expand/collapse trigger. `headerActions` (optional) renders inline next to
/// the title — buttons in the summary stop click propagation in
/// `bindPanelCollapse` so clicking them doesn't toggle the section.
function renderPanel(
  key: string,
  title: string,
  body: string,
  opts: { defaultOpen?: boolean; headerActions?: string; summaryInfo?: string } = {},
): string {
  const open = panelOpen(key, opts.defaultOpen ?? true);
  const actions = opts.headerActions
    ? `<div class="panel-actions" data-no-toggle>${opts.headerActions}</div>`
    : "";
  const info = opts.summaryInfo
    ? `<span class="panel-summary-info">${opts.summaryInfo}</span>`
    : "";
  return /* html */ `
    <details class="panel" data-panel-key="${escapeHtml(key)}" ${open ? "open" : ""}>
      <summary>
        <span class="panel-chevron" aria-hidden="true">▸</span>
        <h2>${title}</h2>
        ${info}
        ${actions}
      </summary>
      <div class="panel-body">
        ${body}
      </div>
    </details>
  `;
}

/// Persist the open/closed state of every collapsible panel to localStorage,
/// and prevent buttons / inputs inside `<summary>` from toggling the panel
/// when clicked (they live there because they belong to the section header).
function bindPanelCollapse() {
  document
    .querySelectorAll<HTMLDetailsElement>("details.panel[data-panel-key]")
    .forEach((el) => {
      const key = el.dataset.panelKey!;
      el.addEventListener("toggle", () => {
        try {
          localStorage.setItem(`panel:${key}`, el.open ? "open" : "closed");
        } catch (_) { /* ignore quota */ }
      });
    });
  // Stop the propagation that would otherwise reach <summary> and toggle
  // the panel. We tag the action wrapper with `data-no-toggle` and catch
  // every interactive event there.
  document
    .querySelectorAll<HTMLElement>("details.panel summary [data-no-toggle]")
    .forEach((el) => {
      ["click", "mousedown", "keydown"].forEach((evt) =>
        el.addEventListener(evt, (e) => e.stopPropagation()),
      );
    });
}

async function refresh(): Promise<StateSnapshot> {
  currentState = await invoke<StateSnapshot>("get_state");

  // Don't replace the DOM while a form control has focus inside our
  // document — that would close native pickers (color, OS combo) and
  // interrupt slider drags. The next refresh after blur picks up.
  const active = document.activeElement;
  if (
    active instanceof HTMLInputElement ||
    active instanceof HTMLSelectElement ||
    active instanceof HTMLTextAreaElement
  ) {
    return currentState;
  }

  if (!currentState.setup_done) {
    runWizard(root, currentState, async () => {
      await refresh();
    });
  } else {
    renderDashboard();
  }
  return currentState;
}

// Resolve UI language before the first render so labels start in the right
// locale. We read settings.language directly (not via refresh→get_state)
// to avoid a flash of FR before the snapshot lands.
const bootSnapshot = await invoke<StateSnapshot>("get_state").catch(() => null);
setLanguage(bootSnapshot?.language ?? "auto");

await loadThemes();
await refresh();

// Background update check. We delay by 3s so it doesn't compete with the
// initial render + WS handshake. The banner mounts itself if a release is
// found; otherwise this stays silent.
setTimeout(() => {
  checkForUpdates().catch((err) =>
    console.warn("update check failed:", err),
  );
}, 3000);

// Live updates pushed by the Rust side.
listen("rlstats://connected", () => refresh());
listen("rlstats://session-changed", () => refresh());
// Right-clicking the HUD's "Toggle lock" entry flips the bool server-side
// without going through the Tauri command — keep the dashboard checkbox in
// sync with the persisted value.
listen("rlstats://hud-lock-changed", () => refresh());
// Match start/stop drives both the floating launcher's auto-hide and the
// dashboard's `match_in_progress` flag — refresh so any in-panel mirror
// (e.g. the launcher checkbox label) tracks reality.
listen("rlstats://match-in-progress", () => refresh());
// Per-match stats stream — already debounced server-side to ~250ms.
listen<MatchStats>("rlstats://match-stats", (event) => {
  if (currentState) {
    currentState.match_stats = event.payload;
  }
  // The match_stats live block only exists on the Réglages tab. Re-rendering
  // any other tab on every tick destroys the user's open <details> sections
  // (statfeed, possession, etc.) — skip those entirely.
  if (activeTopTab !== "settings") return;
  refresh().catch(() => {});
});

// Re-poll every second so the connected dot stays accurate even if events drop.
setInterval(() => {
  // Same reason: don't re-render the analytics tabs on the heartbeat — the
  // `connected` indicator is only shown on Réglages anyway.
  if (activeTopTab !== "settings") return;
  refresh().catch(() => {});
}, 2000);

// ----- Updater --------------------------------------------------------------

async function checkForUpdates(): Promise<void> {
  const update = await checkForUpdate();
  if (!update) return;
  mountUpdateBanner(update.version, async () => {
    await update.downloadAndInstall();
    await relaunch();
  });
}

function mountUpdateBanner(version: string, onInstall: () => Promise<void>): void {
  if (document.getElementById("update-banner")) return;
  const banner = document.createElement("div");
  banner.id = "update-banner";
  banner.style.cssText = [
    "position: fixed",
    "top: 0",
    "left: 0",
    "right: 0",
    "z-index: 9999",
    "padding: 10px 16px",
    "background: #2563eb",
    "color: white",
    "display: flex",
    "align-items: center",
    "justify-content: space-between",
    "gap: 12px",
    "box-shadow: 0 2px 8px rgba(0,0,0,0.25)",
    "font-size: 13px",
  ].join(";");
  banner.innerHTML = /* html */ `
    <span>${t("update.banner", { version: escapeHtml(version) })}</span>
    <span style="display:flex; gap:8px;">
      <button id="btn-update-install" class="primary" style="padding: 4px 12px;">${t("update.install")}</button>
      <button id="btn-update-dismiss" class="ghost" style="padding: 4px 12px; color: white; border-color: rgba(255,255,255,0.4);">${t("update.dismiss")}</button>
    </span>
  `;
  document.body.prepend(banner);
  banner.querySelector("#btn-update-install")?.addEventListener("click", async () => {
    const btn = banner.querySelector("#btn-update-install") as HTMLButtonElement | null;
    if (btn) {
      btn.disabled = true;
      btn.textContent = t("update.downloading");
    }
    try {
      await onInstall();
    } catch (err) {
      console.error("update install failed:", err);
      if (btn) {
        btn.disabled = false;
        btn.textContent = t("update.retry");
      }
    }
  });
  banner.querySelector("#btn-update-dismiss")?.addEventListener("click", () => {
    banner.remove();
  });
}

// ----- Dashboard view -------------------------------------------------------

function renderDashboard() {
  if (!currentState) return;

  if (activeTopTab !== "settings") {
    renderAnalyticsTab().catch((err) => console.error("renderAnalyticsTab failed", err));
    return;
  }
  const s = currentState;

  const obsUrl = s.overlay_url || "starting…";

  root.innerHTML = /* html */ `
    <main>
      <header style="display:flex; justify-content: space-between; align-items: flex-end; margin-bottom: 18px;">
        <div>
          <h1>🎮 RL Stats Overlay</h1>
          <p class="subtitle">${t("header.subtitle")}</p>
        </div>
        <span class="badge"><span class="dot ${s.connected ? "ok" : ""}" id="conn-dot"></span>${
          s.connected ? t("header.connected") : t("header.waiting")
        }</span>
      </header>

      ${renderTopTabsBar()}

      ${renderPanel("session", t("session.title"), /* html */ `
        <div class="session">
          <div class="stat win">
            <input class="num session-edit" type="number" data-field="wins" value="${s.session.wins}" min="0" ${isEditingSession ? "" : "disabled"} />
            <div class="lbl">${t("session.wins")}</div>
          </div>
          <div class="stat loss">
            <input class="num session-edit" type="number" data-field="losses" value="${s.session.losses}" min="0" ${isEditingSession ? "" : "disabled"} />
            <div class="lbl">${t("session.losses")}</div>
          </div>
          <div class="stat streak ${s.session.streak > 0 ? "win" : s.session.streak < 0 ? "loss" : ""}">
            <input class="num session-edit" type="number" data-field="streak" value="${s.session.streak}" ${isEditingSession ? "" : "disabled"} />
            <div class="lbl">${t("session.streak")}</div>
          </div>
        </div>
        <p class="muted session-records" style="margin-top: 12px; font-size: 12px;">
          ${t("session.recordsLeading")}
          <input class="session-edit-inline" type="number" data-field="best_win_streak" value="${s.session.best_win_streak}" min="0" ${isEditingSession ? "" : "disabled"} />
          ${t("session.recordsMid")}
          <input class="session-edit-inline" type="number" data-field="best_loss_streak" value="${s.session.best_loss_streak}" min="0" ${isEditingSession ? "" : "disabled"} />
        </p>

        <div class="team-size-filter" style="margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border, rgba(255,255,255,0.08));">
          <label style="font-weight: 600; font-size: 13px;">${t("session.filter.label")}</label>
          <div class="row" style="gap: 14px; margin-top: 6px;">
            ${[1, 2, 3, 4]
              .map(
                (n) => `
              <label class="team-size-toggle">
                <input type="checkbox" class="team-size-input" data-size="${n}" ${
                  s.count_team_sizes.includes(n) ? "checked" : ""
                } />
                ${n}v${n}
              </label>`,
              )
              .join("")}
          </div>
          <p class="muted" style="margin-top: 8px; font-size: 11px; line-height: 1.4;">
            ${t("session.filter.note")}
          </p>
        </div>
      `, {
        summaryInfo: /* html */ `<strong class="info-win">${s.session.wins}W</strong> · <strong class="info-loss">${s.session.losses}L</strong> · <strong>${s.session.streak >= 0 ? "+" : ""}${s.session.streak}</strong>`,
        headerActions: isEditingSession
            ? /* html */ `
              <button class="ghost" id="btn-session-cancel">${t("session.editCancel")}</button>
              <button class="primary" id="btn-session-save">${t("session.editSave")}</button>`
            : /* html */ `
              <button class="ghost" id="btn-session-edit">${t("session.editStart")}</button>
              <button class="ghost" id="btn-reset">${t("session.reset")}</button>`,
        })}

      ${renderPanel("player", t("player.title"), /* html */ `
        ${s.has_local_platform_candidates
          ? /* html */ `
            <div class="row" style="align-items: center;">
              <div style="flex: 1;">
                <label>${t("player.label")}</label>
                <div class="player-detected">
                  ${s.player_name
                    ? `<strong>${escapeHtml(s.player_name)}</strong>`
                    : `<span class="muted">${t("player.detectedWaiting")}</span>`}
                </div>
              </div>
            </div>
            <p class="muted" style="margin-top: 8px; font-size: 11px;">
              ${t("player.detectedNote")}
            </p>`
          : /* html */ `
            <div class="row">
              <div style="flex: 1;">
                <label for="player-input">${t("player.label")}</label>
                <input type="text" id="player-input" value="${escapeHtml(s.player_name)}" placeholder="${escapeHtml(t("player.placeholder"))}" />
              </div>
              <button class="primary" id="btn-save-name" style="margin-top: 18px;">${t("player.save")}</button>
            </div>`
        }
        <p class="muted" style="margin-top: 8px; font-size: 11px;">
          ${s.primary_id
            ? t("player.idCaptured", { id: escapeHtml(s.primary_id) })
            : t("player.idPending")}
        </p>
      `, {
        summaryInfo: s.player_name
          ? `<strong>${escapeHtml(s.player_name)}</strong>`
          : `<em class="muted">${t("player.detectedWaiting")}</em>`,
      })}

      ${renderPanel("appearance", t("appearance.title"), /* html */ `
        <div class="row" style="margin-top: 6px;">
          <label style="display:flex; align-items:center; gap:8px; font-size: 13px;">
            <input type="checkbox" id="launcher-enable" ${s.launcher_enabled ? "checked" : ""} />
            <span>${t("launcher.enable")}</span>
          </label>
        </div>
        <p class="muted" style="margin: 6px 0 0; font-size: 11px;">${t("launcher.enableHint")}</p>

        <div class="row" style="margin-top: 14px;">
          <label style="display:flex; align-items:center; gap:8px; font-size: 13px;">
            <input type="checkbox" id="auto-hide-hud" ${s.auto_hide_hud_when_offline ? "checked" : ""} />
            <span>${t("hud.autoHide")}</span>
          </label>
        </div>
      `)}

      ${renderPanel("hud", t("hud.title"), /* html */ `
        <p class="muted" style="margin-top: 0;">${t("hud.note")}</p>
        <div class="row" style="margin-top: 12px;">
          <button class="primary" id="btn-toggle-hud">${
            s.hud_visible ? t("hud.hide") : t("hud.show")
          }</button>
          <button id="btn-reload-hud" title="${escapeHtml(t("hud.reloadTitle"))}">${t("hud.reload")}</button>
        </div>

        ${renderMatchStatsBlock(s)}

        <div class="hud-geom">
          <label class="hud-edit-toggle" style="display:flex; align-items:center; gap:8px; font-size: 13px;">
            <input type="checkbox" id="hud-edit-mode" ${s.hud_edit_mode ? "checked" : ""} />
            <strong>${t("hud.editMode")}</strong>
          </label>
          <p class="muted" style="margin: 4px 0 12px; font-size: 11px;">${t("hud.editModeHint")}</p>

          ${renderHudScale(s)}

          <div class="row" style="gap: 16px; align-items: flex-end; margin-top: 14px;">
            <label class="step-picker">${t("hud.step")}
              <select id="geom-step">
                <option value="1">1 px</option>
                <option value="5" selected>5 px</option>
                <option value="10">10 px</option>
                <option value="50">50 px</option>
              </select>
            </label>
          </div>

          <div class="geom-grid">
            ${renderGeomStepper("X",      "geom-x", s.hud_x)}
            ${renderGeomStepper("Y",      "geom-y", s.hud_y)}
            ${renderGeomStepper("Width",  "geom-w", s.hud_w)}
            ${renderGeomStepper("Height", "geom-h", s.hud_h)}
          </div>

          ${renderHudSnapPresets()}
        </div>
      `, {
        summaryInfo: (() => {
          const baseW = s.hud_scale_base_w || 400;
          const pct = Math.round((s.hud_w / baseW) * 100);
          const visible = s.hud_visible
            ? `<span class="info-ok">●</span> ${t("hud.show").replace(/^▶\s*/, "")}`
            : `<span class="muted">○ ${t("hud.hide").replace(/^🟢\s*/, "").replace(/\s*—\s*.*/, "")}</span>`;
          return `${pct}% · ${visible}`;
        })(),
      })}

      ${renderPanel("obs", t("obs.title"), /* html */ `
        <p class="muted" style="margin-top: 0;">${t("obs.note")}</p>
        <div class="url-pill" id="url-pill">${escapeHtml(obsUrl)}</div>
        <div class="row" style="margin-top: 12px;">
          <button class="primary" id="btn-copy-url" ${s.http_port === 0 ? "disabled" : ""}>${t("obs.copy")}</button>
          <button id="btn-open-url" ${s.http_port === 0 ? "disabled" : ""}>${t("obs.preview")}</button>
        </div>
      `, {
        summaryInfo: s.http_port === 0
          ? `<em class="muted">…</em>`
          : `<code class="info-code">localhost:${s.http_port}</code>`,
      })}

      ${renderAnalyticsSettingsPanel(s)}

      ${renderThemeSection(s)}

      <footer>
        <p style="margin: 0;">${t("footer.settingsAt", { path: escapeHtml(s.settings_path) })}</p>
        <div class="row" style="margin: 8px 0 12px; align-items: center; gap: 10px;">
          <button id="btn-open-logs" class="ghost" title="${escapeHtml(t("footer.openLogsTitle"))}">${t("footer.openLogs")}</button>
          <label style="display:flex; align-items:center; gap:6px; font-size: 12px;">${t("footer.language")}
            <select id="lang-select">
              <option value="auto" ${s.language === "auto" ? "selected" : ""}>${t("footer.langAuto")}</option>
              <option value="fr" ${s.language === "fr" ? "selected" : ""}>${t("footer.langFr")}</option>
              <option value="en" ${s.language === "en" ? "selected" : ""}>${t("footer.langEn")}</option>
            </select>
          </label>
        </div>
        <p class="muted" style="margin: 6px 0 12px;">
          ${t("footer.trayHint")}
        </p>
        <button id="btn-quit-app" class="ghost">${t("footer.quit")}</button>
      </footer>
    </main>
  `;

  document.getElementById("btn-save-name")?.addEventListener("click", onSaveName);
  document.getElementById("btn-reset")?.addEventListener("click", onResetSession);
  // Session edit toggle. Edit unlocks all 5 fields, Save commits them
  // atomically via set_session_full, Cancel discards by re-rendering
  // from the persisted snapshot.
  document.getElementById("btn-session-edit")?.addEventListener("click", () => {
    isEditingSession = true;
    renderDashboard();
    // Auto-focus the wins field so the user can start typing right away.
    (document.querySelector<HTMLInputElement>(
      '.session-edit[data-field="wins"]',
    ))?.focus();
  });
  document.getElementById("btn-session-cancel")?.addEventListener("click", () => {
    isEditingSession = false;
    renderDashboard();
  });
  document.getElementById("btn-session-save")?.addEventListener("click", async () => {
    const readField = (field: string): number | null => {
      const el = document.querySelector<HTMLInputElement>(
        `.session-edit[data-field="${field}"], .session-edit-inline[data-field="${field}"]`,
      );
      if (!el) return null;
      const v = parseInt(el.value, 10);
      return Number.isNaN(v) ? null : v;
    };
    const wins = readField("wins") ?? currentState!.session.wins;
    const losses = readField("losses") ?? currentState!.session.losses;
    const streak = readField("streak") ?? currentState!.session.streak;
    const best_win_streak =
      readField("best_win_streak") ?? currentState!.session.best_win_streak;
    const best_loss_streak =
      readField("best_loss_streak") ?? currentState!.session.best_loss_streak;
    try {
      await invoke("set_session_full", {
        wins,
        losses,
        streak,
        bestWinStreak: best_win_streak,
        bestLossStreak: best_loss_streak,
      });
      isEditingSession = false;
      await refresh();
    } catch (err) {
      console.error("set_session_full failed", err);
      alert(String(err));
    }
  });
  bindTeamSizeFilter();
  document.getElementById("btn-toggle-hud")?.addEventListener("click", onToggleHud);
  document.getElementById("btn-reload-hud")?.addEventListener("click", onReloadHud);
  document.getElementById("hud-edit-mode")?.addEventListener("change", async (e) => {
    const enabled = (e.target as HTMLInputElement).checked;
    await invoke("set_hud_edit_mode", { enabled });
    await refresh();
  });
  document.getElementById("auto-hide-hud")?.addEventListener("change", async (e) => {
    const enabled = (e.target as HTMLInputElement).checked;
    await invoke("set_auto_hide_hud_when_offline", { enabled });
  });
  document.getElementById("launcher-enable")?.addEventListener("change", async (e) => {
    const enabled = (e.target as HTMLInputElement).checked;
    await invoke("set_launcher_enabled", { enabled });
    await refresh();
  });
  document.getElementById("btn-copy-url")?.addEventListener("click", onCopyUrl);
  document.getElementById("btn-open-url")?.addEventListener("click", onOpenUrl);
  document.getElementById("btn-open-logs")?.addEventListener("click", () => {
    invoke("open_logs_folder").catch((err) => console.error(err));
  });
  document.getElementById("lang-select")?.addEventListener("change", async (e) => {
    const lang = (e.target as HTMLSelectElement).value as LangPref;
    await invoke("set_language", { language: lang });
    setLanguage(lang);
    await refresh();
  });
  document.getElementById("btn-quit-app")?.addEventListener("click", onQuitApp);

  bindGeomListeners();
  bindHudSnapListeners();
  bindHudScale();
  bindThemeListeners();
  bindAnalyticsSettings();
  bindPanelCollapse();
  bindTopTabs();
}

function bindAnalyticsSettings() {
  document.getElementById("pm-analytics-enabled")?.addEventListener("change", async (e) => {
    const enabled = (e.target as HTMLInputElement).checked;
    if (!enabled) {
      const ok = confirm(t("postMatch.disableConfirm"));
      if (!ok) {
        (e.target as HTMLInputElement).checked = true;
        return;
      }
    }
    await invoke("set_analytics_enabled", { enabled });
    await refresh();
  });
  document.getElementById("pm-show-hud")?.addEventListener("change", async (e) => {
    const enabled = (e.target as HTMLInputElement).checked;
    await invoke("set_show_post_match_hud", { enabled });
  });
  document.getElementById("pm-show-obs")?.addEventListener("change", async (e) => {
    const enabled = (e.target as HTMLInputElement).checked;
    await invoke("set_show_post_match_obs", { enabled });
  });
  document.getElementById("pm-open-folder")?.addEventListener("click", () => {
    invoke("open_data_folder").catch((err) => console.error(err));
  });
  document.getElementById("pm-clear-history")?.addEventListener("click", async () => {
    if (!confirm(t("postMatch.clearConfirm"))) return;
    await invoke("clear_history").catch((err) => alert(err));
    await refresh();
  });
}

// ----- Top tabs bar + analytics tab routing -------------------------------

function renderTopTabsBar(): string {
  // v1 ships with only the post-match HUD surface; analytics tabs stay
  // hidden behind the feature flag (code is wired and ready for v2).
  if (!FEATURE_ANALYTICS_TABS) return "";
  const tab = (id: TopTab, label: string) =>
    `<button class="top-tab ${activeTopTab === id ? "active" : ""}" data-top-tab="${id}">${label}</button>`;
  return /* html */ `
    <nav class="top-tabs" role="tablist">
      ${tab("settings", t("topTabs.settings"))}
      ${tab("history", t("topTabs.history"))}
      ${tab("session", t("topTabs.session"))}
      ${tab("alltime", t("topTabs.alltime"))}
    </nav>
  `;
}

function bindTopTabs() {
  document.querySelectorAll<HTMLButtonElement>("button.top-tab[data-top-tab]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const v = btn.dataset.topTab as TopTab | undefined;
      if (!v || v === activeTopTab) return;
      activeTopTab = v;
      writeTopTab(v);
      if (v !== "settings") analytics.setActiveTab(v);
      renderDashboard();
    });
  });
}

async function renderAnalyticsTab() {
  if (!currentState) return;
  const s = currentState;
  const profileSelectorHtml = await analytics.renderProfileSelector(s.primary_id || "");
  const tabBody = await analytics.renderActiveTab();

  root.innerHTML = /* html */ `
    <main>
      <header style="display:flex; justify-content: space-between; align-items: flex-end; margin-bottom: 18px;">
        <div>
          <h1>🎮 RL Stats Overlay</h1>
          <p class="subtitle">${t("header.subtitle")}</p>
        </div>
        <span class="badge"><span class="dot ${s.connected ? "ok" : ""}" id="conn-dot"></span>${
          s.connected ? t("header.connected") : t("header.waiting")
        }</span>
      </header>

      ${renderTopTabsBar()}

      <div class="analytics-toolbar">
        ${profileSelectorHtml}
      </div>

      <div class="analytics-body">
        ${tabBody}
      </div>
    </main>
  `;

  bindTopTabs();
  analytics.bindProfileSelector(() => renderDashboard());
  analytics.bindHistoryList(() => renderDashboard());
  analytics.bindMatchDetail(() => renderDashboard());
  analytics.bindAggregateView(() => renderDashboard());
  analytics.bindAnalyticsPanelToggles();
  // Re-render whenever the backend writes a new match.
  analytics.ensureMatchRecordedListener(() => {
    if (activeTopTab !== "settings") renderDashboard();
  });
}

// ----- Per-match stats block -----------------------------------------------

/// Tiny "this match: G/S/Sh/A" readout under the HUD panel. Shows the local
/// player's stats by matching `primary_id` against the players[] list. Renders
/// a placeholder before the first match to make it obvious the data flow is
/// live but quiet.
function renderMatchStatsBlock(s: StateSnapshot): string {
  const me = s.match_stats?.players?.find((p) => p.primaryId === s.primary_id);
  const hint = `<p class="muted" style="margin: 6px 0 0; font-size: 11px;">${t("hud.matchHint")}</p>`;
  if (!me) {
    return /* html */ `
      <div class="match-stats" style="margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border, rgba(255,255,255,0.08));">
        <label style="font-weight: 600; font-size: 13px;">${t("hud.matchTitle")}</label>
        <p class="muted" style="margin: 4px 0 0; font-size: 12px;">${t("hud.matchEmpty")}</p>
        ${hint}
      </div>
    `;
  }
  return /* html */ `
    <div class="match-stats" style="margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border, rgba(255,255,255,0.08));">
      <label style="font-weight: 600; font-size: 13px;">${t("hud.matchTitle")}</label>
      <div style="margin-top: 4px; font-variant-numeric: tabular-nums; font-size: 13px;">
        <strong>${me.goals}</strong> G ·
        <strong>${me.saves}</strong> S ·
        <strong>${me.shots}</strong> Sh ·
        <strong>${me.assists}</strong> A
      </div>
      ${hint}
    </div>
  `;
}

// ----- HUD scale slider -----------------------------------------------------

/// Single "Scale %" slider on top of the HUD geometry steppers. 100 % matches
/// the factory-default 400×300 base (kept in sync with `DEFAULT_HUD_W/H` on
/// the Rust side). Reads the current percent from `hud_w / 400` so the slider
/// reflects whatever the user has set via the W/H steppers, the resize-handle
/// drag, or a previous scale call. Backend clamps to 25–400 %; we cap the UI
/// at 250 % to keep the slider's resolution useful.
function renderHudScale(s: StateSnapshot): string {
  const baseW = s.hud_scale_base_w || 400;
  const pct = Math.max(25, Math.min(400, Math.round((s.hud_w / baseW) * 100)));
  return /* html */ `
    <div class="hud-scale">
      <div class="hud-scale-row">
        <label for="hud-scale" class="geom-label">${t("hud.scaleLabel")}</label>
        <output id="hud-scale-value" class="hud-scale-value" for="hud-scale">${pct}%</output>
      </div>
      <input
        type="range"
        id="hud-scale"
        min="50"
        max="250"
        step="5"
        value="${pct}"
      />
      <p class="muted" style="margin: 4px 0 0; font-size: 11px;">${t("hud.scaleHint")}</p>
    </div>
  `;
}

function bindHudScale() {
  const input = document.getElementById("hud-scale") as HTMLInputElement | null;
  const output = document.getElementById("hud-scale-value") as HTMLOutputElement | null;
  if (!input) return;
  // Coalesce rapid drags: throttle the actual `set_hud_scale` calls to one
  // every 60 ms while the user drags. Without this, the slider fires `input`
  // at >60 Hz and we'd hammer the Rust side / disk writes for nothing — the
  // window itself only repaints at the refresh rate.
  let pending: number | null = null;
  let lastSent = 0;
  const flush = async (pct: number) => {
    lastSent = performance.now();
    pending = null;
    try {
      await invoke("set_hud_scale", { percent: pct });
    } catch (err) {
      console.error("set_hud_scale failed", err);
    }
  };
  input.addEventListener("input", () => {
    const pct = Number(input.value || "100");
    if (output) output.value = `${pct}%`;
    const now = performance.now();
    if (now - lastSent >= 60) {
      flush(pct);
    } else if (pending == null) {
      pending = window.setTimeout(() => flush(pct), 60);
    }
  });
  // Final commit on `change` (mouseup) so the last value always lands even
  // if it fell into the throttle window. Refreshes the dashboard so the
  // W/H steppers below pick up the new pixel dimensions on the next render
  // (the focus guard inside `refresh()` keeps the slider visually stable).
  input.addEventListener("change", async () => {
    const pct = Number(input.value || "100");
    if (pending != null) {
      window.clearTimeout(pending);
      pending = null;
    }
    await flush(pct);
    input.blur();
    await refresh();
  });
}

// ----- HUD geometry steppers -----------------------------------------------

function renderGeomStepper(label: string, idPrefix: string, value: number): string {
  return /* html */ `
    <div class="geom-stepper">
      <label class="geom-label">${escapeHtml(label)}</label>
      <div class="geom-row">
        <button class="geom-btn" data-axis="${idPrefix}" data-dir="-1" title="−">−</button>
        <input type="number" class="geom-input" id="${idPrefix}" data-axis="${idPrefix}" value="${value}" />
        <button class="geom-btn" data-axis="${idPrefix}" data-dir="1" title="+">+</button>
      </div>
    </div>`;
}

// ----- HUD snap presets ----------------------------------------------------

/// 3×3 grid of "snap to edge" buttons. Clicking one moves the HUD window so
/// it sits in the chosen corner / edge / center of the current monitor —
/// useful for placing the inline theme flush against the screen edge
/// without dragging.
function renderHudSnapPresets(): string {
  const presets: Array<{ id: string; label: string }> = [
    { id: "top-left",      label: t("hud.snap.topLeft") },
    { id: "top-center",    label: t("hud.snap.topCenter") },
    { id: "top-right",     label: t("hud.snap.topRight") },
    { id: "middle-left",   label: t("hud.snap.middleLeft") },
    { id: "center",        label: t("hud.snap.center") },
    { id: "middle-right",  label: t("hud.snap.middleRight") },
    { id: "bottom-left",   label: t("hud.snap.bottomLeft") },
    { id: "bottom-center", label: t("hud.snap.bottomCenter") },
    { id: "bottom-right",  label: t("hud.snap.bottomRight") },
  ];
  const options = presets
    .map(
      (p) => `<option value="${p.id}">${escapeHtml(p.label)}</option>`,
    )
    .join("");
  return /* html */ `
    <div class="hud-snap">
      <label for="hud-snap-select" class="geom-label">${t("hud.snap.title")}</label>
      <div class="snap-row">
        <select id="hud-snap-select">
          <option value="" disabled selected>${t("hud.snap.placeholder")}</option>
          ${options}
        </select>
      </div>
      <p class="muted" style="margin: 4px 0 0; font-size: 11px;">${t("hud.snap.hint")}</p>
    </div>
  `;
}

function bindHudSnapListeners() {
  const sel = document.getElementById("hud-snap-select") as HTMLSelectElement | null;
  if (!sel) return;
  sel.addEventListener("change", async () => {
    const preset = sel.value;
    if (!preset) return;
    try {
      await invoke("snap_hud_to_preset", { preset });
    } catch (err) {
      console.error("snap_hud_to_preset failed", err);
      return;
    }
    // Reset back to the placeholder so re-picking the same preset re-fires
    // (a `change` event won't fire if the value didn't change).
    sel.value = "";
    await refresh();
  });
}

function bindGeomListeners() {
  const stepEl = document.getElementById("geom-step") as HTMLSelectElement | null;
  const getStep = () => Number(stepEl?.value ?? "5");

  // Map axis id → field name on the backend command.
  const axisMap: Record<string, "x" | "y" | "w" | "h"> = {
    "geom-x": "x", "geom-y": "y", "geom-w": "w", "geom-h": "h",
  };

  document.querySelectorAll<HTMLButtonElement>(".geom-btn").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const axisId = btn.dataset.axis!;
      const dir = Number(btn.dataset.dir!);
      const input = document.getElementById(axisId) as HTMLInputElement | null;
      if (!input) return;
      const next = Number(input.value || "0") + dir * getStep();
      input.value = String(next);
      await invoke("set_hud_geometry", { [axisMap[axisId]]: next });
    });
  });

  // Typing in the field commits on Enter or blur (change event).
  document.querySelectorAll<HTMLInputElement>(".geom-input").forEach((input) => {
    input.addEventListener("change", async () => {
      const axisId = input.dataset.axis!;
      const value = Number(input.value || "0");
      await invoke("set_hud_geometry", { [axisMap[axisId]]: value });
    });
  });
}

// ----- Theme section --------------------------------------------------------

function renderAnalyticsSettingsPanel(s: StateSnapshot): string {
  const obsUrl = s.http_port === 0
    ? "starting…"
    : `http://localhost:${s.http_port}/overlays/post-match-hud.html`;
  return renderPanel("analytics-settings", t("postMatch.title"), /* html */ `
    <p class="muted" style="margin-top: 0;">${t("postMatch.intro")}</p>

    <div class="row" style="margin-top: 12px;">
      <label style="display:flex; align-items:flex-start; gap:8px; font-size: 13px;">
        <input type="checkbox" id="pm-analytics-enabled" ${s.analytics_enabled ? "checked" : ""} />
        <span>
          <strong>${t("postMatch.enable")}</strong>
          <p class="muted" style="margin: 4px 0 0; font-size: 11px;">${t("postMatch.enableHint")}</p>
          <p class="muted" style="margin: 4px 0 0; font-size: 11px; color: var(--warn);">${t("postMatch.enableWarn")}</p>
        </span>
      </label>
    </div>

    <div class="row" style="margin-top: 14px;">
      <label style="display:flex; align-items:flex-start; gap:8px; font-size: 13px;" ${s.analytics_enabled ? "" : 'class="dim"'}>
        <input type="checkbox" id="pm-show-hud" ${s.show_post_match_hud ? "checked" : ""} ${s.analytics_enabled ? "" : "disabled"} />
        <span>
          <strong>${t("postMatch.showHud")}</strong>
          <p class="muted" style="margin: 4px 0 0; font-size: 11px;">${t("postMatch.showHudHint")}</p>
        </span>
      </label>
    </div>

    <div class="row" style="margin-top: 14px;">
      <label style="display:flex; align-items:flex-start; gap:8px; font-size: 13px;">
        <input type="checkbox" id="pm-show-obs" ${s.show_post_match_obs ? "checked" : ""} ${s.analytics_enabled ? "" : "disabled"} />
        <span>
          <strong>${t("postMatch.showObs")}</strong>
          <p class="muted" style="margin: 4px 0 0; font-size: 11px;">${t("postMatch.showObsHint")}</p>
          <div class="url-pill" style="margin-top: 6px;">${escapeHtml(obsUrl)}</div>
        </span>
      </label>
    </div>

    <div class="row" style="margin-top: 18px; gap: 10px;">
      <button class="ghost" id="pm-open-folder">${t("postMatch.openFolder")}</button>
      <button class="ghost danger" id="pm-clear-history">${t("postMatch.clearHistory")}</button>
    </div>
  `);
}

function renderThemeSection(s: StateSnapshot): string {
  const def = themeById(s.theme) ?? THEMES[0];
  const themeOptions = THEMES.map(
    (t) => `<option value="${t.id}" ${t.id === s.theme ? "selected" : ""}>${escapeHtml(t.label)}</option>`,
  ).join("");

  // Group vars by their `group` field for visual organization in the UI.
  const groups = new Map<string, ThemeVarDef[]>();
  for (const v of def.vars) {
    const g = v.group ?? "General";
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g)!.push(v);
  }

  const groupHtml = Array.from(groups.entries())
    .map(([groupName, vars]) => {
      const items = vars.map((v) => renderThemeVarControl(v, s.theme_vars[v.key])).join("");
      return /* html */ `
        <div class="theme-group">
          <h3>${escapeHtml(groupName)}</h3>
          <div class="theme-grid">${items}</div>
        </div>`;
    })
    .join("");

  return renderPanel("theme", t("theme.title"), /* html */ `
    <p class="muted" style="margin-top: 0;">${escapeHtml(def.description)}</p>

    <div class="row" style="margin-bottom: 16px;">
      <label for="theme-select" style="min-width: 80px;">${t("theme.activeLabel")}</label>
      <select id="theme-select">${themeOptions}</select>
    </div>

    ${groupHtml}
  `, {
    summaryInfo: /* html */ `<strong>${escapeHtml(def.label)}</strong>`,
    headerActions: /* html */ `
      <button class="ghost" id="btn-open-themes" title="${escapeHtml(t("theme.openFolderTitle"))}">${t("theme.openFolder")}</button>
      <button class="ghost" id="btn-refresh-themes" title="${escapeHtml(t("theme.refreshTitle"))}">${t("theme.refresh")}</button>
      <button class="ghost" id="btn-reset-theme">${t("theme.resetAll")}</button>`,
  });
}

function renderThemeVarControl(v: ThemeVarDef, current: string | number | boolean | undefined): string {
  const overridden = current !== undefined;
  const label = `${escapeHtml(v.label)}${overridden ? ` <span class="override-dot" title="${escapeHtml(t("theme.varOverride"))}"></span>` : ""}`;

  const resetBtn = `<button class="var-reset" data-key="${v.key}" title="${escapeHtml(t("theme.varReset"))}" ${overridden ? "" : "disabled"}>↺</button>`;

  switch (v.spec.kind) {
    case "color": {
      const value = (current as string) ?? v.spec.default;
      return /* html */ `
        <div class="theme-var">
          <label class="var-label">${label}</label>
          <div class="var-control">
            <input type="color" class="var-input" data-key="${v.key}" data-kind="color" value="${escapeHtml(value)}" />
            <code class="var-code">${escapeHtml(value)}</code>
            ${resetBtn}
          </div>
        </div>`;
    }
    case "number": {
      const value = (current as number) ?? v.spec.default;
      const step = v.spec.step ?? 1;
      const unit = v.spec.unit ?? "";
      return /* html */ `
        <div class="theme-var">
          <label class="var-label">${label}</label>
          <div class="var-control">
            <input type="range" class="var-input" data-key="${v.key}" data-kind="number"
                   min="${v.spec.min}" max="${v.spec.max}" step="${step}" value="${value}" />
            <code class="var-code"><span class="var-num">${value}</span>${escapeHtml(unit)}</code>
            ${resetBtn}
          </div>
        </div>`;
    }
    case "boolean": {
      const value = (current as boolean) ?? v.spec.default;
      return /* html */ `
        <div class="theme-var">
          <label class="var-label">${label}</label>
          <div class="var-control">
            <label class="var-switch">
              <input type="checkbox" class="var-input" data-key="${v.key}" data-kind="boolean" ${value ? "checked" : ""} />
              <span class="slider"></span>
            </label>
            ${resetBtn}
          </div>
        </div>`;
    }
  }
}

function bindThemeListeners() {
  // Theme switch
  const sel = document.getElementById("theme-select") as HTMLSelectElement | null;
  sel?.addEventListener("change", async () => {
    await invoke("set_theme", { name: sel.value });
    await refresh();
  });

  // Open the user-themes folder in Explorer (creates it if missing).
  document.getElementById("btn-open-themes")?.addEventListener("click", async () => {
    try {
      await invoke("open_themes_folder");
    } catch (err) {
      alert(t("theme.openFolderError", { err: String(err) }));
    }
  });

  // Rescan bundled + user themes after a drag-drop.
  document.getElementById("btn-refresh-themes")?.addEventListener("click", async () => {
    await loadThemes();
    flashButton("btn-refresh-themes", t("theme.refreshed"));
    await refresh();
  });

  // Reset all overrides for the active theme — single backend call so the
  // HUD repaints once with the diff-aware applyThemeVars (looping
  // set_theme_var(null) per-var raced with itself and left stale CSS).
  document.getElementById("btn-reset-theme")?.addEventListener("click", async () => {
    const def = themeById(currentState?.theme ?? "");
    if (!def) return;
    if (!confirm(t("theme.resetConfirm"))) return;
    await invoke("reset_theme_vars");
    await refresh();
  });

  // Per-var input changes:
  //   - color: commit on `change` (after picker closes — `input` fires on
  //     every cursor move, that'd be hundreds of writes per drag).
  //   - boolean: commit on `change` (single click).
  //   - number (slider): commit on `input` with a 100ms debounce so the
  //     HUD repaints live while dragging, then a final commit on `change`
  //     so the persisted value matches the released position exactly.
  const commit = (target: HTMLInputElement) => {
    const key = target.dataset.key!;
    const kind = target.dataset.kind!;

    let value: string | number | boolean;
    if (kind === "boolean")     value = target.checked;
    else if (kind === "number") value = Number(target.value);
    else                        value = target.value;

    invoke("set_theme_var", { key, value }).catch(() => {});
  };

  const updateLocalLabel = (target: HTMLInputElement) => {
    const kind = target.dataset.kind!;
    const code = target.parentElement?.querySelector<HTMLElement>(".var-num, .var-code");
    if (!code) return;
    if (kind === "number") code.textContent = String(target.value);
    else if (kind === "color") code.textContent = target.value;
  };

  // Per-element debounce timers for number sliders.
  const sliderDebounce = new WeakMap<HTMLInputElement, number>();
  document.querySelectorAll<HTMLInputElement>(".var-input").forEach((el) => {
    el.addEventListener("change", () => commit(el));
    if (el.dataset.kind === "number") {
      el.addEventListener("input", () => {
        updateLocalLabel(el);
        // Live HUD repaint while dragging — debounced so a fast slider
        // doesn't flood the backend with 60+ writes per second.
        const prev = sliderDebounce.get(el);
        if (prev != null) window.clearTimeout(prev);
        sliderDebounce.set(el, window.setTimeout(() => commit(el), 100));
      });
    }
  });

  // Per-var reset button
  document.querySelectorAll<HTMLButtonElement>(".var-reset").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const key = btn.dataset.key!;
      await invoke("set_theme_var", { key, value: null });
      await refresh();
    });
  });
}

async function onSaveName() {
  const input = document.getElementById("player-input") as HTMLInputElement | null;
  const name = input?.value.trim() || "";
  if (!name) {
    return alert(t("player.savePrompt"));
  }
  await invoke("set_player_name", { name });
  await refresh();
}

async function onResetSession() {
  if (!confirm(t("session.resetConfirm"))) return;
  await invoke("reset_session");
  await refresh();
}

/// Wire up the 1v1/2v2/3v3/4v4 checkboxes. We commit on every change so the
/// user sees the effect on the next match without an explicit save step.
function bindTeamSizeFilter() {
  const inputs = document.querySelectorAll<HTMLInputElement>(".team-size-input");
  inputs.forEach((el) => {
    el.addEventListener("change", async () => {
      const sizes: number[] = [];
      inputs.forEach((other) => {
        if (other.checked) sizes.push(Number(other.dataset.size));
      });
      await invoke("set_count_team_sizes", { sizes });
    });
  });
}

async function onToggleHud() {
  await invoke("toggle_hud");
  await refresh();
}

async function onReloadHud() {
  await invoke("reload_hud");
  flashButton("btn-reload-hud", t("hud.reloaded"));
}

async function onQuitApp() {
  await invoke("quit_app");
}

async function onCopyUrl() {
  const url = currentState?.overlay_url;
  if (!url) return;
  await writeText(url);
  flashButton("btn-copy-url", t("obs.copied"));
}

async function onOpenUrl() {
  const url = currentState?.overlay_url;
  if (!url) return;
  await openExternal(url);
}

// ----- Helpers --------------------------------------------------------------

function flashButton(id: string, text: string) {
  const el = document.getElementById(id) as HTMLButtonElement | null;
  if (!el) return;
  const prev = el.textContent;
  el.textContent = text;
  el.disabled = true;
  setTimeout(() => {
    el.textContent = prev;
    el.disabled = false;
  }, 1200);
}

function escapeHtml(str: string): string {
  return str.replace(/[&<>"']/g, (c) => {
    switch (c) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case '"':
        return "&quot;";
      case "'":
        return "&#39;";
      default:
        return c;
    }
  });
}
