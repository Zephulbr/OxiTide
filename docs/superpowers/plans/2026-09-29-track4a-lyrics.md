# Track 4a Lyrics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Synced and unsynced lyrics in the expanded Now Playing view, fetched TIDAL-first with a silent lrclib fallback and cached per track.

**Architecture:** A `track_lyrics()` method on the TIDAL client hits the legacy `tracks/{id}/lyrics` endpoint; a `LyricsService` in `oxitide-app` owns a memory + disk cache (`cache/lyrics.json`) and the lrclib fallback (exact, then search); pure Rust parses LRC into timed lines; one `get_lyrics` Tauri command feeds a small Svelte store; `LyricsPanel` renders lines, highlights the active one from existing position events, and seeks on click.

**Tech Stack:** Rust (tokio, reqwest, serde, tracing, wiremock for tests), Tauri 2 commands, Svelte 5 runes, vitest, svelte-check.

**Spec:** `docs/superpowers/specs/2026-09-29-oxitide-track4a-lyrics-design.md` — read it with this plan; the research spike is `docs/superpowers/plans/notes/2026-09-29-track4-api-research.md`.

## Global Constraints

- No new auth scopes and no official `openapi.tidal.com` calls: only the legacy v1 endpoints on the existing token (spec L-D3, L-D8).
- All HTTP stays in Rust; the webview never fetches lyrics itself (spec L-D5).
- Tests never hit the live TIDAL or lrclib APIs; use `wiremock` (AGENTS.md).
- Never log or persist token values; lyrics responses carry no tokens, but keep the existing redaction habits.
- IPC arg names are camelCase from `invoke`, snake_case in Rust params; DTO field names go over IPC verbatim (AGENTS.md).
- Icons are inlined Lucide paths in `Icon.svelte`; no unicode glyphs (AGENTS.md).
- Components consume semantic tokens only (`--text`, `--text-muted`, `--line`, `--accent`, `--surface-float`, `--dur-view`, `--ease-in`); no mode branching (AGENTS.md).
- Reduced motion (`appearance.reducedMotion`) disables smooth scrolling.
- lrclib requests send `User-Agent: OxiTide/<version>` and honor `429` + `Retry-After`.
- Manual verification commands: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- Commit style follows `git log` conventions: `feat(tidal): …`, `feat(app): …`, `feat(ui): …`, `test: …`.

## Review Focus

1. TIDAL returns `subtitles` and `lyrics` as present-but-blank strings — must fall through to lrclib, never render an empty lyrics panel (Task 3 test `empty_tidal_lyrics_fall_back_to_lrclib`).
2. Unsynced-only lyrics (plain text, no timestamps) — plain scrollable text: no highlight, no auto-scroll, no click-to-seek (Task 6 `activeLine` untimed case; Task 8 renders them non-tappable).
3. LRC with `[offset:±ms]` and multiple stamps on one line — timestamps must shift/expand or sync drifts (Task 2 tests).
4. Rapid track skipping while Now Playing is open — a late response for an abandoned track must never render under the new track (Task 7 stale-response test).
5. lrclib 404 on exact and search — the miss persists as a negative cache entry; later calls and restarts make no HTTP (Task 3 `confirmed_miss_is_negative_cached` and `disk_cache_survives_a_restart`).

---

### Task 1: Tidal crate — legacy lyrics fetch

**Files:**
- Create: `crates/oxitide-tidal/src/lyrics.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs` (module list)

**Interfaces:**
- Consumes: `TidalClient::get` (`crates/oxitide-tidal/src/client.rs:87`), `Error::NotFound`.
- Produces: `oxitide_tidal::lyrics::TidalLyrics { track_id: u64, lyrics: String, subtitles: String, provider: String, right_to_left: bool }` and `TidalClient::track_lyrics(&self, track_id: u64) -> Result<Option<TidalLyrics>>` (404 → `Ok(None)`).

- [ ] **Step 1: Create the module with a stub and failing tests**

Create `crates/oxitide-tidal/src/lyrics.rs`:

```rust
//! Legacy (v1) track lyrics — synced `subtitles` and plain `lyrics` text.
//! The official JSON:API lyrics resource is INTERNAL-tier, so the legacy
//! endpoint is the TIDAL source (Track 4a spec §5.1, §7).

use serde::Deserialize;

use crate::client::TidalClient;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TidalLyrics {
    #[serde(default, rename = "trackId")]
    pub track_id: u64,
    /// Plain lyrics, newline separated.
    #[serde(default)]
    pub lyrics: String,
    /// Synced LRC lyrics; empty when the track has only plain text.
    #[serde(default)]
    pub subtitles: String,
    #[serde(default, rename = "lyricsProvider")]
    pub provider: String,
    #[serde(default, rename = "isRightToLeft")]
    pub right_to_left: bool,
}

impl TidalClient {
    /// Fetch a track's lyrics. A 404 means "no lyrics for this track", not an
    /// error; everything else propagates.
    pub async fn track_lyrics(&self, track_id: u64) -> Result<Option<TidalLyrics>> {
        let _ = track_id;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn track_lyrics_parses_the_legacy_payload() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .and(query_param("countryCode", "GR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7,
                "lyrics": "line one\nline two",
                "subtitles": "[00:01.00] line one\n[00:02.50] line two",
                "lyricsProvider": "TIDAL",
                "isRightToLeft": false
            })))
            .mount(&server).await;
        let lyrics = client(&server).track_lyrics(7).await.unwrap().unwrap();
        assert_eq!(lyrics.track_id, 7);
        assert_eq!(lyrics.lyrics, "line one\nline two");
        assert!(lyrics.subtitles.contains("[00:01.00]"));
        assert_eq!(lyrics.provider, "TIDAL");
        assert!(!lyrics.right_to_left);
    }

    #[tokio::test]
    async fn track_lyrics_maps_404_to_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/8/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server).await;
        assert_eq!(client(&server).track_lyrics(8).await.unwrap(), None);
    }

    #[tokio::test]
    async fn track_lyrics_propagates_other_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/9/lyrics"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server).await;
        let err = client(&server).track_lyrics(9).await.unwrap_err();
        assert!(matches!(err, Error::Api { status: 500, .. }), "got {err:?}");
    }
}
```

