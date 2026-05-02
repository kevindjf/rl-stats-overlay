// Floating launcher badge — single click handler.
//
// The HTML is loaded over plain HTTP (http://localhost:49124/...) inside a
// Tauri webview, so `window.__TAURI__` is NOT available. We POST to the
// embedded HTTP server, which calls `show_settings_window` on the Tauri
// side. Errors are swallowed — the badge stays on screen and the user can
// retry; we don't have anywhere meaningful to surface a failure here.

document.getElementById("open")?.addEventListener("click", () => {
  fetch("/launcher/open-settings", { method: "POST" }).catch(() => {});
});
