//! Background settings persister.
//!
//! Writing `settings.json` is `fs::write` + atomic rename — on Windows that
//! can spike to tens of milliseconds when Defender scans the file. Doing it
//! synchronously on the TCP read path or inside a `WindowEvent` callback
//! freezes Rocket League (the kernel send buffer fills, RL's `send()` blocks)
//! and stutters the HUD drag (events queue while we wait on disk).
//!
//! This module owns a dedicated task that:
//! * coalesces save requests via a `mpsc::channel(1)` — extra requests during
//!   a debounce window collapse into a single write,
//! * runs the actual file I/O on `tokio::task::spawn_blocking` so the async
//!   runtime threads are never parked on a syscall.
//!
//! Callers update the in-memory `Settings` (under the existing parking_lot
//! mutex), drop the lock, and then call `request_save()`. The writer takes
//! a fresh snapshot when it eventually fires, so racing updates always
//! converge to the latest state.

use std::{sync::Arc, time::Duration};

use tokio::sync::mpsc;
use tracing::warn;

use crate::state::AppState;

const COALESCE_WINDOW: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub struct SettingsWriter {
    tx: mpsc::Sender<()>,
}

impl SettingsWriter {
    /// Schedule a save. Cheap, lock-free, never blocks. If a save is already
    /// queued, this call is a no-op (the queued save will pick up the latest
    /// snapshot when it runs).
    pub fn request_save(&self) {
        let _ = self.tx.try_send(());
    }
}

/// Spawn the writer task on Tauri's async runtime and return a handle that
/// callers use to nudge it. The task lives for the duration of the app.
pub fn spawn(state: Arc<AppState>) -> SettingsWriter {
    let (tx, mut rx) = mpsc::channel::<()>(1);
    tauri::async_runtime::spawn(async move {
        while rx.recv().await.is_some() {
            // Coalesce additional requests that arrive in the next ~150 ms
            // — drag events fire hundreds of times per second, but we only
            // need to persist the final position.
            tokio::time::sleep(COALESCE_WINDOW).await;
            while rx.try_recv().is_ok() {}

            let snapshot = state.settings.lock().clone();
            match tokio::task::spawn_blocking(move || snapshot.save()).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => warn!(?err, "failed to persist settings"),
                Err(err) => warn!(?err, "settings writer task panicked"),
            }
        }
    });
    SettingsWriter { tx }
}