Add `pub mod lyrics;` to `crates/oxitide-tidal/src/lib.rs` (after `pub mod library;`).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal lyrics`
Expected: FAIL — `track_lyrics_parses_the_legacy_payload` asserts on a `None`/default stub (`attempt to unwrap a None value` or field mismatch).

- [ ] **Step 3: Implement the fetch**

Replace the stub `track_lyrics` body with:

```rust
        let path = format!("tracks/{track_id}/lyrics");
        match self.get::<TidalLyrics>(&path, &[]).await {
            Ok(lyrics) => Ok(Some(lyrics)),
            Err(Error::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
```

Delete the `let _ = track_id;` line.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal lyrics`
Expected: 3 passed.

- [ ] **Step 5: Lint and commit**

```bash
cargo fmt
cargo clippy -p oxitide-tidal -- -D warnings
git add crates/oxitide-tidal/src/lyrics.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): fetch legacy track lyrics"
```

---

### Task 2: Pure LRC parser

**Files:**
- Create: `crates/oxitide-app/src/lrc.rs`
- Modify: `crates/oxitide-app/src/main.rs` (module list, line 3-10)

**Interfaces:**
- Produces: `crate::lrc::LyricLine { time_ms: Option<u64>, text: String }` (serde Serialize/Deserialize), `parse_lrc(&str) -> Vec<LyricLine>`, `split_plain(&str) -> Vec<LyricLine>`.

File-structure note: spec §5.3 says "pure function(s) in the same module"; the parser lives in its own `lrc.rs` so `lyrics.rs` (service + cache) stays focused, mirroring the existing `palette.rs`/`artwork.rs` split.

- [ ] **Step 1: Create the module with failing tests**

Add `mod lrc;` to the module list in `crates/oxitide-app/src/main.rs` (keep the alphabetical order: `artwork`, `commands`, `config`, `lrc`, `palette`, `player`, `scheme`, `state`, `window` — `lyrics` joins in Task 3).

Create `crates/oxitide-app/src/lrc.rs`:

```rust
//! Pure LRC parsing (Track 4a spec §5.3, §7). No I/O: every branch is
//! unit-testable.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// Milliseconds from track start; `None` for plain (unsynced) lyrics.
    pub time_ms: Option<u64>,
    pub text: String,
}

/// Parse an LRC document. Multiple stamps on one line expand into one line per
/// stamp; `[offset:±ms]` shifts every stamp; `[mm:ss.xx]`/`[mm:ss.xxx]` are both
/// accepted; metadata tags and malformed lines are skipped; blank timed lines
/// are kept as gaps.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    todo!()
}

/// Split plain lyrics into untimed lines, dropping trailing empty lines.
pub fn split_plain(text: &str) -> Vec<LyricLine> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_centisecond_stamps() {
        let lines = parse_lrc("[00:27.93] Listen\n[00:30.88] Watch");
        assert_eq!(lines, vec![
            LyricLine { time_ms: Some(27_930), text: "Listen".into() },
            LyricLine { time_ms: Some(30_880), text: "Watch".into() },
        ]);
    }

    #[test]
    fn parses_millisecond_stamps() {
        let lines = parse_lrc("[01:02.345] Hi");
        assert_eq!(lines[0].time_ms, Some(62_345));
    }

    #[test]
    fn expands_multiple_stamps_on_one_line() {
        let lines = parse_lrc("[00:10.00][01:20.00] same");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, Some(10_000));
        assert_eq!(lines[1].time_ms, Some(80_000));
        assert_eq!(lines[0].text, "same");
    }

    #[test]
    fn applies_offset_tags() {
        let lines = parse_lrc("[offset:-500]\n[00:01.00] a");
        assert_eq!(lines[0].time_ms, Some(500));
    }

    #[test]
    fn ignores_metadata_and_malformed_lines() {
        let lines = parse_lrc("[ar:Artist]\n[ti:Title]\nnot timed\n[zz:xx] nope\n[00:02.00] ok");
        assert_eq!(lines, vec![LyricLine { time_ms: Some(2_000), text: "ok".into() }]);
    }

    #[test]
    fn keeps_blank_timed_lines() {
        let lines = parse_lrc("[00:01.00] a\n[00:02.00]\n[00:03.00] b");
        assert_eq!(lines[1], LyricLine { time_ms: Some(2_000), text: String::new() });
    }

    #[test]
    fn split_plain_drops_trailing_empties() {
        let lines = split_plain("one\ntwo\n\n");
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.time_ms.is_none()));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app lrc`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement the parser**

```rust
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let offset = offset_ms(text);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let (stamps, body) = split_tags(raw);
        if stamps.is_empty() {
            continue; // metadata or a bare, untimed line
        }
        let body = body.trim_end_matches('\r').to_string();
        for stamp in stamps {
            let shifted = stamp.saturating_add(offset).max(0) as u64;
            lines.push(LyricLine { time_ms: Some(shifted), text: body.clone() });
        }
    }
    lines
}

/// Split leading `[...]` tags off a line, returning the timed stamps (ms) and
/// the remaining text.
fn split_tags(line: &str) -> (Vec<i64>, String) {
    let mut stamps = Vec::new();
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('[') {
            rest = trimmed;
            break;
        }
        let Some(end) = trimmed.find(']') else {
            rest = trimmed;
            break;
        };
        if let Some(ms) = parse_stamp(&trimmed[1..end]) {
            stamps.push(ms);
        }
        rest = &trimmed[end + 1..];
    }
    (stamps, rest.to_string())
}

/// `mm:ss(.xx|.xxx)` or `hh:mm:ss(.xx|.xxx)` → milliseconds.
fn parse_stamp(tag: &str) -> Option<i64> {
    let parts: Vec<&str> = tag.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [m, s] => (0i64, m.parse::<i64>().ok()?, s.parse::<f64>().ok()?),
        [h, m, s] => (h.parse::<i64>().ok()?, m.parse::<i64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    Some(hours * 3_600_000 + minutes * 60_000 + (seconds * 1000.0).round() as i64)
}

/// The last `[offset:±ms]` tag in the document, if any.
fn offset_ms(text: &str) -> i64 {
    let mut offset = 0;
    for line in text.lines() {
        let mut rest = line.trim_start();
        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            if let Some(value) = rest[1..end].strip_prefix("offset:") {
                offset = value.trim().parse::<i64>().unwrap_or(offset);
            }
            rest = &rest[end + 1..];
        }
    }
    offset
}

pub fn split_plain(text: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = text
        .lines()
        .map(|line| LyricLine { time_ms: None, text: line.trim_end_matches('\r').to_string() })
        .collect();
    while lines.last().is_some_and(|line| line.text.trim().is_empty()) {
        lines.pop();
    }
    lines
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app lrc`
Expected: 7 passed.

- [ ] **Step 5: Lint and commit**

```bash
cargo fmt
cargo clippy -p oxitide-app -- -D warnings
git add crates/oxitide-app/src/lrc.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add a pure LRC parser"
```

---

### Task 3: Lyrics service — cache and lrclib fallback

**Files:**
- Create: `crates/oxitide-app/src/lyrics.rs`

**Interfaces:**
- Consumes: `oxitide_tidal::lyrics::TidalLyrics`, `TidalClient::track_lyrics`, `crate::lrc::{parse_lrc, split_plain, LyricLine}`.
- Produces:
  - `crate::lyrics::LyricsDto { track_id: u64, source: String, synced: bool, rtl: bool, lines: Vec<LyricLine> }` (Serialize/Deserialize/PartialEq/Clone)
  - `crate::lyrics::LyricsRequest { track_id: u64, title: String, artist: String, album: Option<String>, duration_secs: u64 }`
  - `LyricsService::new(http: reqwest::Client, lrclib_url: impl Into<String>, disk_path: PathBuf) -> Self`
  - `LyricsService::get(&self, client: &TidalClient, req: &LyricsRequest) -> Option<LyricsDto>` (never errors; confirmed misses persist as `None` entries; transient failures are not cached)

- [ ] **Step 1: Create the service with a minimal stub and failing tests**

Create `crates/oxitide-app/src/lyrics.rs` (service stub plus the full test module). Add `mod lyrics;` to `crates/oxitide-app/src/main.rs` now (alphabetical: after `lrc`), or the test run below cannot see the module.

```rust
//! Lyrics fetch + cache: TIDAL legacy first, lrclib fallback (Track 4a spec
//! §5.2, §7, §8). Only confirmed misses are negative-cached; transient
//! failures return `None` for the attempt and stay retryable.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use oxitide_tidal::client::TidalClient;
use oxitide_tidal::lyrics::TidalLyrics;

use crate::lrc::{parse_lrc, split_plain, LyricLine};

const CACHE_MAX_ENTRIES: usize = 2_000;
const DURATION_TOLERANCE_SECS: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsDto {
    pub track_id: u64,
    pub source: String, // "tidal" | "lrclib"
    pub synced: bool,
    pub rtl: bool,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricsRequest {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    fetched_at: u64,
    lyrics: Option<LyricsDto>,
}

#[derive(Default, Serialize, Deserialize)]
struct DiskCache {
    tracks: HashMap<u64, CacheEntry>,
}

/// The lrclib record fields this feature reads.
#[derive(Debug, Deserialize)]
struct LrclibRecord {
    #[serde(default, rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(default, rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    duration: Option<f64>,
}

enum LyricsOutcome {
    Found(LyricsDto),
    Missing,
    Failed,
}

pub struct LyricsService {
    http: reqwest::Client,
    base_url: String,
    memory: Mutex<HashMap<u64, CacheEntry>>,
    disk_path: PathBuf,
}

impl LyricsService {
    pub fn new(http: reqwest::Client, lrclib_url: impl Into<String>, disk_path: PathBuf) -> Self {
        if let Some(parent) = disk_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let memory = std::fs::read_to_string(&disk_path)
            .ok()
            .and_then(|text| serde_json::from_str::<DiskCache>(&text).ok())
            .map(|cache| cache.tracks)
            .unwrap_or_default();
        Self {
            http,
            base_url: lrclib_url.into().trim_end_matches('/').to_string(),
            memory: Mutex::new(memory),
            disk_path,
        }
    }

    pub async fn get(&self, client: &TidalClient, req: &LyricsRequest) -> Option<LyricsDto> {
        let _ = (client, req);
        None
    }

    fn save_disk(&self) {}
    fn store(&self, _track_id: u64, _lyrics: Option<LyricsDto>) {}
}
```

Append the test module at the bottom of the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_tidal::auth::SessionInfo;
    use oxitide_tidal::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn tidal_client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    fn request(track_id: u64) -> LyricsRequest {
        LyricsRequest {
            track_id,
            title: "The Chain".into(),
            artist: "Fleetwood Mac".into(),
            album: Some("Rumours".into()),
            duration_secs: 271,
        }
    }

    fn synced_body(text: &str) -> serde_json::Value {
        serde_json::json!({
            "syncedLyrics": text,
            "plainLyrics": "plain fallback",
            "instrumental": false,
            "duration": 271
        })
    }

    fn temp_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "oxitide-lyrics-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn tidal_hit_is_parsed_and_cached() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7,
                "lyrics": "plain",
                "subtitles": "[00:01.00] synced line",
                "lyricsProvider": "TIDAL",
                "isRightToLeft": false
            })))
            .expect(1)
            .mount(&tidal).await;
        let lrclib = MockServer::start().await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let client = tidal_client(&tidal);
        let first = service.get(&client, &request(7)).await.unwrap();
        let second = service.get(&client, &request(7)).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(first.source, "tidal");
        assert!(first.synced);
        assert_eq!(first.lines[0].time_ms, Some(1_000));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn empty_tidal_lyrics_fall_back_to_lrclib() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7, "lyrics": "", "subtitles": ""
            })))
            .mount(&tidal).await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/get"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(synced_body("[00:02.00] from lrclib")))
            .mount(&lrclib).await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let dto = service.get(&tidal_client(&tidal), &request(7)).await.unwrap();
        assert_eq!(dto.source, "lrclib");
        assert_eq!(dto.lines[0].text, "from lrclib");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn tidal_error_falls_through_to_lrclib() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&tidal).await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(synced_body("[00:01.00] hi")))
            .mount(&lrclib).await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let dto = service.get(&tidal_client(&tidal), &request(7)).await.unwrap();
        assert_eq!(dto.source, "lrclib");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn exact_miss_searches_and_filters_by_duration() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&tidal).await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/get"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&lrclib).await;
        Mock::given(method("GET")).and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"syncedLyrics": "[00:01.00] wrong", "instrumental": false, "duration": 100.0},
                {"syncedLyrics": "[00:02.00] right", "instrumental": false, "duration": 201.0}
            ])))
            .mount(&lrclib).await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let dto = service.get(&tidal_client(&tidal), &request(7)).await.unwrap();
        assert_eq!(dto.lines[0].text, "right");
        assert_eq!(dto.lines[0].time_ms, Some(2_000));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn instrumental_exact_record_is_a_confirmed_miss() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&tidal).await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "syncedLyrics": null, "plainLyrics": null, "instrumental": true, "duration": 271
            })))
            .mount(&lrclib).await;
        Mock::given(method("GET")).and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([synced_body("[00:01.00] good")])))
            .mount(&lrclib).await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        // A search hit would return lyrics; an instrumental exact match must not search.
        assert_eq!(service.get(&tidal_client(&tidal), &request(7)).await, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn confirmed_miss_is_negative_cached() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404)).expect(1).mount(&tidal).await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/get"))
            .respond_with(ResponseTemplate::new(404)).expect(1).mount(&lrclib).await;
        Mock::given(method("GET")).and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1).mount(&lrclib).await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let client = tidal_client(&tidal);
        assert_eq!(service.get(&client, &request(7)).await, None);
        assert_eq!(service.get(&client, &request(7)).await, None);
        drop(tidal);
        drop(lrclib);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn lrclib_failure_is_not_cached() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404)).expect(2).mount(&tidal).await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/get"))
            .respond_with(ResponseTemplate::new(500)).expect(2).mount(&lrclib).await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let client = tidal_client(&tidal);
        assert_eq!(service.get(&client, &request(7)).await, None);
        assert_eq!(service.get(&client, &request(7)).await, None);
        drop(tidal);
        drop(lrclib);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn lrclib_429_retries_once_after_retry_after() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404)).mount(&tidal).await;
        let lrclib = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));
        let responder = {
            let attempts = attempts.clone();
            move |_req: &wiremock::Request| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(429).insert_header("retry-after", "0")
                } else {
                    ResponseTemplate::new(200).set_body_json(synced_body("[00:01.00] hello"))
                }
            }
        };
        Mock::given(method("GET")).and(path("/api/get"))
            .respond_with(responder).expect(2).mount(&lrclib).await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let dto = service.get(&tidal_client(&tidal), &request(7)).await.unwrap();
        assert_eq!(dto.source, "lrclib");
        assert_eq!(dto.lines[0].time_ms, Some(1_000));
        drop(lrclib);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn disk_cache_survives_a_restart() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7, "lyrics": "", "subtitles": "[00:01.00] synced"
            })))
            .expect(1)
            .mount(&tidal).await;
        let lrclib = MockServer::start().await;
        let dir = temp_dir();
        let client = tidal_client(&tidal);
        let first_service =
            LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let first = first_service.get(&client, &request(7)).await.unwrap();
        let second_service =
            LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let second = second_service.get(&client, &request(7)).await.unwrap();
        assert_eq!(first, second);
        drop(tidal);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn pruning_keeps_the_newest_entries() {
        let lrclib = MockServer::start().await;
        let dir = temp_dir();
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        {
            let mut memory = service.memory.lock().unwrap();
            for id in 0..=2_000u64 {
                memory.insert(id, CacheEntry { fetched_at: id, lyrics: None });
            }
        }
        service.save_disk();
        let reloaded =
            LyricsService::new(reqwest::Client::new(), lrclib.uri(), dir.join("lyrics.json"));
        let memory = reloaded.memory.lock().unwrap();
        assert_eq!(memory.len(), 2_000);
        assert!(!memory.contains_key(&0));
        assert!(memory.contains_key(&2_000));
        drop(memory);
        let _ = std::fs::remove_dir_all(dir);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app lyrics`
Expected: FAIL — the stub returns `None`; `unwrap()` panics or expectations go unmet.

- [ ] **Step 3: Implement the service**

Replace the stub `get`, `store`, and `save_disk`, and add the helpers:

```rust
    pub async fn get(&self, client: &TidalClient, req: &LyricsRequest) -> Option<LyricsDto> {
        if let Some(entry) = self.memory.lock().unwrap().get(&req.track_id) {
            return entry.lyrics.clone();
        }
        match self.fetch(client, req).await {
            LyricsOutcome::Found(dto) => {
                self.store(req.track_id, Some(dto.clone()));
                Some(dto)
            }
            LyricsOutcome::Missing => {
                self.store(req.track_id, None);
                None
            }
            LyricsOutcome::Failed => None, // transient: retry on a later attempt
        }
    }

    async fn fetch(&self, client: &TidalClient, req: &LyricsRequest) -> LyricsOutcome {
        match client.track_lyrics(req.track_id).await {
            Ok(Some(raw)) => {
                if let Some(dto) = dto_from_tidal(req.track_id, raw) {
                    return LyricsOutcome::Found(dto);
                }
            }
            Ok(None) => {}
            Err(error) => tracing::warn!("tidal lyrics failed for {}: {error}", req.track_id),
        }
        self.lrclib_exact(req).await
    }

    async fn lrclib_exact(&self, req: &LyricsRequest) -> LyricsOutcome {
        let mut params: Vec<(&str, String)> = vec![
            ("track_name", req.title.clone()),
            ("artist_name", req.artist.clone()),
            ("duration", req.duration_secs.to_string()),
        ];
        if let Some(album) = req.album.as_deref().filter(|album| !album.is_empty()) {
            params.push(("album_name", album.to_string()));
        }
        match self.get_json::<LrclibRecord>("api/get", &params).await {
            Ok(Some(record)) => match dto_from_lrclib(req.track_id, record) {
                Some(dto) => LyricsOutcome::Found(dto),
                None => LyricsOutcome::Missing,
            },
            Ok(None) => self.lrclib_search(req).await,
            Err(()) => LyricsOutcome::Failed,
        }
    }

    async fn lrclib_search(&self, req: &LyricsRequest) -> LyricsOutcome {
        let params = vec![
            ("track_name", req.title.clone()),
            ("artist_name", req.artist.clone()),
        ];
        match self.get_json::<Vec<LrclibRecord>>("api/search", &params).await {
            Ok(Some(records)) => {
                let want = req.duration_secs as f64;
                let found = records.into_iter().find(|record| {
                    !record.instrumental
                        && record
                            .duration
                            .map(|duration| (duration - want).abs() <= DURATION_TOLERANCE_SECS)
                            .unwrap_or(false)
                });
                match found.and_then(|record| dto_from_lrclib(req.track_id, record)) {
                    Some(dto) => LyricsOutcome::Found(dto),
                    None => LyricsOutcome::Missing,
                }
            }
            Ok(None) => LyricsOutcome::Missing,
            Err(()) => LyricsOutcome::Failed,
        }
    }

    /// GET a JSON body, retrying once after a `429`'s `Retry-After`.
    /// `Ok(None)` is a 404; `Err(())` is any other failure.
    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<Option<T>, ()> {
        for attempt in 0..2 {
            let response = self
                .http
                .get(format!("{}/{}", self.base_url, path))
                .header(reqwest::header::USER_AGENT, concat!("OxiTide/", env!("CARGO_PKG_VERSION")))
                .query(params)
                .send()
                .await
                .map_err(|error| {
                    tracing::warn!("lrclib request failed: {error}");
                })?;
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt == 0 {
                tokio::time::sleep(retry_after(&response)).await;
                continue;
            }
            let status = response.status();
            if status.is_success() {
                return response.json::<T>().await.map(Some).map_err(|error| {
                    tracing::warn!("lrclib decode failed: {error}");
                });
            }
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            tracing::warn!("lrclib returned {status}");
            return Err(());
        }
        Err(())
    }

    fn store(&self, track_id: u64, lyrics: Option<LyricsDto>) {
        let fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        self.memory
            .lock()
            .unwrap()
            .insert(track_id, CacheEntry { fetched_at, lyrics });
        self.save_disk();
    }

    fn save_disk(&self) {
        let cache = {
            let mut memory = self.memory.lock().unwrap();
            if memory.len() > CACHE_MAX_ENTRIES {
                let mut entries: Vec<(u64, CacheEntry)> = memory.drain().collect();
                entries.sort_by(|a, b| b.1.fetched_at.cmp(&a.1.fetched_at));
                entries.truncate(CACHE_MAX_ENTRIES);
                memory.extend(entries);
            }
            DiskCache { tracks: memory.clone() }
        };
        let Ok(text) = serde_json::to_string(&cache) else { return };
        let tmp = self.disk_path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &self.disk_path);
        }
    }
