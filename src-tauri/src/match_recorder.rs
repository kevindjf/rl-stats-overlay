//! In-memory accumulator for a match in progress.
//!
//! Hooked into `ws_client.rs`: `MatchInitialized` / `MatchCreated` create a
//! fresh recorder, every `UpdateState` is sampled at ~20 Hz to update boost /
//! movement / powerslide counters per player, ball / crossbar / goal /
//! statfeed events extend their respective logs, and `MatchEnded` finalizes
//! the aggregate into a [`MatchRecord`] ready for `Storage::record_match`.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage::{
    GoalRecord, MatchPlayerRecord, MatchRecord, SpectatorStats, StatfeedRecord,
};

/// Sample at ~20 Hz: keep one in N ticks (PacketSendRate is typically 30 Hz
/// in our mock and up to 120 Hz in the real game). 20 Hz gives < 1% error on
/// percentage-of-time aggregates over a 5-min match while avoiding 120 Hz
/// per-tick locking work.
const TARGET_SAMPLE_HZ: f32 = 20.0;
const SLOW_SPEED_THRESHOLD: f32 = 1400.0;
const SUPERSONIC_SPEED_THRESHOLD: f32 = 2200.0;
const RL_MAX_SPEED: f32 = 2300.0;

#[derive(Debug, Clone)]
struct PlayerAggregator {
    player_name: String,
    primary_id: Option<String>,
    team_num: u8,
    is_local_team: bool,
    is_local_player: bool,
    // Latest core stats from the last UpdateState we saw for this player.
    goals: u32,
    shots: u32,
    saves: u32,
    assists: u32,
    score: u32,
    demos: u32,
    // SPECTATOR sampling. Only populated when is_local_team == true.
    sample_count: u32,
    // Boost
    boost_sum: f64,
    boost_at_0_count: u32,
    boost_at_100_count: u32,
    boost_buckets: [u32; 4], // 0-25, 25-50, 50-75, 75-100
    boosting_count: u32,
    boost_decrement_total: f32,
    boost_decrement_supersonic: f32,
    last_boost: Option<u8>,
    // Mouvement
    speed_sum: f64,
    distance_sum: f64,
    speed_slow_count: u32,
    speed_boost_count: u32,
    speed_supersonic_count: u32,
    ground_count: u32,
    aerial_count: u32,
    wall_count: u32,
    // Powerslide
    powerslide_active_ticks: u32,
    powerslide_count: u32,
    last_powerslide: bool,
    // Démos
    demos_taken: u32,
    last_demolished: bool,
}

impl PlayerAggregator {
    fn new(name: String, primary_id: Option<String>, team_num: u8, is_local_team: bool, is_local_player: bool) -> Self {
        Self {
            player_name: name,
            primary_id,
            team_num,
            is_local_team,
            is_local_player,
            goals: 0,
            shots: 0,
            saves: 0,
            assists: 0,
            score: 0,
            demos: 0,
            sample_count: 0,
            boost_sum: 0.0,
            boost_at_0_count: 0,
            boost_at_100_count: 0,
            boost_buckets: [0; 4],
            boosting_count: 0,
            boost_decrement_total: 0.0,
            boost_decrement_supersonic: 0.0,
            last_boost: None,
            speed_sum: 0.0,
            distance_sum: 0.0,
            speed_slow_count: 0,
            speed_boost_count: 0,
            speed_supersonic_count: 0,
            ground_count: 0,
            aerial_count: 0,
            wall_count: 0,
            powerslide_active_ticks: 0,
            powerslide_count: 0,
            last_powerslide: false,
            demos_taken: 0,
            last_demolished: false,
        }
    }

