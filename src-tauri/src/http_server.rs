use anyhow::{anyhow, Result};
use axum::{
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::{
    net::SocketAddr,
    sync::{atomic::Ordering, Arc},
};
use tauri::{Emitter, Manager};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::state::AppState;

/// Default port; we try this first and increment if it is busy.
const PREFERRED_PORT: u16 = 49124;
const PORT_SCAN_LIMIT: u16 = 10;

/// Static overlay assets are embedded at compile-time so the binary is fully
/// self-contained. In debug builds, [`RustEmbed`] reads from disk so editing
/// overlay files takes effect without rebuilding.
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../overlays/"]
pub struct OverlayAssets;

/// Spawn the HTTP server on the first free port starting at [`PREFERRED_PORT`].
/// Updates [`AppState::http_port`] with the bound port.
pub async fn start(state: Arc<AppState>) -> Result<()> {
    let listener = bind_first_free_port(state.clone()).await?;
    let addr = listener.local_addr()?;
    info!("HTTP server listening on http://{addr}");

    let router = Router::new()
        .route("/", get(root))
        .route("/api/config", get(api_config))
        .route("/api/state", get(api_state))
        // Stable URL: resolves the active theme on the fly. This is the URL
        // users put in OBS so they don't have to update it when switching
        // themes from the settings window.
        .route("/overlays/boost.html", get(serve_active_boost))
        .route("/overlays/*path", get(serve_overlay))
        // HUD-side actions, reached by the bundled themes' right-click menu
        // and the drag-to-move snippet. They are POSTed from JS so a
        // misclick on a refresh / preview never triggers them. When the HUD
        // window is absent (e.g. the same HTML loaded as an OBS browser
        // source) the handlers return a soft 404 / no-op so the page keeps
        // rendering normally.
        .route("/hud/start-drag", post(hud_start_drag))
        .route("/hud/toggle-lock", post(hud_toggle_lock))
        .route("/session/reset", post(session_reset))
        .route("/app/quit", post(app_quit))
        .with_state(state);

    axum::serve(listener, router.into_make_service())
        .await
        .map_err(|e| anyhow!("http server failed: {e}"))?;
    Ok(())
}

async fn bind_first_free_port(state: Arc<AppState>) -> Result<TcpListener> {
    for offset in 0..PORT_SCAN_LIMIT {
        let port = PREFERRED_PORT + offset;
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        if let Ok(listener) = TcpListener::bind(addr).await {
            state.http_port.store(port, Ordering::SeqCst);
            return Ok(listener);
        }
    }
    Err(anyhow!(
        "no free port available between {PREFERRED_PORT} and {}",
        PREFERRED_PORT + PORT_SCAN_LIMIT
    ))
}

/// Tiny landing page that just points at the overlay path — useful when the
/// user opens http://localhost:49124 in a browser by accident.
async fn root() -> impl IntoResponse {
    let body = "<!doctype html><meta charset=\"utf-8\"><title>RL Stats Overlay</title>\
        <h1>RL Stats Overlay</h1>\
        <p>This is the local server backing the RL Stats Overlay app.</p>\
        <p>Drop <code>http://localhost:49124/overlays/boost.html</code> into OBS as a \
        <em>Browser Source</em>.</p>";
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body)
}

#[derive(Serialize)]
struct OverlayConfig {
    #[serde(rename = "playerName")]
    player_name: String,
    #[serde(rename = "primaryId")]
    primary_id: String,
    theme: String,
    #[serde(rename = "themeVars")]
    theme_vars: std::collections::HashMap<String, serde_json::Value>,
}

async fn api_config(State(state): State<Arc<AppState>>) -> Json<OverlayConfig> {
    let s = state.settings.lock();
    Json(OverlayConfig {
        player_name: s.player_name.clone(),
        primary_id: s.primary_id.clone(),
        theme: s.theme.clone(),
        theme_vars: s.current_theme_vars(),
    })
}

/// Theme names are mapped to folders, so they must contain only safe ASCII.
/// Anything outside the allowed set falls back to the default theme to avoid
/// path traversal or accidental 404s when settings get hand-edited.
fn sanitize_theme(raw: &str) -> &str {
    if raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !raw.is_empty() {
        raw
    } else {
        "circle"
    }
}

async fn serve_active_boost(State(state): State<Arc<AppState>>) -> Response {
    let theme = state.settings.lock().theme.clone();
    let safe = sanitize_theme(&theme).to_string();
    let rel = format!("themes/{safe}/boost.html");
    let bytes = match read_overlay_asset(&rel) {
        Some(b) => b,
        None => {
            return (
                StatusCode::NOT_FOUND,
                format!("Theme '{safe}' is missing boost.html"),
            )
                .into_response();
        }
    };

    // Inject a <base href> right after <head> so all relative paths inside
    // the theme HTML resolve to the right theme folder. Absolute paths
    // (starting with /) and the embedded shared scripts ignore the <base>.
    let html = std::str::from_utf8(&bytes).unwrap_or_default();
    let base_href = format!("/overlays/themes/{safe}/");
    let rewritten = html.replacen("<head>", &format!("<head>\n  <base href=\"{base_href}\">"), 1);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-store, must-revalidate".parse().unwrap());
    (headers, rewritten).into_response()
}