```

Add these free functions at the bottom of the file (above the tests):

```rust
fn dto_from_tidal(track_id: u64, raw: TidalLyrics) -> Option<LyricsDto> {
    let rtl = raw.right_to_left;
    if !raw.subtitles.trim().is_empty() {
        let lines = parse_lrc(&raw.subtitles);
        if !lines.is_empty() {
            return Some(LyricsDto { track_id, source: "tidal".into(), synced: true, rtl, lines });
        }
    }
    if !raw.lyrics.trim().is_empty() {
        let lines = split_plain(&raw.lyrics);
        if !lines.is_empty() {
            return Some(LyricsDto { track_id, source: "tidal".into(), synced: false, rtl, lines });
        }
    }
    None
}

fn dto_from_lrclib(track_id: u64, record: LrclibRecord) -> Option<LyricsDto> {
    if record.instrumental {
        return None;
    }
    if let Some(synced) = record.synced_lyrics.as_deref() {
        let lines = parse_lrc(synced);
        if !lines.is_empty() {
            return Some(LyricsDto {
                track_id, source: "lrclib".into(), synced: true, rtl: false, lines,
            });
        }
    }
    let plain = record.plain_lyrics.as_deref()?;
    if plain.trim().is_empty() {
        return None;
    }
    Some(LyricsDto {
        track_id, source: "lrclib".into(), synced: false, rtl: false,
        lines: split_plain(plain),
    })
}

