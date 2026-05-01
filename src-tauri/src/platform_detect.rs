//! Best-effort detection of the local user's Steam / Epic identifiers so the
//! wizard can skip the "type your in-game name" step and let the first
//! `UpdateState` from the RL Stats API arbitrate which player is us via
//! prefix-matching on `Players[].PrimaryId` (format `Platform|Uid|Splitscreen`).
//!
//! Anything that fails is silently ignored — the wizard falls back to the
//! manual name input in that case. We never block boot on detection.

#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};

/// Returns prefix strings ready to feed into `PrimaryId.starts_with(...)`.
/// Each element is `"Steam|<id>|"` or `"Epic|<id>|"` — the trailing `|` is
/// load-bearing: it prevents an ID-A being a prefix of ID-B and over-matching
/// on the second segment of the PrimaryId triplet.
pub fn local_platform_candidates() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(id) = detect_steam_id() {
        out.push(format!("Steam|{id}|"));
    }
    for id in detect_epic_account_ids() {
        out.push(format!("Epic|{id}|"));
    }
    out
}

// ---------- Windows implementations -----------------------------------------

#[cfg(target_os = "windows")]
fn detect_steam_id() -> Option<String> {
    let root = crate::ini_patcher::steam_root()?;
    let vdf = root.join("config").join("loginusers.vdf");
    let content = std::fs::read_to_string(&vdf).ok()?;
    parse_most_recent_steamid64(&content)
}

#[cfg(target_os = "windows")]
fn detect_epic_account_ids() -> Vec<String> {
    let local = match std::env::var_os("LOCALAPPDATA") {
        Some(v) => PathBuf::from(v),
        None => return Vec::new(),
    };
    let saved_data = local.join("EpicGamesLauncher").join("Saved").join("Data");
    epic_account_ids_in(&saved_data)
}

/// Pull every `*.dat` file stem from the given directory. Subdirectories and
/// non-`.dat` files are ignored. Symlinks are followed implicitly by `read_dir`.
#[cfg(target_os = "windows")]
fn epic_account_ids_in(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("dat") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            // Epic account IDs are 32-char lowercase-hex strings (UUIDs without
            // dashes). The Saved\Data folder also contains noise files like
            // `OC_<id>.dat` (online-cache shadows) — ignore anything that isn't
            // exactly an AccountId-shaped stem.
            if is_epic_account_id(stem) {
                out.push(stem.to_string());
            }
        }
    }
    out
}

// ---------- Non-Windows stubs -----------------------------------------------

#[cfg(not(target_os = "windows"))]
fn detect_steam_id() -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
fn detect_epic_account_ids() -> Vec<String> {
    Vec::new()
}

// ---------- VDF parser ------------------------------------------------------
//
// `loginusers.vdf` looks like:
//
//   "users"
//   {
//       "76561198XXXXXXXXX"
//       {
//           "AccountName"   "alice"
//           "PersonaName"   "Alice"
//           "RememberPassword"  "1"
//           "MostRecent"        "1"
//           "Timestamp"     "1700000000"
//       }
//       "76561198YYYYYYYYY"
//       {
//           ...
//           "MostRecent"        "0"
//           ...
//       }
//   }
//
// We track block depth via `{`/`}` and remember the most recent SteamID64-looking
// key whose block contains a `"MostRecent" "1"` line. No new dependency needed.

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_most_recent_steamid64(content: &str) -> Option<String> {
    let mut depth: i32 = 0;
    // Stack of the most-recent SteamID64 key seen at each depth right before
    // a block opened. Block depth 1 = inside top-level "users" block;
    // an account block opens at depth 2.
    let mut pending_key_at_depth: Vec<Option<String>> = Vec::new();
    let mut current_account: Option<String> = None;
    let mut found: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "{" {
            depth += 1;
            // Whatever key was last seen on a quoted-only line at this depth
            // is the name of the block we're entering.
            let key = pending_key_at_depth.pop().unwrap_or(None);
            if let Some(k) = key {
                if is_steamid64(&k) {
                    current_account = Some(k);
                }
            }
            continue;
        }
        if line == "}" {
            depth -= 1;
            // Leaving an account block — clear the current account marker
            // so siblings don't inherit it.
            if depth <= 1 {
                current_account = None;
            }
            // Defensive: don't dive negative on malformed input.
            if depth < 0 {
                return None;
            }
            continue;
        }

        // Parse a quoted token line. Two shapes matter:
        //   "key"                              → potential block name on next line
        //   "key"      "value"                 → key/value pair
        let tokens = parse_quoted_tokens(line);
        match tokens.len() {
            1 => {
                pending_key_at_depth.push(Some(tokens.into_iter().next().unwrap()));
            }
            2 => {
                let mut it = tokens.into_iter();
                let k = it.next().unwrap();
                let v = it.next().unwrap();
                if k == "MostRecent" && v == "1" {
                    if let Some(acc) = &current_account {
                        // Found it — but keep scanning in case multiple
                        // blocks claim "1" (last one wins). In well-formed
                        // files there's exactly one.
                        found = Some(acc.clone());
                    }
                }
            }
            _ => { /* malformed-ish line, skip */ }
        }
    }

    if depth != 0 {
        // Truncated braces — refuse to guess.
        return None;
    }
    found
}

