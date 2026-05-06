use serde::{Deserialize, Serialize};

/// Per-team aggregates collected by the recorder while a match is in progress.
/// SPECTATOR fields are populated only for players on the local team — the
/// official Stats API omits them for opponents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectatorStats {
    pub boost_avg: f32,
    pub boost_time_at_0_s: f32,
    pub boost_time_at_100_s: f32,
    pub boost_pct_0_25: f32,
    pub boost_pct_25_50: f32,
    pub boost_pct_50_75: f32,
    pub boost_pct_75_100: f32,
    pub boost_pct_boosting: f32,
    /// Approximated from per-tick `Boost` decrements. ~95% accuracy vs replays.
    pub bpm: f32,
    /// Boost units consumed while bSupersonic was true (proxy from decrements).
    pub boost_used_supersonic: f32,
    pub speed_avg_pct: f32,
    pub total_distance: f32,
    pub pct_time_slow: f32,
    pub pct_time_boost_speed: f32,
    pub pct_time_supersonic: f32,
    pub pct_time_ground: f32,
    pub pct_time_aerial: f32,
    pub pct_time_wall: f32,
    pub powerslide_total_s: f32,
    pub powerslide_count: u32,
    pub powerslide_avg_s: f32,
    pub demos_taken: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchPlayerRecord {
    pub player_name: String,
    pub primary_id: Option<String>,
    pub team_num: u8,
    pub is_local_team: bool,
    pub is_local_player: bool,
    // Core stats — present for every player in the match.
    pub goals: u32,
    pub shots: u32,
    pub saves: u32,
    pub assists: u32,
    pub score: u32,
    pub demos: u32,
    /// SPECTATOR-only metrics. `None` for the opposing team.
    pub spectator: Option<SpectatorStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalRecord {
    pub ord: u32,
    pub scored_at_seconds_remaining: Option<i32>,
    pub scorer_name: Option<String>,
    pub scorer_team_num: Option<u8>,
    pub assister_name: Option<String>,
    pub last_touch_name: Option<String>,
    pub goal_speed: Option<f32>,
    pub impact_x: Option<f32>,
    pub impact_y: Option<f32>,
    pub impact_z: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatfeedRecord {
    pub ord: u32,
    pub at_seconds_remaining: Option<i32>,
    pub event_name: String,
    pub type_label: Option<String>,
    pub main_player: Option<String>,
    pub main_team_num: Option<u8>,
    pub secondary_player: Option<String>,
    pub secondary_team_num: Option<u8>,
}

/// Full record handed to `Storage::record_match` once a match is finalized.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchRecord {
    pub match_guid: String,
    pub primary_id: String,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub arena: Option<String>,
    pub team_size: u8,
    pub local_team_num: u8,
    pub winner_team_num: Option<u8>,
    pub is_win: bool,
    pub blue_score: u32,
    pub orange_score: u32,
    pub overtime: bool,
    pub duration_seconds: u32,
    pub ball_hits_blue: u32,
    pub ball_hits_orange: u32,
    pub crossbar_hits: u32,
    pub players: Vec<MatchPlayerRecord>,
    pub goals: Vec<GoalRecord>,
    pub statfeed: Vec<StatfeedRecord>,
}

/// Compact row used in the history list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    pub match_guid: String,
    pub primary_id: String,
    pub ended_at_ms: i64,
    pub started_at_ms: i64,
    pub arena: Option<String>,
    pub team_size: u8,
    pub blue_score: u32,
    pub orange_score: u32,
    pub local_team_num: u8,
    pub is_win: bool,
    pub overtime: bool,
    pub duration_seconds: u32,
}

/// Full match payload used in the detail view. Same shape as `MatchRecord`.
pub type MatchDetail = MatchRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub primary_id: String,
    pub display_name: String,
    pub first_seen_at_ms: i64,
    pub last_seen_at_ms: i64,
    pub match_count: u64,
}

/// Per-bucket / per-playlist breakdown of a session or lifetime aggregate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistBreakdown {
    pub team_size: u8,
    pub wins: u32,
    pub losses: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateStats {
    pub matches: u32,
    pub wins: u32,
    pub losses: u32,
    pub current_streak: i32,
    pub best_win_streak: u32,
    pub best_loss_streak: u32,
    pub avg_goals: f32,
    pub avg_shots: f32,
    pub avg_saves: f32,
    pub avg_assists: f32,
    pub avg_score: f32,
    pub avg_demos: f32,
    /// Local-team SPECTATOR aggregates (averaged over matches that had them).
    pub avg_boost: Option<f32>,
    pub avg_bpm: Option<f32>,
    pub avg_supersonic_pct: Option<f32>,
    pub avg_aerial_pct: Option<f32>,
    pub avg_powerslide_count: Option<f32>,
    /// Extended aggregates used by the post-match HUD's "Session" toggle —
    /// same metric set as a single match, summed/averaged across the scope.
    pub avg_demos_taken: Option<f32>,
    pub avg_boosting_pct: Option<f32>,
    pub avg_boost_time_at_0_s: Option<f32>,
    pub avg_boost_time_at_100_s: Option<f32>,
    pub avg_boost_pct_0_25: Option<f32>,
    pub avg_boost_pct_25_50: Option<f32>,
    pub avg_boost_pct_50_75: Option<f32>,
    pub avg_boost_pct_75_100: Option<f32>,
    pub avg_speed_avg_pct: Option<f32>,
    pub avg_pct_time_slow: Option<f32>,
    pub avg_pct_time_boost_speed: Option<f32>,
    pub avg_pct_time_ground: Option<f32>,
    pub avg_pct_time_wall: Option<f32>,
    /// Sums (not averages) — distance and powerslide totals are meaningful
    /// summed across a session.
    pub total_distance: Option<f32>,
    pub total_powerslide_s: Option<f32>,
    pub total_duration_s: u32,
    pub by_team_size: Vec<PlaylistBreakdown>,
    /// Last 30 win/loss outcomes, oldest first. `true` = win.
    pub recent_outcomes: Vec<bool>,
    /// Best match by player score for the local user (None if no SPECTATOR data).
    pub best_match_score: Option<u32>,
    pub best_match_guid: Option<String>,
    pub worst_match_guid: Option<String>,
    pub started_at_ms: Option<i64>,
}