fn retry_after(response: &reqwest::Response) -> std::time::Duration {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|secs| std::time::Duration::from_secs(secs.min(5)))
        .unwrap_or(std::time::Duration::from_secs(1))
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app lyrics`
Expected: 11 passed.

- [ ] **Step 5: Lint and commit**

```bash
cargo fmt
cargo clippy -p oxitide-app -- -D warnings
git add crates/oxitide-app/src/lyrics.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add the lyrics service with lrclib fallback"
```

---

### Task 4: IPC wiring — state, command, startup

**Files:**
- Modify: `crates/oxitide-app/src/state.rs` (imports, `AppState`, accessors)
- Modify: `crates/oxitide-app/src/commands.rs` (imports + new command)
- Modify: `crates/oxitide-app/src/main.rs` (service construction + handler list)

**Interfaces:**
- Consumes: `LyricsService`, `LyricsRequest`, `LyricsDto` (Task 3); `require_client` (`commands.rs:297`).
- Produces: Tauri command `get_lyrics(track_id: u64, title: String, artist: String, album: Option<String>, duration_secs: u64) -> Result<Option<LyricsDto>, String>`; `AppState::lyrics()` / `set_lyrics()`.

- [ ] **Step 1: Add the state accessors**

In `crates/oxitide-app/src/state.rs`, add to `AppState` (after `pub artwork`):

```rust
    pub lyrics: Mutex<Option<Arc<LyricsService>>>,
