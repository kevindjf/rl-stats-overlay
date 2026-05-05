use anyhow::{Context, Result};
use rusqlite::Connection;

/// Schema version used by `pragma user_version`. Bump and append a new V_*_SQL
/// constant whenever the schema needs a migration.
const CURRENT_VERSION: i32 = 1;

const V1_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS profiles (
    primary_id    TEXT PRIMARY KEY,
    display_name  TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_seen_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    primary_id   TEXT NOT NULL REFERENCES profiles(primary_id),
    started_at   INTEGER NOT NULL,
    ended_at     INTEGER,
    reset_reason TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_profile_active
    ON sessions(primary_id, ended_at);

CREATE TABLE IF NOT EXISTS matches (
    match_guid        TEXT PRIMARY KEY,
    primary_id        TEXT NOT NULL REFERENCES profiles(primary_id),
    started_at        INTEGER NOT NULL,
    ended_at          INTEGER NOT NULL,
    arena             TEXT,
    team_size         INTEGER NOT NULL,
    local_team_num    INTEGER NOT NULL,
    winner_team_num   INTEGER,
    is_win            INTEGER NOT NULL,
    blue_score        INTEGER NOT NULL,
    orange_score      INTEGER NOT NULL,
    overtime          INTEGER NOT NULL,
    duration_seconds  INTEGER NOT NULL,
    ball_hits_blue    INTEGER,
    ball_hits_orange  INTEGER,
    crossbar_hits     INTEGER,
    session_id        INTEGER NOT NULL REFERENCES sessions(id)
);
CREATE INDEX IF NOT EXISTS idx_matches_profile_time
    ON matches(primary_id, ended_at DESC);
CREATE INDEX IF NOT EXISTS idx_matches_session
    ON matches(primary_id, session_id);

CREATE TABLE IF NOT EXISTS match_players (
    match_guid              TEXT NOT NULL REFERENCES matches(match_guid) ON DELETE CASCADE,
    player_name             TEXT NOT NULL,
    primary_id              TEXT,
    team_num                INTEGER NOT NULL,
    is_local_team           INTEGER NOT NULL,
    is_local_player         INTEGER NOT NULL,
    goals                   INTEGER, shots INTEGER, saves INTEGER,
    assists                 INTEGER, score INTEGER, demos INTEGER,
    boost_avg               REAL,
    boost_time_at_0_s       REAL,
    boost_time_at_100_s     REAL,
    boost_pct_0_25          REAL,
    boost_pct_25_50         REAL,
    boost_pct_50_75         REAL,
    boost_pct_75_100        REAL,
    boost_pct_boosting      REAL,
    bpm                     REAL,
    boost_used_supersonic   REAL,
    speed_avg_pct           REAL,
    total_distance          REAL,
    pct_time_slow           REAL,
    pct_time_boost_speed    REAL,
    pct_time_supersonic     REAL,
    pct_time_ground         REAL,
    pct_time_aerial         REAL,
    pct_time_wall           REAL,
    powerslide_total_s      REAL,
    powerslide_count        INTEGER,
    powerslide_avg_s        REAL,
    demos_taken             INTEGER,
    PRIMARY KEY (match_guid, player_name, team_num)
);

CREATE TABLE IF NOT EXISTS match_goals (
    match_guid                  TEXT NOT NULL REFERENCES matches(match_guid) ON DELETE CASCADE,
    ord                         INTEGER NOT NULL,
    scored_at_seconds_remaining INTEGER,
    scorer_name                 TEXT,
    scorer_team_num             INTEGER,
    assister_name               TEXT,
    last_touch_name             TEXT,
    goal_speed                  REAL,
    impact_x                    REAL,
    impact_y                    REAL,
    impact_z                    REAL,
    PRIMARY KEY (match_guid, ord)
);

CREATE TABLE IF NOT EXISTS match_statfeed (
    match_guid          TEXT NOT NULL REFERENCES matches(match_guid) ON DELETE CASCADE,
    ord                 INTEGER NOT NULL,
    at_seconds_remaining INTEGER,
    event_name          TEXT NOT NULL,
    type_label          TEXT,
    main_player         TEXT,
    main_team_num       INTEGER,
    secondary_player    TEXT,
    secondary_team_num  INTEGER,
    PRIMARY KEY (match_guid, ord)
);
"#;

/// Apply pending migrations sequentially. Idempotent — safe to call on every
/// boot. Bumps `pragma user_version` so successive runs are no-ops.
pub fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .context("reading user_version")?;
    if current >= CURRENT_VERSION {
        return Ok(());
    }
    let tx = conn.transaction().context("starting migration tx")?;
    if current < 1 {
        tx.execute_batch(V1_SQL).context("applying V1 schema")?;
    }
    tx.pragma_update(None, "user_version", CURRENT_VERSION)
        .context("bumping user_version")?;
    tx.commit().context("committing migration")?;
    Ok(())
}
