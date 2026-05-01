use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};

use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow,
};
use tracing::{info, warn};

mod http_server;
mod ini_patcher;
mod platform_detect;
mod session;
mod settings;
mod settings_writer;
mod state;
mod theme_manifest;
mod ws_client;

use crate::{
    ini_patcher::{DetectedInstall, PatchOutcome},
    session::Session,
    settings::Settings,
    state::AppState,
};

const SETTINGS_WINDOW: &str = "settings";
const HUD_WINDOW: &str = "hud";

// ---------- Tauri commands exposed to the frontend ---------------------------

#[derive(Serialize)]
struct StateSnapshot {
    connected: bool,
    player_name: String,
    primary_id: String,
    setup_done: bool,
    hud_visible: bool,
    http_port: u16,
    session: Session,
    settings_path: PathBuf,
    overlay_url: String,
    theme: String,
    theme_vars: std::collections::HashMap<String, serde_json::Value>,
    /// Current HUD window geometry in physical pixels. Read live from the
    /// window when possible so it stays accurate after a manual drag.
    hud_x: i32,
    hud_y: i32,
    hud_w: u32,
    hud_h: u32,
    /// Team sizes (1..=4) currently counted toward the W/L tally.
    count_team_sizes: Vec<u8>,
    /// UI language preference: "auto" | "fr" | "en".
    language: String,
    /// True if the boot-time platform-detection found at least one Steam or
    /// Epic candidate ID. The wizard uses this to skip the "type your in-game
    /// name" step. We DON'T expose the raw IDs to JS — they are identifying.
    has_local_platform_candidates: bool,
    /// True when the HUD's position is locked (click-through, drag disabled).
    hud_position_locked: bool,
}

#[tauri::command]
fn get_state(app: AppHandle, state: State<'_, Arc<AppState>>) -> StateSnapshot {
    let settings = state.settings.lock().clone();
    let session = state.session.lock().clone();
    let port = state.http_port.load(Ordering::SeqCst);
    let overlay_url = if port == 0 {
        String::new()
    } else {
        format!("http://localhost:{port}/overlays/boost.html")
    };
    let theme = settings.theme.clone();
    let theme_vars = settings.current_theme_vars();

    // Pull HUD geometry live from the OS window first (it reflects manual
    // drags), and fall back to the persisted values if the window does not
    // exist yet.
    let win = hud_window(&app);
    let (hud_x, hud_y) = win
        .as_ref()
        .and_then(|w| w.outer_position().ok())
        .map(|p| (p.x, p.y))
        .or(settings.hud_pos)
        .unwrap_or((0, 0));
    let (hud_w, hud_h) = win
        .as_ref()
        .and_then(|w| w.outer_size().ok())
        .map(|s| (s.width, s.height))
        .or(settings.hud_size)
        .unwrap_or((400, 300));

    StateSnapshot {
        connected: state.connected.load(Ordering::SeqCst),
        player_name: settings.player_name,
        primary_id: settings.primary_id,
        setup_done: settings.setup_done,
        hud_visible: win.as_ref().map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false),
        http_port: port,
        session,
        settings_path: settings::settings_dir().unwrap_or_default().join("settings.json"),
        overlay_url,
        theme,
        theme_vars,
        hud_x,
        hud_y,
        hud_w,
        hud_h,
        count_team_sizes: settings.count_team_sizes,
        language: settings.language,
        has_local_platform_candidates: !state.local_platform_candidates.lock().is_empty(),
        hud_position_locked: settings.hud_position_locked,
    }
}

#[tauri::command]
fn set_language(language: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let lang = match language.as_str() {
        "auto" | "fr" | "en" => language,
        _ => return Err(format!("unsupported language: {language}")),
    };
    state.settings.lock().language = lang;
    state.request_save_settings();
    Ok(())
}

#[tauri::command]
fn set_player_name(name: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err("name is empty".into());
    }
    let mut settings = state.settings.lock();
    settings.player_name = trimmed;
    // Reset the primary_id so it is re-captured against the new name on the
    // next match — the previous one belonged to the previous player.
    settings.primary_id.clear();
    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn reset_session(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    {
        let mut session = state.session.lock();
        session.reset();
        let snapshot = session.clone();
        let mut settings = state.settings.lock();
        settings.session = snapshot;
        settings.save().map_err(|e| e.to_string())?;
    }
    let _ = app.emit("rlstats://session-changed", ());
    Ok(())
}

#[tauri::command]
fn detect_rocket_league() -> Vec<DetectedInstall> {
    ini_patcher::detect_installations()
}

