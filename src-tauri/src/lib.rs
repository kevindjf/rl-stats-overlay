use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};

use clap::Parser;
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::WebviewWindowBuilder,
    AppHandle, Emitter, Listener, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindow,
};
use tracing::{info, warn};

mod http_server;
mod ini_patcher;
mod platform_detect;
mod rl_process;
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
    state::{AppState, MatchStats},
};

/// CLI flags. Parsed at boot via [`clap`]; `--help` / `--version` exit
/// cleanly before the GUI is created.
#[derive(Debug, clap::Parser)]
#[command(version, about = "Rocket League stats overlay")]
struct Cli {
    /// Override the embedded HTTP server port (default: 49124).
    #[arg(long)]
    http_port: Option<u16>,

    /// Force a specific PrimaryId — skips auto-detection.
    /// Format: `Platform|Uid|Splitscreen` (e.g. `Steam|123|0`).
    #[arg(long)]
    player_id: Option<String>,

    /// Skip the wizard's auto-INI patch step (assumes the Stats API is
    /// already enabled).
    #[arg(long, default_value_t = false)]
    no_auto_install: bool,
}

const SETTINGS_WINDOW: &str = "settings";
const HUD_WINDOW: &str = "hud";
const LAUNCHER_WINDOW: &str = "launcher";

/// Visual diameter of the floating launcher badge, in *logical* pixels at the
/// primary monitor's scale factor. The CSS round shape (`clip-path: circle`)
/// fills this exactly; the OS hit-zone is the same square (no pixel-perfect
/// click-through — see Q5 in the design notes).
const LAUNCHER_LOGICAL_DIAMETER: f64 = 56.0;

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
    /// When true, the HUD auto-shows on RL connect / auto-hides on disconnect.
    auto_hide_hud_when_offline: bool,
    /// Live per-match stats decoded from `UpdateState`. Empty between matches.
    match_stats: MatchStats,
    /// True when the app was launched with `--no-auto-install`. The wizard
    /// uses this to short-circuit the auto-INI-patch step.
    no_auto_install: bool,
    /// True when the floating launcher badge should be created on app start
    /// and re-shown between matches.
    launcher_enabled: bool,
    /// True between MatchInitialized/MatchCreated and MatchDestroyed. The
    /// dashboard uses this to grey out controls that are pointless mid-match
    /// and to mirror the launcher's hidden state.
    match_in_progress: bool,
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
    let launcher_enabled = settings.launcher_enabled;

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
        auto_hide_hud_when_offline: settings.auto_hide_hud_when_offline,
        match_stats: state.match_stats.lock().clone(),
        no_auto_install: state.no_auto_install.load(Ordering::SeqCst),
        launcher_enabled,
        match_in_progress: state.match_in_progress.load(Ordering::SeqCst),
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

/// Atomic bulk-edit of every session counter — recovery for missed
/// match-end events, app crashes mid-match, manual cleanup. The data
/// is local; "anti-cheating" guards would only get in the user's way.
/// Caller passes raw `i64` values; we clamp to each field's actual
/// type (`u32` for counts, `i32` for streak).
#[tauri::command]
fn set_session_full(
    app: AppHandle,
    wins: i64,
    losses: i64,
    streak: i64,
    best_win_streak: i64,
    best_loss_streak: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let clamp_u32 = |v: i64| v.max(0).min(u32::MAX as i64) as u32;
    let clamp_i32 = |v: i64| v.max(i32::MIN as i64).min(i32::MAX as i64) as i32;
    {
        let mut session = state.session.lock();
        session.wins = clamp_u32(wins);
        session.losses = clamp_u32(losses);
        session.streak = clamp_i32(streak);
        session.best_win_streak = clamp_u32(best_win_streak);
        session.best_loss_streak = clamp_u32(best_loss_streak);
        // Touch last_update so a manual edit doesn't trip the 6h-stale
        // expiry on next boot.
        session.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let snapshot = session.clone();
        let mut settings = state.settings.lock();
        settings.session = snapshot;
    }
    state.request_save_settings();
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
    }
    // Async-persist to disk via the coalescing writer — keeps slider drags
    // from hammering the disk on every tick.
    state.request_save_settings();
    push_theme_vars_to_hud(&app);
    let _ = app.emit("rlstats://theme-vars-changed", ());
    Ok(())
}

