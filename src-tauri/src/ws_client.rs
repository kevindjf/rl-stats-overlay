use anyhow::Result;
use parking_lot::Mutex;
use serde::Deserialize;
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};
use tokio::{io::AsyncReadExt, net::TcpStream, time::sleep};
use tracing::{debug, info, warn};

use crate::state::{AppState, MatchStats, PlayerStats};

/// Minimum gap between two `rlstats://match-stats` events. UpdateState
/// arrives at the user's PacketSendRate (up to 120 Hz); we coalesce so the
/// frontend only repaints a few times per second.
const MATCH_STATS_EMIT_DEBOUNCE: Duration = Duration::from_millis(250);

/// Last time we emitted `rlstats://match-stats` and the last value emitted
/// (so we suppress strictly-equal updates entirely). Static state because
/// the WS task is single-instance for the lifetime of the process.
static MATCH_STATS_GATE: Mutex<Option<MatchStatsEmitGate>> = Mutex::new(None);
struct MatchStatsEmitGate {
    last_at: Instant,
    last_value: MatchStats,
}

/// TCP endpoint of the official Rocket League Stats API. Despite older
/// references calling this a "WebSocket", current RL builds expose it as
/// a plain TCP stream of brace-delimited JSON envelopes — no HTTP upgrade,
/// no framing beyond JSON object boundaries.
pub const RL_STATS_API_HOST: &str = "127.0.0.1";
pub const RL_STATS_API_PORT: u16 = 49123;

const RECONNECT_INITIAL_MS: u64 = 1000;
const RECONNECT_MAX_MS: u64 = 10_000;
/// A connection that survives this long is considered "stable" — we reset the
/// reconnect backoff to its initial value when it eventually drops, so a long
/// uptime followed by a single hiccup doesn't penalise the next reconnect.
const STABLE_CONNECTION_MS: u64 = 5_000;
/// If RL was closed for at least this long before reconnecting, we treat the
/// new connection as a fresh gaming session and wipe wins/losses/streak.
/// 5 min absorbs short crashes / restarts / map reloads, while still catching
/// "I closed RL, did something else, came back" flows.
const RL_RELAUNCH_RESET_THRESHOLD: Duration = Duration::from_secs(60 * 5);

/// Spawn a background task that keeps a TCP connection alive to the
/// Rocket League Stats API. The task takes care of (re)connecting, parsing
/// every event, and updating the shared [`AppState`].
///
/// Uses `tauri::async_runtime::spawn` so the task runs on the runtime Tauri
/// owns (Tauri 2 doesn't enter a Tokio reactor on the main thread, so
/// plain `tokio::spawn` would panic).
pub fn spawn(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut delay = RECONNECT_INITIAL_MS;
        // Time at which we lost a previously-active connection. Consumed on
        // the next successful connect to decide whether to auto-reset the
        // session (RL was closed long enough that this is a new run).
        let mut last_disconnect: Option<Instant> = None;
        loop {
            let connected_at = Instant::now();
            match run_connection(&app, &state, &mut last_disconnect).await {
                Ok(()) => {
                    debug!("stats API connection closed cleanly, reconnecting in {delay}ms");
                }
                Err(err) => {
                    debug!(?err, "stats API connection failed, reconnecting in {delay}ms");
                }
            }
            // If we were connected at any point during this iteration, mark
            // the disconnection time so the next reconnect can decide whether
            // to wipe the session.
            let was_connected = state.connected.swap(false, Ordering::SeqCst);
            if was_connected {
                let _ = app.emit("rlstats://connected", false);
                last_disconnect = Some(Instant::now());
                // A connection that stayed up long enough means whatever
                // backoff we'd accumulated isn't relevant any more.
                if connected_at.elapsed() >= Duration::from_millis(STABLE_CONNECTION_MS) {
                    delay = RECONNECT_INITIAL_MS;
                }
                // RL went away mid-match — persist a forensic snapshot of
                // whatever the recorder accumulated so a subsequent boot can
                // reconcile the result. Done before clearing match_in_progress
                // so any listeners that gate on it see a consistent state.
                let had_pending_match = state.recorder.lock().is_some();
                if had_pending_match {
                    let guid = state
                        .recorder
                        .lock()
                        .as_ref()
                        .map(|r| r.match_guid.clone())
                        .unwrap_or_default();
                    finalize_match(
                        &app,
                        &state,
                        &guid,
                        None,
                        MatchEndSource::AbnormalDisconnect,
                    );
                }
                // Clear the match flag and notify listeners so the floating
                // launcher comes back. Only emit on transition (avoids spam
                // if RL crashed between matches).
                if state.match_in_progress.swap(false, Ordering::SeqCst) {
                    let _ = app.emit("rlstats://match-in-progress", false);
                }
            }
            sleep(Duration::from_millis(jittered(delay))).await;
            delay = (delay * 3 / 2).min(RECONNECT_MAX_MS);
        }
    });
}

/// Apply ±20% jitter from a cheap pseudo-random source. Avoids dragging in
/// the `rand` crate just for this, and the entropy is good enough for
/// reconnect spreading.
fn jittered(base_ms: u64) -> u64 {
    let entropy = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
        % 41; // 0..=40
    let pct = entropy as i64 - 20; // -20..=+20
    let delta = (base_ms as i64 * pct) / 100;
    (base_ms as i64 + delta).max(0) as u64
}

