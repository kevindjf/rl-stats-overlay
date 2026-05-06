//! SQLite-backed match history storage.
//!
//! The DB lives at `<config_dir>/RLStatsOverlay/matches.db`. Schema is bumped
//! through `migrations.rs`. A single connection is shared via a `Mutex` —
//! SQLite serializes our small write rate (one INSERT per match) and our
//! reads (Tauri commands and HTTP endpoints) cleanly enough.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

mod migrations;
mod types;

pub use types::*;

pub struct Storage {
    conn: Mutex<Connection>,
    /// Resolved DB file path — exposed for the "Open data folder" UI button.
    path: PathBuf,
}

impl Storage {
    /// Open the matches DB at `<dir>/matches.db`, creating the file and
    /// applying any pending migrations.
    pub fn open(dir: &Path) -> Result<Arc<Self>> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating storage dir {}", dir.display()))?;
        let path = dir.join("matches.db");
        let mut conn = Connection::open(&path)
            .with_context(|| format!("opening sqlite at {}", path.display()))?;
        // FK enforcement is per-connection in sqlite; turn it on so cascades
        // work as the schema declares.
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .context("enabling foreign keys and WAL")?;
        migrations::migrate(&mut conn)?;
        Ok(Arc::new(Self { conn: Mutex::new(conn), path }))
    }

    pub fn db_path(&self) -> &Path {
        &self.path
    }

    /// Insert or refresh a profile entry, bumping `last_seen_at`. No-op on the
    /// happy path beyond a single UPDATE if the profile already exists.
    pub fn upsert_profile(&self, primary_id: &str, display_name: &str) -> Result<()> {
        if primary_id.is_empty() {
            return Ok(());
        }
        let now = now_ms();
        let conn = self.conn.lock();
        conn.execute(
            r#"INSERT INTO profiles(primary_id, display_name, first_seen_at, last_seen_at)
               VALUES(?, ?, ?, ?)
               ON CONFLICT(primary_id) DO UPDATE SET
                   display_name = excluded.display_name,
                   last_seen_at = excluded.last_seen_at"#,
            params![primary_id, display_name, now, now],
        )
        .context("upserting profile")?;
        Ok(())
    }

    /// Return the active session id for a profile, creating one if none exists.
    pub fn current_session_id(&self, primary_id: &str) -> Result<i64> {
        let conn = self.conn.lock();
        // Find the active (open) session, if any.
        let active = conn
            .query_row(
                "SELECT id, started_at FROM sessions WHERE primary_id = ? AND ended_at IS NULL ORDER BY id DESC LIMIT 1",
                params![primary_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .context("looking up active session")?;

        if let Some((session_id, started_at)) = active {
            // Stale auto-rotation: if the most recent activity in this session
            // (latest match's ended_at, or session.started_at if no matches
            // yet) is older than the in-memory session timeout, close it and
            // open a fresh one. Keeps the analytics "Session" tab in sync
            // with the legacy in-memory counter when the user comes back the
            // next day without clicking reset.
            let last_match_ended: Option<i64> = conn
                .query_row(
                    "SELECT MAX(ended_at) FROM matches WHERE session_id = ?",
                    params![session_id],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .optional()
                .context("looking up last match in active session")?
                .flatten();
            let last_activity = last_match_ended.unwrap_or(started_at).max(started_at);
            let now = now_ms() as i64;
            let timeout_ms = crate::session::Session::TIMEOUT_MS as i64;
            if (now - last_activity).max(0) < timeout_ms {
                return Ok(session_id);
            }
            conn.execute(
                "UPDATE sessions SET ended_at = ? WHERE id = ?",
                params![now, session_id],
            )
            .context("closing stale session")?;
        }

        let now = now_ms();
        conn.execute(
            "INSERT INTO sessions(primary_id, started_at, reset_reason) VALUES(?, ?, 'auto')",
            params![primary_id, now],
        )
        .context("creating initial session")?;
        Ok(conn.last_insert_rowid())
    }

    /// Close the current active session (if any) and start a new one.
    pub fn start_new_session(&self, primary_id: &str, reason: &str) -> Result<i64> {
        let conn = self.conn.lock();
        let now = now_ms();
        conn.execute(
            "UPDATE sessions SET ended_at = ? WHERE primary_id = ? AND ended_at IS NULL",
            params![now, primary_id],
        )
        .context("closing previous session")?;
        conn.execute(
            "INSERT INTO sessions(primary_id, started_at, reset_reason) VALUES(?, ?, ?)",
            params![primary_id, now, reason],
        )
        .context("inserting new session")?;
        Ok(conn.last_insert_rowid())
    }

    /// Persist a finalized match: the matches row, all per-player aggregates,
    /// goals timeline and statfeed log, atomically.
    pub fn record_match(&self, record: &MatchRecord) -> Result<()> {
        // Profile + session id are guaranteed up-front.
        self.upsert_profile(&record.primary_id, default_local_name(&record.players))?;
        let session_id = self.current_session_id(&record.primary_id)?;
        let mut conn = self.conn.lock();
        let tx = conn.transaction().context("starting record_match tx")?;
        tx.execute(
            r#"INSERT OR REPLACE INTO matches(
                match_guid, primary_id, started_at, ended_at, arena, team_size,
                local_team_num, winner_team_num, is_win, blue_score, orange_score,
                overtime, duration_seconds, ball_hits_blue, ball_hits_orange,
                crossbar_hits, session_id
            ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#,
            params![
                record.match_guid,
                record.primary_id,
                record.started_at_ms,
                record.ended_at_ms,
                record.arena,
                record.team_size as i64,
                record.local_team_num as i64,
                record.winner_team_num.map(|v| v as i64),
                record.is_win as i64,
                record.blue_score as i64,
                record.orange_score as i64,
                record.overtime as i64,
                record.duration_seconds as i64,
                record.ball_hits_blue as i64,
                record.ball_hits_orange as i64,
                record.crossbar_hits as i64,
                session_id,
            ],
        )
        .context("inserting matches row")?;

        // Replace any rows from a previous insert of the same guid.
        tx.execute("DELETE FROM match_players WHERE match_guid = ?", params![record.match_guid])?;
        tx.execute("DELETE FROM match_goals   WHERE match_guid = ?", params![record.match_guid])?;
        tx.execute("DELETE FROM match_statfeed WHERE match_guid = ?", params![record.match_guid])?;

        for p in &record.players {
            let s = p.spectator.as_ref();
            tx.execute(
                r#"INSERT INTO match_players(
                    match_guid, player_name, primary_id, team_num,
                    is_local_team, is_local_player,
                    goals, shots, saves, assists, score, demos,
                    boost_avg, boost_time_at_0_s, boost_time_at_100_s,
                    boost_pct_0_25, boost_pct_25_50, boost_pct_50_75, boost_pct_75_100,
                    boost_pct_boosting, bpm, boost_used_supersonic,
                    speed_avg_pct, total_distance,
                    pct_time_slow, pct_time_boost_speed, pct_time_supersonic,
                    pct_time_ground, pct_time_aerial, pct_time_wall,
                    powerslide_total_s, powerslide_count, powerslide_avg_s, demos_taken
                ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#,
                params![
                    record.match_guid,
                    p.player_name,
                    p.primary_id,
                    p.team_num as i64,
                    p.is_local_team as i64,
                    p.is_local_player as i64,
                    p.goals as i64,
                    p.shots as i64,
                    p.saves as i64,
                    p.assists as i64,
                    p.score as i64,
                    p.demos as i64,
                    s.map(|x| x.boost_avg),
                    s.map(|x| x.boost_time_at_0_s),
                    s.map(|x| x.boost_time_at_100_s),
                    s.map(|x| x.boost_pct_0_25),
                    s.map(|x| x.boost_pct_25_50),
                    s.map(|x| x.boost_pct_50_75),
                    s.map(|x| x.boost_pct_75_100),
                    s.map(|x| x.boost_pct_boosting),
                    s.map(|x| x.bpm),
                    s.map(|x| x.boost_used_supersonic),
                    s.map(|x| x.speed_avg_pct),
                    s.map(|x| x.total_distance),
                    s.map(|x| x.pct_time_slow),
                    s.map(|x| x.pct_time_boost_speed),
                    s.map(|x| x.pct_time_supersonic),
                    s.map(|x| x.pct_time_ground),
                    s.map(|x| x.pct_time_aerial),
                    s.map(|x| x.pct_time_wall),
                    s.map(|x| x.powerslide_total_s),
                    s.map(|x| x.powerslide_count as i64),
                    s.map(|x| x.powerslide_avg_s),
                    s.map(|x| x.demos_taken as i64),
                ],
            )
            .context("inserting match_players row")?;
        }
        for g in &record.goals {
            tx.execute(
                r#"INSERT INTO match_goals(
                    match_guid, ord, scored_at_seconds_remaining,
                    scorer_name, scorer_team_num, assister_name, last_touch_name,
                    goal_speed, impact_x, impact_y, impact_z
                ) VALUES(?,?,?,?,?,?,?,?,?,?,?)"#,
                params![
                    record.match_guid,
                    g.ord as i64,
                    g.scored_at_seconds_remaining,
                    g.scorer_name,
                    g.scorer_team_num.map(|v| v as i64),
                    g.assister_name,
                    g.last_touch_name,
                    g.goal_speed,
                    g.impact_x,
                    g.impact_y,
                    g.impact_z,
                ],
            )
            .context("inserting match_goals row")?;
        }
        for sf in &record.statfeed {
            tx.execute(
                r#"INSERT INTO match_statfeed(
                    match_guid, ord, at_seconds_remaining, event_name, type_label,
                    main_player, main_team_num, secondary_player, secondary_team_num
                ) VALUES(?,?,?,?,?,?,?,?,?)"#,
                params![
                    record.match_guid,
                    sf.ord as i64,
                    sf.at_seconds_remaining,
                    sf.event_name,
                    sf.type_label,
                    sf.main_player,
                    sf.main_team_num.map(|v| v as i64),
                    sf.secondary_player,
                    sf.secondary_team_num.map(|v| v as i64),
                ],
            )
            .context("inserting match_statfeed row")?;
        }
        tx.commit().context("committing record_match")?;
        Ok(())
    }

    pub fn recent_matches(
        &self,
        primary_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MatchSummary>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"SELECT match_guid, primary_id, ended_at, started_at, arena, team_size,
                       blue_score, orange_score, local_team_num, is_win, overtime, duration_seconds
               FROM matches
               WHERE primary_id = ?
               ORDER BY ended_at DESC
               LIMIT ? OFFSET ?"#,
        )?;
        let rows = stmt
            .query_map(params![primary_id, limit as i64, offset as i64], row_to_summary)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn match_detail(&self, match_guid: &str) -> Result<Option<MatchDetail>> {
        let conn = self.conn.lock();
        let mut head_stmt = conn.prepare(
            r#"SELECT match_guid, primary_id, started_at, ended_at, arena, team_size,
                       local_team_num, winner_team_num, is_win, blue_score, orange_score,
                       overtime, duration_seconds, ball_hits_blue, ball_hits_orange, crossbar_hits
               FROM matches WHERE match_guid = ?"#,
        )?;
        let head = head_stmt
            .query_row(params![match_guid], |row| {
                Ok(MatchRecord {
                    match_guid: row.get(0)?,
                    primary_id: row.get(1)?,
                    started_at_ms: row.get(2)?,
                    ended_at_ms: row.get(3)?,
                    arena: row.get(4)?,
                    team_size: int_u8(row, 5)?,
                    local_team_num: int_u8(row, 6)?,
                    winner_team_num: row.get::<_, Option<i64>>(7)?.map(|v| v as u8),
                    is_win: row.get::<_, i64>(8)? != 0,
                    blue_score: int_u32(row, 9)?,
                    orange_score: int_u32(row, 10)?,
                    overtime: row.get::<_, i64>(11)? != 0,
                    duration_seconds: int_u32(row, 12)?,
                    ball_hits_blue: row.get::<_, Option<i64>>(13)?.unwrap_or(0) as u32,
                    ball_hits_orange: row.get::<_, Option<i64>>(14)?.unwrap_or(0) as u32,
                    crossbar_hits: row.get::<_, Option<i64>>(15)?.unwrap_or(0) as u32,
                    players: vec![],
                    goals: vec![],
                    statfeed: vec![],
                })
            })
            .optional()
            .context("loading match head")?;
        let mut head = match head {
            Some(h) => h,
            None => return Ok(None),
        };

        let mut p_stmt = conn.prepare(
            r#"SELECT player_name, primary_id, team_num, is_local_team, is_local_player,
                       goals, shots, saves, assists, score, demos,
                       boost_avg, boost_time_at_0_s, boost_time_at_100_s,
                       boost_pct_0_25, boost_pct_25_50, boost_pct_50_75, boost_pct_75_100,
                       boost_pct_boosting, bpm, boost_used_supersonic,
                       speed_avg_pct, total_distance,
                       pct_time_slow, pct_time_boost_speed, pct_time_supersonic,
                       pct_time_ground, pct_time_aerial, pct_time_wall,
                       powerslide_total_s, powerslide_count, powerslide_avg_s, demos_taken
               FROM match_players WHERE match_guid = ?
               ORDER BY is_local_team DESC, is_local_player DESC, team_num, player_name"#,
        )?;
        head.players = p_stmt
            .query_map(params![match_guid], row_to_player)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut g_stmt = conn.prepare(
            r#"SELECT ord, scored_at_seconds_remaining, scorer_name, scorer_team_num,
                       assister_name, last_touch_name, goal_speed, impact_x, impact_y, impact_z
               FROM match_goals WHERE match_guid = ? ORDER BY ord"#,
        )?;
        head.goals = g_stmt
            .query_map(params![match_guid], |row| {
                Ok(GoalRecord {
                    ord: int_u32(row, 0)?,
                    scored_at_seconds_remaining: row.get(1)?,
                    scorer_name: row.get(2)?,
                    scorer_team_num: row.get::<_, Option<i64>>(3)?.map(|v| v as u8),
                    assister_name: row.get(4)?,
                    last_touch_name: row.get(5)?,
                    goal_speed: row.get(6)?,
                    impact_x: row.get(7)?,
                    impact_y: row.get(8)?,
                    impact_z: row.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut s_stmt = conn.prepare(
            r#"SELECT ord, at_seconds_remaining, event_name, type_label,
                       main_player, main_team_num, secondary_player, secondary_team_num
               FROM match_statfeed WHERE match_guid = ? ORDER BY ord"#,
        )?;
        head.statfeed = s_stmt
            .query_map(params![match_guid], |row| {
                Ok(StatfeedRecord {
                    ord: int_u32(row, 0)?,
                    at_seconds_remaining: row.get(1)?,
                    event_name: row.get(2)?,
                    type_label: row.get(3)?,
                    main_player: row.get(4)?,
                    main_team_num: row.get::<_, Option<i64>>(5)?.map(|v| v as u8),
                    secondary_player: row.get(6)?,
                    secondary_team_num: row.get::<_, Option<i64>>(7)?.map(|v| v as u8),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(Some(head))
    }

    pub fn list_profiles(&self) -> Result<Vec<ProfileInfo>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"SELECT p.primary_id, p.display_name, p.first_seen_at, p.last_seen_at,
                       (SELECT COUNT(*) FROM matches m WHERE m.primary_id = p.primary_id) AS match_count
               FROM profiles p
               ORDER BY p.last_seen_at DESC"#,
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ProfileInfo {
                    primary_id: row.get(0)?,
                    display_name: row.get(1)?,
                    first_seen_at_ms: row.get(2)?,
                    last_seen_at_ms: row.get(3)?,
                    match_count: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn delete_match(&self, match_guid: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM matches WHERE match_guid = ?", params![match_guid])
            .context("deleting match")?;
        Ok(())
    }

    pub fn aggregate(&self, primary_id: &str, session_id: Option<i64>) -> Result<AggregateStats> {
        let conn = self.conn.lock();
        let scope_clause = if session_id.is_some() {
            "WHERE primary_id = ? AND session_id = ?"
        } else {
            "WHERE primary_id = ?"
        };

        // Core counters from the matches table.
        let sql_core = format!(
            r#"SELECT COUNT(*) AS matches,
                       SUM(CASE WHEN is_win=1 THEN 1 ELSE 0 END) AS wins,
                       SUM(CASE WHEN is_win=0 THEN 1 ELSE 0 END) AS losses,
                       MIN(started_at) AS started_at
                FROM matches {scope_clause}"#
        );
        let (matches, wins, losses, started_at_ms) = if let Some(sid) = session_id {
            conn.query_row(&sql_core, params![primary_id, sid], |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })?
        } else {
            conn.query_row(&sql_core, params![primary_id], |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })?
        };

        // Per-team-size breakdown.
        let sql_ts = format!(
            r#"SELECT team_size,
                       SUM(CASE WHEN is_win=1 THEN 1 ELSE 0 END),
                       SUM(CASE WHEN is_win=0 THEN 1 ELSE 0 END)
                FROM matches {scope_clause}
                GROUP BY team_size
                ORDER BY team_size"#
        );
        let mut ts_stmt = conn.prepare(&sql_ts)?;
        let ts_rows = if let Some(sid) = session_id {
            ts_stmt
                .query_map(params![primary_id, sid], |row| {
                    Ok(PlaylistBreakdown {
                        team_size: int_u8(row, 0)?,
                        wins: row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u32,
                        losses: row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            ts_stmt
                .query_map(params![primary_id], |row| {
                    Ok(PlaylistBreakdown {
                        team_size: int_u8(row, 0)?,
                        wins: row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u32,
                        losses: row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };

        // Local-player core averages — pull the local row from each match.
        let sql_local = format!(
            r#"SELECT
                AVG(mp.goals), AVG(mp.shots), AVG(mp.saves),
                AVG(mp.assists), AVG(mp.score), AVG(mp.demos),
                AVG(mp.boost_avg), AVG(mp.bpm),
                AVG(mp.pct_time_supersonic), AVG(mp.pct_time_aerial),
                AVG(mp.powerslide_count)
                FROM matches m
                JOIN match_players mp ON mp.match_guid = m.match_guid
                {scope_inner}
                AND mp.is_local_player = 1"#,
            scope_inner = if session_id.is_some() {
                "WHERE m.primary_id = ? AND m.session_id = ?"
            } else {
                "WHERE m.primary_id = ?"
            }
        );
        let local_avg = if let Some(sid) = session_id {
            conn.query_row(&sql_local, params![primary_id, sid], |row| {
                Ok((
                    row.get::<_, Option<f32>>(0)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(1)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(2)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(3)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(4)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(5)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(6)?,
                    row.get::<_, Option<f32>>(7)?,
                    row.get::<_, Option<f32>>(8)?,
                    row.get::<_, Option<f32>>(9)?,
                    row.get::<_, Option<f32>>(10)?,
                ))
            })?
        } else {
            conn.query_row(&sql_local, params![primary_id], |row| {
                Ok((
                    row.get::<_, Option<f32>>(0)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(1)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(2)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(3)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(4)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(5)?.unwrap_or(0.0),
                    row.get::<_, Option<f32>>(6)?,
                    row.get::<_, Option<f32>>(7)?,
                    row.get::<_, Option<f32>>(8)?,
                    row.get::<_, Option<f32>>(9)?,
                    row.get::<_, Option<f32>>(10)?,
                ))
            })?
        };

        // Extended SPECTATOR aggregates used by the post-match HUD's
        // "Session" toggle. Same scope clause as the core averages — kept in
        // a separate query so the simpler analytics dashboard doesn't pay the
        // cost when only the basic columns are needed.
        let sql_ext = format!(
            r#"SELECT
                AVG(mp.demos_taken),
                AVG(mp.boost_pct_boosting),
                AVG(mp.boost_time_at_0_s),
                AVG(mp.boost_time_at_100_s),
                AVG(mp.boost_pct_0_25),
                AVG(mp.boost_pct_25_50),
                AVG(mp.boost_pct_50_75),
                AVG(mp.boost_pct_75_100),
                AVG(mp.speed_avg_pct),
                AVG(mp.pct_time_slow),
                AVG(mp.pct_time_boost_speed),
                AVG(mp.pct_time_ground),
                AVG(mp.pct_time_wall),
                SUM(mp.total_distance),
                SUM(mp.powerslide_total_s),
                SUM(m.duration_seconds)
                FROM matches m
                JOIN match_players mp ON mp.match_guid = m.match_guid
                {scope_inner}
                AND mp.is_local_player = 1"#,
            scope_inner = if session_id.is_some() {
                "WHERE m.primary_id = ? AND m.session_id = ?"
            } else {
                "WHERE m.primary_id = ?"
            }
        );
        // 16 columns: 13 AVG (Option<f32> from possibly-NULL columns), 2 SUM
        // (Option<f32> when set has no rows), 1 SUM of i64 (duration).
        type ExtRow = (
            Option<f32>, Option<f32>, Option<f32>, Option<f32>,
            Option<f32>, Option<f32>, Option<f32>, Option<f32>,
            Option<f32>, Option<f32>, Option<f32>, Option<f32>,
            Option<f32>,
            Option<f32>, Option<f32>,
            Option<i64>,
        );
        let map_ext = |row: &Row| -> rusqlite::Result<ExtRow> {
            Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?,
                row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                row.get(12)?,
                row.get(13)?, row.get(14)?,
                row.get(15)?,
            ))
        };
        let ext = if let Some(sid) = session_id {
            conn.query_row(&sql_ext, params![primary_id, sid], map_ext)?
        } else {
            conn.query_row(&sql_ext, params![primary_id], map_ext)?
        };

        // Recent W/L outcomes (oldest first).
        let sql_recent = format!(
            r#"SELECT is_win FROM matches {scope_clause}
                ORDER BY ended_at DESC LIMIT 30"#
        );
        let mut r_stmt = conn.prepare(&sql_recent)?;
        let mut recent = if let Some(sid) = session_id {
            r_stmt
                .query_map(params![primary_id, sid], |row| Ok(row.get::<_, i64>(0)? != 0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            r_stmt
                .query_map(params![primary_id], |row| Ok(row.get::<_, i64>(0)? != 0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        recent.reverse();

        // Streaks computed from outcomes ordered chronologically (oldest first).
        let (current_streak, best_win, best_loss) = compute_streaks(&recent);

        // Best / worst match (by local player score).
        let sql_best = format!(
            r#"SELECT m.match_guid, mp.score
                FROM matches m
                JOIN match_players mp ON mp.match_guid = m.match_guid
                {scope_inner}
                AND mp.is_local_player = 1
                ORDER BY mp.score DESC LIMIT 1"#,
            scope_inner = if session_id.is_some() {
                "WHERE m.primary_id = ? AND m.session_id = ?"
            } else {
                "WHERE m.primary_id = ?"
            }
        );
        let sql_worst = format!(
            r#"SELECT m.match_guid
                FROM matches m
                JOIN match_players mp ON mp.match_guid = m.match_guid
                {scope_inner}
                AND mp.is_local_player = 1
                ORDER BY mp.score ASC LIMIT 1"#,
            scope_inner = if session_id.is_some() {
                "WHERE m.primary_id = ? AND m.session_id = ?"
            } else {
                "WHERE m.primary_id = ?"
            }
        );
        let best = if let Some(sid) = session_id {
            conn.query_row(&sql_best, params![primary_id, sid], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u32))
            })
            .optional()?
        } else {
            conn.query_row(&sql_best, params![primary_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u32))
            })
            .optional()?
        };
        let worst = if let Some(sid) = session_id {
            conn.query_row(&sql_worst, params![primary_id, sid], |row| row.get::<_, String>(0))
                .optional()?
        } else {
            conn.query_row(&sql_worst, params![primary_id], |row| row.get::<_, String>(0))
                .optional()?
        };

        Ok(AggregateStats {
            matches,
            wins,
            losses,
            current_streak,
            best_win_streak: best_win,
            best_loss_streak: best_loss,
            avg_goals: local_avg.0,
            avg_shots: local_avg.1,
            avg_saves: local_avg.2,
            avg_assists: local_avg.3,
            avg_score: local_avg.4,
            avg_demos: local_avg.5,
            avg_boost: local_avg.6,
            avg_bpm: local_avg.7,
            avg_supersonic_pct: local_avg.8,
            avg_aerial_pct: local_avg.9,
            avg_powerslide_count: local_avg.10,
            avg_demos_taken: ext.0,
            avg_boosting_pct: ext.1,
            avg_boost_time_at_0_s: ext.2,
            avg_boost_time_at_100_s: ext.3,
            avg_boost_pct_0_25: ext.4,
            avg_boost_pct_25_50: ext.5,
            avg_boost_pct_50_75: ext.6,
            avg_boost_pct_75_100: ext.7,
            avg_speed_avg_pct: ext.8,
            avg_pct_time_slow: ext.9,
            avg_pct_time_boost_speed: ext.10,
            avg_pct_time_ground: ext.11,
            avg_pct_time_wall: ext.12,
            total_distance: ext.13,
            total_powerslide_s: ext.14,
            total_duration_s: ext.15.unwrap_or(0).max(0) as u32,
            by_team_size: ts_rows,
            recent_outcomes: recent,
            best_match_score: best.as_ref().map(|(_, s)| *s),
            best_match_guid: best.map(|(g, _)| g),
            worst_match_guid: worst,
            started_at_ms,
        })
    }

    /// Convenience for tests / setup probing.
    #[allow(dead_code)]
    pub fn count_matches(&self) -> Result<u32> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row("SELECT COUNT(*) FROM matches", [], |row| row.get::<_, i64>(0))?
            as u32)
    }

    /// Wipe every match, goal, statfeed, profile and session record. Schema
    /// stays — only the data goes. Used by the "Clear all history" button.
    pub fn clear_all(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction().context("starting clear_all tx")?;
        for table in [
            "match_statfeed",
            "match_goals",
            "match_players",
            "matches",
            "sessions",
            "profiles",
        ] {
            tx.execute_batch(&format!("DELETE FROM {table};"))
                .with_context(|| format!("clearing {table}"))?;
        }
        tx.commit().context("committing clear_all")?;
        Ok(())
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn default_local_name(players: &[MatchPlayerRecord]) -> &str {
    players
        .iter()
        .find(|p| p.is_local_player)
        .map(|p| p.player_name.as_str())
        .unwrap_or("")
}

fn int_u8(row: &Row<'_>, idx: usize) -> rusqlite::Result<u8> {
    Ok(row.get::<_, i64>(idx)? as u8)
}
fn int_u32(row: &Row<'_>, idx: usize) -> rusqlite::Result<u32> {
    Ok(row.get::<_, i64>(idx)? as u32)
}

fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<MatchSummary> {
    Ok(MatchSummary {
        match_guid: row.get(0)?,
        primary_id: row.get(1)?,
        ended_at_ms: row.get(2)?,
        started_at_ms: row.get(3)?,
        arena: row.get(4)?,
        team_size: int_u8(row, 5)?,
        blue_score: int_u32(row, 6)?,
        orange_score: int_u32(row, 7)?,
        local_team_num: int_u8(row, 8)?,
        is_win: row.get::<_, i64>(9)? != 0,
        overtime: row.get::<_, i64>(10)? != 0,
        duration_seconds: int_u32(row, 11)?,
    })
}

fn row_to_player(row: &Row<'_>) -> rusqlite::Result<MatchPlayerRecord> {
    let goals: Option<i64> = row.get(5)?;
    let shots: Option<i64> = row.get(6)?;
    let saves: Option<i64> = row.get(7)?;
    let assists: Option<i64> = row.get(8)?;
    let score: Option<i64> = row.get(9)?;
    let demos: Option<i64> = row.get(10)?;
    let boost_avg: Option<f32> = row.get(11)?;
    let spectator = boost_avg.map(|_| SpectatorStats {
        boost_avg: row.get::<_, Option<f32>>(11).unwrap().unwrap_or(0.0),
        boost_time_at_0_s: row.get::<_, Option<f32>>(12).unwrap().unwrap_or(0.0),
        boost_time_at_100_s: row.get::<_, Option<f32>>(13).unwrap().unwrap_or(0.0),
        boost_pct_0_25: row.get::<_, Option<f32>>(14).unwrap().unwrap_or(0.0),
        boost_pct_25_50: row.get::<_, Option<f32>>(15).unwrap().unwrap_or(0.0),
        boost_pct_50_75: row.get::<_, Option<f32>>(16).unwrap().unwrap_or(0.0),
        boost_pct_75_100: row.get::<_, Option<f32>>(17).unwrap().unwrap_or(0.0),
        boost_pct_boosting: row.get::<_, Option<f32>>(18).unwrap().unwrap_or(0.0),
        bpm: row.get::<_, Option<f32>>(19).unwrap().unwrap_or(0.0),
        boost_used_supersonic: row.get::<_, Option<f32>>(20).unwrap().unwrap_or(0.0),
        speed_avg_pct: row.get::<_, Option<f32>>(21).unwrap().unwrap_or(0.0),
        total_distance: row.get::<_, Option<f32>>(22).unwrap().unwrap_or(0.0),
        pct_time_slow: row.get::<_, Option<f32>>(23).unwrap().unwrap_or(0.0),
        pct_time_boost_speed: row.get::<_, Option<f32>>(24).unwrap().unwrap_or(0.0),
        pct_time_supersonic: row.get::<_, Option<f32>>(25).unwrap().unwrap_or(0.0),
        pct_time_ground: row.get::<_, Option<f32>>(26).unwrap().unwrap_or(0.0),
        pct_time_aerial: row.get::<_, Option<f32>>(27).unwrap().unwrap_or(0.0),
        pct_time_wall: row.get::<_, Option<f32>>(28).unwrap().unwrap_or(0.0),
        powerslide_total_s: row.get::<_, Option<f32>>(29).unwrap().unwrap_or(0.0),
        powerslide_count: row.get::<_, Option<i64>>(30).unwrap().unwrap_or(0) as u32,
        powerslide_avg_s: row.get::<_, Option<f32>>(31).unwrap().unwrap_or(0.0),
        demos_taken: row.get::<_, Option<i64>>(32).unwrap().unwrap_or(0) as u32,
    });
    Ok(MatchPlayerRecord {
        player_name: row.get(0)?,
        primary_id: row.get(1)?,
        team_num: int_u8(row, 2)?,
        is_local_team: row.get::<_, i64>(3)? != 0,
        is_local_player: row.get::<_, i64>(4)? != 0,
        goals: goals.unwrap_or(0) as u32,
        shots: shots.unwrap_or(0) as u32,
        saves: saves.unwrap_or(0) as u32,
        assists: assists.unwrap_or(0) as u32,
        score: score.unwrap_or(0) as u32,
        demos: demos.unwrap_or(0) as u32,
        spectator,
    })
}

fn compute_streaks(outcomes: &[bool]) -> (i32, u32, u32) {
    if outcomes.is_empty() {
        return (0, 0, 0);
    }
    let mut best_win = 0u32;
    let mut best_loss = 0u32;
    let mut run_win = 0u32;
    let mut run_loss = 0u32;
    for &won in outcomes {
        if won {
            run_win += 1;
            run_loss = 0;
            if run_win > best_win {
                best_win = run_win;
            }
        } else {
            run_loss += 1;
            run_win = 0;
            if run_loss > best_loss {
                best_loss = run_loss;
            }
        }
    }
    let current = if *outcomes.last().unwrap() {
        run_win as i32
    } else {
        -(run_loss as i32)
    };
    (current, best_win, best_loss)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_temp() -> Arc<Storage> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        std::mem::forget(dir); // keep on disk for the duration of the test
        Storage::open(&path).unwrap()
    }

    fn fixture_record(guid: &str, win: bool) -> MatchRecord {
        MatchRecord {
            match_guid: guid.into(),
            primary_id: "Mock|1|0".into(),
            started_at_ms: 1000,
            ended_at_ms: 1300,
            arena: Some("Stadium_P".into()),
            team_size: 2,
            local_team_num: 0,
            winner_team_num: Some(if win { 0 } else { 1 }),
            is_win: win,
            blue_score: if win { 3 } else { 1 },
            orange_score: if win { 1 } else { 3 },
            overtime: false,
            duration_seconds: 300,
            ball_hits_blue: 50,
            ball_hits_orange: 40,
            crossbar_hits: 1,
            players: vec![
                MatchPlayerRecord {
                    player_name: "MoiPseudo".into(),
                    primary_id: Some("Mock|1|0".into()),
                    team_num: 0,
                    is_local_team: true,
                    is_local_player: true,
                    goals: 2, shots: 4, saves: 1, assists: 1, score: 420, demos: 1,
                    spectator: Some(SpectatorStats {
                        boost_avg: 50.0,
                        boost_time_at_0_s: 12.0,
                        boost_time_at_100_s: 24.0,
                        boost_pct_0_25: 30.0, boost_pct_25_50: 40.0,
                        boost_pct_50_75: 20.0, boost_pct_75_100: 10.0,
                        boost_pct_boosting: 28.0,
                        bpm: 320.0,
                        boost_used_supersonic: 200.0,
                        speed_avg_pct: 55.0,
                        total_distance: 350000.0,
                        pct_time_slow: 38.0,
                        pct_time_boost_speed: 34.0,
                        pct_time_supersonic: 28.0,
                        pct_time_ground: 70.0,
                        pct_time_aerial: 25.0,
                        pct_time_wall: 5.0,
                        powerslide_total_s: 12.0,
                        powerslide_count: 18,
                        powerslide_avg_s: 0.66,
                        demos_taken: 0,
                    }),
                },
                MatchPlayerRecord {
                    player_name: "Opponent".into(),
                    primary_id: None,
                    team_num: 1,
                    is_local_team: false,
                    is_local_player: false,
                    goals: 1, shots: 2, saves: 0, assists: 0, score: 130, demos: 0,
                    spectator: None,
                },
            ],
            goals: vec![GoalRecord {
                ord: 0,
                scored_at_seconds_remaining: Some(270),
                scorer_name: Some("MoiPseudo".into()),
                scorer_team_num: Some(0),
                assister_name: None,
                last_touch_name: Some("MoiPseudo".into()),
                goal_speed: Some(90.0),
                impact_x: Some(0.0), impact_y: Some(5120.0), impact_z: Some(320.0),
            }],
            statfeed: vec![StatfeedRecord {
                ord: 0,
                at_seconds_remaining: Some(265),
                event_name: "Save".into(),
                type_label: Some("Save".into()),
                main_player: Some("MoiPseudo".into()),
                main_team_num: Some(0),
                secondary_player: None,
                secondary_team_num: None,
            }],
        }
    }

    #[test]
    fn open_and_record_round_trip() {
        let s = open_temp();
        s.upsert_profile("Mock|1|0", "MoiPseudo").unwrap();
        s.record_match(&fixture_record("M1", true)).unwrap();
        let recent = s.recent_matches("Mock|1|0", 10, 0).unwrap();
        assert_eq!(recent.len(), 1);
        assert!(recent[0].is_win);
        let detail = s.match_detail("M1").unwrap().expect("match present");
        assert_eq!(detail.players.len(), 2);
        assert!(detail.players.iter().any(|p| p.is_local_player));
        assert_eq!(detail.goals.len(), 1);
        assert_eq!(detail.statfeed.len(), 1);
    }

    #[test]
    fn aggregate_counts_wins_and_losses() {
        let s = open_temp();
        s.upsert_profile("Mock|1|0", "MoiPseudo").unwrap();
        s.record_match(&fixture_record("M1", true)).unwrap();
        s.record_match(&fixture_record("M2", false)).unwrap();
        s.record_match(&fixture_record("M3", true)).unwrap();
        let agg = s.aggregate("Mock|1|0", None).unwrap();
        assert_eq!(agg.matches, 3);
        assert_eq!(agg.wins, 2);
        assert_eq!(agg.losses, 1);
        assert_eq!(agg.current_streak, 1); // last match was a win
        assert_eq!(agg.best_win_streak, 1);
        assert_eq!(agg.best_loss_streak, 1);
        assert!(agg.by_team_size.iter().any(|b| b.team_size == 2));
    }

    #[test]
    fn current_session_rotates_when_stale() {
        let s = open_temp();
        s.upsert_profile("Mock|1|0", "MoiPseudo").unwrap();
        let s1 = s.current_session_id("Mock|1|0").unwrap();
        // Forge a stale session: backdate started_at by > TIMEOUT_MS.
        let stale_at = (now_ms() as i64) - (crate::session::Session::TIMEOUT_MS as i64) - 1_000;
        {
            let conn = s.conn.lock();
            conn.execute("UPDATE sessions SET started_at = ? WHERE id = ?",
                         params![stale_at, s1]).unwrap();
        }
        let s2 = s.current_session_id("Mock|1|0").unwrap();
        assert_ne!(s1, s2, "stale session must rotate to a fresh one");
        // The old session should now have an ended_at set.
        let conn = s.conn.lock();
        let ended: Option<i64> = conn
            .query_row("SELECT ended_at FROM sessions WHERE id = ?", params![s1], |r| r.get(0))
            .unwrap();
        assert!(ended.is_some(), "stale session must be closed");
    }

    #[test]
    fn current_session_keeps_active_when_fresh() {
        let s = open_temp();
        s.upsert_profile("Mock|1|0", "MoiPseudo").unwrap();
        let s1 = s.current_session_id("Mock|1|0").unwrap();
        let s2 = s.current_session_id("Mock|1|0").unwrap();
        assert_eq!(s1, s2, "fresh session must not rotate");
    }

    #[test]
    fn start_new_session_closes_previous() {
        let s = open_temp();
        s.upsert_profile("Mock|1|0", "MoiPseudo").unwrap();
        let s1 = s.current_session_id("Mock|1|0").unwrap();
        let s2 = s.start_new_session("Mock|1|0", "manual").unwrap();
        assert_ne!(s1, s2);
        let s3 = s.current_session_id("Mock|1|0").unwrap();
        assert_eq!(s2, s3);
    }

    #[test]
    fn delete_match_cascades() {
        let s = open_temp();
        s.upsert_profile("Mock|1|0", "MoiPseudo").unwrap();
        s.record_match(&fixture_record("M1", true)).unwrap();
        s.delete_match("M1").unwrap();
        assert!(s.match_detail("M1").unwrap().is_none());
        assert_eq!(s.count_matches().unwrap(), 0);
    }

    #[test]
    fn streaks_compute_correctly() {
        // Oldest first: W L L L W W → ends on +2 streak, best_loss=3, best_win=2.
        let outcomes = vec![true, false, false, false, true, true];
        let (cur, best_w, best_l) = compute_streaks(&outcomes);
        assert_eq!(cur, 2);
        assert_eq!(best_w, 2);
        assert_eq!(best_l, 3);
    }
}
