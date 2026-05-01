use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::session::Session;

const APP_DIR_NAME: &str = "RLStatsOverlay";
const SETTINGS_FILE: &str = "settings.json";

/// Persistent configuration written to `%APPDATA%/RLStatsOverlay/settings.json`
/// (or the OS equivalent on macOS / Linux for development).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// In-game display name configured by the user.
    pub player_name: String,
    /// Stable platform identifier captured from the first match
    /// (e.g. "Epic|abc123|0"). Survives name changes in-game.
    #[serde(default)]
    pub primary_id: String,
    /// Path of the patched DefaultStatsAPI.ini, used for re-checking after RL updates.
    #[serde(default)]
    pub ini_path: Option<PathBuf>,
    /// Whether the first-run wizard has completed.
    #[serde(default)]
    pub setup_done: bool,
    /// Last known live session — restored on app start.
    #[serde(default)]
    pub session: Session,
    /// In-game HUD window position, in physical pixels.
    #[serde(default)]
    pub hud_pos: Option<(i32, i32)>,
    /// In-game HUD window size, in physical pixels.
    #[serde(default)]
    pub hud_size: Option<(u32, u32)>,
    /// Whether the in-game HUD should be opened on app start.
    #[serde(default)]
    pub hud_visible: bool,
    /// When true, the HUD is click-through (cursor events pass to the game)
    /// and the drag/right-click endpoints become no-ops. Lets the user pin a
    /// position once and prevent accidental drags during gameplay.
    #[serde(default)]
    pub hud_position_locked: bool,
    /// Active overlay theme. Maps to the folder name under `overlays/themes/`.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Per-theme overrides applied on top of each theme's CSS defaults.
    /// Outer key = theme name, inner key = camelCase var name, value = JSON
    /// scalar (string for colors, number for sizes, boolean for toggles).
    #[serde(default)]
    pub theme_overrides: HashMap<String, HashMap<String, serde_json::Value>>,
    /// Team sizes counted toward the W/L tally — values 1..=4. The official
    /// Stats API does not expose ranked-vs-casual playlist info, but it does
    /// give us per-team player counts, so users can at least scope their
    /// session to "3v3 only" / "2v2 only" etc. Default: count everything.
    #[serde(default = "default_team_sizes")]
    pub count_team_sizes: Vec<u8>,
    /// UI language: "auto" (use navigator.language), "fr", or "en".
    /// We treat any unrecognized value as "auto" on the frontend.
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_team_sizes() -> Vec<u8> {
    vec![1, 2, 3, 4]
}

fn default_theme() -> String {
    "circle".into()
}

fn default_language() -> String {
    "auto".into()
}

impl Settings {
    /// Returns the override map for the currently active theme, or an empty
    /// map if the user has never tweaked it.
    pub fn current_theme_vars(&self) -> HashMap<String, serde_json::Value> {
        self.theme_overrides
            .get(&self.theme)
            .cloned()
            .unwrap_or_default()
    }

    /// Set or clear a single var on the active theme. `None` removes the
    /// override (the CSS default takes over).
    pub fn set_theme_var(&mut self, key: String, value: Option<serde_json::Value>) {
        let entry = self.theme_overrides.entry(self.theme.clone()).or_default();
        match value {
            Some(v) => {
                entry.insert(key, v);
            }
            None => {
                entry.remove(&key);
            }
        }
    }

    /// True when the persisted HUD size is still the implicit default — i.e.
    /// the user has never resized the HUD. Used by the boot-time DPI scaler
    /// to apply a one-shot 4K bump only on virgin installs.
    pub fn hud_size_is_default(&self) -> bool {
        self.hud_size.is_none()
    }
}

impl Settings {
    pub fn load() -> Result<Self> {
        let path = settings_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let parsed: Self = serde_json::from_str(&data).unwrap_or_default();
        Ok(parsed)
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        atomic_write(&path, json.as_bytes())?;
        Ok(())
    }
}

/// Returns the absolute path of the settings file, creating the parent
/// directory if it does not already exist.
fn settings_path() -> Result<PathBuf> {
    let dir = settings_dir()?;
    Ok(dir.join(SETTINGS_FILE))
}

pub fn settings_dir() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .or_else(dirs::data_local_dir)
        .context("no platform config directory available")?;
    Ok(base.join(APP_DIR_NAME))
}

pub fn logs_dir() -> Result<PathBuf> {
    Ok(settings_dir()?.join("logs"))
}

/// Writes to a tmp file and renames to target — prevents corruption if the
/// process is killed mid-write.
fn atomic_write(target: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = target.with_extension("tmp");
    fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, target)
        .with_context(|| format!("renaming {} → {}", tmp.display(), target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_size_is_default_on_fresh_settings() {
        let s = Settings::default();
        assert!(s.hud_size_is_default());
    }

    #[test]
    fn hud_size_is_default_false_after_user_override() {
        let mut s = Settings::default();
        s.hud_size = Some((600, 450));
        assert!(!s.hud_size_is_default());
    }

    #[test]
    fn hud_position_locked_default_is_false() {
        let s = Settings::default();
        assert!(!s.hud_position_locked);
    }

    #[test]
    fn settings_round_trip_preserves_hud_position_locked() {
        let mut s = Settings::default();
        s.hud_position_locked = true;
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert!(back.hud_position_locked);
    }
}
