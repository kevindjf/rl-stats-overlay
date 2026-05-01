import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { t } from "./i18n";

// Local copies of the types the wizard cares about. Kept narrow so the
// wizard module stays self-contained — `main.ts` re-imports the same shape
// in its `StateSnapshot`.

interface DetectedInstall {
  platform: string;
  install_dir: string;
  ini_path: string;
}

interface PatchOutcome {
  already_correct: boolean;
  backup_path: string | null;
}

interface WizardSnapshot {
  has_local_platform_candidates: boolean;
}

// Wizard-local state. Lives at the module level (same as the previous inline
// implementation) because `renderWizard` re-renders the same root element
// across step transitions and needs the choice to survive across calls.
let wizardStep: 1 | 2 = 1;
let detectedInstalls: DetectedInstall[] = [];
let chosenIniPath: string | null = null;
let lastPatchOutcome: PatchOutcome | null = null;
let autoDetected = false;
let activeRoot: HTMLElement | null = null;
let onCompleteCb: (() => Promise<void>) | null = null;

/// Public entry point. Renders into `root` and calls `onComplete` once the
/// user finishes the wizard so the host (`main.ts`) can refresh the state
/// snapshot and switch to the dashboard view.
///
/// `snapshot.has_local_platform_candidates` controls whether the second step
/// asks for an in-game name (manual flow, 3 dots) or just confirms (auto
/// flow, 2 dots).
///
/// Idempotent on subsequent calls: while the wizard is already mounted on
/// `root`, re-invocations from the host's polling loop are ignored so the
/// user's step progress isn't reset on the next 2s tick.
export function runWizard(
  root: HTMLElement,
  snapshot: WizardSnapshot,
  onComplete: () => Promise<void>,
): void {
  if (activeRoot === root) {
    // Already mounted — refresh the callback (host may have rebuilt its
    // closure) but leave step / detection state intact.
    onCompleteCb = onComplete;
    return;
  }
  activeRoot = root;
  onCompleteCb = onComplete;
  autoDetected = snapshot.has_local_platform_candidates;
  wizardStep = 1;
  void renderWizard();
}