/// Epic AccountIds are UUIDs without dashes — exactly 32 hex characters,
/// lowercase. Anything else in `Saved\Data\` (e.g. `OC_<id>.dat` cache
/// shadows, `Staged\` subdirs) is noise we don't want to feed into the
/// candidate list.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn is_epic_account_id(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// SteamID64s are 17-digit decimals starting with `7656119`. Keep the check
/// loose enough to tolerate older / newer ranges but tight enough that a
/// random key like `"AccountName"` won't pass.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn is_steamid64(s: &str) -> bool {
    s.len() == 17 && s.starts_with("7656") && s.chars().all(|c| c.is_ascii_digit())
}

/// Pull `"..."`-quoted tokens from a line. Backslash-escapes are passed
/// through as-is; we never need them for the keys we care about.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_quoted_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                // Allow escaped quotes — though Valve doesn't emit them in
                // loginusers.vdf, being lenient costs nothing.
                if bytes[j] == b'\\' && j + 1 < bytes.len() {
                    j += 2;
                    continue;
                }
                j += 1;
            }
            if j >= bytes.len() {
                // Unterminated string → bail; caller treats as malformed.
                return Vec::new();
            }
            let token = std::str::from_utf8(&bytes[start..j])
                .unwrap_or("")
                .to_string();
            out.push(token);
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_ACCOUNT_VDF: &str = r#"
"users"
{
    "76561198000000001"
    {
        "AccountName"       "old"
        "PersonaName"       "Old"
        "RememberPassword"  "1"
        "MostRecent"        "0"
        "Timestamp"         "1500000000"
    }
    "76561198000000002"
    {
        "AccountName"       "new"
        "PersonaName"       "New"
        "RememberPassword"  "1"
        "MostRecent"        "1"
        "Timestamp"         "1700000000"
    }
}
"#;

    #[test]
    fn parses_steam_id_from_loginusers_vdf() {
        let id = parse_most_recent_steamid64(TWO_ACCOUNT_VDF);
        assert_eq!(id.as_deref(), Some("76561198000000002"));
    }

    #[test]
    fn picks_most_recent_when_multiple_blocks() {
        // Same fixture; the older block has MostRecent=0, the newer one has 1.
        let id = parse_most_recent_steamid64(TWO_ACCOUNT_VDF);
        assert_eq!(id.as_deref(), Some("76561198000000002"));
    }

    #[test]
    fn returns_none_on_malformed_vdf() {
        let truncated = r#"
"users"
{
    "76561198000000002"
    {
        "MostRecent"   "1"
"#;
        assert!(parse_most_recent_steamid64(truncated).is_none());
    }

    #[test]
    fn returns_none_when_no_most_recent() {
        let no_recent = r#"
"users"
{
    "76561198000000001"
    {
        "AccountName"   "a"
        "MostRecent"    "0"
    }
    "76561198000000002"
    {
        "AccountName"   "b"
        "MostRecent"    "0"
    }
}
"#;
        assert!(parse_most_recent_steamid64(no_recent).is_none());
    }

    /// Rejects a key that looks vaguely like a number but isn't a SteamID64,
    /// so noise-blocks don't get promoted.
    #[test]
    fn ignores_non_steamid64_keys() {
        let bogus = r#"
"users"
{
    "12345"
    {
        "MostRecent"   "1"
    }
}
"#;
        assert!(parse_most_recent_steamid64(bogus).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn epic_filename_extraction() {
        // Exercise the directory-scan helper directly with a synthetic dir.
        // We don't depend on `tempfile`: build a path under the OS temp dir
        // ourselves and tear it down explicitly. Rationale documented below.
        //
        // Adding a new `tempfile` dev-dependency for one test was deemed
        // higher friction than this hand-rolled tempdir, per the
        // implementation plan's leeway.
        use std::time::{SystemTime, UNIX_EPOCH};
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!("rlstats_epic_test_{unique}"));
        std::fs::create_dir_all(&base).unwrap();
        // Three .dat files, one .bak file, one nested directory.
        std::fs::write(base.join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.dat"), b"x").unwrap();
        std::fs::write(base.join("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.dat"), b"x").unwrap();
        // Real-world noise we observed in Saved\Data: OC_-prefixed shadows
        // and an upper-case-hex stem. Both must be filtered out.
        std::fs::write(base.join("OC_cccccccccccccccccccccccccccccccc.dat"), b"x").unwrap();
        std::fs::write(base.join("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.dat"), b"x").unwrap();
        std::fs::write(base.join("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz.bak"), b"x").unwrap();
        std::fs::create_dir_all(base.join("subdir")).unwrap();

        let mut ids = epic_account_ids_in(&base);
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ]
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
