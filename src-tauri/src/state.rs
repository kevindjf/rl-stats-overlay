use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, AtomicU16, AtomicU8},
    Arc,
};

use crate::{session::Session, settings::Settings, settings_writer::SettingsWriter};

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
}

impl AppState {
    pub fn new(settings: Settings) -> Arc<Self> {
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