async function renderWizard(): Promise<void> {
  const root = activeRoot;
  if (!root) return;

  if (wizardStep === 1) {
    detectedInstalls = await invoke<DetectedInstall[]>("detect_rocket_league");
  }

  // 2 dots when we auto-detected the platform ID (no name step), 3 otherwise.
  const dots = autoDetected
    ? `
        <div class="step-indicator">
          <div class="step-dot ${wizardStep === 1 ? "active" : "done"}">1</div>
          <div class="step-dot ${wizardStep === 2 ? "active" : ""}">2</div>
        </div>`
    : `
        <div class="step-indicator">
          <div class="step-dot ${wizardStep === 1 ? "active" : "done"}">1</div>
          <div class="step-dot ${wizardStep === 2 ? "active" : ""}">2</div>
          <div class="step-dot">3</div>
        </div>`;

  if (wizardStep === 1) {
    root.innerHTML = /* html */ `
      <main class="wizard">
        ${dots}
        <h1>${t("wizard.welcome")}</h1>
        <p class="subtitle">${t("wizard.welcomeSub")}</p>

        <section class="panel">
          <h2>${t("wizard.installTitle")}</h2>
          ${
            detectedInstalls.length === 0
              ? `<p class="muted">${t("wizard.notDetected")}</p>`
              : detectedInstalls
                  .map(
                    (d, i) => `
            <div class="install-card" data-idx="${i}">
              <div style="font-size: 24px;">${d.platform === "Steam" ? "🕹" : "🎮"}</div>
              <div style="flex: 1;">
                <div class="platform">${t("wizard.installLabel", { platform: d.platform })}</div>
                <div class="path">${escapeHtml(d.install_dir)}</div>
              </div>
              <div style="color: var(--accent); font-weight: 800;">→</div>
            </div>
          `,
                  )
                  .join("")
          }
          <div class="row" style="margin-top: 12px;">
            <button id="btn-browse">${t("wizard.browse")}</button>
          </div>
        </section>
      </main>
    `;

    document.querySelectorAll<HTMLElement>(".install-card").forEach((el) => {
      el.addEventListener("click", () => {
        const idx = parseInt(el.dataset.idx || "0", 10);
        chosenIniPath = detectedInstalls[idx]?.ini_path ?? null;
        wizardStep = 2;
        void applyPatch();
      });
    });
    document.getElementById("btn-browse")?.addEventListener("click", onBrowse);
  } else if (wizardStep === 2) {
    // Two flavours of step 2:
    //  - autoDetected: confirm-only, button = Finish.
    //  - manual: name input + Finish (existing flow).
    const playerSection = autoDetected
      ? /* html */ `
        <section class="panel">
          <h2>${t("wizard.autoDetectedTitle")}</h2>
          <p class="muted" style="margin-top: 0;">${t("wizard.autoDetectedNote")}</p>
        </section>`
      : /* html */ `
        <section class="panel">
          <h2>${t("wizard.playerTitle")}</h2>
          <label for="wizard-name">${t("wizard.playerLabel")}</label>
          <input type="text" id="wizard-name" placeholder="${escapeHtml(t("wizard.playerPlaceholder"))}" autofocus />
        </section>`;

    root.innerHTML = /* html */ `
      <main class="wizard">
        ${dots}
        <h1>${t("wizard.apiTitle")}</h1>

        <section class="panel">
          ${
            lastPatchOutcome?.already_correct
              ? `<div class="alert success">${t("wizard.apiAlreadyOk")}</div>`
              : `<div class="alert success">${t("wizard.apiApplied", { path: escapeHtml(chosenIniPath || "?") })}${
                  lastPatchOutcome?.backup_path
                    ? `<br/>${t("wizard.apiBackup", { path: escapeHtml(lastPatchOutcome.backup_path) })}`
                    : ""
                }</div>`
          }
          <p class="muted" style="margin-top: 12px;">
            ${t("wizard.apiNote1")}
          </p>
          <p class="muted" style="margin-top: 12px;">
            ${t("wizard.apiNote2")}
          </p>
        </section>

        ${playerSection}

        <div class="row">
          <div class="spacer"></div>
          <button class="primary" id="btn-finish">${t("wizard.finish")}</button>
        </div>
      </main>
    `;

    document.getElementById("btn-finish")?.addEventListener("click", onFinishWizard);
    if (!autoDetected) {
      document.getElementById("wizard-name")?.addEventListener("keydown", (e) => {
        if ((e as KeyboardEvent).key === "Enter") void onFinishWizard();
      });
    }
  }
}

async function onBrowse(): Promise<void> {
  const picked = await openDialog({
    multiple: false,
    directory: true,
    title: t("wizard.browseTitle"),
  });
  if (!picked || typeof picked !== "string") return;
  chosenIniPath = picked;
  wizardStep = 2;
  await applyPatch();
}

async function applyPatch(): Promise<void> {
  if (!chosenIniPath) return;
  try {
    lastPatchOutcome = await invoke<PatchOutcome>("patch_ini", { path: chosenIniPath });
    await renderWizard();
  } catch (err) {
    alert(t("wizard.patchError", { err: String(err) }));
    wizardStep = 1;
    await renderWizard();
  }
}

async function onFinishWizard(): Promise<void> {
  if (autoDetected) {
    // Auto-detected flow: skip the name input. The empty player_name on the
    // backend is fine — the first UpdateState's prefix-match against the
    // boot-time platform candidates will identify the user, and
    // on_update_state then captures and persists the stable PrimaryId.
    await invoke("complete_setup");
  } else {
    const input = document.getElementById("wizard-name") as HTMLInputElement | null;
    const name = input?.value.trim() || "";
    if (!name) {
      alert(t("wizard.finishPrompt"));
      return;
    }
    await invoke("set_player_name", { name });
    await invoke("complete_setup");
  }
  // Wizard is dismissed — release the mount guard so a future setup-reset
  // (e.g. user clears settings.json) re-enters cleanly.
  activeRoot = null;
  if (onCompleteCb) await onCompleteCb();
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