```

Add the import:

```rust
use crate::lyrics::LyricsService;
```

And the accessors after the artwork ones:

```rust
    pub fn set_lyrics(&self, service: LyricsService) {
        *self.lyrics.lock().unwrap() = Some(Arc::new(service));
    }

    pub fn lyrics(&self) -> Option<Arc<LyricsService>> {
        self.lyrics.lock().unwrap().clone()
    }
```

- [ ] **Step 2: Add the command**

In `crates/oxitide-app/src/commands.rs`, extend the `crate::` imports (line ~19-24):

```rust
use crate::lyrics::{LyricsDto, LyricsRequest};
```

Add after `get_feed` (line ~379):

```rust
#[tauri::command]
pub async fn get_lyrics(
    state: State<'_, AppState>,
    track_id: u64,
    title: String,
    artist: String,
    album: Option<String>,
    duration_secs: u64,
) -> Result<Option<LyricsDto>, String> {
    let service = state.lyrics().ok_or_else(|| "lyrics unavailable".to_string())?;
    let client = require_client(&state)?;
    let req = LyricsRequest { track_id, title, artist, album, duration_secs };
    Ok(service.get(&client, &req).await)
}
```

- [ ] **Step 3: Construct the service at startup and register the command**

In `crates/oxitide-app/src/main.rs`, after `state.set_artwork(...)` (line ~126-130):

```rust
    state.set_lyrics(lyrics::LyricsService::new(
        reqwest::Client::new(),
        "https://lrclib.net",
        config::default_cache_dir().unwrap_or_default().join("lyrics.json"),
    ));
