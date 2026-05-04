// Shared HUD interaction snippet — drag-to-move, drag-to-resize, auto-scale
// and right-click context menu.
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

  // ---- Auto-fit scale ------------------------------------------------------
  //
  // Every theme draws its `.panel` at a fixed pixel size (200×124, 260×150,
  // 400×264, …). Without intervention the panel stays centered at its
  // intrinsic size when the host window grows or shrinks — which is why
  // resizing the Tauri HUD or the OBS Browser Source previously had no
  // visible effect on the contents.
  //
  // We solve this by exposing a `--hud-scale` CSS var on :root, computed from
  // the window's current dimensions relative to the factory-default 400×300
  // base, and applying `transform: scale(var(--hud-scale))` directly on the
  // `.panel` element. The transform-origin is the panel's center so the
  // visual centering of `position:absolute; inset:0; margin:auto` is
  // preserved across scale values. We use `min(sx, sy)` so the panel always
  // fits inside the window without clipping, regardless of the window's
  // aspect ratio. Works identically in OBS — at a 400×300 source the scale
  // is 1.0 (no visual change vs. the previous behaviour), at 800×600 it
  // becomes 2.0 (content fills the source).
  const SCALE_BASE_W = 400;
  const SCALE_BASE_H = 300;
  function applyHudScale() {
    const sx = window.innerWidth / SCALE_BASE_W;
    const sy = window.innerHeight / SCALE_BASE_H;
    // 0.25 floor matches the Rust-side hud size minimum (80×60 ≈ 0.20×) and
    // keeps the panel readable; ceiling left open — large windows are fine.
    const scale = Math.max(0.25, Math.min(sx, sy));
    document.documentElement.style.setProperty("--hud-scale", String(scale));
    const panel = document.querySelector(".panel");
    if (panel) {
      panel.style.transformOrigin = "top left";
      panel.style.transform = `scale(${scale})`;
    }
  }
  // Apply once now (the panel is already in the DOM by the time this script
  // tag runs — it sits at the bottom of <body>) and on every window resize.
  applyHudScale();
  window.addEventListener("resize", applyHudScale, { passive: true });

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
      'input, button, select, textarea, a, [contenteditable=""], [contenteditable="true"], [role="button"], #hud-resize-handle',
    );
  }

  // ---- Drag-to-resize handle ----------------------------------------------
  //
  // A small grip in the bottom-right corner that lets the user resize the
  // HUD with the mouse. Stays mostly invisible to keep the OBS Browser
  // Source preview clean, fades in on hover. Click → POST to
  // `/hud/start-resize`, the Rust side hands the mouse loop to the OS via
  // `start_resize_dragging(SouthEast)` and the resize tracks until release.
  // Skipped when the HUD position is locked (the server short-circuits, but
  // we also hide the visual cue locally so the user sees the lock taking
  // effect — DOM attribute is wired by the lock-changed event below).
  const resizeHandle = document.createElement("div");
  resizeHandle.id = "hud-resize-handle";
  resizeHandle.setAttribute("aria-hidden", "true");
  resizeHandle.title = lang === "fr" ? "Redimensionner" : "Resize";
  resizeHandle.addEventListener(
    "mousedown",
    (ev) => {
      if (ev.button !== 0) return;
      ev.preventDefault();
      ev.stopPropagation();
      hideMenu();
      post("/hud/start-resize");
    },
    { passive: false },
  );
  document.body.appendChild(resizeHandle);

  // ---- Drag-to-move --------------------------------------------------------

  // Non-passive listener so we can preventDefault on the mousedown — without
  // it, WebView2 can swallow the down/up pair and the OS drag never starts.
  document.addEventListener(
    "mousedown",
    (ev) => {
      if (ev.button !== 0) return; // left-click only — right-click is the menu
      if (isInteractive(ev.target)) return;
      // Resize handle handles its own mousedown (stopPropagation above), but
      // belt-and-braces in case a child element bubbles an event up here.
      if (ev.target instanceof Element && ev.target.closest("#hud-resize-handle"))
        return;
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
