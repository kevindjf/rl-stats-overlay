use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Live, in-memory session state. Persisted to disk via [`crate::settings`] so
/// the user's wins/losses survive an app restart.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Session {
    pub wins: u32,
    pub losses: u32,
    /// Positive = current win streak, negative = current loss streak.
    pub streak: i32,
    pub best_win_streak: u32,
    pub best_loss_streak: u32,
    /// Unix epoch ms of the last update — used to expire stale sessions.
    pub last_update: u64,
}

impl Session {
    /// Sessions older than this are dropped on app start so the previous day's
    /// numbers don't leak into a new gaming session.
    pub const TIMEOUT_MS: u64 = 1000 * 60 * 60 * 6;

    pub fn record_win(&mut self) {
        self.wins += 1;
        self.streak = if self.streak >= 0 { self.streak + 1 } else { 1 };
        if self.streak > 0 && (self.streak as u32) > self.best_win_streak {
            self.best_win_streak = self.streak as u32;
        }
        self.touch();
    }

    pub fn record_loss(&mut self) {
        self.losses += 1;
        self.streak = if self.streak <= 0 { self.streak - 1 } else { -1 };
        let loss_run = self.streak.unsigned_abs();
        if self.streak < 0 && loss_run > self.best_loss_streak {
            self.best_loss_streak = loss_run;
        }
        self.touch();
    }

    pub fn reset(&mut self) {
        *self = Self::default();
        self.touch();
    }

    /// Return true and clear the session if it is older than [`Self::TIMEOUT_MS`].
    pub fn expire_if_stale(&mut self) -> bool {
        let now = now_ms();
        if self.last_update == 0 {
            self.last_update = now;
            return false;
        }
        if now.saturating_sub(self.last_update) >= Self::TIMEOUT_MS {
            *self = Self::default();
            self.last_update = now;
            return true;
        }
        false
    }

    fn touch(&mut self) {
        self.last_update = now_ms();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