#[derive(Serialize)]
struct StateSnapshot {
    connected: bool,
    #[serde(rename = "playerName")]
    player_name: String,
    #[serde(rename = "primaryId")]
    primary_id: String,
    session: crate::session::Session,
    #[serde(rename = "setupDone")]
    setup_done: bool,
    #[serde(rename = "hudVisible")]
    hud_visible: bool,
    #[serde(rename = "httpPort")]
    http_port: u16,
}

async fn api_state(State(state): State<Arc<AppState>>) -> Json<StateSnapshot> {
    let settings = state.settings.lock().clone();
    let session = state.session.lock().clone();
    Json(StateSnapshot {
        connected: state.connected.load(Ordering::SeqCst),
        player_name: settings.player_name,
        primary_id: settings.primary_id,
        session,
        setup_done: settings.setup_done,
        hud_visible: settings.hud_visible,
        http_port: state.http_port.load(Ordering::SeqCst),
    })
}

/// HUD drag handler. Calls Tauri's native window `start_dragging()` so the OS
/// takes over the move loop (no JS-side mouse tracking needed). Short-circuits
/// when the user has locked the position. Returns 404 when the HUD window
/// doesn't exist — that's the OBS browser source case, where the same HTML
/// is loaded outside of Tauri; the frontend snippet ignores 404s.
async fn hud_start_drag(State(state): State<Arc<AppState>>) -> Response {
    if state.settings.lock().hud_position_locked {
        return StatusCode::OK.into_response();
    }
    let app = match state.app_handle.get() {
        Some(a) => a,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "app not ready").into_response(),
    };
    let hud = match app.get_webview_window("hud") {
        Some(w) => w,
        None => return (StatusCode::NOT_FOUND, "no hud window").into_response(),
    };
    if let Err(err) = hud.start_dragging() {
        warn!(?err, "start_dragging failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }
    StatusCode::OK.into_response()
}

/// Right-click "Toggle position lock" handler. Flips the persisted bool and
/// updates the HUD's `ignore_cursor_events` so the lock has a visible effect:
/// locked = click-through (cursor passes to the game), unlocked = interactive.
async fn hud_toggle_lock(State(state): State<Arc<AppState>>) -> Response {
    let new_locked = {
        let mut s = state.settings.lock();
        s.hud_position_locked = !s.hud_position_locked;
        s.hud_position_locked
    };
    state.request_save_settings();
    if let Some(app) = state.app_handle.get() {
        if let Some(hud) = app.get_webview_window("hud") {
            let _ = hud.set_ignore_cursor_events(new_locked);
        }
        let _ = app.emit("rlstats://hud-lock-changed", new_locked);
    }
    Json(serde_json::json!({ "locked": new_locked })).into_response()
}

/// Right-click "Reset session" handler. Mirrors the `reset_session` Tauri
/// command but reachable without `window.__TAURI__` (the HUD's webview is
/// loaded over plain HTTP, not the tauri:// protocol).
async fn session_reset(State(state): State<Arc<AppState>>) -> Response {
    {
        let mut session = state.session.lock();
        session.reset();
        let snapshot = session.clone();
        let mut settings = state.settings.lock();
        settings.session = snapshot;
    }
    state.request_save_settings();
    if let Some(app) = state.app_handle.get() {
        let _ = app.emit("rlstats://session-changed", ());
    }
    StatusCode::OK.into_response()
}

/// Right-click "Quit" handler. Triggers the same exit path as the tray menu
/// entry — drops the tray icon, stops the HTTP / WS tasks, settings get one
/// last flush via the writer's drop guards.
async fn app_quit(State(state): State<Arc<AppState>>) -> Response {
    if let Some(app) = state.app_handle.get() {
        app.exit(0);
    }
    StatusCode::OK.into_response()
}

async fn serve_overlay(AxumPath(path): AxumPath<String>) -> Response {
    let normalised = path.trim_start_matches('/');
    let bytes = match read_overlay_asset(normalised) {
        Some(b) => b,
        None => return (StatusCode::NOT_FOUND, "Not found").into_response(),
    };

    let mime = mime_guess::from_path(normalised)
        .first_or_octet_stream()
        .to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        mime.parse().unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
    );
    // Browser sources cache aggressively — disable so users see updates after
    // the app is upgraded without manually clearing the cache.
    headers.insert(
        header::CACHE_CONTROL,
        "no-store, must-revalidate".parse().unwrap(),
    );

    (headers, bytes).into_response()
}

/// Resolve a `themes/...` (or other overlay) request against bundled
/// assets first, then against `%APPDATA%/RLStatsOverlay/themes/` for
/// user-installed themes. The disk path is sanitised to prevent
/// `..` traversal escaping the themes directory.
fn read_overlay_asset(rel: &str) -> Option<Vec<u8>> {
    if let Some(asset) = OverlayAssets::get(rel) {
        return Some(asset.data.into_owned());
    }
    // Disk fallback only for `themes/<id>/...` to avoid exposing
    // anything else under the user data folder.
    let theme_path = rel.strip_prefix("themes/")?;
    if theme_path.contains("..") || theme_path.contains('\\') {
        return None;
    }
    let user_dir = crate::theme_manifest::user_themes_dir()?;
    let canonical_root = std::fs::canonicalize(&user_dir).ok()?;
    let target = user_dir.join(theme_path);
    let canonical_target = std::fs::canonicalize(&target).ok()?;
    if !canonical_target.starts_with(&canonical_root) {
        return None;
    }
    std::fs::read(canonical_target).ok()
}