/// Wipe every override for the active theme in one call. Faster than the
/// frontend looping `set_theme_var(key, null)` per-var (which races with
/// itself) AND uses the diff-aware `applyThemeVars` so the HUD repaints to
/// the theme defaults without a full reload.
#[tauri::command]
fn reset_theme_vars(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut settings = state.settings.lock();
        let active = settings.theme.clone();
        settings.theme_overrides.remove(&active);
    }
    state.request_save_settings();
    push_theme_vars_to_hud(&app);
    let _ = app.emit("rlstats://theme-vars-changed", ());
    Ok(())
}

/// Re-fetch `/api/config` from inside the HUD webview and reapply theme
/// vars without a page reload. Shared by `set_theme_var` and
/// `reset_theme_vars` so they stay perfectly in sync.
fn push_theme_vars_to_hud(app: &AppHandle) {
    if let Some(hud) = hud_window(app) {
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
}

#[tauri::command]
fn toggle_hud(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let window = hud_window(&app).ok_or("HUD window not initialised")?;
    let now_visible = if window.is_visible().unwrap_or(false) {
        hide_hud_window(&window).map_err(|e| e.to_string())?;
        false
    } else {
        show_hud_window(&window, &state).map_err(|e| e.to_string())?;
        true
    };
    {
        let mut s = state.settings.lock();
        s.hud_visible = now_visible;
    }
    state.request_save_settings();
    Ok(now_visible)
}

/// Show the HUD webview, force a one-shot cache-busted reload on the very
/// first cold show (so the embedded HTTP server has a chance to come up),
/// and reapply the persisted geometry. Shared between [`toggle_hud`] and
/// the auto-show listener for `rlstats://connected`.
fn show_hud_window(
    window: &WebviewWindow,
    state: &Arc<AppState>,
) -> Result<(), tauri::Error> {
    if !state.hud_loaded.swap(true, Ordering::SeqCst) {
        let _ = window.eval(
            "window.location.href = window.location.pathname + '?t=' + Date.now()",
        );
    }
    window.show()?;
    let settings = state.settings.lock();
    if let Some((x, y)) = settings.hud_pos {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
    if let Some((w, h)) = settings.hud_size {
        let _ = window.set_size(PhysicalSize::new(w, h));
    }
    Ok(())
}

fn hide_hud_window(window: &WebviewWindow) -> Result<(), tauri::Error> {
    window.hide()
}

/// Toggle the "auto-show HUD on RL connect / auto-hide on disconnect"
/// preference. Persisted to `settings.json` so it survives a restart.
///
/// Also reconciles the HUD against the *current* connection state — the
/// `rlstats://connected` listener only fires on transitions, so without
/// this immediate apply the user could enable the setting while RL is
/// already offline and the HUD would stay visible until the next reconnect.
#[tauri::command]
fn set_auto_hide_hud_when_offline(
    app: AppHandle,
    enabled: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut s = state.settings.lock();
        s.auto_hide_hud_when_offline = enabled;
    }
    state.request_save_settings();
    if let Some(window) = hud_window(&app) {
        if enabled {
            // Just turned ON — reconcile to the live connection state so the
            // HUD reflects "RL connected?" right now, not just on the next
            // transition.
            if state.connected.load(Ordering::SeqCst) {
                let _ = show_hud_window(&window, &state);
            } else {
                let _ = hide_hud_window(&window);
            }
        } else {
            // Just turned OFF — hand control back to the user's manual
            // preference (`settings.hud_visible`, set by the dashboard's
            // Show/Hide buttons). Without this, a HUD hidden by auto-hide
            // would stay hidden indefinitely after the toggle is disabled.
            let want_visible = state.settings.lock().hud_visible;
            if want_visible {
                let _ = show_hud_window(&window, &state);
            } else {
                let _ = hide_hud_window(&window);
            }
        }
    }
    Ok(())
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

/// Toggle the floating launcher badge on/off. When enabled and out-of-match,
/// the badge is created on the left edge of the primary monitor; when
/// disabled, the existing window is *closed* (not hidden) so a re-enable
/// rebuilds it cleanly with fresh geometry.
#[tauri::command]
fn set_launcher_enabled(
    app: AppHandle,
    enabled: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    {
        let mut s = state.settings.lock();
        s.launcher_enabled = enabled;
    }
    state.request_save_settings();
    let state_arc = state.inner().clone();
    reconcile_launcher_visibility(&app, &state_arc);
    Ok(())
}

/// Click handler for the floating launcher badge — brings the Settings
/// window to the front. Mirrors the tray-icon left-click path; the only
/// reason this is a separate command is to make it explicit in invoke
/// permission rules and easier to grep in logs.
#[tauri::command]
fn open_settings_from_launcher(app: AppHandle) {
    show_settings_window(&app);
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

fn launcher_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LAUNCHER_WINDOW)
}

/// Build the floating launcher badge — a small round always-on-top window
/// pinned to the left edge of the primary monitor, vertically centered.
/// Stays hidden until [`reconcile_launcher_visibility`] decides to show it
/// (we don't want a one-frame flash before the in-match check runs).
///
/// Returns silently if the HTTP server hasn't bound yet (port 0) — the
/// setup callback will retry once the bind completes.
fn create_launcher_window(handle: &AppHandle) -> tauri::Result<()> {
    if launcher_window(handle).is_some() {
        return Ok(());
    }
    let state = match handle.try_state::<Arc<AppState>>() {
        Some(s) => s,
        None => return Ok(()),
    };
    let port = state.http_port.load(Ordering::SeqCst);
    if port == 0 {
        // HTTP server not yet bound — retry later. Returning Ok keeps the
        // setup chain quiet; the caller will reconcile when the port lands.
        return Ok(());
    }

    let url = format!("http://localhost:{port}/overlays/launcher/launcher.html");
    let parsed_url: tauri::Url = url
        .parse()
        .map_err(|e: <tauri::Url as std::str::FromStr>::Err| {
            tauri::Error::AssetNotFound(e.to_string())
        })?;
    let win = WebviewWindowBuilder::new(handle, LAUNCHER_WINDOW, WebviewUrl::External(parsed_url))
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false)
        .build()?;

    // Probe the primary monitor *after* build so we can use the same window
    // handle to query the OS — `AppHandle::primary_monitor` is also fine but
    // this keeps every geometry decision next to the window it affects.
    if let Ok(Some(monitor)) = win.primary_monitor() {
        let scale = monitor.scale_factor().max(1.0);
        let physical_diameter =
            (LAUNCHER_LOGICAL_DIAMETER * scale).round().max(16.0) as u32;
        let monitor_size = monitor.size();
        let monitor_pos = monitor.position();
        // Left edge of the monitor, vertically centered.
        let x = monitor_pos.x;
        let y = monitor_pos.y
            + ((monitor_size.height as i32 - physical_diameter as i32) / 2).max(0);
        let _ = win.set_size(PhysicalSize::new(physical_diameter, physical_diameter));
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    if !state.match_in_progress.load(Ordering::SeqCst) {
        let _ = win.show();
    }
    Ok(())
}

/// Tear down the launcher window when the user disables the toggle. We use
/// `.close()` (not `.hide()`) so a re-enable rebuilds it cleanly with fresh
/// geometry — the user may have moved the primary monitor in the meantime.
fn destroy_launcher_window(handle: &AppHandle) {
    if let Some(win) = launcher_window(handle) {
        let _ = win.close();
    }
}

/// Single point of truth for the launcher's visibility. Called from setup,
/// the `set_launcher_enabled` command, and the `match-in-progress` listener.
/// Cheap enough to invoke on every transition — no work is done if the state
/// already matches.
fn reconcile_launcher_visibility(handle: &AppHandle, state: &Arc<AppState>) {
    let enabled = state.settings.lock().launcher_enabled;
    let in_match = state.match_in_progress.load(Ordering::SeqCst);

    if !enabled {
        destroy_launcher_window(handle);
        return;
    }

    if in_match {
        // Match running — keep the window alive (cheap to hide), just
        // suppress it visually so the badge doesn't sit over the game.
        if let Some(win) = launcher_window(handle) {
            let _ = win.hide();
        }
        return;
    }

    // Enabled and out-of-match: ensure the window exists, then show it.
    if launcher_window(handle).is_none() {
        if let Err(err) = create_launcher_window(handle) {
            warn!(?err, "failed to create launcher window");
            return;
        }
    }
    if let Some(win) = launcher_window(handle) {
        let _ = win.show();
    }
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

/// Bring the settings window back from a hidden state — used by the tray
/// left click, the "Show" menu entry, and the floating launcher's click
/// handler. Every entry-point flips `user_wants_settings_open = true` so
/// the auto-hide-on-match-start listener knows the user genuinely opened
/// the window (and so the next match-start hide is a real suppression,
/// not a no-op against an already-hidden window).
fn show_settings_window(app: &AppHandle) {
    if let Some(win) = settings_window(app) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        state.user_wants_settings_open.store(true, Ordering::SeqCst);
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
    // Parse CLI flags first so `--help` / `--version` exit cleanly without
    // bringing up the Tauri window. `Cli::parse()` calls `std::process::exit`
    // internally for those paths.
    let cli = Cli::parse();

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
    // `--player-id`: override the persisted PrimaryId for this run only.
    // We don't persist it — the flag is meant for ephemeral / OBS-only
    // headless setups where the user wants a known identity per launch.
    if let Some(ref pid) = cli.player_id {
        info!(player_id = %pid, "CLI override for primary_id");
        loaded.primary_id = pid.clone();
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
    // Apply `--no-auto-install` to shared state so the wizard frontend can
    // read it via `StateSnapshot::no_auto_install`.
    app_state.no_auto_install.store(cli.no_auto_install, Ordering::SeqCst);
    // Apply `--http-port` if provided. The HTTP server reads this on bind:
    // a non-zero value is treated as "use this exact port, don't scan".
    if let Some(port) = cli.http_port {
        app_state.http_port.store(port, Ordering::SeqCst);
        info!(http_port = port, "CLI override for http_port");
    }

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
            set_session_full,
            detect_rocket_league,
            patch_ini,
            complete_setup,
            toggle_hud,
            reload_hud,
            set_hud_geometry,
            set_theme,
            set_theme_var,
            reset_theme_vars,
            quit_app,
            list_themes,
            open_themes_folder,
            open_logs_folder,
            set_count_team_sizes,
            set_language,
            set_hud_locked,
            set_auto_hide_hud_when_offline,
            set_launcher_enabled,
            open_settings_from_launcher,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            // Stash the AppHandle so the embedded HTTP server can reach the
            // HUD window and `app.exit(0)` from its handlers (the HUD is
            // loaded over plain HTTP, not tauri://, so JS there can't call
            // Tauri commands directly — see CLAUDE.md "Architecture caveat").
            let _ = app_state.app_handle.set(handle.clone());

            // The Settings window is `visible: true` in tauri.conf.json, so
            // the user always sees it on cold boot. Treat that as "user
            // wants it open" — without this the very first match-start
            // would NOT auto-hide the window (because the suppression check
            // gates on `user_wants_settings_open` to distinguish a hidden
            // window from one the user actively dismissed).
            app_state
                .user_wants_settings_open
                .store(true, Ordering::SeqCst);

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

            // Auto-show / auto-hide the HUD based on RL connection — only
            // active when the user opted in via `auto_hide_hud_when_offline`.
            // The `rlstats://connected` event payload is a JSON-encoded bool
            // (`"true"` / `"false"`); parse defensively and ignore other
            // shapes so unrelated emits never flap the window.
            {
                let handle_listen = handle.clone();
                let state_listen = app_state.clone();
                handle.listen("rlstats://connected", move |event| {
                    let connected = serde_json::from_str::<bool>(event.payload())
                        .ok()
                        .unwrap_or(false);
                    if !state_listen.settings.lock().auto_hide_hud_when_offline {
                        return;
                    }
                    let Some(window) = hud_window(&handle_listen) else { return };
                    if connected {
                        let _ = show_hud_window(&window, &state_listen);
                    } else {
                        let _ = hide_hud_window(&window);
                    }
                });
            }

            // Floating launcher badge:
            //   1. Create it once the embedded HTTP server has bound a port
            //      (polled because `start()` is async and we don't want to
            //      block the setup callback waiting on it).
            //   2. Wire the `rlstats://match-in-progress` listener so the
            //      badge auto-hides during a match, the Settings window also
            //      auto-hides on match start (only if currently visible AND
            //      flagged as user-opened), and the badge re-shows when the
            //      match ends. We deliberately do *not* re-show the Settings
            //      window on match-end — the user reopens it manually via the
            //      badge or tray (per Q7 of the design notes).
            {
                let handle_launcher = handle.clone();
                let state_launcher = app_state.clone();
                tauri::async_runtime::spawn(async move {
                    // Give the HTTP server up to ~5s to bind a port. The
                    // typical bind is sub-100ms; the long ceiling just
                    // covers an unlucky port-conflict scan.
                    for _ in 0..50 {
                        if state_launcher.http_port.load(Ordering::SeqCst) != 0 {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    // Always run reconcile — it is the single point of truth
                    // and decides "create vs. destroy vs. nothing" based on
                    // settings + match flag.
                    reconcile_launcher_visibility(&handle_launcher, &state_launcher);
                });
            }
            {
                let handle_listen = handle.clone();
                let state_listen = app_state.clone();
                handle.listen("rlstats://match-in-progress", move |event| {
                    let in_match = serde_json::from_str::<bool>(event.payload())
                        .ok()
                        .unwrap_or(false);
                    reconcile_launcher_visibility(&handle_listen, &state_listen);
                    if in_match {
                        // Match just started — hide the Settings window
                        // *only if* it's currently visible and the user
                        // had explicitly opened it. We don't change
                        // `user_wants_settings_open` so the boolean keeps
                        // tracking the user's intent (the launcher click
                        // path will set it true again on reopen).
                        if let Some(win) = settings_window(&handle_listen) {
                            let visible = win.is_visible().unwrap_or(false);
                            let wants =
                                state_listen.user_wants_settings_open.load(Ordering::SeqCst);
                            if visible && wants {
                                let _ = win.hide();
                            }
                        }
                    }
                });
            }

            // Settings window: closing the "X" hides to the tray instead of
            // quitting, so the HUD and the global hotkey stay live in the
            // background. Use the tray menu (or the in-app Quit button) to
            // actually exit.
            if let Some(win) = settings_window(&handle) {
                let _ = win.set_title("RL Stats Overlay");
                let win_for_close = win.clone();
                let state_for_close = app_state.clone();
                win.on_window_event(move |ev| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                        api.prevent_close();
                        let _ = win_for_close.hide();
                        // The user just dismissed the window — clear the
                        // "wants open" flag so the next match-start hide
                        // doesn't re-trigger against an already-hidden
                        // window (and so the launcher remains the only
                        // path back to settings during a match).
                        state_for_close
                            .user_wants_settings_open
                            .store(false, Ordering::SeqCst);
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
