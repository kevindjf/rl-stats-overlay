//! Best-effort detection of whether `RocketLeague.exe` is currently running.
//!
//! Used by [`crate::ws_client`] to short-circuit the kernel TCP keep-alive
//! probe loop when RL exits. RL's "Quit" button terminates the process without
//! sending FIN on the Stats API socket; Windows then takes ~8 s (10
//! hard-coded keep-alive probes since Vista — `socket2::TcpKeepalive::with_retries`
//! is Linux-only) to surface the dead connection. Polling the process list
//! once per second collapses that latency to <1 s without losing the
//! keep-alive belt-and-suspenders for crash / hang cases.
//!
//! On non-Windows targets the probe is a no-op that always returns `true` so
//! the existing TCP keep-alive path remains the sole disconnect signal —
//! the in-game HUD only ships on Windows anyway, the macOS/Linux dev builds
//! are debugging surfaces.
//!
//! Cost: a `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)` plus a
//! `Process32FirstW` / `Process32NextW` walk over a typical 200-500 process
//! list — sub-millisecond on a modern Windows host. Cheap enough to call
//! synchronously from a Tokio task without `spawn_blocking`.
//!
//! Failure mode: on snapshot error (rare — UAC / Defender / handle exhaustion)
//! we return `true` ("RL might be alive"). False negative is preferable to a
//! false positive flap that would tear down a healthy connection.

/// Process name we consider authoritative for "RL is running". Compared
/// case-insensitively against the `szExeFile` field of `PROCESSENTRY32W`.
///
/// Companion processes (`RocketLeague_BE.exe` BattlEye launcher,
/// `RocketLeagueOSS.exe` console build) are intentionally excluded — only
/// the main game process produces Stats API events.
pub(crate) const RL_PROCESS_NAME: &str = "RocketLeague.exe";

/// Public probe used by the WS task. Returns `true` if RL appears to be
/// running (or if we can't tell), `false` only when we have positively
/// confirmed it's gone.
pub(crate) fn rl_process_alive() -> bool {
    #[cfg(target_os = "windows")]
    {
        match enumerate_process_names() {
            Some(iter) => rl_process_alive_via(iter),
            None => true, // snapshot failed → assume alive, let TCP keep-alive arbitrate
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

/// Pure-logic core, factored out so unit tests can feed a synthetic iterator.
/// `RocketLeague.exe` matches; `RocketLeague_BE.exe`, prefix-only matches,
/// and case-shifted variants must all be handled correctly.
pub(crate) fn rl_process_alive_via<I, S>(names: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    names
        .into_iter()
        .any(|n| n.as_ref().eq_ignore_ascii_case(RL_PROCESS_NAME))
}

// ---------- Windows snapshot iterator ---------------------------------------

#[cfg(target_os = "windows")]
fn enumerate_process_names() -> Option<impl Iterator<Item = String>> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: `CreateToolhelp32Snapshot` is a synchronous kernel call that
    // returns a handle or `INVALID_HANDLE_VALUE`. No memory / lifetime
    // invariants on our side beyond closing the handle.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        return None;
    }

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    // SAFETY: `Process32FirstW` requires `entry.dwSize` to be set (done above).
    // Returns 0 (FALSE) if the snapshot is empty — extremely unlikely on a
    // live Windows session, but treat as "no RL" rather than panic.
    if unsafe { Process32FirstW(snapshot, &mut entry) } == 0 {
        unsafe { CloseHandle(snapshot) };
        return Some(Vec::<String>::new().into_iter());
    }

    let mut names = Vec::with_capacity(256);
    loop {
        names.push(decode_szexefile(&entry.szExeFile));
        // SAFETY: `entry` is a valid `PROCESSENTRY32W` with `dwSize` set,
        // and `snapshot` is a live handle (we own it until `CloseHandle`).
        if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
            break;
        }
    }
    // SAFETY: handle returned from `CreateToolhelp32Snapshot`, not yet closed.
    unsafe { CloseHandle(snapshot) };
    Some(names.into_iter())
}

/// Decode a UTF-16 NUL-terminated `szExeFile` field into an owned `String`.
/// Lossy decode keeps us robust against the rare process with a non-UTF-16
/// name (Windows accepts surrogate halves in some legacy code paths).
#[cfg(target_os = "windows")]
fn decode_szexefile(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_rocket_league_exe() {
        let snapshot = vec!["explorer.exe", "chrome.exe", "RocketLeague.exe", "code.exe"];
        assert!(rl_process_alive_via(snapshot));
    }

    #[test]
    fn matches_case_insensitively() {
        // RL itself spawns with the canonical case, but Windows is
        // case-insensitive on filenames and some snapshot tooling
        // normalises differently — be lenient.
        let snapshot = vec!["rocketleague.exe"];
        assert!(rl_process_alive_via(snapshot));
        let snapshot = vec!["ROCKETLEAGUE.EXE"];
        assert!(rl_process_alive_via(snapshot));
    }

    #[test]
    fn does_not_match_companion_or_substring_processes() {
        // BattlEye launcher and OSS console build live alongside RL but
        // aren't the Stats-API-producing process. Substring/prefix matches
        // (`RocketLeagueOSS.exe`, `MyRocketLeague.exe`) must NOT trigger.
        let snapshot = vec![
            "RocketLeague_BE.exe",
            "RocketLeagueOSS.exe",
            "MyRocketLeague.exe",
            "rocketleague",         // missing extension
            "RocketLeague.exe.bak", // suffix
        ];
        assert!(!rl_process_alive_via(snapshot));
    }

    #[test]
    fn empty_snapshot_returns_false() {
        let snapshot: Vec<&str> = vec![];
        assert!(!rl_process_alive_via(snapshot));
    }
}