#[tauri::command]
fn patch_ini(
    path: PathBuf,
    state: State<'_, Arc<AppState>>,
) -> Result<PatchOutcome, String> {
    let target = ini_patcher::resolve_ini_path(&path).map_err(|e| e.to_string())?;
    let outcome = ini_patcher::patch_ini(&target).map_err(|e| e.to_string())?;
    let mut settings = state.settings.lock();
    settings.ini_path = Some(target);
    settings.save().map_err(|e| e.to_string())?;
    Ok(outcome)
}

#[tauri::command]
fn complete_setup(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.settings.lock();
    settings.setup_done = true;
    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_theme(
    app: AppHandle,
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let cleaned = name.trim().to_string();
    if cleaned.is_empty() || !cleaned.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("invalid theme name".into());
    }
    {
        let mut settings = state.settings.lock();
        settings.theme = cleaned;
        settings.save().map_err(|e| e.to_string())?;
    }
    reload_hud_with_cache_bust(&app);
    let _ = app.emit("rlstats://theme-changed", ());
    Ok(())
}

/// Force the in-game HUD webview to fully refetch its URL by appending a
/// timestamp query string. `window.location.reload()` alone is unreliable
/// on macOS WKWebView when headers don't change between responses, even
/// with Cache-Control: no-store; the unique `?t=<ms>` query side-steps
/// every cache layer.
fn reload_hud_with_cache_bust(app: &AppHandle) {
    if let Some(hud) = hud_window(app) {
        let _ = hud.eval(
            "window.location.href = window.location.pathname + '?t=' + Date.now()",
        );
    }
}

#[tauri::command]
fn set_theme_var(
    app: AppHandle,
    key: String,
    value: Option<serde_json::Value>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut settings = state.settings.lock();
        settings.set_theme_var(key, value);
        settings.save().map_err(|e| e.to_string())?;
    }
    // Live-update the in-game HUD without a full reload by re-fetching
    // /api/config from inside the existing webview.
    if let Some(hud) = hud_window(&app) {
        let _ = hud.eval(
            r#"
            (async () => {
              try {
                const m = await import('/overlays/shared/ws-client.js');
                const cfg = await m.loadOverlayConfig();
                m.applyThemeVars(cfg.themeVars);
              } catch (_) { window.location.reload(); }
            })();
            "#,
        );
    }
    let _ = app.emit("rlstats://theme-vars-changed", ());
    Ok(())
}

#[tauri::command]
fn toggle_hud(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let window = hud_window(&app).ok_or("HUD window not initialised")?;
    let now_visible = if window.is_visible().unwrap_or(false) {
        window.hide().map_err(|e| e.to_string())?;
        false
    } else {
        // The HUD's URL points at our embedded HTTP server, which may not
        // have been up yet when the webview was first created at app
        // launch. We force-fetch with a fresh `?t=` query *only on the very
        // first show* — every subsequent toggle should keep the loaded page
        // and just call `show()`, otherwise the user sees a flash + font
        // reload + animation reset every time the HUD is toggled.
        if !state.hud_loaded.swap(true, Ordering::SeqCst) {
            let _ = window.eval(
                "window.location.href = window.location.pathname + '?t=' + Date.now()",
            );
        }
        window.show().map_err(|e| e.to_string())?;
        // Reapply geometry on every show — Tauri can lose it on hide.
        let settings = state.settings.lock();
        if let Some((x, y)) = settings.hud_pos {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        if let Some((w, h)) = settings.hud_size {
            let _ = window.set_size(PhysicalSize::new(w, h));
        }
        true
    };
    {
        let mut s = state.settings.lock();
        s.hud_visible = now_visible;
    }
    state.request_save_settings();
    Ok(now_visible)
}

/// Force-refresh the HUD webview. Useful after editing theme files in
/// debug builds, or to recover from any half-loaded state without
/// closing/reopening the window.
#[tauri::command]
fn reload_hud(app: AppHandle) -> Result<(), String> {
    reload_hud_with_cache_bust(&app);
    Ok(())
}

/// Cleanly stop the whole app — closes the HUD, drops the tray icon, and
/// terminates the embedded HTTP / WS tasks. The "X" on the settings window
/// hides to tray instead, so this is the only path that fully exits.
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Update which team sizes (1..=4) are counted toward the session.
/// Empty list = nothing is counted (the user opted out of every size),
/// which we still allow — useful to "freeze" the session manually.
#[tauri::command]
fn set_count_team_sizes(
    sizes: Vec<u8>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let cleaned: Vec<u8> = sizes
        .into_iter()
        .filter(|s| (1..=4).contains(s))
        .collect();
    {
        let mut settings = state.settings.lock();
        settings.count_team_sizes = cleaned;
    }
    state.request_save_settings();
    Ok(())
}

/// Toggle (or explicitly set) the HUD position lock. When locked, the HUD
/// becomes click-through (cursor events pass to the game), the drag handler
/// short-circuits, and the dashboard checkbox renders checked. The change is
/// persisted asynchronously via the coalescing settings writer.
#[tauri::command]
fn set_hud_locked(
    app: AppHandle,
    locked: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut s = state.settings.lock();
        s.hud_position_locked = locked;
    }
    state.request_save_settings();
    if let Some(hud) = hud_window(&app) {
        let _ = hud.set_ignore_cursor_events(locked);
    }
    let _ = app.emit("rlstats://hud-lock-changed", locked);
    Ok(())
}

