//! Discovery of theme manifests.
//!
//! Two sources are merged:
//! 1. Bundled themes embedded at compile time via `rust-embed`
//!    (`overlays/themes/<id>/theme.json`).
//! 2. User-installed themes dropped into
//!    `%APPDATA%/RLStatsOverlay/themes/<id>/theme.json` — no rebuild
//!    required, just hit the "Refresh" button in the settings UI.
//!
//! User themes win on conflicting ids: someone wanting to override a
//! bundled theme can put a folder with the same id in the user dir.

use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tracing::warn;

use crate::settings;

/// Manifest schema mirrored exactly into the JSON emitted to the frontend
/// — `vars[]` shape is intentionally identical to what `src/main.ts`
/// renders, so the settings UI can consume it without translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeManifest {
    #[serde(rename = "manifestVersion", default = "default_manifest_version")]
    pub manifest_version: u32,
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    pub vars: Vec<ThemeVarDef>,
    /// True when this manifest comes from `%APPDATA%/.../themes/`. Lets the
    /// UI badge user themes and offer to open the source folder.
    #[serde(skip_deserializing, default)]
    pub user_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeVarDef {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub spec: VarSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum VarSpec {
    Color {
        default: String,
    },
    Number {
        default: f64,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        step: Option<f64>,
        #[serde(default)]
        unit: Option<String>,
    },
    Boolean {
        default: bool,
    },
}

fn default_manifest_version() -> u32 {
    1
}

/// Returns `%APPDATA%/RLStatsOverlay/themes/`. Same parent as `settings.json`
/// so a user only ever has one folder to look at.
pub fn user_themes_dir() -> Option<PathBuf> {
    settings::settings_dir().ok().map(|d| d.join("themes"))
}

/// Walk both sources and return one manifest per discovered theme.
pub fn discover<F>(bundled_iter: F) -> Vec<ThemeManifest>
where
    F: IntoIterator<Item = (String, Vec<u8>)>,
{
    let mut out: Vec<ThemeManifest> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. User-installed first so they win on id collisions.
    if let Some(dir) = user_themes_dir() {
        if dir.exists() {
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let folder = entry.path();
                if !folder.is_dir() {
                    continue;
                }
                // Skip hidden / template folders.
                let name = folder.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name.is_empty() || name.starts_with('.') || name.starts_with('_') {
                    continue;
                }
                let manifest_path = folder.join("theme.json");
                if !manifest_path.exists() {
                    continue;
                }
                match fs::read_to_string(&manifest_path) {
                    Ok(text) => match serde_json::from_str::<ThemeManifest>(&text) {
                        Ok(mut m) => {
                            m.user_installed = true;
                            // Pin id to the folder name so a copy-pasted manifest
                            // can't shadow an unrelated bundled theme.
                            m.id = name.to_string();
                            if seen.insert(m.id.clone()) {
                                out.push(m);
                            }
                        }
                        Err(err) => warn!(?err, path = %manifest_path.display(), "ignoring user theme: bad manifest"),
                    },
                    Err(err) => warn!(?err, path = %manifest_path.display(), "ignoring user theme: read failed"),
                }
            }
        }
    }

    // 2. Bundled — only add ones the user hasn't overridden.
    for (path, bytes) in bundled_iter {
        // Expect "themes/<id>/theme.json"
        let rel = path.strip_prefix("themes/").unwrap_or(&path);
        let mut parts = rel.splitn(2, '/');
        let id = match parts.next() {
            Some(s) if !s.starts_with('_') => s,
            _ => continue,
        };
        if parts.next() != Some("theme.json") {
            continue;
        }
        if seen.contains(id) {
            continue;
        }
        match serde_json::from_slice::<ThemeManifest>(&bytes) {
            Ok(mut m) => {
                m.id = id.to_string();
                m.user_installed = false;
                seen.insert(id.to_string());
                out.push(m);
            }
            Err(err) => warn!(?err, theme = id, "ignoring bundled theme: bad manifest"),
        }
    }

    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}
