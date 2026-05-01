use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU16, AtomicU8},
    Arc,
};
use tauri::AppHandle;

use crate::{session::Session, settings::Settings, settings_writer::SettingsWriter};

/// Per-player stats decoded from each `UpdateState` tick. Mirrors the official
/// fields documented in `docs/stats-api-reference.md` (Players[]). Missing
/// fields default to zero so a partial payload never poisons the rendered UI.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub primary_id: String,
    pub name: String,
    pub team_num: u8,
    pub goals: u32,
    pub saves: u32,
    pub shots: u32,
    pub assists: u32,
    pub score: u32,
}

/// Snapshot of the live match (per-player + per-team) refreshed on every
/// `UpdateState` tick and reset between matches. Surface for the dashboard
/// and themes that want to display in-match stats alongside the rolling
/// session counters.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchStats {
    /// One entry per player currently in the match.
    pub players: Vec<PlayerStats>,
    /// `[blue, orange]` team scores from `Game.Teams[].Score`.
    pub team_scores: [u32; 2],
    /// Seconds remaining on the match clock (`Game.TimeSeconds`).
    pub time_seconds: u32,
    /// True when `Game.bOvertime` is set.
    pub overtime: bool,
}

/// State shared across the WebSocket worker, the embedded HTTP server, and
/// the Tauri command handlers. All fields are wrapped in cheap synchronization
/// primitives to keep ownership simple.
pub struct AppState {
    pub settings: Mutex<Settings>,
    pub session: Mutex<Session>,
    /// True while the WebSocket connection to RL's Stats API is alive.
    pub connected: AtomicBool,
    /// Actual port the embedded HTTP server bound to (49124 by default,
    /// or the next free one if it was already in use).
    pub http_port: AtomicU16,
    /// Local player's TeamNum, captured from each `UpdateState` tick.
    pub local_team: Mutex<Option<i32>>,
    /// Last MatchGuid we already counted toward the W/L tally — guards
    /// against double-counting if events replay.
    pub last_counted_match: Mutex<Option<String>>,
    /// Team size of the current match (1, 2, 3, 4) — the max players-per-team
    /// count we've seen in the latest UpdateState. 0 = unknown / no data yet.
    /// Read at `MatchEnded` to decide whether to count the result toward the
    /// session, given the user's `settings.count_team_sizes` filter.
    pub current_team_size: AtomicU8,
    /// True once the HUD webview has loaded its URL at least once. Prevents
    /// re-running the cache-busting reload on every show — see `toggle_hud`.
    pub hud_loaded: AtomicBool,
    /// Background writer for `settings.json`. Set once at boot, before any
    /// `UpdateState` can land. Wrapped in `OnceCell` to keep `AppState::new`
    /// callable before the writer task is spawned.
    pub settings_writer: OnceCell<SettingsWriter>,
    /// Tauri `AppHandle`, populated in the `setup` callback. The embedded
    /// HTTP server needs it to reach the HUD window (drag, lock toggle) and
    /// to call `app.exit(0)` from the right-click menu's "Quit" entry.
    pub app_handle: OnceCell<AppHandle>,
    /// `"Platform|Uid|"` prefixes detected on this machine (Steam
    /// `loginusers.vdf` + Epic `Saved\Data\*.dat` filenames). Refreshed at
    /// boot **and** every time the WebSocket reconnects to RL — the latter
    /// catches account additions made while the app stayed open (closing RL
    /// to switch Epic accounts is the only realistic path to a new account
    /// mid-session, and that path always cycles the WebSocket). Empty when
    /// nothing was detected (non-Windows, or the user hasn't logged into
    /// Steam/Epic on this machine).
    pub local_platform_candidates: Mutex<Vec<String>>,
    /// Live per-player + per-team match stats decoded from `UpdateState`.
    /// Reset to defaults between matches. See [`MatchStats`].
    pub match_stats: Mutex<MatchStats>,
    /// Mirrors the `--no-auto-install` CLI flag. When true, the wizard
    /// short-circuits the auto-INI-patch step (the user is asserting that
    /// the Stats API is already enabled). Surface via [`StateSnapshot`].
    pub no_auto_install: AtomicBool,
}

impl AppState {
    pub fn new(settings: Settings, local_platform_candidates: Vec<String>) -> Arc<Self> {
        let session = settings.session.clone();
        Arc::new(Self {
            settings: Mutex::new(settings),
            session: Mutex::new(session),
            connected: AtomicBool::new(false),
            http_port: AtomicU16::new(0),
            local_team: Mutex::new(None),
            last_counted_match: Mutex::new(None),
            current_team_size: AtomicU8::new(0),
            hud_loaded: AtomicBool::new(false),
            settings_writer: OnceCell::new(),
            app_handle: OnceCell::new(),
            local_platform_candidates: Mutex::new(local_platform_candidates),
            match_stats: Mutex::new(MatchStats::default()),
            no_auto_install: AtomicBool::new(false),
        })
    }

    /// Schedule an async write of `settings.json`. Drops cleanly if the writer
    /// hasn't been initialised yet (only happens during the very narrow boot
    /// window before `lib::run` spawns it).
    pub fn request_save_settings(&self) {
        if let Some(w) = self.settings_writer.get() {
            w.request_save();
        }
    }
}