    /// Apply a SPECTATOR sample (one decimated UpdateState tick).
    fn sample(&mut self, p: &serde_json::Value, dt_s: f32) {
        if !self.is_local_team {
            return;
        }
        self.sample_count += 1;

        let boost = json_u8(p.get("Boost"));
        let boosting = json_bool(p.get("bBoosting"));
        let on_ground = json_bool(p.get("bOnGround"));
        let on_wall = json_bool(p.get("bOnWall"));
        let powersliding = json_bool(p.get("bPowersliding"));
        let supersonic = json_bool(p.get("bSupersonic"));
        let demolished = json_bool(p.get("bDemolished"));
        let speed = json_f32(p.get("Speed"));

        // Boost
        self.boost_sum += boost as f64;
        if boost == 0 { self.boost_at_0_count += 1; }
        if boost == 100 { self.boost_at_100_count += 1; }
        let bucket = match boost {
            0..=24 => 0,
            25..=49 => 1,
            50..=74 => 2,
            _ => 3,
        };
        self.boost_buckets[bucket] += 1;
        if boosting { self.boosting_count += 1; }
        if let Some(prev) = self.last_boost {
            if boost < prev {
                let consumed = (prev - boost) as f32;
                self.boost_decrement_total += consumed;
                if supersonic {
                    self.boost_decrement_supersonic += consumed;
                }
            }
        }
        self.last_boost = Some(boost);

        // Mouvement
        self.speed_sum += speed as f64;
        self.distance_sum += (speed as f64) * (dt_s as f64);
        if speed >= SUPERSONIC_SPEED_THRESHOLD {
            self.speed_supersonic_count += 1;
        } else if speed >= SLOW_SPEED_THRESHOLD {
            self.speed_boost_count += 1;
        } else {
            self.speed_slow_count += 1;
        }
        // Test wall first: a car driving on a wall has its 4 wheels touching
        // the world but is semantically "on the wall", not "on the ground".
        // Order matters because RL's bOnGround can be true while bOnWall is
        // also true depending on the surface normal — wall wins.
        if on_wall { self.wall_count += 1; }
        else if on_ground { self.ground_count += 1; }
        else { self.aerial_count += 1; }

        // Powerslide
        if powersliding { self.powerslide_active_ticks += 1; }
        if powersliding && !self.last_powerslide {
            self.powerslide_count += 1;
        }
        self.last_powerslide = powersliding;

        // Démos
        if demolished && !self.last_demolished {
            self.demos_taken += 1;
        }
        self.last_demolished = demolished;
    }

    /// Capture the latest core stats from each tick (overwrites). The Stats
    /// API delivers running totals, so the last tick before MatchEnded is the
    /// final state.
    fn update_core(&mut self, p: &serde_json::Value) {
        self.goals = json_u32(p.get("Goals"));
        self.shots = json_u32(p.get("Shots"));
        self.saves = json_u32(p.get("Saves"));
        self.assists = json_u32(p.get("Assists"));
        self.score = json_u32(p.get("Score"));
        self.demos = json_u32(p.get("Demos"));
    }

