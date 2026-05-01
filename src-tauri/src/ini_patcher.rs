use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

const INI_REL_PATH: &str = r"TAGame\Config\DefaultStatsAPI.ini";
const TARGET_PACKET_RATE: u32 = 30;
const TARGET_PORT: u16 = 49123;

/// Information about a detected Rocket League installation.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedInstall {
    pub platform: &'static str,
    pub install_dir: PathBuf,
    pub ini_path: PathBuf,
}

/// Search every known Rocket League install location and return everything
/// that contains the expected `TAGame\Config\` folder.
pub fn detect_installations() -> Vec<DetectedInstall> {
    let mut found = Vec::new();
    for (platform, dir) in candidate_dirs() {
        let ini = dir.join(INI_REL_PATH);
        // We accept any folder that contains TAGame\Config — the .ini may not
        // exist yet on a fresh install but its parent always does.
        if ini.parent().map(|p| p.exists()).unwrap_or(false) {
            found.push(DetectedInstall {
                platform,
                install_dir: dir,
                ini_path: ini,
            });
        }
    }
    found
}

/// Result of a `patch_ini` call, surfaced to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct PatchOutcome {
    pub already_correct: bool,
    pub backup_path: Option<PathBuf>,
}

/// Read the given DefaultStatsAPI.ini, ensure the `[TAGame.MatchStatsExporter_TA]`
/// section has the right `PacketSendRate` and `Port`, and rewrite it. This is
/// the section Rocket League actually reads — by default it ships with
/// `PacketSendRate=0`, which keeps the WebSocket listener closed. We bump it
/// to a non-zero rate to enable the listener.
///
/// The previous file is backed up next to the original as `*.bak` (only on the
/// first patch — subsequent patches leave the existing backup untouched).
pub fn patch_ini(path: &Path) -> Result<PatchOutcome> {
    let parent = path
        .parent()
        .with_context(|| format!("invalid path: {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("creating {}", parent.display()))?;

    // Load existing or default content.
    let mut conf = if path.exists() {
        ini::Ini::load_from_file(path).with_context(|| format!("reading {}", path.display()))?
    } else {
        ini::Ini::new()
    };

    const SECTION: &str = "TAGame.MatchStatsExporter_TA";
    // Older versions of this app patched a different section that RL ignores.
    // Strip it so it doesn't leave dead config behind.
    const LEGACY_SECTION: &str = "/Script/TAGame.StatsAPIClient";

    let current_rate: Option<u32> = conf
        .get_from(Some(SECTION), "PacketSendRate")
        .and_then(|s| s.trim().parse().ok());
    let current_port: Option<u16> = conf
        .get_from(Some(SECTION), "Port")
        .and_then(|s| s.trim().parse().ok());
    let has_legacy = conf.section(Some(LEGACY_SECTION)).is_some();

    if current_rate == Some(TARGET_PACKET_RATE)
        && current_port == Some(TARGET_PORT)
        && !has_legacy
    {
        return Ok(PatchOutcome {
            already_correct: true,
            backup_path: None,
        });
    }

    // Backup if the file existed and we have not already saved one.
    let mut backup_path = None;
    if path.exists() {
        let backup = path.with_extension("ini.bak");
        if !backup.exists() {
            fs::copy(path, &backup)
                .with_context(|| format!("backing up to {}", backup.display()))?;
            backup_path = Some(backup);
        }
    }

    if has_legacy {
        conf.delete(Some(LEGACY_SECTION));
    }
    conf.with_section(Some(SECTION))
        .set("PacketSendRate", TARGET_PACKET_RATE.to_string())
        .set("Port", TARGET_PORT.to_string());

    // ini::Ini::write_to_file produces inconsistent line endings on Windows;
    // round-trip through a string buffer to keep it predictable.
    let mut buf = Vec::new();
    conf.write_to(&mut buf)
        .with_context(|| format!("serializing ini for {}", path.display()))?;
    fs::write(path, buf).with_context(|| format!("writing {}", path.display()))?;

    Ok(PatchOutcome {
        already_correct: false,
        backup_path,
    })
}

// ----- Platform detection -----------------------------------------------------

#[cfg(target_os = "windows")]
fn candidate_dirs() -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    out.extend(steam_install_dirs().into_iter().map(|p| ("Steam", p)));
    out.extend(epic_install_dirs().into_iter().map(|p| ("Epic", p)));
    out
}

#[cfg(not(target_os = "windows"))]
fn candidate_dirs() -> Vec<(&'static str, PathBuf)> {
    // Rocket League's Stats API is Windows-only. On macOS/Linux we surface no
    // candidates — the user can still browse to a custom path manually.
    Vec::new()
}

/// Best-effort Steam installation root. Tries the registry first
/// (`HKCU\Software\Valve\Steam\SteamPath`), then the classic
/// `Program Files (x86)\Steam` fallback. Used by both the RL install
/// detection here and the local-account-id detection in `platform_detect`.
#[cfg(target_os = "windows")]
pub(crate) fn steam_root() -> Option<PathBuf> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(path) = hkcu.get_value::<String, _>("SteamPath") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        let p = PathBuf::from(pf86).join("Steam");
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        let p = PathBuf::from(pf).join("Steam");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn steam_install_dirs() -> Vec<PathBuf> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(path) = hkcu.get_value::<String, _>("SteamPath") {
            roots.push(PathBuf::from(path));
        }
    }

    // Fallback: classic Steam location.
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(pf86).join("Steam"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(pf).join("Steam"));
    }

    let mut found = Vec::new();
    for root in roots {
        // Default install
        let main = root.join("steamapps").join("common").join("rocketleague");
        if main.exists() {
            found.push(main);
        }

        // Library folders configured by the user — Steam stores them in
        // libraryfolders.vdf. We do a best-effort plain-text parse.
        let vdf = root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(content) = std::fs::read_to_string(&vdf) {
            for path in parse_steam_library_folders(&content) {
                let candidate = path.join("steamapps").join("common").join("rocketleague");
                if candidate.exists() {
                    found.push(candidate);
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

#[cfg(target_os = "windows")]
fn epic_install_dirs() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let manifests_dir = std::path::Path::new(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests");
    if let Ok(entries) = std::fs::read_dir(manifests_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("item") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let display_name = json
                        .get("DisplayName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let app_name = json
                        .get("AppName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    // Both fields used because Epic has been inconsistent over the years.
                    if !display_name.eq_ignore_ascii_case("Rocket League")
                        && !app_name.eq_ignore_ascii_case("Sugar")
                    {
                        continue;
                    }
                    if let Some(install) = json
                        .get("InstallLocation")
                        .and_then(|v| v.as_str())
                        .map(PathBuf::from)
                    {
                        if install.exists() {
                            found.push(install);
                        }
                    }
                }
            }
        }
    }
    found
}

#[cfg(target_os = "windows")]
fn parse_steam_library_folders(content: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // Naive parsing — picks up every "path" "..." line. Good enough since we
    // only care about extracting library roots; Valve doesn't put any other
    // "path" key inside libraryfolders.vdf.
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("\"path\"") {
            if let Some(start) = rest.find('"') {
                if let Some(end) = rest[start + 1..].find('"') {
                    let raw = &rest[start + 1..start + 1 + end];
                    out.push(PathBuf::from(raw.replace("\\\\", "\\")));
                }
            }
        }
    }
    out
}

/// Returns the absolute path that should be used to write the patched ini
/// from a user-selected directory or file.
pub fn resolve_ini_path(input: &Path) -> Result<PathBuf> {
    if input.is_dir() {
        let nested = input.join(INI_REL_PATH);
        return Ok(nested);
    }
    if input
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("DefaultStatsAPI.ini"))
        .unwrap_or(false)
    {
        return Ok(input.to_path_buf());
    }
    Err(anyhow!(
        "expected a Rocket League install folder or DefaultStatsAPI.ini, got {}",
        input.display()
    ))
}