/// Discovery of every installed theme — bundled + user-dropped.
/// Called from the settings UI on boot and after the user clicks
/// "Refresh themes" so a freshly-dropped folder appears without restart.
#[tauri::command]
fn list_themes() -> Vec<theme_manifest::ThemeManifest> {
    theme_manifest::discover(
        http_server::OverlayAssets::iter()
            .filter(|p| p.ends_with("/theme.json") && p.starts_with("themes/"))
            .filter_map(|p| {
                http_server::OverlayAssets::get(p.as_ref())
                    .map(|a| (p.into_owned(), a.data.into_owned()))
            }),
    )
}

/// Reveal the user themes folder in Explorer / Finder, creating it if it
/// doesn't exist yet so the click always lands on a real directory.
#[tauri::command]
fn open_themes_folder() -> Result<(), String> {
    let dir = theme_manifest::user_themes_dir()
        .ok_or_else(|| "no user data directory available".to_string())?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    open_folder_in_explorer(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_logs_folder() -> Result<(), String> {
    let dir = settings::logs_dir().map_err(|e| e.to_string())?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    open_folder_in_explorer(&dir).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn open_folder_in_explorer(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("explorer").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_folder_in_explorer(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("open").arg(path).spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_folder_in_explorer(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

/// Apply absolute HUD geometry from the settings UI: each of x/y/w/h is
/// optional, only the provided fields are touched. The new values are
/// pushed to the live window first (visual feedback is immediate) and
/// then persisted to settings so they survive a relaunch.
#[tauri::command]
fn set_hud_geometry(
    app: AppHandle,
    x: Option<i32>,
    y: Option<i32>,
    w: Option<u32>,
    h: Option<u32>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let window = hud_window(&app).ok_or("HUD window not initialised")?;

    // Position: read current, override the requested axes, apply.
    if x.is_some() || y.is_some() {
        let current = window.outer_position().map_err(|e| e.to_string())?;
        let new_x = x.unwrap_or(current.x);
        let new_y = y.unwrap_or(current.y);
        window
            .set_position(PhysicalPosition::new(new_x, new_y))
            .map_err(|e| e.to_string())?;
        let mut settings = state.settings.lock();
        settings.hud_pos = Some((new_x, new_y));
        settings.save().map_err(|e| e.to_string())?;
    }

    // Size: same pattern. We clamp to a sensible minimum so the window
    // stays usable if the user drags a slider too far.
    if w.is_some() || h.is_some() {
        let current = window.outer_size().map_err(|e| e.to_string())?;
        let new_w = w.unwrap_or(current.width).max(80);
        let new_h = h.unwrap_or(current.height).max(60);
        window
            .set_size(PhysicalSize::new(new_w, new_h))
            .map_err(|e| e.to_string())?;
        let mut settings = state.settings.lock();
        settings.hud_size = Some((new_w, new_h));
        settings.save().map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ---------- Helpers ----------------------------------------------------------

fn hud_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(HUD_WINDOW)
}

/// Default HUD size (`tauri.conf.json`: 400×300 physical pixels). Kept in
/// sync with the window declaration — if you bump the conf, bump this.
const DEFAULT_HUD_W: u32 = 400;
const DEFAULT_HUD_H: u32 = 300;

/// Pick a one-shot HUD size based on the monitor the HUD lands on at boot.
/// Returns `None` when we can't read the monitor (we'd rather keep the
/// configured default than guess wrong); otherwise scales the 400×300 base
/// up by the same multiplier so the HUD stays visually the same fraction
/// of screen height across resolutions:
///
/// * ≤1080p → 1.00× (no change)
/// * ≤1440p → 1.25×
/// * ≤2160p (4K) → 1.50×
/// * larger (5K+) → 2.00×
///
/// Decision is based on the monitor's physical *height* — width-only ladders
/// misclassify ultrawides where 3440×1440 is logically 1440p, not 2K-wide.
fn dpi_default_hud_size(hud: &WebviewWindow) -> Option<(u32, u32)> {
    let monitor = hud.current_monitor().ok().flatten()?;
    let h = monitor.size().height;
    let factor: f64 = if h <= 1080 {
        1.0
    } else if h <= 1440 {
        1.25
    } else if h <= 2160 {
        1.5
    } else {
        2.0
    };
    if (factor - 1.0).abs() < f64::EPSILON {
        // Nothing to scale — return None so the caller leaves `hud_size`
        // as `None` and a future tauri.conf bump propagates naturally.
        return None;
    }
    let w = (DEFAULT_HUD_W as f64 * factor).round() as u32;
    let new_h = (DEFAULT_HUD_H as f64 * factor).round() as u32;
    Some((w, new_h))
}

fn settings_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(SETTINGS_WINDOW)
}

/// Bring the settings window back from a hidden state — used by both the
/// tray-icon left click and the "Show" menu entry.
fn show_settings_window(app: &AppHandle) {
    if let Some(win) = settings_window(app) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

fn install_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(
        app,
        "tray-show",
        "Afficher les paramètres",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "tray-quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("tray icon".into()))?;

    TrayIconBuilder::with_id("main")
        .tooltip("RL Stats Overlay")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-show" => show_settings_window(app),
            "tray-quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_settings_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

// ---------- App bootstrap ----------------------------------------------------

pub fn run() {
    // We keep the daily appender's flush guard alive for the entire process —
    // dropping it would silently lose the tail of buffered log lines on exit.
    // We `mem::forget` it because tracing's global subscriber lives until
    // process shutdown anyway; trying to drop it cleanly after `tauri::run`
    // returns would race with other globals.
    let file_layer_and_guard = match settings::logs_dir() {
        Ok(dir) => match std::fs::create_dir_all(&dir) {
            Ok(()) => {
                let appender =
                    tracing_appender::rolling::daily(&dir, "rl-stats-overlay.log");
                let (nb, guard) = tracing_appender::non_blocking(appender);
                Some((nb, guard))
            }
            Err(err) => {
                eprintln!("could not create logs dir {}: {err}", dir.display());
                None
            }
        },
        Err(err) => {
            eprintln!("could not resolve logs dir: {err}");
            None
        }
    };

    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
    let env_filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact()
        .with_filter(env_filter());
    let registry = tracing_subscriber::registry().with(stderr_layer);
    if let Some((writer, guard)) = file_layer_and_guard {
        std::mem::forget(guard);
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_ansi(false)
            .with_target(false)
            .with_filter(env_filter());
        registry.with(file_layer).init();
    } else {
        registry.init();
    }

    // Load settings synchronously before Tauri starts so command handlers can
    // assume they're populated.
    let mut loaded = Settings::load().unwrap_or_default();
    let stale_session_dropped = loaded.session.expire_if_stale();
    if stale_session_dropped {
        info!("previous session was older than 6h, starting fresh");
        let _ = loaded.save();
    }
    // Detect Steam / Epic local IDs once at boot. Cheap (a couple of file
    // reads); used by `find_local_player` to arbitrate the user across
    // `Players[].PrimaryId` without requiring them to type a name.
    let candidates = platform_detect::local_platform_candidates();
    info!(
        count = candidates.len(),
        "local platform candidates detected"
    );
    let app_state = AppState::new(loaded, candidates);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_player_name,
            reset_session,
            detect_rocket_league,
            patch_ini,
            complete_setup,
            toggle_hud,
            reload_hud,
            set_hud_geometry,
            set_theme,
            set_theme_var,
            quit_app,
            list_themes,
            open_themes_folder,
            open_logs_folder,
            set_count_team_sizes,
            set_language,
            set_hud_locked,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            // Stash the AppHandle so the embedded HTTP server can reach the
            // HUD window and `app.exit(0)` from its handlers (the HUD is
            // loaded over plain HTTP, not tauri://, so JS there can't call
            // Tauri commands directly — see CLAUDE.md "Architecture caveat").
            let _ = app_state.app_handle.set(handle.clone());

            // Make sure the HUD window starts hidden, then restore the
            // user's saved visibility/geometry.
            //
            // Click-through (`set_ignore_cursor_events`) is now driven by the
            // position-lock toggle: locked → click-through (game gets cursor
            // events as before), unlocked → interactive (the user can drag
            // the HUD with the mouse, right-click for a context menu).
            if let Some(hud) = hud_window(&handle) {
                let _ = hud.hide();
                {
                    let mut settings = app_state.settings.lock();
                    let _ = hud.set_ignore_cursor_events(settings.hud_position_locked);
                    if let Some((x, y)) = settings.hud_pos {
                        let _ = hud.set_position(PhysicalPosition::new(x, y));
                    }
                    // First-launch DPI auto-scale: only when the user hasn't
                    // resized the HUD yet. Persists immediately so subsequent
                    // launches don't re-scale (one-shot init). See
                    // `dpi_default_hud_size` for the scale ladder.
                    let scaled = if settings.hud_size_is_default() {
                        dpi_default_hud_size(&hud)
                    } else {
                        None
                    };
                    let (target_w, target_h) = settings
                        .hud_size
                        .or(scaled)
                        .unwrap_or((400, 300));
                    let _ = hud.set_size(PhysicalSize::new(target_w, target_h));
                    if scaled.is_some() {
                        settings.hud_size = scaled;
                        // Sync to disk now — the writer is spawned a few lines
                        // below, so the usual `request_save_settings` would
                        // silently drop. A blocking write at boot is fine
                        // (single shot, no hot path).
                        if let Err(err) = settings.save() {
                            warn!(?err, "failed to persist DPI-scaled HUD size");
                        }
                    }
                    let should_show = settings.hud_visible;
                    drop(settings);
                    if should_show {
                        let _ = hud.show();
                        // The webview already loads its URL on creation, so the
                        // user's first toggle should skip the cache-busting
                        // reload that `toggle_hud` does for cold paths.
                        app_state.hud_loaded.store(true, Ordering::SeqCst);
                    }
                }
            }

            // Persist HUD geometry whenever the user moves or resizes it in
            // edit mode. Windows fires Moved at every pixel during a drag
            // (>500/s on a high-refresh display); we update the in-memory
            // value cheaply on each event but defer the disk write to the
            // coalescing settings writer, so a long drag produces ~1 file
            // write instead of ~500.
            if let Some(hud) = hud_window(&handle) {
                let state_clone = app_state.clone();
                hud.on_window_event(move |ev| match ev {
                    tauri::WindowEvent::Moved(pos) => {
                        {
                            let mut s = state_clone.settings.lock();
                            s.hud_pos = Some((pos.x, pos.y));
                        }
                        state_clone.request_save_settings();
                    }
                    tauri::WindowEvent::Resized(size) => {
                        {
                            let mut s = state_clone.settings.lock();
                            s.hud_size = Some((size.width, size.height));
                        }
                        state_clone.request_save_settings();
                    }
                    _ => {}
                });
            }

            // Spawn the background settings writer FIRST — it must be ready
            // before any UpdateState message can land on the WS task,
            // otherwise saves silently drop. See `settings_writer.rs` for
            // why writes have to live off the hot path.
            let writer = settings_writer::spawn(app_state.clone());
            let _ = app_state.settings_writer.set(writer);

            // Spawn the HTTP server (overlay assets + /api/config) and the
            // RL Stats API listener on Tauri's async runtime. Tauri 2 doesn't
            // auto-install a Tokio reactor on the main thread, so plain
            // tokio::spawn would panic with "there is no reactor running".
            // tauri::async_runtime::spawn picks the runtime Tauri actually owns.
            {
                let state_for_http = app_state.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = http_server::start(state_for_http).await {
                        warn!(?err, "embedded HTTP server stopped");
                    }
                });
            }
            ws_client::spawn(handle.clone(), app_state.clone());

            // Settings window: closing the "X" hides to the tray instead of
            // quitting, so the HUD and the global hotkey stay live in the
            // background. Use the tray menu (or the in-app Quit button) to
            // actually exit.
            if let Some(win) = settings_window(&handle) {
                let _ = win.set_title("RL Stats Overlay");
                let win_for_close = win.clone();
                win.on_window_event(move |ev| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                        api.prevent_close();
                        let _ = win_for_close.hide();
                    }
                });
            }

            // System tray icon. Left-click reopens settings (the most common
            // intent after sending the window to tray), right-click opens the
            // menu with explicit Show / Quit items.
            install_tray(&handle)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
