import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { open as openExternal } from "@tauri-apps/plugin-shell";

// ----- Types ----------------------------------------------------------------

interface Session {
  wins: number;
  losses: number;
  streak: number;
  best_win_streak: number;
  best_loss_streak: number;
  last_update: number;
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
  count_team_sizes: number[];
}

interface DetectedInstall {
  platform: string;
  install_dir: string;
  ini_path: string;
}

interface PatchOutcome {
  already_correct: boolean;
  backup_path: string | null;
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
];

function themeById(id: string): ThemeDef | undefined {
  return THEMES.find((t) => t.id === id);
}

// ----- Bootstrap ------------------------------------------------------------

const root = document.getElementById("app")!;
let currentState: StateSnapshot | null = null;

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
    renderWizard();
  } else {
    renderDashboard();
  }
  return currentState;
}

await loadThemes();
await refresh();

// Live updates pushed by the Rust side.
listen("rlstats://connected", () => refresh());
listen("rlstats://session-changed", () => refresh());

// Re-poll every second so the connected dot stays accurate even if events drop.
setInterval(() => {
  refresh().catch(() => {});
}, 2000);

// ----- Dashboard view -------------------------------------------------------

function renderDashboard() {
  if (!currentState) return;
  const s = currentState;

  const streakLabel = streakDisplay(s.session.streak);
  const obsUrl = s.overlay_url || "starting…";

  root.innerHTML = /* html */ `
    <main>
      <header style="display:flex; justify-content: space-between; align-items: flex-end; margin-bottom: 18px;">
        <div>
          <h1>🎮 RL Stats Overlay</h1>
          <p class="subtitle">Powered by the official Rocket League Stats API. Fully EAC-safe.</p>
        </div>
        <span class="badge"><span class="dot ${s.connected ? "ok" : ""}" id="conn-dot"></span>${
          s.connected ? "Connecté au jeu" : "En attente du jeu"
        }</span>
      </header>

      <section class="panel">
        <div class="panel-header">
          <h2>Session en cours</h2>
          <button class="ghost" id="btn-reset">Reset</button>
        </div>
        <div class="session">
          <div class="stat win"><div class="num">${s.session.wins}</div><div class="lbl">Wins</div></div>
          <div class="stat loss"><div class="num">${s.session.losses}</div><div class="lbl">Losses</div></div>
          <div class="stat streak ${s.session.streak > 0 ? "win" : s.session.streak < 0 ? "loss" : ""}">
            <div class="num">${streakLabel}</div>
            <div class="lbl">Streak</div>
          </div>
        </div>
        <p class="muted" style="margin-top: 12px; font-size: 12px;">
          Records de la session — meilleure série de wins : <strong>${s.session.best_win_streak}</strong> ·
          pire série de losses : <strong>${s.session.best_loss_streak}</strong>
        </p>

        <div class="team-size-filter" style="margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border, rgba(255,255,255,0.08));">
          <label style="font-weight: 600; font-size: 13px;">Compter les matchs en :</label>
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
            Ne distingue pas Ranked et Casual : l'API officielle de Rocket League n'expose pas le mode
            de matchmaking. Le filtre ci-dessus se base uniquement sur la <strong>taille des équipes</strong>
            détectée en début de match.
          </p>
        </div>
      </section>

      <section class="panel">
        <h2>Joueur</h2>
        <div class="row">
          <div style="flex: 1;">
            <label for="player-input">Pseudo en jeu</label>
            <input type="text" id="player-input" value="${escapeHtml(s.player_name)}" placeholder="Ton pseudo Rocket League" />
          </div>
          <button class="primary" id="btn-save-name" style="margin-top: 18px;">Enregistrer</button>
        </div>
        ${
          s.primary_id
            ? `<p class="muted" style="margin-top: 8px; font-size: 11px;">Identifiant stable capturé : <code>${escapeHtml(
                s.primary_id,
              )}</code></p>`
            : `<p class="muted" style="margin-top: 8px; font-size: 11px;">L'identifiant stable sera capturé automatiquement au prochain match.</p>`
        }
      </section>

      <section class="panel">
        <h2>HUD en jeu</h2>
        <p class="muted" style="margin-top: 0;">
          Affiche l'overlay en fenêtre transparente par-dessus Rocket League. Fonctionne uniquement en
          <strong>plein écran fenêtré (borderless)</strong>.
        </p>
        <div class="row" style="margin-top: 12px;">
          <button class="primary" id="btn-toggle-hud">${
            s.hud_visible ? "🟢 HUD activé — masquer" : "▶ Afficher le HUD"
          }</button>
          <button id="btn-reload-hud" title="Recharge le HUD pour forcer un fetch frais des assets">🔄 Recharger</button>
        </div>

        <div class="hud-geom">
          <div class="row" style="gap: 16px; align-items: flex-end;">
            <label class="step-picker">Pas
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
        </div>
      </section>

      <section class="panel">
        <h2>OBS Browser Source</h2>
        <p class="muted" style="margin-top: 0;">Colle cette URL dans une <em>Browser Source</em> OBS, dimensions <code>400 × 300</code>.</p>
        <div class="url-pill" id="url-pill">${escapeHtml(obsUrl)}</div>
        <div class="row" style="margin-top: 12px;">
          <button class="primary" id="btn-copy-url" ${s.http_port === 0 ? "disabled" : ""}>📋 Copier l'URL</button>
          <button id="btn-open-url" ${s.http_port === 0 ? "disabled" : ""}>👁 Aperçu navigateur</button>
        </div>
      </section>

      ${renderThemeSection(s)}

      <footer>
        <p style="margin: 0;">Settings stockés dans <code>${escapeHtml(s.settings_path)}</code></p>
        <p class="muted" style="margin: 6px 0 12px;">
          La croix de la fenêtre envoie l'app dans la zone de notification — utilise le bouton
          ci-dessous (ou clic droit sur l'icône système) pour quitter complètement.
        </p>
        <button id="btn-quit-app" class="ghost">⏻ Quitter l'application</button>
      </footer>
    </main>
  `;

  document.getElementById("btn-save-name")?.addEventListener("click", onSaveName);
  document.getElementById("btn-reset")?.addEventListener("click", onResetSession);
  bindTeamSizeFilter();
  document.getElementById("btn-toggle-hud")?.addEventListener("click", onToggleHud);
  document.getElementById("btn-reload-hud")?.addEventListener("click", onReloadHud);
  document.getElementById("btn-copy-url")?.addEventListener("click", onCopyUrl);
  document.getElementById("btn-open-url")?.addEventListener("click", onOpenUrl);
  document.getElementById("btn-quit-app")?.addEventListener("click", onQuitApp);

  bindGeomListeners();
  bindThemeListeners();
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

  return /* html */ `
    <section class="panel">
      <div class="panel-header">
        <h2>Theme</h2>
        <div class="row" style="gap: 8px;">
          <button class="ghost" id="btn-open-themes" title="Ouvre le dossier où déposer un thème custom">📁 Dossier des thèmes</button>
          <button class="ghost" id="btn-refresh-themes" title="Rescanne le dossier après un drag-drop">🔄 Rafraîchir</button>
          <button class="ghost" id="btn-reset-theme">Reset all overrides</button>
        </div>
      </div>
      <p class="muted" style="margin-top: 0;">${escapeHtml(def.description)}</p>

      <div class="row" style="margin-bottom: 16px;">
        <label for="theme-select" style="min-width: 80px;">Active theme</label>
        <select id="theme-select">${themeOptions}</select>
      </div>

      ${groupHtml}
    </section>
  `;
}

function renderThemeVarControl(v: ThemeVarDef, current: string | number | boolean | undefined): string {
  const overridden = current !== undefined;
  const label = `${escapeHtml(v.label)}${overridden ? " <span class=\"override-dot\" title=\"Override active\"></span>" : ""}`;

  const resetBtn = `<button class="var-reset" data-key="${v.key}" title="Reset to default" ${overridden ? "" : "disabled"}>↺</button>`;

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
      alert(`Impossible d'ouvrir le dossier des thèmes: ${err}`);
    }
  });

  // Rescan bundled + user themes after a drag-drop.
  document.getElementById("btn-refresh-themes")?.addEventListener("click", async () => {
    await loadThemes();
    flashButton("btn-refresh-themes", "✓ Rafraîchi");
    await refresh();
  });

  // Reset all overrides for the active theme
  document.getElementById("btn-reset-theme")?.addEventListener("click", async () => {
    const def = themeById(currentState?.theme ?? "");
    if (!def) return;
    if (!confirm("Réinitialiser tous les réglages de ce thème ?")) return;
    for (const v of def.vars) {
      await invoke("set_theme_var", { key: v.key, value: null });
    }
    await refresh();
  });

  // Per-var input changes commit only on `change` (color picker close,
  // slider release, checkbox click) so we never invoke set_theme_var
  // mid-interaction. This keeps native pickers open as long as the user
  // wants and is the standard "settings dialog" pattern.
  //
  // Sliders still get live numeric feedback through `input` — but only
  // for the local text label, the persist itself waits for `change`.
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

  document.querySelectorAll<HTMLInputElement>(".var-input").forEach((el) => {
    el.addEventListener("change", () => commit(el));
    if (el.dataset.kind === "number") {
      // Live numeric readout while dragging, no persist yet.
      el.addEventListener("input", () => updateLocalLabel(el));
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
    return alert("Renseigne un pseudo avant d'enregistrer.");
  }
  await invoke("set_player_name", { name });
  await refresh();
}

async function onResetSession() {
  if (!confirm("Réinitialiser la session ? (les wins/losses repartent à zéro)")) return;
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
  flashButton("btn-reload-hud", "✓ Rechargé");
}

async function onQuitApp() {
  await invoke("quit_app");
}

async function onCopyUrl() {
  const url = currentState?.overlay_url;
  if (!url) return;
  await writeText(url);
  flashButton("btn-copy-url", "✓ Copié");
}

async function onOpenUrl() {
  const url = currentState?.overlay_url;
  if (!url) return;
  await openExternal(url);
}

// ----- Wizard view ----------------------------------------------------------

let wizardStep: 1 | 2 | 3 = 1;
let detectedInstalls: DetectedInstall[] = [];
let chosenIniPath: string | null = null;
let lastPatchOutcome: PatchOutcome | null = null;

async function renderWizard() {
  if (wizardStep === 1) {
    detectedInstalls = await invoke<DetectedInstall[]>("detect_rocket_league");
  }

  const dots = (1 as number) <= wizardStep
    ? `
        <div class="step-indicator">
          <div class="step-dot ${wizardStep === 1 ? "active" : "done"}">1</div>
          <div class="step-dot ${wizardStep === 2 ? "active" : wizardStep > 2 ? "done" : ""}">2</div>
          <div class="step-dot ${wizardStep === 3 ? "active" : ""}">3</div>
        </div>`
    : "";

  if (wizardStep === 1) {
    root.innerHTML = /* html */ `
      <main class="wizard">
        ${dots}
        <h1>🎮 Bienvenue dans RL Stats Overlay</h1>
        <p class="subtitle">Configurons ton installation Rocket League. Ça prend 30 secondes.</p>

        <section class="panel">
          <h2>1. Installation détectée</h2>
          ${
            detectedInstalls.length === 0
              ? `<p class="muted">Aucune installation détectée automatiquement. Tu peux indiquer le dossier manuellement.</p>`
              : detectedInstalls
                  .map(
                    (d, i) => `
            <div class="install-card" data-idx="${i}">
              <div style="font-size: 24px;">${d.platform === "Steam" ? "🕹" : "🎮"}</div>
              <div style="flex: 1;">
                <div class="platform">Rocket League — ${d.platform}</div>
                <div class="path">${escapeHtml(d.install_dir)}</div>
              </div>
              <div style="color: var(--accent); font-weight: 800;">→</div>
            </div>
          `,
                  )
                  .join("")
          }
          <div class="row" style="margin-top: 12px;">
            <button id="btn-browse">📂 Indiquer un dossier manuellement</button>
          </div>
        </section>
      </main>
    `;

    document.querySelectorAll<HTMLElement>(".install-card").forEach((el) => {
      el.addEventListener("click", () => {
        const idx = parseInt(el.dataset.idx || "0", 10);
        chosenIniPath = detectedInstalls[idx]?.ini_path ?? null;
        wizardStep = 2;
        applyPatch();
      });
    });
    document.getElementById("btn-browse")?.addEventListener("click", onBrowse);
  } else if (wizardStep === 2) {
    root.innerHTML = /* html */ `
      <main class="wizard">
        ${dots}
        <h1>2. Activation de la Stats API</h1>

        <section class="panel">
          ${
            lastPatchOutcome?.already_correct
              ? `<div class="alert success">La Stats API était déjà activée correctement. Aucune modification nécessaire.</div>`
              : `<div class="alert success">Configuration appliquée à <code>${escapeHtml(
                  chosenIniPath || "?",
                )}</code>${
                  lastPatchOutcome?.backup_path
                    ? `<br/>Sauvegarde de l'ancien fichier dans <code>${escapeHtml(lastPatchOutcome.backup_path)}</code>.`
                    : ""
                }</div>`
          }
          <p class="muted" style="margin-top: 12px;">
            La Stats API est une fonctionnalité <strong>officielle Psyonix</strong>, compatible Easy Anti-Cheat.
            Aucune injection dans le jeu, uniquement la lecture de l'API qu'il expose lui-même.
          </p>
          <p class="muted" style="margin-top: 12px;">
            ⚠️ <strong>Redémarre Rocket League</strong> si le jeu était lancé pour que le changement prenne effet.
          </p>
        </section>

        <section class="panel">
          <h2>3. Ton pseudo en jeu</h2>
          <label for="wizard-name">Tape exactement le pseudo affiché en match (sensible aux espaces)</label>
          <input type="text" id="wizard-name" placeholder="ex: Pooley" autofocus />
        </section>

        <div class="row">
          <div class="spacer"></div>
          <button class="primary" id="btn-finish">Terminer ▶</button>
        </div>
      </main>
    `;

    document.getElementById("btn-finish")?.addEventListener("click", onFinishWizard);
    document.getElementById("wizard-name")?.addEventListener("keydown", (e) => {
      if ((e as KeyboardEvent).key === "Enter") onFinishWizard();
    });
  }
}

async function onBrowse() {
  const picked = await openDialog({
    multiple: false,
    directory: true,
    title: "Sélectionne le dossier d'installation de Rocket League",
  });
  if (!picked || typeof picked !== "string") return;
  chosenIniPath = picked;
  wizardStep = 2;
  await applyPatch();
}

async function applyPatch() {
  if (!chosenIniPath) return;
  try {
    lastPatchOutcome = await invoke<PatchOutcome>("patch_ini", { path: chosenIniPath });
    await renderWizard();
  } catch (err) {
    alert(
      `Impossible de modifier la configuration de la Stats API :\n${err}\n\nVérifie que tu as les droits d'écriture sur le dossier d'installation.`,
    );
    wizardStep = 1;
    await renderWizard();
  }
}

async function onFinishWizard() {
  const input = document.getElementById("wizard-name") as HTMLInputElement | null;
  const name = input?.value.trim() || "";
  if (!name) {
    return alert("Renseigne ton pseudo en jeu avant de continuer.");
  }
  await invoke("set_player_name", { name });
  await invoke("complete_setup");
  await refresh();
}

// ----- Helpers --------------------------------------------------------------

function streakDisplay(streak: number): string {
  if (streak === 0) return "—";
  if (streak > 0) return `🔥 W${streak}`;
  return `❄️ L${-streak}`;
}

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