    fn finalize(self, dt_s: f32) -> MatchPlayerRecord {
        let n = self.sample_count.max(1) as f32;
        let total_s = (self.sample_count as f32) * dt_s;
        let bpm = if total_s > 0.0 {
            self.boost_decrement_total * 60.0 / total_s
        } else {
            0.0
        };
        let powerslide_total_s = self.powerslide_active_ticks as f32 * dt_s;
        let powerslide_avg_s = if self.powerslide_count > 0 {
            powerslide_total_s / self.powerslide_count as f32
        } else {
            0.0
        };
        let spectator = if self.is_local_team && self.sample_count > 0 {
            Some(SpectatorStats {
                boost_avg: (self.boost_sum / n as f64) as f32,
                boost_time_at_0_s: self.boost_at_0_count as f32 * dt_s,
                boost_time_at_100_s: self.boost_at_100_count as f32 * dt_s,
                boost_pct_0_25: pct(self.boost_buckets[0], n),
                boost_pct_25_50: pct(self.boost_buckets[1], n),
                boost_pct_50_75: pct(self.boost_buckets[2], n),
                boost_pct_75_100: pct(self.boost_buckets[3], n),
                boost_pct_boosting: pct(self.boosting_count, n),
                bpm,
                boost_used_supersonic: self.boost_decrement_supersonic,
                speed_avg_pct: (self.speed_sum / n as f64) as f32 / RL_MAX_SPEED * 100.0,
                total_distance: self.distance_sum as f32,
                pct_time_slow: pct(self.speed_slow_count, n),
                pct_time_boost_speed: pct(self.speed_boost_count, n),
                pct_time_supersonic: pct(self.speed_supersonic_count, n),
                pct_time_ground: pct(self.ground_count, n),
                pct_time_aerial: pct(self.aerial_count, n),
                pct_time_wall: pct(self.wall_count, n),
                powerslide_total_s,
                powerslide_count: self.powerslide_count,
                powerslide_avg_s,
                demos_taken: self.demos_taken,
            })
        } else {
            None
        };
        MatchPlayerRecord {
            player_name: self.player_name,
            primary_id: self.primary_id,
            team_num: self.team_num,
            is_local_team: self.is_local_team,
            is_local_player: self.is_local_player,
            goals: self.goals,
            shots: self.shots,
            saves: self.saves,
            assists: self.assists,
            score: self.score,
            demos: self.demos,
            spectator,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InProgressMatch {
    pub match_guid: String,
    pub started_at_ms: i64,
    pub local_primary_id: String,
    pub arena: Option<String>,
    pub team_size: u8,
    pub local_team_num: u8,
    pub blue_score: u32,
    pub orange_score: u32,
    pub last_seconds_remaining: i32,
    pub overtime: bool,
    /// Decimation : we keep one tick out of `tick_skip + 1`.
    tick_skip_target: u32,
    tick_seen: u32,
    sample_count: u32,
    /// Effective dt seen between two kept samples — recomputed on the first
    /// few samples from wall-clock to handle non-30-Hz mocks gracefully.
    sample_dt_s: f32,
    last_sample_time_ms: Option<i64>,
    aggregators: HashMap<(String, u8), PlayerAggregator>,
    pub goals: Vec<GoalRecord>,
    pub statfeed: Vec<StatfeedRecord>,
    pub crossbar_hits: u32,
    pub ball_hits_blue: u32,
    pub ball_hits_orange: u32,
}

impl InProgressMatch {
    pub fn begin(match_guid: String, local_primary_id: String) -> Self {
        Self {
            match_guid,
            started_at_ms: now_ms(),
            local_primary_id,
            arena: None,
            team_size: 0,
            local_team_num: 0,
            blue_score: 0,
            orange_score: 0,
            last_seconds_remaining: 0,
            overtime: false,
            tick_skip_target: 0,
            tick_seen: 0,
            sample_count: 0,
            sample_dt_s: 1.0 / TARGET_SAMPLE_HZ,
            last_sample_time_ms: None,
            aggregators: HashMap::new(),
            goals: Vec::new(),
            statfeed: Vec::new(),
            crossbar_hits: 0,
            ball_hits_blue: 0,
            ball_hits_orange: 0,
        }
    }

    /// Decide whether this tick should be processed (decimated to ~20 Hz),
    /// then update both core stats and SPECTATOR samples for every player.
    pub fn ingest_update_state(
        &mut self,
        players: &[serde_json::Value],
        game: Option<&serde_json::Value>,
        local_team_num: Option<u8>,
    ) {
        // Always update core stats from every tick — they're cheap and the
        // last tick before MatchEnded must reflect the final score.
        self.refresh_core_and_meta(players, game, local_team_num);

        // Decide whether to sample SPECTATOR fields on this tick. We aim for
        // roughly TARGET_SAMPLE_HZ and infer the upstream tick rate from the
        // first few intervals.
        self.tick_seen = self.tick_seen.saturating_add(1);
        if self.tick_skip_target == 0 {
            // Calibration phase: process every tick for the first 60 ticks
            // and learn the rate from wall-clock.
            let now = now_ms();
            if let Some(prev) = self.last_sample_time_ms {
                let dt_ms = (now - prev).max(1);
                let observed_hz = 1000.0 / dt_ms as f32;
                if observed_hz >= TARGET_SAMPLE_HZ * 1.4 {
                    let skip = (observed_hz / TARGET_SAMPLE_HZ).round() as u32;
                    self.tick_skip_target = skip.max(1);
                }
            }
            self.last_sample_time_ms = Some(now);
            self.sample_now(players);
            return;
        }
        // Steady-state decimation.
        if self.tick_seen % self.tick_skip_target != 0 {
            return;
        }
        self.sample_now(players);
    }

    fn sample_now(&mut self, players: &[serde_json::Value]) {
        // Compute dt from the previous accepted sample to keep the
        // decimation honest under variable upstream rates.
        let now = now_ms();
        if let Some(prev) = self.last_sample_time_ms {
            let dt = ((now - prev) as f32) / 1000.0;
            if dt > 0.0 && dt < 1.0 {
                // Smooth the dt estimate to avoid noise at cold start.
                self.sample_dt_s = self.sample_dt_s * 0.5 + dt * 0.5;
            }
        }
        self.last_sample_time_ms = Some(now);
        self.sample_count += 1;
        for p in players {
            let name = json_str(p.get("Name"));
            let team = json_u8(p.get("TeamNum"));
            let key = (name.clone(), team);
            let agg = self.aggregators.entry(key).or_insert_with(|| {
                let pid = p.get("PrimaryId").and_then(|v| v.as_str()).map(str::to_string);
                let is_local_team = team == self.local_team_num;
                let is_local_player = pid.as_deref() == Some(self.local_primary_id.as_str());
                PlayerAggregator::new(name.clone(), pid, team, is_local_team, is_local_player)
            });
            agg.sample(p, self.sample_dt_s);
        }
    }

    fn refresh_core_and_meta(
        &mut self,
        players: &[serde_json::Value],
        game: Option<&serde_json::Value>,
        local_team_num: Option<u8>,
    ) {
        if let Some(t) = local_team_num {
            self.local_team_num = t;
        }
        // Capture arena and team size on first sight.
        if let Some(g) = game {
            if self.arena.is_none() {
                if let Some(a) = g.get("Arena").and_then(|v| v.as_str()) {
                    if !a.is_empty() {
                        self.arena = Some(a.to_string());
                    }
                }
            }
            if let Some(teams) = g.get("Teams").and_then(|v| v.as_array()) {
                for t in teams {
                    let idx = t.get("TeamNum").and_then(|v| v.as_i64()).unwrap_or(-1);
                    let score = json_u32(t.get("Score"));
                    if idx == 0 { self.blue_score = score; }
                    if idx == 1 { self.orange_score = score; }
                }
            }
            self.last_seconds_remaining = g.get("TimeSeconds").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            self.overtime = g.get("bOvertime").and_then(|v| v.as_bool()).unwrap_or(false);
        }
        // Team size = max players per team
        let mut counts = [0u8; 4];
        for p in players {
            if let Some(t) = p.get("TeamNum").and_then(|v| v.as_i64()) {
                if (0..counts.len() as i64).contains(&t) {
                    counts[t as usize] = counts[t as usize].saturating_add(1);
                }
            }
        }
        let computed = counts.iter().copied().max().unwrap_or(0);
        if computed > self.team_size {
            self.team_size = computed;
        }
        // Update core stats on every tick.
        for p in players {
            let name = json_str(p.get("Name"));
            let team = json_u8(p.get("TeamNum"));
            let key = (name.clone(), team);
            let agg = self.aggregators.entry(key).or_insert_with(|| {
                let pid = p.get("PrimaryId").and_then(|v| v.as_str()).map(str::to_string);
                let is_local_team = Some(team) == local_team_num;
                let is_local_player = pid.as_deref() == Some(self.local_primary_id.as_str());
                PlayerAggregator::new(name.clone(), pid, team, is_local_team, is_local_player)
            });
            agg.update_core(p);
            // Backfill is_local_team if it was unknown when first seen.
            if let Some(t) = local_team_num {
                agg.is_local_team = team == t;
            }
            // Backfill primary_id (sometimes missing on first ticks).
            if agg.primary_id.is_none() {
                if let Some(pid) = p.get("PrimaryId").and_then(|v| v.as_str()) {
                    if !pid.is_empty() {
                        agg.primary_id = Some(pid.to_string());
                        if pid == self.local_primary_id {
                            agg.is_local_player = true;
                        }
                    }
                }
            }
        }
    }

    pub fn note_ball_hit(&mut self, team_num: u8) {
        match team_num {
            0 => self.ball_hits_blue = self.ball_hits_blue.saturating_add(1),
            1 => self.ball_hits_orange = self.ball_hits_orange.saturating_add(1),
            _ => {}
        }
    }

    pub fn note_crossbar(&mut self) {
        self.crossbar_hits = self.crossbar_hits.saturating_add(1);
    }

    pub fn note_goal(&mut self, payload: &serde_json::Value, seconds_remaining: i32) {
        let scorer = payload.get("Scorer");
        let assister = payload.get("Assister");
        let last_touch = payload.get("BallLastTouch").and_then(|x| x.get("Player"));
        let impact = payload.get("ImpactLocation");
        self.goals.push(GoalRecord {
            ord: self.goals.len() as u32,
            scored_at_seconds_remaining: Some(seconds_remaining),
            scorer_name: scorer.and_then(|s| s.get("Name")).and_then(|v| v.as_str()).map(str::to_string),
            scorer_team_num: scorer.and_then(|s| s.get("TeamNum")).and_then(|v| v.as_i64()).map(|n| n as u8),
            assister_name: assister.and_then(|a| a.get("Name")).and_then(|v| v.as_str()).map(str::to_string),
            last_touch_name: last_touch.and_then(|p| p.get("Name")).and_then(|v| v.as_str()).map(str::to_string),
            goal_speed: payload.get("GoalSpeed").and_then(|v| v.as_f64()).map(|v| v as f32),
            impact_x: impact.and_then(|i| i.get("X")).and_then(|v| v.as_f64()).map(|v| v as f32),
            impact_y: impact.and_then(|i| i.get("Y")).and_then(|v| v.as_f64()).map(|v| v as f32),
            impact_z: impact.and_then(|i| i.get("Z")).and_then(|v| v.as_f64()).map(|v| v as f32),
        });
    }

    pub fn note_statfeed(&mut self, payload: &serde_json::Value) {
        let main = payload.get("MainTarget");
        let secondary = payload.get("SecondaryTarget");
        self.statfeed.push(StatfeedRecord {
            ord: self.statfeed.len() as u32,
            at_seconds_remaining: Some(self.last_seconds_remaining),
            event_name: payload.get("EventName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            type_label: payload.get("Type").and_then(|v| v.as_str()).map(str::to_string),
            main_player: main.and_then(|m| m.get("Name")).and_then(|v| v.as_str()).map(str::to_string),
            main_team_num: main.and_then(|m| m.get("TeamNum")).and_then(|v| v.as_i64()).map(|n| n as u8),
            secondary_player: secondary.and_then(|s| s.get("Name")).and_then(|v| v.as_str()).map(str::to_string),
            secondary_team_num: secondary.and_then(|s| s.get("TeamNum")).and_then(|v| v.as_i64()).map(|n| n as u8),
        });
    }

    /// Build the final MatchRecord. `winner_team_num` from MatchEnded payload.
    pub fn finalize(mut self, winner_team_num: Option<u8>) -> MatchRecord {
        // Backfill is_local_team / is_local_player on every aggregator now
        // that local_team_num and local_primary_id are at their final values.
        // This catches the cold-start race where the recorder was created
        // before the first UpdateState arrived (PrimaryId not yet captured).
        for agg in self.aggregators.values_mut() {
            agg.is_local_team = agg.team_num == self.local_team_num;
            agg.is_local_player = agg
                .primary_id
                .as_deref()
                .map(|p| !self.local_primary_id.is_empty() && p == self.local_primary_id)
                .unwrap_or(false);
        }

        let started_at_ms = self.started_at_ms;
        let ended_at_ms = now_ms();
        let duration_seconds = ((ended_at_ms - started_at_ms).max(0) / 1000) as u32;
        let dt = self.sample_dt_s;
        let is_win = matches!(winner_team_num, Some(w) if w == self.local_team_num);
        let arena = self.arena.clone();
        let blue_score = self.blue_score;
        let orange_score = self.orange_score;
        let team_size = self.team_size.max(1);
        let local_team_num = self.local_team_num;
        let overtime = self.overtime;
        let ball_hits_blue = self.ball_hits_blue;
        let ball_hits_orange = self.ball_hits_orange;
        let crossbar_hits = self.crossbar_hits;
        let goals = self.goals.clone();
        let statfeed = self.statfeed.clone();
        let players: Vec<MatchPlayerRecord> = self
            .aggregators
            .into_values()
            .map(|a| a.finalize(dt))
            .collect();
        MatchRecord {
            match_guid: self.match_guid,
            primary_id: self.local_primary_id,
            started_at_ms,
            ended_at_ms,
            arena,
            team_size,
            local_team_num,
            winner_team_num,
            is_win,
            blue_score,
            orange_score,
            overtime,
            duration_seconds,
            ball_hits_blue,
            ball_hits_orange,
            crossbar_hits,
            players,
            goals,
            statfeed,
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn json_u8(v: Option<&serde_json::Value>) -> u8 {
    v.and_then(|x| x.as_i64()).map(|n| n.clamp(0, 100) as u8).unwrap_or(0)
}

fn json_u32(v: Option<&serde_json::Value>) -> u32 {
    v.and_then(|x| x.as_i64()).map(|n| if n < 0 { 0 } else { n as u32 }).unwrap_or(0)
}

fn json_f32(v: Option<&serde_json::Value>) -> f32 {
    v.and_then(|x| x.as_f64()).map(|n| n as f32).unwrap_or(0.0)
}

fn json_bool(v: Option<&serde_json::Value>) -> bool {
    v.and_then(|x| x.as_bool()).unwrap_or(false)
}

fn json_str(v: Option<&serde_json::Value>) -> String {
    v.and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

fn pct(count: u32, total: f32) -> f32 {
    if total <= 0.0 { 0.0 } else { (count as f32) / total * 100.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn me_tick(boost: u8, speed: f32, boosting: bool, supersonic: bool, on_ground: bool, powersliding: bool) -> serde_json::Value {
        json!({
            "Name": "MoiPseudo",
            "PrimaryId": "Mock|1|0",
            "TeamNum": 0,
            "Goals": 1, "Shots": 2, "Saves": 0, "Assists": 0, "Score": 250, "Demos": 0,
            "Boost": boost,
            "Speed": speed,
            "bBoosting": boosting,
            "bSupersonic": supersonic,
            "bOnGround": on_ground,
            "bOnWall": false,
            "bPowersliding": powersliding,
            "bDemolished": false,
        })
    }

    #[test]
    fn aggregates_basic_boost_and_speed() {
        let mut m = InProgressMatch::begin("M1".into(), "Mock|1|0".into());
        m.local_team_num = 0;
        let game = json!({ "Arena": "Stadium_P", "Teams": [{"TeamNum":0,"Score":2},{"TeamNum":1,"Score":1}], "TimeSeconds": 250, "bOvertime": false });
        // Force the recorder out of calibration: pretend many ticks elapsed.
        m.tick_skip_target = 1; // sample every tick
        let players_a = vec![me_tick(50, 1500.0, true, false, true, false)];
        let players_b = vec![me_tick(20, 2300.0, true, true, false, false)];
        let players_c = vec![me_tick(0, 600.0, false, false, true, true)];
        m.ingest_update_state(&players_a, Some(&game), Some(0));
        m.ingest_update_state(&players_b, Some(&game), Some(0));
        m.ingest_update_state(&players_c, Some(&game), Some(0));
        let record = m.finalize(Some(0));
        assert_eq!(record.players.len(), 1);
        let p = &record.players[0];
        assert!(p.is_local_player);
        assert_eq!(p.goals, 1);
        let s = p.spectator.as_ref().expect("local team has spectator stats");
        // 3 samples, avg boost = (50 + 20 + 0)/3 ≈ 23.33
        assert!((s.boost_avg - 23.33).abs() < 1.0);
        // Powerslide hit once on the third tick
        assert_eq!(s.powerslide_count, 1);
        // Aerial happened on tick B (off ground)
        assert!(s.pct_time_aerial > 0.0);
        // Supersonic on tick B
        assert!(s.pct_time_supersonic > 0.0);
    }

    #[test]
    fn opponent_team_has_no_spectator_stats() {
        let mut m = InProgressMatch::begin("M1".into(), "Mock|1|0".into());
        m.local_team_num = 0;
        m.tick_skip_target = 1;
        let game = json!({ "Arena": "Stadium_P", "Teams": [], "TimeSeconds": 250, "bOvertime": false });
        let opp = json!({
            "Name": "Opponent", "PrimaryId": "Mock|2|0", "TeamNum": 1,
            "Goals": 0, "Shots": 1, "Saves": 0, "Assists": 0, "Score": 100, "Demos": 0,
            "Boost": 0, "Speed": 0, "bBoosting": false, "bSupersonic": false,
            "bOnGround": true, "bOnWall": false, "bPowersliding": false, "bDemolished": false,
        });
        m.ingest_update_state(&vec![opp], Some(&game), Some(0));
        let record = m.finalize(Some(0));
        let opp_rec = record.players.iter().find(|p| p.team_num == 1).unwrap();
        assert!(!opp_rec.is_local_team);
        assert!(opp_rec.spectator.is_none());
    }

    #[test]
    fn bpm_proxy_counts_decrements() {
        let mut m = InProgressMatch::begin("M1".into(), "Mock|1|0".into());
        m.local_team_num = 0;
        m.tick_skip_target = 1;
        let game = json!({ "Arena": "Stadium_P", "Teams": [], "TimeSeconds": 250, "bOvertime": false });
        // Sequence: 100 → 80 → 50 → 30. Decrements summed = 70. Increments
        // (e.g. pickups) are ignored.
        for boost in [100u8, 80, 50, 30] {
            m.ingest_update_state(&vec![me_tick(boost, 1500.0, true, false, true, false)], Some(&game), Some(0));
        }
        let record = m.finalize(Some(0));
        let p = &record.players[0];
        let s = p.spectator.as_ref().unwrap();
        // 4 samples × dt; even though dt is approximate, BPM should be > 0.
        assert!(s.bpm > 0.0);
    }

    #[test]
    fn ball_hits_and_crossbar_counted() {
        let mut m = InProgressMatch::begin("M1".into(), "Mock|1|0".into());
        m.local_team_num = 0;
        m.note_ball_hit(0);
        m.note_ball_hit(0);
        m.note_ball_hit(1);
        m.note_crossbar();
        let r = m.finalize(Some(0));
        assert_eq!(r.ball_hits_blue, 2);
        assert_eq!(r.ball_hits_orange, 1);
        assert_eq!(r.crossbar_hits, 1);
    }

    #[test]
    fn full_pipeline_recorder_to_storage() {
        let dir = tempfile::tempdir().unwrap();
        let storage = crate::storage::Storage::open(dir.path()).unwrap();
        storage.upsert_profile("Mock|1|0", "MoiPseudo").unwrap();

        let mut m = InProgressMatch::begin("M-PIPELINE".into(), "Mock|1|0".into());
        m.local_team_num = 0;
        m.tick_skip_target = 1;
        let game = json!({
            "Arena": "Stadium_P",
            "Teams": [{"TeamNum":0,"Score":3},{"TeamNum":1,"Score":1}],
            "TimeSeconds": 250, "bOvertime": false
        });
        // Three sample ticks with varying SPECTATOR fields.
        m.ingest_update_state(&vec![me_tick(80, 1500.0, true, false, true, false)], Some(&game), Some(0));
        m.ingest_update_state(&vec![me_tick(40, 2300.0, true, true, false, false)], Some(&game), Some(0));
        m.ingest_update_state(&vec![me_tick(0, 600.0, false, false, true, true)], Some(&game), Some(0));
        // One ball hit per team, one crossbar, one goal, one statfeed.
        m.note_ball_hit(0);
        m.note_ball_hit(1);
        m.note_crossbar();
        m.note_goal(
            &json!({
                "Scorer": { "Name": "MoiPseudo", "TeamNum": 0 },
                "BallLastTouch": { "Player": { "Name": "MoiPseudo", "TeamNum": 0 } },
                "GoalSpeed": 88.0,
                "ImpactLocation": { "X": 0.0, "Y": 5120.0, "Z": 320.0 }
            }),
            240,
        );
        m.note_statfeed(&json!({
            "EventName": "Save",
            "Type": "Save",
            "MainTarget": { "Name": "MoiPseudo", "TeamNum": 0 }
        }));
        let record = m.finalize(Some(0));
        assert!(record.is_win);
        storage.record_match(&record).unwrap();

        let detail = storage.match_detail("M-PIPELINE").unwrap().unwrap();
        assert!(detail.is_win);
        assert_eq!(detail.ball_hits_blue, 1);
        assert_eq!(detail.ball_hits_orange, 1);
        assert_eq!(detail.crossbar_hits, 1);
        assert_eq!(detail.goals.len(), 1);
        assert_eq!(detail.statfeed.len(), 1);
        let local = detail.players.iter().find(|p| p.is_local_player).unwrap();
        let s = local.spectator.as_ref().expect("local has spectator stats");
        // Three samples → all four boost buckets exercised: 80→bucket3, 40→1, 0→0.
        assert!(s.boost_avg > 0.0);
        assert!(s.boost_pct_75_100 > 0.0);
        assert!(s.boost_pct_25_50 > 0.0);
        assert!(s.boost_time_at_0_s > 0.0);
        // Powerslide hit once on the third sample.
        assert_eq!(s.powerslide_count, 1);
    }

    #[test]
    fn goal_record_captures_assister_and_speed() {
        let mut m = InProgressMatch::begin("M1".into(), "Mock|1|0".into());
        m.local_team_num = 0;
        let payload = json!({
            "Scorer": { "Name": "MoiPseudo", "TeamNum": 0 },
            "Assister": { "Name": "Coéquipier", "TeamNum": 0 },
            "BallLastTouch": { "Player": { "Name": "MoiPseudo", "TeamNum": 0 }, "Speed": 90 },
            "GoalSpeed": 92.5,
            "ImpactLocation": { "X": 100.0, "Y": 5120.0, "Z": 320.0 }
        });
        m.note_goal(&payload, 270);
        let r = m.finalize(Some(0));
        assert_eq!(r.goals.len(), 1);
        let g = &r.goals[0];
        assert_eq!(g.scorer_name.as_deref(), Some("MoiPseudo"));
        assert_eq!(g.assister_name.as_deref(), Some("Coéquipier"));
        assert!((g.goal_speed.unwrap() - 92.5).abs() < 0.1);
    }
}