async fn run_connection(
    app: &AppHandle,
    state: &Arc<AppState>,
    last_disconnect: &mut Option<Instant>,
) -> Result<()> {
    let mut stream = TcpStream::connect((RL_STATS_API_HOST, RL_STATS_API_PORT)).await?;
    info!("connected to Rocket League Stats API on {RL_STATS_API_HOST}:{RL_STATS_API_PORT}");
    // Each new connection starts as "real source" until the peer asserts
    // otherwise via a `MockHandshake` event. Without this reset, switching
    // from the dev mock back to the real game would keep the session-skip
    // behavior stuck on.
    state.mock_source.store(false, Ordering::SeqCst);

    // TCP keep-alive: probe after 1 s of idle, then every 1 s. Caps
    // disconnect detection at ~2-4 s on Windows when RL exits without
    // sending FIN (which it does — even on "Quit", it just terminates the
    // process). Without keep-alive the kernel default would be 2 hours.
    // We're on localhost so the per-second probe cost is negligible.
    // `with_retries` isn't available on Windows in socket2, so we accept
    // the OS default count there. Best-effort — log and continue.
    {
        let sock = socket2::SockRef::from(&stream);
        let ka = socket2::TcpKeepalive::new()
            .with_time(Duration::from_secs(1))
            .with_interval(Duration::from_secs(1));
        if let Err(err) = sock.set_tcp_keepalive(&ka) {
            warn!(?err, "failed to enable TCP keep-alive on stats API socket");
        }
    }

    // Re-scan local Steam/Epic identifiers on every WS connect. A user who
    // closes RL, switches Epic accounts in the launcher (writing a fresh
    // `.dat`), and relaunches RL would otherwise be stuck with the boot-time
    // candidate list. The scan is cheap (a couple of file lookups) and the
    // reconnect is the canonical "something just changed" event for us.
    let fresh = crate::platform_detect::local_platform_candidates();
    {
        // Scoped tightly so the `MutexGuard` (not `Send`) doesn't get held
        // across the next `.await` — required by `tauri::async_runtime::spawn`.
        let mut guard = state.local_platform_candidates.lock();
        if *guard != fresh {
            info!(
                count = fresh.len(),
                "platform candidates refreshed on WS reconnect"
            );
            *guard = fresh;
        }
    }

    // RL was closed long enough → start a fresh session. `take()` makes sure
    // we only fire this once per disconnect event.
    if let Some(t) = last_disconnect.take() {
        let elapsed = t.elapsed();
        if elapsed >= RL_RELAUNCH_RESET_THRESHOLD {
            info!(
                "RL was closed for {}m{}s — auto-reset session",
                elapsed.as_secs() / 60,
                elapsed.as_secs() % 60
            );
            reset_session_for_relaunch(app, state);
        }
    }

    mark_connected(app, state);

    // RL streams JSON envelopes back-to-back with no length prefix or
    // delimiter. We drain every complete object out of a growing byte buffer
    // and feed `serde_json` slices directly — never `String::from_utf8_lossy`,
    // which would corrupt multi-byte UTF-8 sequences split across reads
    // (a single bad byte poisons the whole envelope and we'd lose every
    // event already buffered behind it).
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];

    // Poll RocketLeague.exe presence on a 1 s tick and race it against the
    // socket read. Why: when the user clicks "Quit" in RL, the process
    // terminates without sending FIN on the Stats API socket. Windows
    // hard-codes the keep-alive probe count to 10 since Vista (the
    // `with_retries` knob in `socket2::TcpKeepalive` is Linux-only), so
    // detecting the dead socket via TCP keep-alive alone takes ~8 s. The
    // process-presence poll catches the same event in <1 s. We keep the
    // keep-alive code above as belt-and-suspenders for the case where RL
    // hangs but the process stays resident.
    //
    // First tick fires immediately; we skip it to avoid a redundant probe
    // right after we've just successfully connected (RL is obviously alive).
    let mut process_poll = tokio::time::interval(Duration::from_secs(1));
    process_poll.tick().await;

    loop {
        tokio::select! {
            // Bias toward the read so a busy tick (120 Hz UpdateState) doesn't
            // get starved by the once-a-second probe. `select!` would otherwise
            // pseudo-randomly pick between ready branches.
            biased;
            read = stream.read(&mut chunk) => {
                let n = read?;
                if n == 0 {
                    break; // RL closed the socket
                }
                buf.extend_from_slice(&chunk[..n]);
                drain_complete_envelopes(app, state, &mut buf);
            }
            _ = process_poll.tick() => {
                if !crate::rl_process::rl_process_alive() {
                    info!("RocketLeague.exe is not running — closing Stats API socket");
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Pull every complete JSON envelope out of `buf` and dispatch each. Anything
/// after the last complete envelope (a partial object straddling the next
/// read) stays in `buf` for the next call.
fn drain_complete_envelopes(app: &AppHandle, state: &Arc<AppState>, buf: &mut Vec<u8>) {
    loop {
        // Skip leading whitespace cheaply on the byte slice.
        let start = buf
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(buf.len());
        if start >= buf.len() {
            buf.clear();
            return;
        }

        let mut de = serde_json::Deserializer::from_slice(&buf[start..])
            .into_iter::<serde_json::Value>();
        match de.next() {
            Some(Ok(value)) => {
                let consumed = start + de.byte_offset();
                handle_message(app, state, value);
                buf.drain(..consumed);
            }
            Some(Err(e)) if e.is_eof() => {
                if start > 0 {
                    buf.drain(..start);
                }
                return;
            }
            Some(Err(err)) => {
                // Genuine parse error on a complete-looking payload. Skip up
                // to the next plausible start byte so one bad envelope doesn't
                // throw away every queued event behind it.
                warn!(?err, "stats API: skipping malformed JSON envelope");
                let after = start + 1;
                match buf[after..]
                    .iter()
                    .position(|&b| b == b'{' || b == b'[')
                {
                    Some(pos) => buf.drain(..after + pos),
                    None => {
                        buf.clear();
                        return;
                    }
                };
            }
            None => return,
        }
    }
}

fn handle_message(app: &AppHandle, state: &Arc<AppState>, value: serde_json::Value) {
    // `from_value` consumes the parsed tree without re-serialising it — the
    // previous version round-tripped Value → String → WsEvent which allocated
    // a few KB per message at 30 Hz.
    let payload: WsEvent = match serde_json::from_value(value) {
        Ok(p) => p,
        Err(err) => {
            warn!(?err, "skipping malformed Stats API message");
            return;
        }
    };

    // RL ships `Data` as a JSON-encoded string for at least UpdateState /
    // MatchEnded; older docs / mocks send it as a raw object. Normalise.
    let data = match payload.data {
        serde_json::Value::String(s) => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(err) => {
                warn!(?err, event = %payload.event, "Data field not parseable as JSON");
                return;
            }
        },
        other => other,
    };

    match payload.event.as_str() {
        // Sentinel sent by `dev/mock-server.ts` right after a TCP client
        // connects. Flips this connection into mock mode so MatchEnded
        // events don't increment the user's real session counter and
        // PrimaryId auto-switching is suspended.
        "MockHandshake" => {
            if !state.mock_source.swap(true, Ordering::SeqCst) {
                info!("mock source identified → session counter + PrimaryId auto-switch suspended");
            }
        }
        "UpdateState" => on_update_state(app, state, &data),
        "MatchEnded" => {
            // The actual whistle. Clear the in-progress flag here (not on
            // MatchDestroyed) so the launcher / settings unhide as soon as
            // the scoreboard appears. MatchDestroyed only fires when the
            // user leaves the lobby — it never fires in private matches
            // where you stay on the same custom lobby. Tracking gameplay,
            // not lobby lifetime, is what users actually want.
            if state.match_in_progress.swap(false, Ordering::SeqCst) {
                let _ = app.emit("rlstats://match-in-progress", false);
            }
            on_match_ended(app, state, &data);
        }
        "MatchInitialized" | "MatchCreated" => {
            reset_match_stats(app, state);
            // Reset local team identification so a cancelled lobby (opponents
            // never join, no UpdateState ever fires) lands in `on_match_ended`
            // with `local_team == None` and silently returns instead of
            // incrementing the W/L tally with a phantom loss based on the
            // previous match's stale team. Real matches re-set this on the
            // first UpdateState via `find_local_player`.
            *state.local_team.lock() = None;
            begin_recorder(state, &data);
            // Mark the match window — drives the launcher's auto-hide and
            // the Settings auto-hide-on-match-start. Only emit on transition
            // so a sequence of MatchInitialized/MatchCreated doesn't flap UI.
            if !state.match_in_progress.swap(true, Ordering::SeqCst) {
                let _ = app.emit("rlstats://match-in-progress", true);
            }
            let _ = app.emit("rlstats://match-started", ());
        }
        "MatchDestroyed" => {
            // Backup MatchEnded path: if RL never fired MatchEnded for this
            // match (forfeit-leave, post-game leave before the whistle, RL
            // bug), the recorder is still `Some` here. Run the same finalize
            // flow as MatchEnded with `api_winner=None` so the score-fallback
            // can still resolve a result. Idempotent vs. a real MatchEnded
            // that already fired: `last_counted_match` will short-circuit on
            // the second call.
            let has_pending = state.recorder.lock().is_some();
            if has_pending {
                let guid = data
                    .get("MatchGuid")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                finalize_match(app, state, &guid, None, MatchEndSource::MatchDestroyedBackup);
            }
            // Defensive cleanup — finalize_match already drops the recorder
            // on every path, but a subsequent MatchInitialized would also
            // overwrite it, so this just makes the intermediate state explicit.
            *state.recorder.lock() = None;
            reset_match_stats(app, state);
            // Same rationale as MatchInitialized — clear the stale team so
            // the next cancelled-lobby MatchEnded can't be mis-counted.
            *state.local_team.lock() = None;
            if state.match_in_progress.swap(false, Ordering::SeqCst) {
                let _ = app.emit("rlstats://match-in-progress", false);
            }
        }
        "GoalScored" => {
            // Persist into the in-progress match before forwarding the event.
            if let Some(rec) = state.recorder.lock().as_mut() {
                let seconds_left = rec.last_seconds_remaining;
                rec.note_goal(&data, seconds_left);
            }
            let _ = app.emit("rlstats://goal-scored", data);
        }
        "BallHit" => {
            if let Some(rec) = state.recorder.lock().as_mut() {
                if let Some(team) = data
                    .get("Players")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|p| p.get("TeamNum"))
                    .and_then(|v| v.as_i64())
                {
                    if (0..=1).contains(&team) {
                        rec.note_ball_hit(team as u8);
                    }
                }
            }
        }
        "CrossbarHit" => {
            if let Some(rec) = state.recorder.lock().as_mut() {
                rec.note_crossbar();
            }
        }
        "StatfeedEvent" => {
            if let Some(rec) = state.recorder.lock().as_mut() {
                rec.note_statfeed(&data);
            }
        }
        _ => {}
    }
}

/// Initialize the in-progress match accumulator. The local team isn't always
/// known from the very first MatchInitialized payload (no UpdateState yet),
/// so it's filled in lazily as ticks arrive — see `ingest_update_state`.
///
/// Honors the `analytics_enabled` opt-in: when the user hasn't turned the
/// post-match feature on we skip allocating the recorder entirely, so no
/// rows ever land in matches.db. `set_analytics_enabled` is responsible for
/// clearing any in-flight recorder if they toggle off mid-match.
fn begin_recorder(state: &Arc<AppState>, data: &serde_json::Value) {
    let storage_present = state.storage.get().is_some();
    if !storage_present {
        return;
    }
    if !state.settings.lock().analytics_enabled {
        return;
    }
    let guid = data
        .get("MatchGuid")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let local_pid = state.settings.lock().primary_id.clone();
    *state.recorder.lock() = Some(crate::match_recorder::InProgressMatch::begin(
        guid,
        local_pid,
    ));
}

/// Wipe per-match stats between matches so a stale Goals/Saves count doesn't
/// bleed from one map into the next.
fn reset_match_stats(app: &AppHandle, state: &Arc<AppState>) {
    let mut guard = state.match_stats.lock();
    if *guard != MatchStats::default() {
        *guard = MatchStats::default();
        drop(guard);
        let snapshot = state.match_stats.lock().clone();
        emit_match_stats(app, snapshot, true);
    }
}

fn on_update_state(app: &AppHandle, state: &Arc<AppState>, data: &serde_json::Value) {
    let players = match data.get("Players").and_then(|v| v.as_array()) {
        Some(p) => p,
        None => return,
    };

    // Lazy-start the recorder: in some flows (mock or RL relaunching mid-match)
    // we get UpdateState before MatchInitialized. Spin it up on first sight so
    // we don't lose the early ticks of a match. Same opt-in gate as
    // `begin_recorder` — skip allocation when analytics is disabled.
    {
        let mut rec_guard = state.recorder.lock();
        if rec_guard.is_none()
            && state.storage.get().is_some()
            && state.settings.lock().analytics_enabled
        {
            let guid = data
                .get("MatchGuid")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if !guid.is_empty() {
                let local_pid = state.settings.lock().primary_id.clone();
                *rec_guard = Some(crate::match_recorder::InProgressMatch::begin(
                    guid, local_pid,
                ));
            }
        }
    }

    // Cache the current match team size (max players-per-team) so MatchEnded
    // can decide whether to count this match given the user's filter. We
    // refresh this on every UpdateState so it survives an app boot mid-match.
    let team_size = compute_team_size(players);
    if team_size > 0 {
        state
            .current_team_size
            .store(team_size, Ordering::Relaxed);
    }

    // Cold-boot / WS-reconnect mid-match: we may receive UpdateStates
    // without having seen a MatchInitialized for this match. Bump
    // `match_in_progress` here so the launcher auto-hide and the
    // post-match HUD hide react even when we miss the lifecycle event.
    // CAS on swap ensures the transition emit only fires once per
    // "off→on" edge, no matter how many UpdateStates land.
    if !state.match_in_progress.swap(true, Ordering::SeqCst) {
        let _ = app.emit("rlstats://match-in-progress", true);
    }

    // Feed the recorder with the latest tick. Local team is known from
    // `state.local_team` once we've identified ourselves below; pass it
    // through so the aggregators can flag is_local_team correctly.
    {
        let local_team_u8 = state
            .local_team
            .lock()
            .as_ref()
            .and_then(|t| u8::try_from(*t).ok());
        if let Some(rec) = state.recorder.lock().as_mut() {
            rec.ingest_update_state(players, data.get("Game"), local_team_u8);
        }
    }

    // Decode the full match stats (per-player + per-team) and store them.
    // Cheap enough to do on every tick — players[] is at most 8 entries.
    let parsed = parse_match_stats(players, data.get("Game"));
    {
        let mut guard = state.match_stats.lock();
        if *guard != parsed {
            *guard = parsed.clone();
            drop(guard);
            emit_match_stats(app, parsed, false);
        }
    }

    // Single read-lock to extract everything we need before searching.
    let (player_name, stored_primary_id) = {
        let s = state.settings.lock();
        (s.player_name.clone(), s.primary_id.clone())
    };
    // Snapshot the candidates list so we drop the lock before the (possibly
    // longer) search through `Players[]`. The list is small (1-5 entries),
    // clone is free.
    let candidates = state.local_platform_candidates.lock().clone();

    let me = match find_local_player(players, &player_name, &stored_primary_id, &candidates) {
        Some(p) => p,
        None => return,
    };

    if let Some(team) = me
        .get("TeamNum")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
    {
        *state.local_team.lock() = Some(team);
    }

    // Auto-learn / refresh the PrimaryId. We capture on first match (so future
    // renames don't lose us), and we refresh whenever the player we identified
    // has a PrimaryId different from the one we stored — that detects an
    // account switch on the same pseudonym.
    //
    // We also opportunistically backfill `player_name` from the matched
    // player. The auto-detect wizard skips the name input, so without this
    // backfill the dashboard's "Player" panel stays empty and the user has
    // no visual confirmation that detection worked. Only fill in when the
    // user hasn't typed anything themselves — never overwrite a manual entry.
    let new_pid = me
        .get("PrimaryId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    let new_name = me
        .get("Name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    // While a mock source is connected, never let its events rewrite the
    // active profile. The mock auto-syncs PrimaryId from /api/state and
    // would otherwise create an identity feedback loop that pollutes the
    // user's real settings.
    if state.mock_source.load(Ordering::Relaxed) {
        return;
    }
    let pid_changed = !new_pid.is_empty() && new_pid != stored_primary_id;
    // Two modes governing how `player_name` is treated:
    //   - Auto mode (candidates non-empty): the user can't edit the pseudo
    //     in the UI. The name always tracks the current account, including
    //     across account switches.
    //   - Manual mode (no candidates): the user typed it. Never overwrite.
    let auto_mode = !candidates.is_empty();
    let name_should_update = !new_name.is_empty()
        && new_name != player_name
        && (player_name.is_empty() || (auto_mode && pid_changed));
    if pid_changed || name_should_update {
        {
            let mut s = state.settings.lock();
            if pid_changed {
                s.primary_id = new_pid.to_string();
            }
            if name_should_update {
                s.player_name = new_name.to_string();
            }
        }
        // Persist asynchronously — see `settings_writer.rs` for why this must
        // never run on the TCP read task synchronously.
        state.request_save_settings();
        if pid_changed {
            if stored_primary_id.is_empty() {
                info!(
                    primary_id = %new_pid,
                    name = %new_name,
                    "captured stable PrimaryId"
                );
            } else {
                info!(
                    old = %stored_primary_id,
                    new = %new_pid,
                    name = %new_name,
                    "refreshed PrimaryId (account switch)"
                );
            }
        } else if name_should_update {
            info!(name = %new_name, "backfilled player_name from match");
        }
    }
}

/// Score-based winner inference, used when `MatchEnded` arrives without a
/// `WinnerTeamNum` (forfeits, abandons, host-DC handicap losses) or when
/// the WS connection died mid-match. Returns `None` for ties — including
/// 0-0 (no UpdateState ever propagated, can't tell a real tie from a stub
/// lobby) — so the caller still has a clear signal to skip the tally.
fn derive_winner_from_scores(team_scores: [u32; 2]) -> Option<i32> {
    if team_scores[0] == team_scores[1] {
        return None;
    }
    Some(if team_scores[0] > team_scores[1] { 0 } else { 1 })
}

fn on_match_ended(app: &AppHandle, state: &Arc<AppState>, data: &serde_json::Value) {
    let guid = data
        .get("MatchGuid")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let api_winner: Option<i32> = data
        .get("WinnerTeamNum")
        .and_then(|v| v.as_i64())
        .map(|w| w as i32);
    finalize_match(app, state, &guid, api_winner, MatchEndSource::MatchEnded);
}

/// Where the finalize call originated. Affects logging only — the tally /
/// persistence logic is identical so a backup call from `MatchDestroyed`
/// produces the same on-disk record as the primary `MatchEnded` would.
#[derive(Debug, Clone, Copy)]
enum MatchEndSource {
    /// Primary path: RL fired `MatchEnded` cleanly.
    MatchEnded,
    /// Backup path: `MatchDestroyed` arrived while a recorder was still in
    /// flight (no MatchEnded ever fired, e.g. forfeit-leave). Last-ditch
    /// chance to count the result — relies on the score fallback.
    MatchDestroyedBackup,
    /// Backup path: WS connection died mid-match. Persists a forensic record
    /// for boot-time reconciliation; the session counter is intentionally
    /// not touched to avoid double-counting if RL reconnects with a delayed
    /// MatchEnded for the same GUID.
    AbnormalDisconnect,
}

/// The shared end-of-match flow used by both MatchEnded and the backup
/// MatchDestroyed/abnormal-disconnect paths. Idempotent on the same
/// `match_guid` thanks to `state.last_counted_match`.
fn finalize_match(
    app: &AppHandle,
    state: &Arc<AppState>,
    guid: &str,
    api_winner: Option<i32>,
    source: MatchEndSource,
) {
    // Cancelled-lobby guard: `local_team` is reset to None on every
    // MatchInitialized / MatchCreated / MatchDestroyed and only set again
    // when an UpdateState identifies the local player. If it's still None
    // here, either RL fired MatchEnded for a stub lobby (opponents never
    // joined → no UpdateState) or the app booted after the match had
    // already ended. Either way we don't have enough info to count the
    // result correctly, so skip silently — but log so we can tell from
    // the field whether a missing tally was a real bug.
    let team = match *state.local_team.lock() {
        Some(t) => t,
        None => {
            info!(
                event = "match_skip",
                reason = "local_team_unknown",
                source = ?source,
                match_guid = guid,
                "skipping W/L tally — likely cancelled lobby or cold-boot mid-end"
            );
            *state.recorder.lock() = None;
            return;
        }
    };

    // Score-fallback: RL sometimes ships MatchEnded without a WinnerTeamNum
    // (forfeits, host disconnects, mate-left handicap auto-loss). Derive the
    // winner from the latest UpdateState's team_scores when the API stays
    // silent and the scores aren't a tie. For abnormal disconnects we apply
    // the same logic so the persisted record carries an actionable result.
    let team_scores = state.match_stats.lock().team_scores;
    let derived_winner: Option<i32> = if api_winner.is_some() {
        None
    } else {
        derive_winner_from_scores(team_scores)
    };
    let winner: Option<i32> = api_winner.or(derived_winner);
    let result_source: &str = match (api_winner.is_some(), derived_winner.is_some(), source) {
        (true, _, _) => "api",
        (false, true, _) => "score-fallback",
        (false, false, MatchEndSource::AbnormalDisconnect) => "abnormal-disconnect",
        (false, false, _) => "unknown",
    };
    if api_winner.is_none() {
        if let Some(w) = derived_winner {
            info!(
                event = "winner_derived",
                source = ?source,
                match_guid = guid,
                derived = w,
                blue = team_scores[0],
                orange = team_scores[1],
                "WinnerTeamNum absent — derived winner from last UpdateState team scores"
            );
        } else {
            warn!(
                event = "winner_unknown",
                source = ?source,
                match_guid = guid,
                blue = team_scores[0],
                orange = team_scores[1],
                "MatchEnded without WinnerTeamNum and team scores tied — session counter not updated"
            );
        }
    }

    // Match-size filter — let the user count only e.g. 3v3s. team_size == 0
    // means we never saw enough UpdateStates to know the match shape; in
    // that case we count the match (don't punish the user for racing the
    // app boot vs the match start).
    let team_size = state.current_team_size.load(Ordering::Relaxed);
    if team_size != 0 {
        let allowed = state.settings.lock().count_team_sizes.clone();
        if !allowed.contains(&team_size) {
            info!(
                event = "match_skip",
                reason = "team_size_filter",
                source = ?source,
                match_guid = guid,
                team_size,
                "match excluded by team-size filter"
            );
            return;
        }
    }

    // Avoid double-counting: any prior path (MatchEnded, MatchDestroyed
    // backup, abnormal-disconnect persistence) that already counted this
    // GUID claims it via `last_counted_match`. The same GUID landing again
    // through a different route is a no-op.
    {
        let mut last = state.last_counted_match.lock();
        if !guid.is_empty() && last.as_deref() == Some(guid) {
            info!(
                event = "match_skip",
                reason = "dedup_replay",
                source = ?source,
                match_guid = guid,
                "match already counted via prior path"
            );
            // Recorder may still be Some on the dedup path (e.g. backup
            // path racing a primary). Drop it so the next match starts clean.
            *state.recorder.lock() = None;
            return;
        }
        if !guid.is_empty() {
            *last = Some(guid.to_string());
        }
    }

    // Tally the W/L counter. Skip cases:
    //   * mock source — dev mock-server's matches must not pollute the real
    //     session, but we still persist to matches.db below so the post-match
    //     HUD remains exercisable from the mock.
    //   * `winner` is None — both API and score-fallback came up empty
    //     (cancelled lobby that somehow had local_team set, or 0-0 tie).
    //   * `AbnormalDisconnect` source — record the forensic snapshot but
    //     don't tally yet; if RL reconnects with a delayed MatchEnded for
    //     this GUID, the dedup guard handles it. If it never does, the
    //     boot-time reconciliation against matches.db will pick the result up.
    let mock_mode = state.mock_source.load(Ordering::Relaxed);
    let counts_session = !mock_mode
        && winner.is_some()
        && !matches!(source, MatchEndSource::AbnormalDisconnect);
    if mock_mode {
        info!(
            event = "match_skip",
            reason = "mock_source",
            source = ?source,
            match_guid = guid,
            "mock source connected → not touching real session counter"
        );
    } else if winner.is_none() {
        // Already logged above with reason=winner_unknown. Nothing to add here.
    } else if matches!(source, MatchEndSource::AbnormalDisconnect) {
        info!(
            event = "match_skip",
            reason = "abnormal_disconnect_no_tally",
            source = ?source,
            match_guid = guid,
            "WS died mid-match — persisting forensic record only, boot reconciliation will tally"
        );
    }
    if counts_session {
        if let Some(winner) = winner {
            {
                let mut session = state.session.lock();
                if winner == team {
                    session.record_win();
                } else {
                    session.record_loss();
                }
                let snapshot = session.clone();
                let mut settings = state.settings.lock();
                settings.session = snapshot;
            }
            state.request_save_settings();
            let _ = app.emit("rlstats://session-changed", ());
            info!(
                event = "match_counted",
                source = ?source,
                match_guid = guid,
                team,
                winner,
                blue = team_scores[0],
                orange = team_scores[1],
                result_source,
                "session counter updated"
            );
        }
    }

    // Finalize and persist the recorder, if any. Done after the W/L tally so
    // a slow DB write never blocks the existing flow.
    let recorder = state.recorder.lock().take();
    if let (Some(mut rec), Some(storage)) = (recorder, state.storage.get().cloned()) {
        // Backfill the local team in case UpdateState fired with the team
        // info but the recorder was started before that (cold-start race).
        rec.local_team_num = team as u8;
        // Backfill primary_id: the recorder is created on MatchCreated, but
        // PrimaryId is only captured from the first UpdateState (auto-learn).
        // Without this, the very first match of a fresh install would be
        // recorded with an empty primary_id and rejected by the FK constraint.
        if rec.local_primary_id.is_empty() {
            rec.local_primary_id = state.settings.lock().primary_id.clone();
        }
        if rec.local_primary_id.is_empty() {
            warn!(
                event = "match_skip",
                reason = "primary_id_unknown",
                source = ?source,
                match_guid = guid,
                "primary_id still unknown — skipping persistence"
            );
            return;
        }
        let record = rec.finalize(winner.map(|w| w as u8));
        let guid_for_event = record.match_guid.clone();
        let app_clone = app.clone();
        // Run the write off the WS read task to avoid stalling on disk I/O.
        tauri::async_runtime::spawn(async move {
            let result = tauri::async_runtime::spawn_blocking(move || {
                storage.record_match(&record)
            })
            .await;
            match result {
                Ok(Ok(())) => {
                    let _ = app_clone.emit("rlstats://match-recorded", &guid_for_event);
                }
                Ok(Err(err)) => warn!(?err, "failed to persist match record"),
                Err(err) => warn!(?err, "spawn_blocking failed for record_match"),
            }
        });
    }

    // The match is over — wipe per-match stats so the next match starts clean.
    reset_match_stats(app, state);
}

fn mark_connected(app: &AppHandle, state: &Arc<AppState>) {
    if !state.connected.swap(true, Ordering::SeqCst) {
        let _ = app.emit("rlstats://connected", true);
    }
}

/// Reset session counters and persist. Same effect as the user clicking
/// "Reset", but triggered automatically when we detect RL was closed and
/// reopened. Also clears the local team and last-counted match guard so
/// the next match starts from a clean slate.
fn reset_session_for_relaunch(app: &AppHandle, state: &Arc<AppState>) {
    {
        let mut session = state.session.lock();
        session.reset();
        let snapshot = session.clone();
        let mut settings = state.settings.lock();
        settings.session = snapshot;
    }
    *state.local_team.lock() = None;
    *state.last_counted_match.lock() = None;
    state.request_save_settings();
    let _ = app.emit("rlstats://session-changed", ());
}

/// Decode the per-player + per-team subset of an `UpdateState` payload into
/// a [`MatchStats`]. Missing fields default to zero — the official API drops
/// optional fields silently and a partial tick must not zero out the rest of
/// the document.
pub(crate) fn parse_match_stats(
    players: &[serde_json::Value],
    game: Option<&serde_json::Value>,
) -> MatchStats {
    let players = players
        .iter()
        .map(|p| PlayerStats {
            primary_id: p
                .get("PrimaryId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            name: p
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            team_num: p
                .get("TeamNum")
                .and_then(|v| v.as_i64())
                .and_then(|n| u8::try_from(n).ok())
                .unwrap_or(0),
            goals: u32_or_zero(p.get("Goals")),
            saves: u32_or_zero(p.get("Saves")),
            shots: u32_or_zero(p.get("Shots")),
            assists: u32_or_zero(p.get("Assists")),
            score: u32_or_zero(p.get("Score")),
        })
        .collect();

    let mut team_scores = [0u32; 2];
    let mut time_seconds: u32 = 0;
    let mut overtime = false;
    if let Some(game) = game {
        if let Some(teams) = game.get("Teams").and_then(|v| v.as_array()) {
            for t in teams {
                let idx = t
                    .get("TeamNum")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1);
                if (0..=1).contains(&idx) {
                    team_scores[idx as usize] = u32_or_zero(t.get("Score"));
                }
            }
        }
        time_seconds = u32_or_zero(game.get("TimeSeconds"));
        overtime = game
            .get("bOvertime")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    }

    MatchStats { players, team_scores, time_seconds, overtime }
}

fn u32_or_zero(v: Option<&serde_json::Value>) -> u32 {
    v.and_then(|x| x.as_i64())
        .map(|n| if n < 0 { 0 } else { n as u32 })
        .unwrap_or(0)
}

/// Push the latest [`MatchStats`] to the frontend, debounced to at most one
/// emit per [`MATCH_STATS_EMIT_DEBOUNCE`] window. Identical successive
/// payloads are suppressed entirely. Pass `force = true` to bypass both
/// guards (used on between-match resets so the UI clears immediately).
fn emit_match_stats(app: &AppHandle, stats: MatchStats, force: bool) {
    let mut gate = MATCH_STATS_GATE.lock();
    let now = Instant::now();
    if !force {
        if let Some(g) = gate.as_ref() {
            if g.last_value == stats && now.duration_since(g.last_at) < MATCH_STATS_EMIT_DEBOUNCE {
                return;
            }
            if now.duration_since(g.last_at) < MATCH_STATS_EMIT_DEBOUNCE {
                return;
            }
        }
    }
    let _ = app.emit("rlstats://match-stats", &stats);
    *gate = Some(MatchStatsEmitGate { last_at: now, last_value: stats });
}

/// Returns the max players-per-team count among the up-to-4 teams seen
/// in the payload, clamped to u8. Used as a proxy for "match shape"
/// (1v1 / 2v2 / 3v3 / 4v4) since the official Stats API does not expose
/// the matchmaking playlist.
fn compute_team_size(players: &[serde_json::Value]) -> u8 {
    let mut counts = [0u8; 4];
    for p in players {
        let Some(t) = p.get("TeamNum").and_then(|v| v.as_i64()) else { continue };
        if (0..counts.len() as i64).contains(&t) {
            counts[t as usize] = counts[t as usize].saturating_add(1);
        }
    }
    counts.into_iter().max().unwrap_or(0)
}

fn find_local_player<'a>(
    players: &'a [serde_json::Value],
    name: &str,
    primary_id: &str,
    candidates: &[String],
) -> Option<&'a serde_json::Value> {
    if !primary_id.is_empty() {
        if let Some(found) = players
            .iter()
            .find(|p| p.get("PrimaryId").and_then(|v| v.as_str()) == Some(primary_id))
        {
            return Some(found);
        }
    }
    // Prefix-match against the boot-time-detected Steam/Epic candidates. The
    // trailing `|` baked into each candidate prevents an ID-A being a prefix
    // of ID-B (`Epic|ab|` mustn't match `Epic|abcdef|0`).
    if !candidates.is_empty() {
        if let Some(found) = players.iter().find(|p| {
            p.get("PrimaryId")
                .and_then(|v| v.as_str())
                .map(|pid| candidates.iter().any(|c| pid.starts_with(c)))
                .unwrap_or(false)
        }) {
            return Some(found);
        }
    }
    if !name.is_empty() {
        let needle = name.to_ascii_lowercase();
        return players.iter().find(|p| {
            p.get("Name")
                .and_then(|v| v.as_str())
                .map(|n| n.eq_ignore_ascii_case(&needle))
                .unwrap_or(false)
        });
    }
    None
}

#[derive(Debug, Deserialize)]
struct WsEvent {
    #[serde(rename = "Event", default)]
    event: String,
    #[serde(rename = "Data", default)]
    data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn players_fixture() -> Vec<serde_json::Value> {
        vec![
            json!({ "Name": "Alice",   "PrimaryId": "Epic|aaa|0", "TeamNum": 0 }),
            json!({ "Name": "MyName",  "PrimaryId": "Epic|me|0",  "TeamNum": 1 }),
            json!({ "Name": "Bob",     "PrimaryId": "Epic|bbb|0", "TeamNum": 1 }),
        ]
    }

    #[test]
    fn finds_by_primary_id_first() {
        let players = players_fixture();
        let me = find_local_player(&players, "wrong-name", "Epic|me|0", &[]).unwrap();
        assert_eq!(me.get("Name").and_then(|v| v.as_str()), Some("MyName"));
    }

    #[test]
    fn falls_back_to_name_when_primary_id_empty() {
        let players = players_fixture();
        let me = find_local_player(&players, "myname", "", &[]).unwrap();
        assert_eq!(me.get("Name").and_then(|v| v.as_str()), Some("MyName"));
    }

    #[test]
    fn returns_none_when_neither_matches() {
        let players = players_fixture();
        assert!(find_local_player(&players, "ghost", "", &[]).is_none());
        assert!(find_local_player(&players, "", "", &[]).is_none());
    }

    /// With no stored primary_id and no name, the boot-time-detected
    /// platform candidates must arbitrate which player is us.
    #[test]
    fn finds_by_platform_prefix_when_id_unknown() {
        let players = players_fixture();
        let candidates = vec!["Epic|me|".to_string()];
        let me = find_local_player(&players, "", "", &candidates).unwrap();
        assert_eq!(me.get("Name").and_then(|v| v.as_str()), Some("MyName"));
    }

    /// The trailing `|` discipline prevents ID-A being a prefix of ID-B.
    /// `Epic|ab|` MUST NOT match a player with `PrimaryId="Epic|abcdef|0"`.
    #[test]
    fn prefix_match_does_not_overreach() {
        let players = vec![
            json!({ "Name": "Other", "PrimaryId": "Epic|abcdef|0", "TeamNum": 0 }),
        ];
        let candidates = vec!["Epic|ab|".to_string()];
        assert!(find_local_player(&players, "", "", &candidates).is_none());
    }

    /// One UTF-8 multi-byte character split across reads must not poison the
    /// buffer — the previous `String::from_utf8_lossy` impl produced U+FFFD
    /// and the JSON behind it was lost.
    #[test]
    fn drain_handles_split_utf8_across_chunks() {
        // Use a placeholder AppHandle? We can't easily build one — instead,
        // exercise the parser-only path through a small internal helper.
        // The interesting property: feeding the buffer in pieces should
        // recover one envelope per `{...}` once the bytes are complete.
        let envelope = r#"{"Event":"X","Data":"héllo"}"#.as_bytes();
        // Split mid multi-byte char (the é = 0xC3 0xA9 — split between them).
        let split_at = envelope
            .iter()
            .position(|&b| b == 0xC3)
            .expect("test envelope must contain a multi-byte char");
        let (a, b) = envelope.split_at(split_at + 1);

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(a);
        // Parsing right now must yield Eof, not a malformed-error. We probe
        // the deserializer directly.
        let mut de = serde_json::Deserializer::from_slice(&buf[..])
            .into_iter::<serde_json::Value>();
        match de.next() {
            Some(Err(e)) if e.is_eof() => {}
            other => panic!("expected EOF on partial UTF-8, got {other:?}"),
        }
        buf.extend_from_slice(b);
        let mut de = serde_json::Deserializer::from_slice(&buf[..])
            .into_iter::<serde_json::Value>();
        let v = de.next().expect("envelope present").expect("parses");
        assert_eq!(v.get("Event").and_then(|x| x.as_str()), Some("X"));
    }

    #[test]
    fn jitter_stays_within_twenty_percent() {
        for _ in 0..1000 {
            let j = jittered(1000);
            assert!(j >= 800 && j <= 1200, "jitter out of range: {j}");
        }
    }

    /// Score-fallback decision matrix. Captures the design intent:
    ///   - tied scores (including 0-0) → None (caller skips tally)
    ///   - blue ahead → team 0 wins
    ///   - orange ahead → team 1 wins
    /// 0-0 is intentionally treated like any other tie: a forfeit whose
    /// scores never propagated is indistinguishable from a cancelled lobby
    /// here, and we'd rather miss a tally than fabricate a phantom loss.
    #[test]
    fn score_fallback_returns_none_on_ties() {
        assert_eq!(derive_winner_from_scores([0, 0]), None);
        assert_eq!(derive_winner_from_scores([3, 3]), None);
        assert_eq!(derive_winner_from_scores([7, 7]), None);
    }

    #[test]
    fn score_fallback_picks_higher_team() {
        // Forfeit scenario: blue won 4-1 with the API silent on WinnerTeamNum.
        assert_eq!(derive_winner_from_scores([4, 1]), Some(0));
        // Mate-left handicap loss: orange ran away.
        assert_eq!(derive_winner_from_scores([0, 5]), Some(1));
        // One-goal forfeit: still resolvable, no tie protection trips.
        assert_eq!(derive_winner_from_scores([2, 1]), Some(0));
        assert_eq!(derive_winner_from_scores([1, 2]), Some(1));
    }

    #[test]
    fn parse_match_stats_decodes_full_payload() {
        let players = vec![
            json!({
                "Name": "Alice",
                "PrimaryId": "Steam|11|0",
                "TeamNum": 0,
                "Goals": 2,
                "Saves": 1,
                "Shots": 4,
                "Assists": 1,
                "Score": 350,
            }),
            json!({
                "Name": "Bob",
                "PrimaryId": "Epic|22|0",
                "TeamNum": 1,
                "Goals": 1,
                // Saves missing — must default to 0.
                "Shots": 3,
                "Assists": 0,
                "Score": 200,
            }),
        ];
        let game = json!({
            "Teams": [
                { "TeamNum": 0, "Score": 2 },
                { "TeamNum": 1, "Score": 1 },
            ],
            "TimeSeconds": 137,
            "bOvertime": false,
        });
        let stats = parse_match_stats(&players, Some(&game));
        assert_eq!(stats.players.len(), 2);
        assert_eq!(stats.players[0].name, "Alice");
        assert_eq!(stats.players[0].team_num, 0);
        assert_eq!(stats.players[0].goals, 2);
        assert_eq!(stats.players[0].saves, 1);
        assert_eq!(stats.players[0].shots, 4);
        assert_eq!(stats.players[0].assists, 1);
        assert_eq!(stats.players[0].score, 350);
        // Missing field defaulted to zero.
        assert_eq!(stats.players[1].saves, 0);
        assert_eq!(stats.team_scores, [2, 1]);
        assert_eq!(stats.time_seconds, 137);
        assert!(!stats.overtime);
    }

    #[test]
    fn parse_match_stats_handles_missing_game_block() {
        let players = vec![json!({ "Name": "Solo", "TeamNum": 0, "Goals": 1 })];
        let stats = parse_match_stats(&players, None);
        assert_eq!(stats.players.len(), 1);
        assert_eq!(stats.players[0].goals, 1);
        assert_eq!(stats.team_scores, [0, 0]);
        assert_eq!(stats.time_seconds, 0);
        assert!(!stats.overtime);
    }

    #[test]
    fn team_size_is_max_per_team() {
        // 3v3 with one extra spectator on team 2 (shouldn't really happen
        // but sanity-checks the per-team max logic).
        let players = vec![
            json!({ "TeamNum": 0 }),
            json!({ "TeamNum": 0 }),
            json!({ "TeamNum": 0 }),
            json!({ "TeamNum": 1 }),
            json!({ "TeamNum": 1 }),
            json!({ "TeamNum": 1 }),
        ];
        assert_eq!(compute_team_size(&players), 3);

        let duel = vec![json!({ "TeamNum": 0 }), json!({ "TeamNum": 1 })];
        assert_eq!(compute_team_size(&duel), 1);

        // Empty / no TeamNum → 0 (treated as "unknown" by the filter).
        assert_eq!(compute_team_size(&[]), 0);
        assert_eq!(compute_team_size(&[json!({ "Name": "x" })]), 0);
    }
}
