// Shared HUD interaction snippet — drag-to-move + right-click context menu.
//
// Loaded by every bundled theme (circle, default, minimal, redesigned) via a
// plain <script> tag pointing at /overlays/themes/_shared/menu.js. Custom /
// user-dropped themes can opt in by including the same tag.
//
// The HUD's webview is loaded over plain HTTP (the embedded axum server), so
// `window.__TAURI__` is NOT injected — every action goes through HTTP POSTs
// on the same origin. When the HUD HTML is loaded outside of Tauri (e.g. as
// an OBS Browser Source) the endpoints either return 404 (no `hud` window)
// or the lock check short-circuits; we silently ignore failures so the
// preview stays clean.
//
// Bilingual labels: detect once at load. We keep this tiny on purpose — no
// catalog, just the four strings the menu needs.

(function () {
  "use strict";

  const lang =
    (document.documentElement.lang || navigator.language || "fr")
      .toLowerCase()
      .startsWith("en")
      ? "en"
      : "fr";

  const labels = {
    fr: {
      reset: "Réinitialiser la session",
      lock: "Verrouiller / déverrouiller",
      quit: "Quitter",
    },
    en: {
      reset: "Reset session",
      lock: "Lock / unlock",
      quit: "Quit",
    },
  }[lang];

  /** POST to the embedded HTTP server. Always swallows network errors so a
   *  missing endpoint (OBS browser source case) never leaks into the page. */
  async function post(path) {
    try {
      await fetch(path, {
        method: "POST",
        cache: "no-store",
        // Empty body avoids preflight / Content-Length quirks on Windows
        // WebView2 — no JSON, just a bare POST.
      });
    } catch (_) {
      /* ignore — page is probably outside Tauri */
    }
  }

  /** Skip drag/menu when the user clicked something interactive (so future
   *  themes that include real buttons keep working). */
  function isInteractive(el) {
    if (!(el instanceof Element)) return false;
    return !!el.closest(
      'input, button, select, textarea, a, [contenteditable=""], [contenteditable="true"], [role="button"]',
    );
  }

  // ---- Drag-to-move --------------------------------------------------------

  // Non-passive listener so we can preventDefault on the mousedown — without
  // it, WebView2 can swallow the down/up pair and the OS drag never starts.
  document.addEventListener(
    "mousedown",
    (ev) => {
      if (ev.button !== 0) return; // left-click only — right-click is the menu
      if (isInteractive(ev.target)) return;
      // Hide the menu if it was open — clicking elsewhere should dismiss it.
      hideMenu();
      // Fire-and-forget: the OS takes over the mouse loop once start_dragging
      // returns, so we don't need to await.
      post("/hud/start-drag");
    },
    { passive: false },
  );

  // ---- Right-click context menu --------------------------------------------

  let menuEl = null;

  function ensureMenu() {
    if (menuEl) return menuEl;
    menuEl = document.createElement("ul");
    menuEl.id = "hud-context-menu";
    menuEl.setAttribute("role", "menu");
    menuEl.innerHTML =
      '<li class="item" data-act="reset" role="menuitem" tabindex="0">' +
      escapeHtml(labels.reset) +
      "</li>" +
      '<li class="item" data-act="lock" role="menuitem" tabindex="0">' +
      escapeHtml(labels.lock) +
      "</li>" +
      '<li class="item danger" data-act="quit" role="menuitem" tabindex="0">' +
      escapeHtml(labels.quit) +
      "</li>";
    document.body.appendChild(menuEl);

    menuEl.addEventListener("click", (ev) => {
      const item = ev.target instanceof Element ? ev.target.closest(".item") : null;
      if (!item) return;
      const act = item.getAttribute("data-act");
      hideMenu();
      if (act === "reset") post("/session/reset");
      else if (act === "lock") post("/hud/toggle-lock");
      else if (act === "quit") post("/app/quit");
    });

    return menuEl;
  }

  function showMenu(x, y) {
    const el = ensureMenu();
    // Place at cursor first so we can measure the actual rendered size, then
    // clamp into the viewport so the menu never spills off the right/bottom.
    el.style.left = x + "px";
    el.style.top = y + "px";
    el.dataset.visible = "true";
    requestAnimationFrame(() => {
      const rect = el.getBoundingClientRect();
      const maxX = window.innerWidth - rect.width - 4;
      const maxY = window.innerHeight - rect.height - 4;
      el.style.left = Math.max(2, Math.min(x, maxX)) + "px";
      el.style.top = Math.max(2, Math.min(y, maxY)) + "px";
    });
  }

  function hideMenu() {
    if (menuEl) menuEl.dataset.visible = "false";
  }

  document.addEventListener("contextmenu", (ev) => {
    if (isInteractive(ev.target)) return;
    ev.preventDefault();
    showMenu(ev.clientX, ev.clientY);
  });

  // Dismiss on outside click / Escape — matches OS context-menu conventions.
  document.addEventListener("click", (ev) => {
    if (!menuEl || menuEl.dataset.visible !== "true") return;
    if (menuEl.contains(ev.target)) return;
    hideMenu();
  });
  document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape") hideMenu();
  });

  // ---- Stylesheet injection ------------------------------------------------
  // Add the shared CSS once, so themes only need the single <script> tag.
  // Idempotent: themes that include the .css explicitly still work.

  if (!document.getElementById("hud-context-menu-style")) {
    const link = document.createElement("link");
    link.id = "hud-context-menu-style";
    link.rel = "stylesheet";
    link.href = "/overlays/themes/_shared/menu.css";
    document.head.appendChild(link);
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) =>
      c === "&"
        ? "&amp;"
        : c === "<"
          ? "&lt;"
          : c === ">"
            ? "&gt;"
            : c === '"'
              ? "&quot;"
              : "&#39;",
    );
  }
})();