```

Add `commands::get_lyrics,` to the `generate_handler![]` list (after `commands::get_mix,`).

- [ ] **Step 4: Verify the workspace compiles and all tests pass**

Run: `cargo test --workspace`
Expected: all pass (no new tests here; this catches wiring mistakes).
Run: `cargo clippy --workspace -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose the get_lyrics command"
```

---

### Task 5: UI API contract

**Files:**
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Produces: `LyricLine`, `LyricsDto` interfaces and `api.getLyrics(trackId, title, artist, album, durationSecs): Promise<LyricsDto | null>`.

- [ ] **Step 1: Add the interfaces and wrapper**

In `ui/src/lib/api.ts`, after `MixDetail` (line 42):

```ts
export interface LyricLine { time_ms: number | null; text: string }
export interface LyricsDto {
  track_id: number;
  source: "tidal" | "lrclib";
  synced: boolean;
  rtl: boolean;
  lines: LyricLine[];
}
```

In the `api` object, after `getMix` (line 148):

```ts
  getLyrics: (trackId: number, title: string, artist: string, album: string, durationSecs: number) =>
    invoke<LyricsDto | null>("get_lyrics", { trackId, title, artist, album, durationSecs }),
```

- [ ] **Step 2: Type-check**

Run: `npm --prefix ui run check`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add ui/src/lib/api.ts
git commit -m "feat(ui): add the lyrics API contract"
```

---

### Task 6: Pure active-line helper

**Files:**
- Create: `ui/src/lib/lyrics.ts`
- Create: `ui/src/lib/lyrics.test.ts`

**Interfaces:**
- Consumes: `LyricLine` (Task 5).
- Produces: `activeLine(lines: LyricLine[], positionMs: number, lookaheadMs?: number): number` (index or -1) and `LYRICS_LOOKAHEAD_MS = 150`.

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/lyrics.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { LyricLine } from "./api";
import { activeLine } from "./lyrics";

const sync = (...times: number[]): LyricLine[] =>
  times.map((time, index) => ({ time_ms: time, text: `line ${index}` }));

