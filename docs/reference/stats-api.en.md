# Psyonix Stats API reference

The full reference content lives in the **French page** ([Stats API Psyonix](stats-api.md)), kept in its original Psyonix-style English-with-French-comments form. Translating the entire offline mirror would double the maintenance cost for a low-traffic reference doc, so we keep a single canonical version.

The reference covers:

- WebSocket connection details (`ws://localhost:49123`)
- `DefaultStatsAPI.ini` configuration knobs
- The full `UpdateState` tick payload schema
- Every event type schema: `BallHit`, `MatchCreated`, `MatchInitialized`, `MatchEnded`, `MatchDestroyed`, `GoalScored`, `StatfeedEvent`, `Replay*`, `PodiumStart`

If you spot a Psyonix-side change that breaks the cached version, the source URL + fetch date are in the file's header — re-fetch from <https://www.rocketleague.com/en/developer/stats-api> and regenerate.