describe("activeLine", () => {
  const lines = sync(1000, 5000, 9000);

  it("is -1 before the first line", () => {
    expect(activeLine(lines, 0)).toBe(-1);
  });

  it("picks the latest line at or before the threshold", () => {
    expect(activeLine(lines, 6000)).toBe(1);
  });

  it("counts a line exactly at its timestamp", () => {
    expect(activeLine(lines, 5000, 0)).toBe(1);
  });

  it("applies the lookahead window", () => {
    expect(activeLine(lines, 4900)).toBe(1);
    expect(activeLine(lines, 4800)).toBe(0);
  });

  it("skips blank timed lines", () => {
    const spaced: LyricLine[] = [
      { time_ms: 0, text: "a" },
      { time_ms: 1000, text: "   " },
    ];
    expect(activeLine(spaced, 2000)).toBe(0);
  });

  it("returns -1 when there are no timed lines", () => {
    expect(activeLine([{ time_ms: null, text: "plain" }], 10_000)).toBe(-1);
    expect(activeLine([], 10_000)).toBe(-1);
  });

  it("tolerates out-of-order timestamps", () => {
    const shuffled: LyricLine[] = [sync(5000)[0], sync(1000)[0]];
    expect(activeLine(shuffled, 6000)).toBe(0);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/lyrics.test.ts`
Expected: FAIL — cannot resolve `./lyrics`.

- [ ] **Step 3: Implement the helper**

Create `ui/src/lib/lyrics.ts`:

```ts
import type { LyricLine } from "./api";

/// How early a line highlights, compensating for position-event cadence.
export const LYRICS_LOOKAHEAD_MS = 150;

/// Index of the line that should be highlighted at `positionMs`, or -1 when no
/// line is active yet. Only timed, non-blank lines can be active; timestamps may
/// arrive out of order, so the latest qualifying line by time wins.
export function activeLine(
  lines: LyricLine[],
  positionMs: number,
  lookaheadMs = LYRICS_LOOKAHEAD_MS,
): number {
  const threshold = positionMs + lookaheadMs;
  let active = -1;
  let best = -Infinity;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.time_ms === null || line.text.trim() === "") continue;
    if (line.time_ms <= threshold && line.time_ms >= best) {
      best = line.time_ms;
      active = i;
    }
  }
  return active;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/lyrics.test.ts`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/lyrics.ts ui/src/lib/lyrics.test.ts
git commit -m "feat(ui): add the active-line helper"
```

---

### Task 7: Lyrics store

**Files:**
- Create: `ui/src/lib/lyrics.svelte.ts`
- Create: `ui/src/lib/lyricsStore.test.ts`

**Interfaces:**
- Consumes: `api.getLyrics` (Task 5), `LyricsDto`, `QueueItem`.
- Produces: `createLyrics(): LyricsStore` and singleton `lyrics`, where

```ts
interface LyricsStore {
  readonly loading: boolean;
  readonly lyrics: LyricsDto | null;
  readonly tab: "lyrics" | "next";
  open(track: QueueItem | null): Promise<void>;
  setTab(tab: "lyrics" | "next"): void;
}
```

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/lyricsStore.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api, type LyricsDto, type QueueItem } from "./api";
import { createLyrics } from "./lyrics.svelte";

vi.mock("./api", () => ({ api: { getLyrics: vi.fn() } }));

const track = (id: number): QueueItem => ({
  track_id: id, title: `T${id}`, artist: "A", album: "B", duration_secs: 200, cover: null,
});

const dto = (id: number): LyricsDto => ({
  track_id: id, source: "tidal", synced: true, rtl: false,
  lines: [{ time_ms: 0, text: "hi" }],
});

describe("lyrics store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads lyrics for a track and clears loading", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    expect(api.getLyrics).toHaveBeenCalledWith(1, "T1", "A", "B", 200);
    expect(model.lyrics?.track_id).toBe(1);
    expect(model.loading).toBe(false);
  });

  it("does not refetch the same track", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    await model.open(track(1));
    expect(api.getLyrics).toHaveBeenCalledTimes(1);
  });

  it("drops a stale response when the track changed mid-flight", async () => {
    let resolveFirst: (value: LyricsDto | null) => void = () => {};
    vi.mocked(api.getLyrics)
      .mockImplementationOnce(
        () => new Promise<LyricsDto | null>((resolve) => (resolveFirst = resolve)),
      )
      .mockResolvedValueOnce(dto(2));
    const model = createLyrics();
    const first = model.open(track(1));
    await model.open(track(2));
    resolveFirst(dto(1));
    await first;
    expect(model.lyrics?.track_id).toBe(2);
    expect(model.loading).toBe(false);
  });

  it("treats failures as no lyrics", async () => {
    vi.mocked(api.getLyrics).mockRejectedValue(new Error("offline"));
    const model = createLyrics();
    await model.open(track(3));
    expect(model.lyrics).toBeNull();
    expect(model.loading).toBe(false);
  });

  it("resets the tab to lyrics on a track change", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    model.setTab("next");
    vi.mocked(api.getLyrics).mockResolvedValue(dto(2));
    await model.open(track(2));
    expect(model.tab).toBe("lyrics");
  });

  it("clears state for a null track", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    await model.open(null);
    expect(model.lyrics).toBeNull();
    expect(model.loading).toBe(false);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/lyricsStore.test.ts`
Expected: FAIL — cannot resolve `./lyrics.svelte`.

- [ ] **Step 3: Implement the store**

Create `ui/src/lib/lyrics.svelte.ts`:

```ts
import { api, type LyricsDto, type QueueItem } from "./api";

export interface LyricsStore {
  readonly loading: boolean;
  readonly lyrics: LyricsDto | null;
  readonly tab: "lyrics" | "next";
  open(track: QueueItem | null): Promise<void>;
  setTab(tab: "lyrics" | "next"): void;
}

export function createLyrics(): LyricsStore {
  let loading = $state(false);
  let lyrics = $state<LyricsDto | null>(null);
  let tab = $state<"lyrics" | "next">("lyrics");
  let currentId: number | null = null;

  async function open(track: QueueItem | null) {
    if (!track) {
      currentId = null;
      lyrics = null;
      loading = false;
      tab = "lyrics";
      return;
    }
    if (track.track_id === currentId) return;
    currentId = track.track_id;
    tab = "lyrics";
    lyrics = null;
    loading = true;
    let result: LyricsDto | null = null;
    try {
      result = await api.getLyrics(
        track.track_id,
        track.title,
        track.artist,
        track.album,
        track.duration_secs,
      );
    } catch {
      result = null;
    }
    if (currentId !== track.track_id) return; // a newer track superseded this fetch
    lyrics = result;
    loading = false;
  }

  return {
    get loading() { return loading; },
    get lyrics() { return lyrics; },
    get tab() { return tab; },
    open,
    setTab: (next) => { tab = next; },
  };
}

export const lyrics = createLyrics();
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/lyricsStore.test.ts`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/lyrics.svelte.ts ui/src/lib/lyricsStore.test.ts
git commit -m "feat(ui): add the lyrics store"
```

---

### Task 8: Now Playing — lyrics panel and column layout

**Files:**
- Modify: `ui/src/lib/components/Icon.svelte` (add `locate-fixed`)
- Create: `ui/src/lib/components/LyricsPanel.svelte`
- Modify: `ui/src/views/NowPlaying.svelte`

**Interfaces:**
- Consumes: `lyrics` store (Task 7), `activeLine` (Task 6), `Player` (`ui/src/lib/player.svelte.ts:148`), `appearance.reducedMotion`.
- Produces: `LyricsPanel` component with props `{ player: Player; dto: LyricsDto }`; Now Playing column with tabs, loading row, lyrics or Next Up.

- [ ] **Step 1: Add the jump-to-current icon**

In `ui/src/lib/components/Icon.svelte`, add to `ICONS` (after `"chevron-down"`):

```ts
    "locate-fixed": '<line x1="2" x2="5" y1="12" y2="12" /> <line x1="19" x2="22" y1="12" y2="12" /> <line x1="12" x2="12" y1="2" y2="5" /> <line x1="12" x2="12" y1="19" y2="22" /> <circle cx="12" cy="12" r="7" /> <circle cx="12" cy="12" r="3" />',
```

- [ ] **Step 2: Create the panel**

Create `ui/src/lib/components/LyricsPanel.svelte`:

```svelte
<script lang="ts">
  import type { LyricsDto } from "../api";
  import { appearance } from "../appearance.svelte";
  import { activeLine } from "../lyrics";
  import type { Player } from "../player.svelte";
  import Icon from "./Icon.svelte";

  let { player, dto }: { player: Player; dto: LyricsDto } = $props();

  let scroller = $state<HTMLElement | null>(null);
  let following = $state(true);
  let lineEls: HTMLElement[] = [];

  const active = $derived(activeLine(dto.lines, player.state.position_ms));

  // Keep the active line ~40% from the top; users who scroll pause this.
  $effect(() => {
    const index = active;
    const container = scroller;
    if (!dto.synced || !following || index < 0 || !container) return;
    const el = lineEls[index];
    if (!el) return;
    container.scrollTo({
      top: Math.max(0, el.offsetTop - container.clientHeight * 0.4),
      behavior: appearance.reducedMotion ? "auto" : "smooth",
    });
  });

  function seek(line: { time_ms: number | null }) {
    if (line.time_ms === null) return;
    following = true;
    void player.seek(line.time_ms);
  }
</script>

<div class="lyrics-wrap">
  <div
    class="lyrics"
    class:rtl={dto.rtl}
    bind:this={scroller}
    onwheel={() => (following = false)}
    onpointerdown={() => (following = false)}
    ontouchstart={() => (following = false)}
  >
    {#each dto.lines as line, i (i)}
      <button
        class="line"
        class:active={dto.synced && i === active}
        class:tappable={dto.synced && line.time_ms !== null}
        bind:this={lineEls[i]}
        disabled={!dto.synced || line.time_ms === null}
        onclick={() => seek(line)}
      >
        {line.text}
      </button>
    {/each}
  </div>
  {#if dto.synced && !following}
    <button class="jump" onclick={() => (following = true)} aria-label="Jump to current line">
      <Icon name="locate-fixed" size={16} />
    </button>
  {/if}
</div>

<style>
  .lyrics-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .lyrics {
    position: relative;
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
    mask-image: linear-gradient(180deg, transparent 0, #000 10%, #000 90%, transparent 100%);
    scrollbar-width: none;
  }
  .lyrics::-webkit-scrollbar {
    width: 0;
    height: 0;
  }
  /* Room for the first and last lines to rest on the 40% mark. */
  .lyrics::before {
    content: "";
    display: block;
    height: 40%;
  }
  .lyrics::after {
    content: "";
    display: block;
    height: 60%;
  }
  .line {
    display: block;
    width: 100%;
    margin: 0;
    padding: 6px 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: 15px;
    line-height: 1.45;
    text-align: start;
    white-space: pre-wrap;
    transition: color var(--dur-view) var(--ease-in);
  }
  .line.tappable {
    cursor: pointer;
  }
  .line.active {
    color: var(--text);
    font-weight: 600;
  }
  .line:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
  .jump {
    position: absolute;
    right: 0;
    bottom: 4px;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 1px solid var(--line);
    background: var(--surface-float);
    color: var(--text);
    cursor: pointer;
  }
</style>
```

- [ ] **Step 3: Wire the column in Now Playing**

In `ui/src/views/NowPlaying.svelte`, add to the imports:

```ts
  import LyricsPanel from "../lib/components/LyricsPanel.svelte";
  import { lyrics } from "../lib/lyrics.svelte";
```

After the `nextUp` derived, add:

```ts
  $effect(() => {
    void lyrics.open(track);
  });
```

Change the root element to:

```svelte
<div class="now-playing" class:with-lyrics={lyrics.tab === "lyrics" && lyrics.lyrics !== null} out:npOut>
```

Replace the block from `<hr />` through the empty `div.lyrics-slot` (line 72-91) with:

```svelte
      <hr />

      {#if lyrics.loading}
        <p class="muted loading">Loading lyrics…</p>
      {:else}
        {#if lyrics.lyrics}
          <div class="tabs">
            <button class:active={lyrics.tab === "lyrics"} onclick={() => lyrics.setTab("lyrics")}>
              Lyrics
            </button>
            <button class:active={lyrics.tab === "next"} onclick={() => lyrics.setTab("next")}>
              Next up
            </button>
          </div>
        {:else}
          <span class="eyebrow">Next up</span>
        {/if}

        {#if lyrics.tab === "lyrics" && lyrics.lyrics}
          {@const shown = lyrics.lyrics}
          {#key shown.track_id}
            <LyricsPanel {player} dto={shown} />
          {/key}
        {:else}
          {#if nextUp.length > 0}
            <ul class="next-up">
              {#each nextUp as item (item.track_id)}
                <li>
                  <span class="thumb"><Cover uuid={item.cover} size={80} radius={6} alt="" /></span>
                  <span class="next-title">{item.title}</span>
                  <span class="muted next-artist">{item.artist}</span>
                  <span class="muted tnum">{formatDuration(item.duration_secs)}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">Queue is empty</p>
          {/if}
        {/if}
      {/if}
```

Add the CSS (before the closing `</style>`; keep the existing rules):

```css
  .meta {
    display: flex;
    flex-direction: column;
  }
  .now-playing.with-lyrics {
    align-items: start;
  }
  .now-playing.with-lyrics .art {
    align-self: center;
  }
  .now-playing.with-lyrics .meta {
    height: 100%;
    min-height: 0;
  }
  .tabs {
    display: flex;
    gap: 14px;
    margin-bottom: 10px;
  }
  .tabs button {
    padding: 0 0 3px;
    border: none;
    border-bottom: 1px solid transparent;
    background: none;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-muted);
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .tabs button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
  .loading {
    margin: 0;
  }
```

Delete the now-unused `.lyrics-slot` rule.

- [ ] **Step 4: Verify types, tests, and the build**

Run: `npm --prefix ui run check`
Expected: no errors.
Run: `npm --prefix ui test`
Expected: all suites pass (including the new `lyrics` and `lyricsStore`).
Run: `npm --prefix ui run app:dev` (human, with Vite running from the script)
Expected: Now Playing shows lyrics for a track with them and Next Up otherwise; documented further in Task 9.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/Icon.svelte ui/src/lib/components/LyricsPanel.svelte ui/src/views/NowPlaying.svelte
git commit -m "feat(ui): show lyrics in now playing"
```

---

### Task 9: Manual smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-29-lyrics-smoke.md`

**Interfaces:**
- Consumes: everything above; requires a human, a real account, and a browser login.
- Produces: the human verification record for the feature.

- [ ] **Step 1: Write the checklist**

Create `docs/superpowers/plans/notes/2026-09-29-lyrics-smoke.md`:

```markdown
# Lyrics — manual smoke checklist

GUI smoke checks for the Track 4a lyrics feature (2026-09-29). Requires a human,
a real account, and a browser login. Run with `npm --prefix ui run app:dev`.
Reference: `docs/superpowers/specs/2026-09-29-oxitide-track4a-lyrics-design.md`.

## Real-response check (fixture-first)

- [ ] Play a well-known track with lyrics, open Now Playing: the lyrics panel
      appears and lines highlight in time. If they don't, capture the raw
      TIDAL response (temporary `tracing::debug!` in `track_lyrics`), compare it
      with `TidalLyrics`/`parse_lrc`, and update the DTO/tests to match.
- [ ] TIDAL's response for the track shows in the lyrics panel (not just lrclib):
      pick a track you expect TIDAL to cover and confirm lines render.
- [ ] A track TIDAL lacks falls back to lrclib and still renders.

## Sync behavior

- [ ] The active line tracks playback and stays in sync through pause/resume
      and a seek by clicking the waveform.
- [ ] Clicking a lyric line seeks to it and following resumes.
- [ ] Manual wheel/drag scroll pauses following; the jump-to-current glyph
      appears; clicking it resumes and recenters the active line.
- [ ] Reduced motion (Windows "Show animations" off) jumps instead of scrolling.
- [ ] RTL lyrics (if any track can be found) render right-aligned.

## States

- [ ] Track with no lyrics keeps the plain Next Up column (no tabs, no empty panel).
- [ ] Instrumental track keeps Next Up.
- [ ] Unsynced lyrics render as plain text: no highlight, no auto-scroll, no cursor.
- [ ] Track change while Now Playing is open swaps lyrics without showing the
      previous track's lines.
- [ ] Rapidly skipping tracks never shows mismatched lyrics.
- [ ] Closing and reopening Now Playing on the same track shows lyrics instantly.

## Cache and restart

- [ ] On a second visit to a track, lyrics appear without a loading row.
- [ ] Quit and relaunch, then play a cached track: lyrics appear immediately
      (disk cache) with no lrclib traffic.
- [ ] A track with no lyrics is not re-queried after a restart (no loading row,
      Next Up immediately).

## Design match

- [ ] Looks correct in both Atmosphere and Instrument modes and both densities.
- [ ] Resize the window to the 720×520 minimum: lyrics scroll, artwork stays
      centered, nothing overlaps the player bar.
- [ ] Tabs match the quiet Micro labels elsewhere; the jump glyph is an inlined
      Lucide icon (no unicode).
```

- [ ] **Step 2: Run the full verification suite one last time**

Run: `cargo test --workspace`
Run: `cargo clippy --workspace -- -D warnings`
Run: `npm --prefix ui run check`
Run: `npm --prefix ui test`
Expected: all green.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/notes/2026-09-29-lyrics-smoke.md
git commit -m "test: record the lyrics smoke checklist"
```

- [ ] **Step 4: Hand the checklist to the human**

Report the checklist path and wait for the human run; fix anything it surfaces
and re-run the affected task's tests before closing the plan.
