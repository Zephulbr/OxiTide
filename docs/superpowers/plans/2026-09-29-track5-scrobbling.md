# Track 5 Scrobbling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fan one playback event feed out to Last.fm and ListenBrainz, each independently connectable, toggleable, and retryable, with now-playing pings and completed scrobbles flowing from the engine.

**Architecture:** A Rust `ScrobblerService` in `oxitide-app` (Approach A) owns both HTTP clients, keyring-backed credentials, per-service in-memory queues, and a pure `ScrobbleTracker`. A second `engine.subscribe` callback feeds the tracker; a dedicated tokio worker sends now-playing/scrobbles and drains queues with backoff. Commands expose status/connect/disconnect/toggle to an extended Connections tab.

**Tech Stack:** Rust (Tauri 2 commands/events, `tokio`, `reqwest`, `serde`, `thiserror`, `keyring` 3 windows-native, `md-5`), Svelte 5 runes, vitest, `wiremock`.

**Spec:** `docs/superpowers/specs/2026-09-29-oxitide-track5-scrobbling-design.md`

## Global Constraints

- Both services in parallel with independent toggles; one shared listen-event feed (S-D1).
- Scrobble when `duration_secs > 30` **and** `listened_ms >= min(duration/2, 240_000 ms)` (whichever comes first); now-playing at track start and on resume after a >=30 s pause (S-D7).
- Queue is memory-only — no scrobble queue file; no backfill (S-D4, S-D9).
- Last.fm key/secret: `OXITIDE_LASTFM_API_KEY` / `OXITIDE_LASTFM_API_SECRET` env win over `config.toml`; never committed (S-D3).
- Secrets live in Windows Credential Manager, service `oxitide`, users `lastfm` / `listenbrainz`; never logged (S-D8).
- All HTTP lives in Rust; the UI only invokes commands and listens to events (S-D2).
- Every new `#[tauri::command]` must be listed in `generate_handler![]` in `crates/oxitide-app/src/main.rs`.
- No new TIDAL auth scopes.
- UI: icons via `lib/components/Icon.svelte`, semantic tokens only, no mode branching (S-D10).
- Tests must never hit live Last.fm/ListenBrainz — `wiremock` only. Unit tests use `MemoryCredentialStore`.
- Windows-first.

## Review Focus

- **Empty artist/title metadata** — a `QueueItem` with an empty artist or title must never produce a now-playing ping or a scrobble. Owned by Task 3.
- **Forward seek across the threshold** — seeking to 90% must not scrobble a track that was barely listened to. Owned by Task 3.
- **Repeat-one / replaying the same track id** — each full play is a separate scrobble and must not be deduped by `track_id` alone. Owned by Tasks 3 and 4.
- **Last.fm transient codes (16) and rate limit (29)** — must queue and back off, never drop or toast. Owned by Task 8.
- **Invalid/revoked session mid-play** — one toast, service marked `needs_reconnect`, the other service and playback unaffected. Owned by Task 8.

## File Structure

**New**
- `crates/oxitide-app/src/scrobble/mod.rs` — `ScrobblerService`, worker, status DTOs, sinks.
- `crates/oxitide-app/src/scrobble/tracker.rs` — pure `ScrobbleTracker`.
- `crates/oxitide-app/src/scrobble/lastfm.rs` — signing + Last.fm client.
- `crates/oxitide-app/src/scrobble/listenbrainz.rs` — ListenBrainz client.
- `crates/oxitide-app/src/scrobble/credentials.rs` — credential store + redacting type.
- `crates/oxitide-app/src/scrobble/queue.rs` — bounded per-service queue.
- `ui/src/lib/scrobble.ts` + `ui/src/lib/scrobble.test.ts` — pure UI helpers.
- `docs/superpowers/plans/notes/2026-09-29-track5-scrobbling-smoke.md` — manual checklist.

**Modified**
- `crates/oxitide-app/Cargo.toml` — add `keyring`, `md-5`, `thiserror`, `url`.
- `crates/oxitide-app/src/config.rs` — `[scrobbling]` section.
- `crates/oxitide-app/src/state.rs` — `scrobbler` field + accessors.
- `crates/oxitide-app/src/main.rs` — module, construction, sinks, start, `generate_handler![]`.
- `crates/oxitide-app/src/commands.rs` — status/connect/disconnect/toggle commands; engine subscription.
- `ui/src/lib/api.ts` — types, event constants, invoke methods.
- `ui/src/App.svelte` — auth-error toast listener.
- `ui/src/views/Settings.svelte` — live Connections rows.

---

### Task 1: Config `[scrobbling]` section and Last.fm credential resolution

**Files:**
- Modify: `crates/oxitide-app/src/config.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `crate::config::ScrobblingConfig { lastfm_enabled: bool, listenbrainz_enabled: bool, lastfm_api_key: String, lastfm_api_secret: String }`; `Config.scrobbling`; `ScrobblingConfig::lastfm_credentials(&self) -> (String, String)`; `ScrobblingConfig::pick(env: Option<String>, config: &str) -> String`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `config.rs`:

```rust
#[test]
fn scrobbling_defaults_and_round_trip() {
    let dir = temp_dir();
    let path = dir.join("config.toml");
    let fresh = Config::default();
    assert!(!fresh.scrobbling.lastfm_enabled);
    assert!(!fresh.scrobbling.listenbrainz_enabled);
    assert_eq!(fresh.scrobbling.lastfm_api_key, "");
    let mut config = Config::default();
    config.scrobbling.lastfm_enabled = true;
    config.scrobbling.lastfm_api_key = "key".into();
    config.scrobbling.lastfm_api_secret = "secret".into();
    config.save_to(&path).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("[scrobbling]"), "{text}");
    assert!(text.contains("lastfm_enabled = true"));
    assert_eq!(Config::load_from(&path), config);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn pre_scrobbling_file_fills_defaults() {
    let dir = temp_dir();
    let path = dir.join("config.toml");
    fs::write(&path, "[ui]\nquality_display = \"detailed\"\n").unwrap();
    let config = Config::load_from(&path);
    assert!(!config.scrobbling.lastfm_enabled);
    assert_eq!(config.scrobbling.lastfm_api_key, "");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn credential_pick_prefers_non_empty_env() {
    assert_eq!(ScrobblingConfig::pick(Some("env".into()), "cfg"), "env");
    assert_eq!(ScrobblingConfig::pick(Some(String::new()), "cfg"), "cfg");
    assert_eq!(ScrobblingConfig::pick(None, "cfg"), "cfg");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app config::tests`
Expected: FAIL — `no field 'scrobbling'` / `no function 'pick'`.

- [ ] **Step 3: Implement**

In `config.rs`, add `scrobbling: ScrobblingConfig,` to the `Config` struct (after `cache`), then add the type below the `CacheConfig` impl:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrobblingConfig {
    pub lastfm_enabled: bool,
    pub listenbrainz_enabled: bool,
    pub lastfm_api_key: String,
    pub lastfm_api_secret: String,
}

impl Default for ScrobblingConfig {
    fn default() -> Self {
        Self {
            lastfm_enabled: false,
            listenbrainz_enabled: false,
            lastfm_api_key: String::new(),
            lastfm_api_secret: String::new(),
        }
    }
}

impl ScrobblingConfig {
    /// Env wins over config (spec S-D3). Empty values never shadow a fallback.
    pub fn pick(env: Option<String>, config: &str) -> String {
        env.filter(|value| !value.is_empty())
            .unwrap_or_else(|| config.to_string())
    }

    pub fn lastfm_credentials(&self) -> (String, String) {
        (
            Self::pick(std::env::var("OXITIDE_LASTFM_API_KEY").ok(), &self.lastfm_api_key),
            Self::pick(std::env::var("OXITIDE_LASTFM_API_SECRET").ok(), &self.lastfm_api_secret),
        )
    }
}
```

`Config` derives `Default`, so the new field is covered automatically. The `partial_file_fills_defaults` test in the same file may need one added assertion; leave the existing assertions intact.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app config::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/config.rs
git commit -m "feat(app): add the scrobbling config section"
```

---

### Task 2: `scrobble` module skeleton, dependencies, and credential store

**Files:**
- Create: `crates/oxitide-app/src/scrobble/mod.rs`, `credentials.rs`, `tracker.rs`, `queue.rs`, `lastfm.rs`, `listenbrainz.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `crates/oxitide-app/Cargo.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: `crate::scrobble::credentials::{CredentialStore, KeyringCredentialStore, MemoryCredentialStore, StoredCredential}`; `StoredCredential { username: String, secret: String }` with redacting `Debug`.

- [ ] **Step 1: Add dependencies**

In `crates/oxitide-app/Cargo.toml` `[dependencies]`, add:

```toml
keyring = { version = "3", features = ["windows-native"] }
thiserror.workspace = true
url.workspace = true
md-5 = "0.10"
```

- [ ] **Step 2: Create the module skeleton**

`crates/oxitide-app/src/scrobble/mod.rs`:

```rust
//! Scrobbling to Last.fm and ListenBrainz (Track 5 spec).

pub mod credentials;
pub mod lastfm;
pub mod listenbrainz;
pub mod queue;
pub mod tracker;
```

Create `lastfm.rs`, `listenbrainz.rs`, `queue.rs`, `tracker.rs` as empty files (filled in Tasks 3–6). Add `mod scrobble;` to `crates/oxitide-app/src/main.rs` beside the other `mod` declarations.

- [ ] **Step 3: Write the credential store**

`crates/oxitide-app/src/scrobble/credentials.rs`:

```rust
use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const KEYRING_SERVICE: &str = "oxitide";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredCredential {
    pub username: String,
    pub secret: String,
}

impl std::fmt::Debug for StoredCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredCredential")
            .field("username", &self.username)
            .field("secret", &"<redacted>")
            .finish()
    }
}

pub trait CredentialStore: Send + Sync {
    fn save(&self, service: &str, credential: &StoredCredential) -> Result<(), String>;
    fn load(&self, service: &str) -> Result<Option<StoredCredential>, String>;
    fn clear(&self, service: &str) -> Result<(), String>;
}

pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    fn entry(service: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, service)
            .map_err(|e| format!("keyring unavailable: {e}"))
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn save(&self, service: &str, credential: &StoredCredential) -> Result<(), String> {
        let json = serde_json::to_string(credential).map_err(|e| e.to_string())?;
        Self::entry(service)?
            .set_password(&json)
            .map_err(|e| format!("keyring write failed: {e}"))
    }

    fn load(&self, service: &str) -> Result<Option<StoredCredential>, String> {
        match Self::entry(service)?.get_password() {
            Ok(json) => serde_json::from_str(&json).map(Some).map_err(|e| e.to_string()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("keyring read failed: {e}")),
        }
    }

    fn clear(&self, service: &str) -> Result<(), String> {
        match Self::entry(service)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keyring delete failed: {e}")),
        }
    }
}

#[derive(Default)]
pub struct MemoryCredentialStore {
    inner: Mutex<HashMap<String, StoredCredential>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn save(&self, service: &str, credential: &StoredCredential) -> Result<(), String> {
        self.inner
            .lock()
            .unwrap()
            .insert(service.to_string(), credential.clone());
        Ok(())
    }

    fn load(&self, service: &str) -> Result<Option<StoredCredential>, String> {
        Ok(self.inner.lock().unwrap().get(service).cloned())
    }

    fn clear(&self, service: &str) -> Result<(), String> {
        self.inner.lock().unwrap().remove(service);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryCredentialStore::default();
        assert!(store.load("lastfm").unwrap().is_none());
        let cred = StoredCredential { username: "v".into(), secret: "s".into() };
        store.save("lastfm", &cred).unwrap();
        assert_eq!(store.load("lastfm").unwrap(), Some(cred));
        store.clear("lastfm").unwrap();
        assert!(store.load("lastfm").unwrap().is_none());
    }

    #[test]
    fn debug_redacts_the_secret() {
        let cred = StoredCredential { username: "v".into(), secret: "super-secret".into() };
        let text = format!("{cred:?}");
        assert!(!text.contains("super-secret"), "{text}");
        assert!(text.contains("<redacted>"));
        assert!(text.contains("v"));
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app credentials::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/Cargo.toml crates/oxitide-app/src/main.rs crates/oxitide-app/src/scrobble
git commit -m "feat(app): add the scrobble module skeleton and credential store"
```

### Task 3: Pure `ScrobbleTracker`

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/tracker.rs`

**Interfaces:**
- Consumes: `oxitide_audio::engine::EngineEvent`, `oxitide_audio::queue::QueueItem`.
- Produces: `ScrobbleTracker::default()`, `ScrobbleTracker::on_event(&mut self, event: &EngineEvent, now: i64) -> Vec<ScrobbleIntent>`; `enum ScrobbleIntent { NowPlaying(QueueItem), Scrobble { item: QueueItem, started_at: i64 } }`; consts `MIN_TRACK_SECS`, `SEEK_MAX_MS`, `SCROBBLE_MIN_MS`, `PAUSE_RESEND_SECS`.

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/oxitide-app/src/scrobble/tracker.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64, secs: u32) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_secs: secs,
            cover: None,
        }
    }

    fn start(tracker: &mut ScrobbleTracker, item: QueueItem, now: i64) -> Vec<ScrobbleIntent> {
        tracker.on_event(&EngineEvent::NowPlaying(Some(item)), now)
    }

    fn pos(tracker: &mut ScrobbleTracker, ms: i64, dur_ms: i64) -> Vec<ScrobbleIntent> {
        tracker.on_event(
            &EngineEvent::Position { position_ms: ms as u64, duration_ms: dur_ms as u64 },
            0,
        )
    }

    fn scrobbled(out: &[ScrobbleIntent]) -> bool {
        out.iter().any(|i| matches!(i, ScrobbleIntent::Scrobble { .. }))
    }

    #[test]
    fn short_track_never_scrobbles() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 20), 100);
        let mut out = Vec::new();
        for step in 1..=20 {
            out.extend(pos(&mut tracker, step * 1000, 20_000));
        }
        assert!(!scrobbled(&out), "{out:?}");
    }

    #[test]
    fn scrobbles_at_half_duration() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 200), 100);
        let mut out = Vec::new();
        for step in 1..=100 {
            out.extend(pos(&mut tracker, step * 1000, 200_000));
        }
        let scrobbles: Vec<_> = out
            .into_iter()
            .filter(|i| matches!(i, ScrobbleIntent::Scrobble { .. }))
            .collect();
        assert_eq!(scrobbles.len(), 1);
        match &scrobbles[0] {
            ScrobbleIntent::Scrobble { started_at, .. } => assert_eq!(*started_at, 100),
            _ => unreachable!(),
        }
    }

    #[test]
    fn long_track_scrobbles_at_four_minutes() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 900), 100); // half would be 450s; 240s comes first
        let mut fired = false;
        for step in 1..=240 {
            fired |= scrobbled(&pos(&mut tracker, step * 1000, 900_000));
        }
        assert!(fired, "the 4-minute bound should have fired");
    }

    #[test]
    fn forward_seek_does_not_count() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 200), 100);
        assert!(!scrobbled(&pos(&mut tracker, 1_000, 200_000)));
        // Jump to 90%; the 179s delta must not be counted.
        assert!(!scrobbled(&pos(&mut tracker, 180_000, 200_000)));
        assert!(!scrobbled(&pos(&mut tracker, 181_000, 200_000)));
    }

    #[test]
    fn empty_artist_is_not_scrobblable() {
        let mut tracker = ScrobbleTracker::default();
        let mut broken = item(1, 200);
        broken.artist = String::new();
        assert!(start(&mut tracker, broken, 100).is_empty());
        assert!(!scrobbled(&pos(&mut tracker, 1_000, 200_000)));
    }

    #[test]
    fn repeat_resets_and_scrobbles_again() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 100), 100);
        for step in 1..=50 {
            pos(&mut tracker, step * 1000, 100_000);
        }
        // Replaying the same id must be a fresh play.
        let out = start(&mut tracker, item(1, 100), 200);
        assert!(matches!(out.as_slice(), [ScrobbleIntent::NowPlaying(_)]));
        let mut fired = false;
        for step in 1..=50 {
            fired |= scrobbled(&pos(&mut tracker, step * 1000, 100_000));
        }
        assert!(fired);
    }

    #[test]
    fn resume_after_long_pause_re_pings() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 200), 100);
        let state = |playing: bool| EngineEvent::State(playback_state(playing));
        assert!(tracker.on_event(&state(false), 1_000).is_empty());
        assert!(tracker.on_event(&state(true), 1_010).is_empty(), "short pause should not re-ping");
        tracker.on_event(&state(false), 2_000);
        let out = tracker.on_event(&state(true), 2_100);
        assert!(matches!(out.as_slice(), [ScrobbleIntent::NowPlaying(_)]));
    }

    fn playback_state(playing: bool) -> oxitide_audio::engine::PlaybackState {
        oxitide_audio::engine::PlaybackState {
            playing,
            buffering: false,
            current: Some(item(1, 200)),
            position_ms: 0,
            duration_ms: 200_000,
            volume: 1.0,
            shuffle: false,
            repeat: oxitide_audio::queue::RepeatMode::Off,
            preferred_quality: oxitide_tidal::stream::Quality::Lossless,
            output_mode: oxitide_audio::output::OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            queue: Vec::new(),
            stream: None,
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app tracker::tests`
Expected: FAIL — `ScrobbleTracker` not defined.

- [ ] **Step 3: Implement the tracker**

Prepend to `tracker.rs` (above the `mod tests`):

```rust
//! Pure listen-event state machine (spec §6). No I/O: given playback events
//! and a wall clock, it emits now-playing and scrobble intents.

use oxitide_audio::engine::EngineEvent;
use oxitide_audio::queue::QueueItem;

pub const MIN_TRACK_SECS: u32 = 30;
pub const SEEK_MAX_MS: i64 = 3_000;
pub const SCROBBLE_MIN_MS: i64 = 240_000;
pub const PAUSE_RESEND_SECS: i64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum ScrobbleIntent {
    NowPlaying(QueueItem),
    Scrobble { item: QueueItem, started_at: i64 },
}

#[derive(Default)]
pub struct ScrobbleTracker {
    current: Option<QueueItem>,
    started_at: i64,
    last_pos_ms: i64,
    listened_ms: i64,
    scrobbled: bool,
    paused_since: Option<i64>,
}

impl ScrobbleTracker {
    pub fn on_event(&mut self, event: &EngineEvent, now: i64) -> Vec<ScrobbleIntent> {
        match event {
            EngineEvent::NowPlaying(Some(item)) => self.start(item.clone(), now),
            EngineEvent::NowPlaying(None) => {
                *self = Self::default();
                Vec::new()
            }
            EngineEvent::Position { position_ms, duration_ms } => {
                self.position(*position_ms as i64, *duration_ms as i64)
            }
            EngineEvent::State(state) => self.state(state.playing, now),
            EngineEvent::Error { .. } => Vec::new(),
        }
    }

    fn start(&mut self, item: QueueItem, now: i64) -> Vec<ScrobbleIntent> {
        self.started_at = now;
        self.last_pos_ms = 0;
        self.listened_ms = 0;
        self.scrobbled = false;
        self.paused_since = None;
        if item.artist.is_empty() || item.title.is_empty() {
            self.current = None;
            return Vec::new();
        }
        self.current = Some(item.clone());
        vec![ScrobbleIntent::NowPlaying(item)]
    }

    fn position(&mut self, position_ms: i64, duration_ms: i64) -> Vec<ScrobbleIntent> {
        let Some(item) = self.current.clone() else { return Vec::new() };
        let delta = position_ms - self.last_pos_ms;
        if delta > 0 && delta <= SEEK_MAX_MS {
            self.listened_ms += delta;
        }
        self.last_pos_ms = position_ms;
        if self.scrobbled {
            return Vec::new();
        }
        let duration = if duration_ms > 0 { duration_ms } else { item.duration_secs as i64 * 1000 };
        let threshold = (duration / 2).min(SCROBBLE_MIN_MS);
        if item.duration_secs > MIN_TRACK_SECS && self.listened_ms >= threshold {
            self.scrobbled = true;
            return vec![ScrobbleIntent::Scrobble { item, started_at: self.started_at }];
        }
        Vec::new()
    }

    fn state(&mut self, playing: bool, now: i64) -> Vec<ScrobbleIntent> {
        if !playing {
            if self.paused_since.is_none() {
                self.paused_since = Some(now);
            }
            return Vec::new();
        }
        let Some(paused_since) = self.paused_since.take() else { return Vec::new() };
        if now - paused_since >= PAUSE_RESEND_SECS {
            if let Some(item) = self.current.clone() {
                return vec![ScrobbleIntent::NowPlaying(item)];
            }
        }
        Vec::new()
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app tracker::tests`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/scrobble/tracker.rs
git commit -m "feat(app): add the pure scrobble tracker"
```

---

### Task 4: Bounded per-service `ScrobbleQueue`

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/queue.rs`

**Interfaces:**
- Consumes: `oxitide_audio::queue::QueueItem`.
- Produces: `QueuedScrobble { item: QueueItem, started_at: i64 }`; `ScrobbleQueue::default()`, `push`, `len`, `is_empty`, `front_batch(n) -> Vec<QueuedScrobble>`, `remove_front(n)`, `clear`; `QUEUE_CAP = 1000`.

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/oxitide-app/src/scrobble/queue.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, at: i64) -> QueuedScrobble {
        QueuedScrobble {
            item: QueueItem {
                track_id: id,
                title: "T".into(),
                artist: "A".into(),
                album: String::new(),
                duration_secs: 200,
                cover: None,
            },
            started_at: at,
        }
    }

    #[test]
    fn dedupes_same_track_and_timestamp() {
        let mut queue = ScrobbleQueue::default();
        queue.push(entry(1, 100));
        queue.push(entry(1, 100));
        assert_eq!(queue.len(), 1);
        // A replay (same id, new timestamp) is a distinct scrobble.
        queue.push(entry(1, 200));
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn caps_by_dropping_oldest() {
        let mut queue = ScrobbleQueue::default();
        for id in 0..(QUEUE_CAP as u64 + 5) {
            queue.push(entry(id + 1, id as i64));
        }
        assert_eq!(queue.len(), QUEUE_CAP);
        assert_eq!(
            queue.front_batch(1)[0].item.track_id,
            6,
            "oldest entries should have been dropped"
        );
    }

    #[test]
    fn front_batch_and_remove_front_are_fifo() {
        let mut queue = ScrobbleQueue::default();
        queue.push(entry(1, 1));
        queue.push(entry(2, 2));
        queue.push(entry(3, 3));
        let batch = queue.front_batch(2);
        assert_eq!(batch.iter().map(|e| e.item.track_id).collect::<Vec<_>>(), vec![1, 2]);
        queue.remove_front(2);
        assert_eq!(queue.front_batch(1)[0].item.track_id, 3);
        queue.clear();
        assert!(queue.is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app queue::tests`
Expected: FAIL — `ScrobbleQueue` not defined.

- [ ] **Step 3: Implement**

Prepend to `queue.rs`:

```rust
use std::collections::VecDeque;

use oxitide_audio::queue::QueueItem;

pub const QUEUE_CAP: usize = 1_000;

#[derive(Debug, Clone, PartialEq)]
pub struct QueuedScrobble {
    pub item: QueueItem,
    pub started_at: i64,
}

#[derive(Default)]
pub struct ScrobbleQueue {
    items: VecDeque<QueuedScrobble>,
}

impl ScrobbleQueue {
    pub fn push(&mut self, entry: QueuedScrobble) {
        let duplicate = self
            .items
            .iter()
            .any(|e| e.item.track_id == entry.item.track_id && e.started_at == entry.started_at);
        if duplicate {
            return;
        }
        self.items.push_back(entry);
        while self.items.len() > QUEUE_CAP {
            self.items.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn front_batch(&self, count: usize) -> Vec<QueuedScrobble> {
        self.items.iter().take(count).cloned().collect()
    }

    pub fn remove_front(&mut self, count: usize) {
        for _ in 0..count {
            self.items.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app queue::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/scrobble/queue.rs
git commit -m "feat(app): add the bounded scrobble queue"
```

---

### Task 5: Last.fm client (signing + endpoints)

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/lastfm.rs`

**Interfaces:**
- Consumes: `oxitide_audio::queue::QueueItem`.
- Produces: `api_sig(params: &[(String, String)], secret: &str) -> String`; `LastFmClient::new(http, base_url, api_key, secret)`, `api_key()`, `auth_url(token)`, `get_token()`, `get_session(token)`, `now_playing(session_key, item)`, `scrobble(session_key, plays: &[(QueueItem, i64)])`; `LastFmSession { username, key }`; `LastFmError::{Retryable, Auth, NotAuthorized, Permanent}`.

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/oxitide-app/src/scrobble/lastfm.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method as http_method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn encode(input: &str) -> String {
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("x", input)
            .finish()
            .trim_start_matches("x=")
            .to_string()
    }

    fn client(server: &MockServer) -> LastFmClient {
        LastFmClient::new(reqwest::Client::new(), server.uri(), "abc123".into(), "s3cr3t".into())
    }

    fn item() -> QueueItem {
        QueueItem {
            track_id: 7,
            title: "Song".into(),
            artist: "Band".into(),
            album: "Record".into(),
            duration_secs: 210,
            cover: None,
        }
    }

    #[test]
    fn signature_matches_a_known_vector() {
        // md5("api_key"+"abc123"+"method"+"auth.getToken"+"token"+"tok"+"s3cr3t")
        let params = vec![
            ("api_key".to_string(), "abc123".to_string()),
            ("method".to_string(), "auth.getToken".to_string()),
            ("token".to_string(), "tok".to_string()),
        ];
        assert_eq!(api_sig(&params, "s3cr3t"), "50d433db8fa2c45553410dd8d1230608");
    }

    #[test]
    fn signature_excludes_format_and_callback() {
        let base = vec![
            ("api_key".to_string(), "abc123".to_string()),
            ("method".to_string(), "auth.getToken".to_string()),
        ];
        let mut with_noise = base.clone();
        with_noise.push(("format".to_string(), "json".to_string()));
        with_noise.push(("callback".to_string(), "cb".to_string()));
        assert_eq!(api_sig(&base, "s"), api_sig(&with_noise, "s"));
    }

    #[tokio::test]
    async fn get_token_returns_the_token() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "token": "tok" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).get_token().await.unwrap(), "tok");
    }

    #[tokio::test]
    async fn get_session_maps_error_14_to_not_authorized() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "error": 14, "message": "unauthorized token" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).get_session("tok").await, Err(LastFmError::NotAuthorized));
    }

    #[tokio::test]
    async fn get_session_parses_the_session() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "session": { "name": "v", "key": "sk" } }),
            ))
            .mount(&server)
            .await;
        let session = client(&server).get_session("tok").await.unwrap();
        assert_eq!(session.username, "v");
        assert_eq!(session.key, "sk");
    }

    #[tokio::test]
    async fn scrobble_sends_signed_indexed_form_fields() {
        let server = MockServer::start().await;
        let expected_sig = api_sig(
            &[
                ("album[0]".into(), "Record".into()),
                ("api_key".into(), "abc123".into()),
                ("artist[0]".into(), "Band".into()),
                ("duration[0]".into(), "210".into()),
                ("method".into(), "track.scrobble".into()),
                ("sk".into(), "sk".into()),
                ("timestamp[0]".into(), "1700000000".into()),
                ("track[0]".into(), "Song".into()),
            ],
            "s3cr3t",
        );
        Mock::given(http_method("POST"))
            .and(path("/"))
            .and(body_string_contains(format!("api_sig={}", encode(&expected_sig))))
            .and(body_string_contains("method=track.scrobble"))
            .and(body_string_contains("artist%5B0%5D=Band"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "scrobbles": {} })),
            )
            .mount(&server)
            .await;
        client(&server)
            .scrobble("sk", &[(item(), 1_700_000_000)])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn error_9_is_auth_and_16_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 9 })))
            .mount(&server)
            .await;
        assert_eq!(client(&server).now_playing("sk", &item()).await, Err(LastFmError::Auth));

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 16 })))
            .mount(&server)
            .await;
        assert_eq!(client(&server).now_playing("sk", &item()).await, Err(LastFmError::Retryable));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app scrobble::lastfm`
Expected: FAIL — `LastFmClient` not defined.

- [ ] **Step 3: Implement**

Prepend to `lastfm.rs`:

```rust
//! Last.fm client: signed API calls (spec §4, §7).

use md5::{Digest, Md5};
use oxitide_audio::queue::QueueItem;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LastFmError {
    #[error("last.fm is temporarily unavailable")]
    Retryable,
    #[error("last.fm authentication failed")]
    Auth,
    #[error("last.fm authorization is not complete")]
    NotAuthorized,
    #[error("last.fm rejected the request")]
    Permanent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LastFmSession {
    pub username: String,
    pub key: String,
}

/// `api_sig`: MD5 hex of name+value pairs (excluding `format`/`callback`)
/// sorted by name, with the shared secret appended (spec §4).
pub fn api_sig(params: &[(String, String)], secret: &str) -> String {
    let mut pairs: Vec<(&str, &str)> = params
        .iter()
        .filter(|(key, _)| key != "format" && key != "callback")
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let mut material = String::new();
    for (key, value) in pairs {
        material.push_str(key);
        material.push_str(value);
    }
    material.push_str(secret);
    let mut hasher = Md5::new();
    hasher.update(material.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub struct LastFmClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    secret: String,
}

impl LastFmClient {
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        api_key: String,
        secret: String,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            secret,
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn auth_url(&self, token: &str) -> String {
        format!("https://www.last.fm/api/auth/?api_key={}&token={}", self.api_key, token)
    }

    fn signed(&self, method: &str, mut params: Vec<(String, String)>) -> Vec<(String, String)> {
        params.push(("method".into(), method.to_string()));
        params.push(("api_key".into(), self.api_key.clone()));
        params.push(("format".into(), "json".into()));
        let signature = api_sig(&params, &self.secret);
        params.push(("api_sig".into(), signature));
        params
    }

    async fn get(
        &self,
        method: &str,
        params: Vec<(String, String)>,
    ) -> Result<serde_json::Value, LastFmError> {
        let query = self.signed(method, params);
        let response = self
            .http
            .get(&self.base_url)
            .query(&query)
            .send()
            .await
            .map_err(|_| LastFmError::Retryable)?;
        self.parse(response).await
    }

    async fn post(
        &self,
        method: &str,
        params: Vec<(String, String)>,
    ) -> Result<serde_json::Value, LastFmError> {
        let form = self.signed(method, params);
        let response = self
            .http
            .post(&self.base_url)
            .form(&form)
            .send()
            .await
            .map_err(|_| LastFmError::Retryable)?;
        self.parse(response).await
    }

    async fn parse(&self, response: reqwest::Response) -> Result<serde_json::Value, LastFmError> {
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            return Err(LastFmError::Retryable);
        }
        let value: serde_json::Value = response.json().await.map_err(|_| LastFmError::Retryable)?;
        if let Some(code) = value.get("error").and_then(|field| field.as_u64()) {
            return Err(match code {
                14 => LastFmError::NotAuthorized,
                9 | 4 | 10 => LastFmError::Auth,
                16 | 29 => LastFmError::Retryable,
                _ => LastFmError::Permanent,
            });
        }
        if !status.is_success() {
            return Err(if status.is_client_error() {
                LastFmError::Permanent
            } else {
                LastFmError::Retryable
            });
        }
        Ok(value)
    }

    pub async fn get_token(&self) -> Result<String, LastFmError> {
        let value = self.get("auth.getToken", Vec::new()).await?;
        value
            .get("token")
            .and_then(|field| field.as_str())
            .map(str::to_string)
            .ok_or(LastFmError::Permanent)
    }

    pub async fn get_session(&self, token: &str) -> Result<LastFmSession, LastFmError> {
        let value = self
            .get("auth.getSession", vec![("token".into(), token.to_string())])
            .await?;
        let session = value.get("session").ok_or(LastFmError::Permanent)?;
        let username = session
            .get("name")
            .and_then(|field| field.as_str())
            .ok_or(LastFmError::Permanent)?
            .to_string();
        let key = session
            .get("key")
            .and_then(|field| field.as_str())
            .ok_or(LastFmError::Permanent)?
            .to_string();
        Ok(LastFmSession { username, key })
    }

    pub async fn now_playing(
        &self,
        session_key: &str,
        item: &QueueItem,
    ) -> Result<(), LastFmError> {
        let mut params = vec![("sk".to_string(), session_key.to_string())];
        params.extend(track_params(item, None));
        self.post("track.updateNowPlaying", params).await?;
        Ok(())
    }

    pub async fn scrobble(
        &self,
        session_key: &str,
        plays: &[(QueueItem, i64)],
    ) -> Result<(), LastFmError> {
        if plays.is_empty() {
            return Ok(());
        }
        let mut params = vec![("sk".to_string(), session_key.to_string())];
        for (index, (item, timestamp)) in plays.iter().enumerate() {
            params.push((format!("artist[{index}]"), item.artist.clone()));
            params.push((format!("track[{index}]"), item.title.clone()));
            params.push((format!("timestamp[{index}]"), timestamp.to_string()));
            if !item.album.is_empty() {
                params.push((format!("album[{index}]"), item.album.clone()));
            }
            if item.duration_secs > 0 {
                params.push((format!("duration[{index}]"), item.duration_secs.to_string()));
            }
        }
        self.post("track.scrobble", params).await?;
        Ok(())
    }
}

fn track_params(item: &QueueItem, timestamp: Option<i64>) -> Vec<(String, String)> {
    let mut params = vec![
        ("artist".to_string(), item.artist.clone()),
        ("track".to_string(), item.title.clone()),
    ];
    if !item.album.is_empty() {
        params.push(("album".to_string(), item.album.clone()));
    }
    if item.duration_secs > 0 {
        params.push(("duration".to_string(), item.duration_secs.to_string()));
    }
    if let Some(timestamp) = timestamp {
        params.push(("timestamp".to_string(), timestamp.to_string()));
    }
    params
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app scrobble::lastfm`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/scrobble/lastfm.rs
git commit -m "feat(app): add the Last.fm client with API signing"
```

---

### Task 6: ListenBrainz client

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/listenbrainz.rs`

**Interfaces:**
- Consumes: `oxitide_audio::queue::QueueItem`.
- Produces: `ListenBrainzClient::new(http, base_url)`, `validate(token) -> Result<String, ListenBrainzError>` (username), `playing_now(token, item)`, `submit_listen(token, item, listened_at)`; `track_metadata(item) -> serde_json::Value`; `ListenBrainzError::{Retryable, Auth, Permanent}`.

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/oxitide-app/src/scrobble/listenbrainz.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method as http_method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn item() -> QueueItem {
        QueueItem {
            track_id: 7,
            title: "Song".into(),
            artist: "Band".into(),
            album: "Record".into(),
            duration_secs: 210,
            cover: None,
        }
    }

    fn client(server: &MockServer) -> ListenBrainzClient {
        ListenBrainzClient::new(reqwest::Client::new(), server.uri())
    }

    #[tokio::test]
    async fn validate_returns_the_username() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(path("/validate-token"))
            .and(header("authorization", "Token tok"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "valid": true, "user_name": "v" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).validate("tok").await.unwrap(), "v");
    }

    #[tokio::test]
    async fn validate_rejects_an_invalid_token() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "valid": false, "message": "bad token" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).validate("tok").await, Err(ListenBrainzError::Auth));
    }

    #[tokio::test]
    async fn validate_maps_401_to_auth() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        assert_eq!(client(&server).validate("tok").await, Err(ListenBrainzError::Auth));
    }

    #[tokio::test]
    async fn playing_now_sends_the_expected_payload() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/submit-listens"))
            .and(header("authorization", "Token tok"))
            .and(body_partial_json(serde_json::json!({
                "listen_type": "playing_now",
                "payload": [{
                    "track_metadata": {
                        "artist_name": "Band",
                        "track_name": "Song",
                        "release_name": "Record",
                        "additional_info": { "listening_from": "OxiTide", "duration_ms": 210000 }
                    }
                }]
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        client(&server).playing_now("tok", &item()).await.unwrap();
    }

    #[tokio::test]
    async fn submit_listen_adds_listened_at() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/submit-listens"))
            .and(body_partial_json(serde_json::json!({
                "listen_type": "single",
                "payload": [{ "listened_at": 1700000000 }]
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        client(&server)
            .submit_listen("tok", &item(), 1_700_000_000)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn submit_listen_maps_5xx_to_retryable() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).submit_listen("tok", &item(), 1).await,
            Err(ListenBrainzError::Retryable)
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app scrobble::listenbrainz`
Expected: FAIL — `ListenBrainzClient` not defined.

- [ ] **Step 3: Implement**

Prepend to `listenbrainz.rs`:

```rust
//! ListenBrainz client (spec §4, §7).

use oxitide_audio::queue::QueueItem;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ListenBrainzError {
    #[error("listenbrainz is temporarily unavailable")]
    Retryable,
    #[error("listenbrainz authentication failed")]
    Auth,
    #[error("listenbrainz rejected the request")]
    Permanent,
}

pub struct ListenBrainzClient {
    http: reqwest::Client,
    base_url: String,
}

impl ListenBrainzClient {
    pub fn new(http: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub async fn validate(&self, token: &str) -> Result<String, ListenBrainzError> {
        let response = self
            .http
            .get(self.url("/validate-token"))
            .header("Authorization", format!("Token {token}"))
            .send()
            .await
            .map_err(|_| ListenBrainzError::Retryable)?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ListenBrainzError::Auth);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            return Err(ListenBrainzError::Retryable);
        }
        if !status.is_success() {
            return Err(ListenBrainzError::Permanent);
        }
        let value: serde_json::Value =
            response.json().await.map_err(|_| ListenBrainzError::Permanent)?;
        let valid = value.get("valid").and_then(|field| field.as_bool()).unwrap_or(false);
        if !valid {
            return Err(ListenBrainzError::Auth);
        }
        value
            .get("user_name")
            .and_then(|field| field.as_str())
            .map(str::to_string)
            .ok_or(ListenBrainzError::Permanent)
    }

    pub async fn playing_now(
        &self,
        token: &str,
        item: &QueueItem,
    ) -> Result<(), ListenBrainzError> {
        let body = serde_json::json!({
            "listen_type": "playing_now",
            "payload": [{ "track_metadata": track_metadata(item) }]
        });
        self.submit(token, &body).await
    }

    pub async fn submit_listen(
        &self,
        token: &str,
        item: &QueueItem,
        listened_at: i64,
    ) -> Result<(), ListenBrainzError> {
        let body = serde_json::json!({
            "listen_type": "single",
            "payload": [{
                "track_metadata": track_metadata(item),
                "listened_at": listened_at
            }]
        });
        self.submit(token, &body).await
    }

    async fn submit(
        &self,
        token: &str,
        body: &serde_json::Value,
    ) -> Result<(), ListenBrainzError> {
        let response = self
            .http
            .post(self.url("/submit-listens"))
            .header("Authorization", format!("Token {token}"))
            .json(body)
            .send()
            .await
            .map_err(|_| ListenBrainzError::Retryable)?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ListenBrainzError::Auth);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            return Err(ListenBrainzError::Retryable);
        }
        if !status.is_success() {
            return Err(ListenBrainzError::Permanent);
        }
        Ok(())
    }
}

pub fn track_metadata(item: &QueueItem) -> serde_json::Value {
    let mut info = serde_json::Map::new();
    info.insert("listening_from".into(), serde_json::json!("OxiTide"));
    if item.duration_secs > 0 {
        info.insert("duration_ms".into(), serde_json::json!(u64::from(item.duration_secs) * 1000));
    }
    let mut metadata = serde_json::Map::new();
    metadata.insert("artist_name".into(), serde_json::json!(item.artist));
    metadata.insert("track_name".into(), serde_json::json!(item.title));
    if !item.album.is_empty() {
        metadata.insert("release_name".into(), serde_json::json!(item.album));
    }
    metadata.insert("additional_info".into(), serde_json::Value::Object(info));
    serde_json::Value::Object(metadata)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app scrobble::listenbrainz`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/scrobble/listenbrainz.rs
git commit -m "feat(app): add the ListenBrainz client"
```

### Task 7: `ScrobblerService` core (status, toggles, intents)

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/mod.rs`

**Interfaces:**
- Consumes: `credentials::*`, `lastfm::*`, `listenbrainz::*`, `queue::*`, `tracker::*`.
- Produces: `ScrobblerService::new(http, lastfm_base, listenbrainz_base, store, lastfm_api_key, lastfm_api_secret, lastfm_enabled, listenbrainz_enabled) -> Arc<Self>`; `status() -> ScrobbleStatusDto`; `set_enabled(service: &str, enabled: bool)`; `on_player_event(&EngineEvent)`; `handle_intent(ScrobbleIntent)` (async); `set_status_sink(StatusSink)`, `set_auth_error_sink(AuthErrorSink)`; consts `LASTFM_SERVICE`, `LISTENBRAINZ_SERVICE`; DTOs `ScrobbleStatusDto`, `ScrobbleServiceStatus` (all `Clone + Serialize + PartialEq`).

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/oxitide-app/src/scrobble/mod.rs` with:

```rust
//! Scrobbling to Last.fm and ListenBrainz (Track 5 spec).
//!
//! `ScrobblerService` is fed raw playback events, turns them into now-playing
//! and scrobble intents with the pure `ScrobbleTracker`, and fans them out to
//! both services. All HTTP happens on a dedicated worker (Tasks 8–9).

pub mod credentials;
pub mod lastfm;
pub mod listenbrainz;
pub mod queue;
pub mod tracker;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxitide_audio::engine::EngineEvent;
use serde::Serialize;
use tokio::sync::mpsc;

use credentials::{CredentialStore, StoredCredential};
use lastfm::LastFmClient;
use listenbrainz::ListenBrainzClient;
use queue::{QueuedScrobble, ScrobbleQueue};
use tracker::{ScrobbleIntent, ScrobbleTracker};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrobble::credentials::MemoryCredentialStore;
    use oxitide_audio::queue::QueueItem;

    fn service(store: Arc<dyn CredentialStore>) -> Arc<ScrobblerService> {
        ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            false,
            false,
        )
    }

    fn queue_item() -> QueueItem {
        QueueItem {
            track_id: 7,
            title: "Song".into(),
            artist: "Band".into(),
            album: "Record".into(),
            duration_secs: 200,
            cover: None,
        }
    }

    #[test]
    fn status_defaults_are_disconnected() {
        let service = service(Arc::new(MemoryCredentialStore::default()));
        let status = service.status();
        assert!(!status.lastfm.connected);
        assert!(!status.lastfm.enabled);
        assert!(status.lastfm.configured, "key+secret are present");
        assert_eq!(status.lastfm.queued, 0);
        assert!(!status.listenbrainz.connected);
    }

    #[test]
    fn missing_key_is_not_configured() {
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            Arc::new(MemoryCredentialStore::default()),
            String::new(),
            String::new(),
            false,
            false,
        );
        assert!(!service.status().lastfm.configured);
    }

    #[test]
    fn toggles_are_independent() {
        let service = service(Arc::new(MemoryCredentialStore::default()));
        service.set_enabled(LASTFM_SERVICE, true);
        assert!(service.status().lastfm.enabled);
        assert!(!service.status().listenbrainz.enabled);
        service.set_enabled(LISTENBRAINZ_SERVICE, true);
        assert!(service.status().listenbrainz.enabled);
    }

    #[tokio::test]
    async fn a_scrobble_intent_enqueues_for_enabled_services() {
        let service = service(Arc::new(MemoryCredentialStore::default()));
        service.set_enabled(LASTFM_SERVICE, true);
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        assert_eq!(service.status().lastfm.queued, 1);
        assert_eq!(service.status().listenbrainz.queued, 0, "disabled service must not enqueue");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app scrobble::tests`
Expected: FAIL — `ScrobblerService` not defined.

- [ ] **Step 3: Implement the service core**

Replace the top of `mod.rs` (the module doc + `pub mod` lines) so the file reads, in order: the doc comment, `pub mod` declarations, imports, then this code, then the `#[cfg(test)] mod tests` block from Step 1:

```rust
pub const LASTFM_SERVICE: &str = "lastfm";
pub const LISTENBRAINZ_SERVICE: &str = "listenbrainz";

pub type StatusSink = Arc<dyn Fn(ScrobbleStatusDto) + Send + Sync>;
pub type AuthErrorSink = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScrobbleServiceStatus {
    pub id: String,
    pub enabled: bool,
    pub connected: bool,
    pub username: Option<String>,
    pub queued: usize,
    pub needs_reconnect: bool,
    pub last_error: Option<String>,
    pub configured: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScrobbleStatusDto {
    pub lastfm: ScrobbleServiceStatus,
    pub listenbrainz: ScrobbleServiceStatus,
}

#[derive(Default)]
struct Backoff {
    failures: u32,
    next_at: Option<Instant>,
}

impl Backoff {
    fn ready(&self) -> bool {
        self.next_at.map(|at| Instant::now() >= at).unwrap_or(true)
    }

    fn fail(&mut self) {
        self.failures = (self.failures + 1).min(7);
        let seconds = (5u64 << self.failures).min(300);
        self.next_at = Some(Instant::now() + Duration::from_secs(seconds));
    }

    fn reset(&mut self) {
        self.failures = 0;
        self.next_at = None;
    }
}

#[derive(Default)]
struct Inner {
    tracker: ScrobbleTracker,
    lastfm_enabled: bool,
    listenbrainz_enabled: bool,
    lastfm: Option<StoredCredential>,
    listenbrainz: Option<StoredCredential>,
    lastfm_queue: ScrobbleQueue,
    listenbrainz_queue: ScrobbleQueue,
    lastfm_error: Option<String>,
    listenbrainz_error: Option<String>,
    lastfm_needs_reconnect: bool,
    listenbrainz_needs_reconnect: bool,
    lastfm_backoff: Backoff,
    listenbrainz_backoff: Backoff,
    lastfm_pending_token: Option<String>,
}

pub fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

pub struct ScrobblerService {
    lastfm: LastFmClient,
    listenbrainz: ListenBrainzClient,
    store: Arc<dyn CredentialStore>,
    lastfm_configured: bool,
    tx: mpsc::UnboundedSender<ScrobbleIntent>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<ScrobbleIntent>>>,
    inner: Mutex<Inner>,
    on_status: Mutex<Option<StatusSink>>,
    on_auth_error: Mutex<Option<AuthErrorSink>>,
}

impl ScrobblerService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        http: reqwest::Client,
        lastfm_base: impl Into<String>,
        listenbrainz_base: impl Into<String>,
        store: Arc<dyn CredentialStore>,
        lastfm_api_key: String,
        lastfm_api_secret: String,
        lastfm_enabled: bool,
        listenbrainz_enabled: bool,
    ) -> Arc<Self> {
        let lastfm = LastFmClient::new(
            http.clone(),
            lastfm_base,
            lastfm_api_key.clone(),
            lastfm_api_secret.clone(),
        );
        let listenbrainz = ListenBrainzClient::new(http, listenbrainz_base);
        let lastfm_credential = store.load(LASTFM_SERVICE).ok().flatten();
        let listenbrainz_credential = store.load(LISTENBRAINZ_SERVICE).ok().flatten();
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            lastfm,
            listenbrainz,
            store,
            lastfm_configured: !lastfm_api_key.is_empty() && !lastfm_api_secret.is_empty(),
            tx,
            rx: Mutex::new(Some(rx)),
            inner: Mutex::new(Inner {
                lastfm_enabled,
                listenbrainz_enabled,
                lastfm: lastfm_credential,
                listenbrainz: listenbrainz_credential,
                ..Inner::default()
            }),
            on_status: Mutex::new(None),
            on_auth_error: Mutex::new(None),
        })
    }

    pub fn set_status_sink(&self, sink: StatusSink) {
        *self.on_status.lock().unwrap() = Some(sink);
    }

    pub fn set_auth_error_sink(&self, sink: AuthErrorSink) {
        *self.on_auth_error.lock().unwrap() = Some(sink);
    }

    fn emit_status(&self) {
        if let Some(sink) = self.on_status.lock().unwrap().as_ref() {
            sink(self.status());
        }
    }

    fn notify_auth_error(&self, service: &str) {
        if let Some(sink) = self.on_auth_error.lock().unwrap().as_ref() {
            sink(service.to_string());
        }
        self.emit_status();
    }

    pub fn status(&self) -> ScrobbleStatusDto {
        let inner = self.inner.lock().unwrap();
        ScrobbleStatusDto {
            lastfm: ScrobbleServiceStatus {
                id: LASTFM_SERVICE.to_string(),
                enabled: inner.lastfm_enabled,
                connected: inner.lastfm.is_some(),
                username: inner.lastfm.as_ref().map(|c| c.username.clone()),
                queued: inner.lastfm_queue.len(),
                needs_reconnect: inner.lastfm_needs_reconnect,
                last_error: inner.lastfm_error.clone(),
                configured: self.lastfm_configured,
            },
            listenbrainz: ScrobbleServiceStatus {
                id: LISTENBRAINZ_SERVICE.to_string(),
                enabled: inner.listenbrainz_enabled,
                connected: inner.listenbrainz.is_some(),
                username: inner.listenbrainz.as_ref().map(|c| c.username.clone()),
                queued: inner.listenbrainz_queue.len(),
                needs_reconnect: inner.listenbrainz_needs_reconnect,
                last_error: inner.listenbrainz_error.clone(),
                configured: true,
            },
        }
    }

    pub fn set_enabled(&self, service: &str, enabled: bool) {
        {
            let mut inner = self.inner.lock().unwrap();
            match service {
                LASTFM_SERVICE => inner.lastfm_enabled = enabled,
                LISTENBRAINZ_SERVICE => inner.listenbrainz_enabled = enabled,
                _ => return,
            }
        }
        self.emit_status();
    }

    pub fn on_player_event(&self, event: &EngineEvent) {
        let intents = {
            let mut inner = self.inner.lock().unwrap();
            inner.tracker.on_event(event, now_epoch())
        };
        for intent in intents {
            let _ = self.tx.send(intent);
        }
    }

    fn enabled_flags(&self) -> (bool, bool) {
        let inner = self.inner.lock().unwrap();
        (inner.lastfm_enabled, inner.listenbrainz_enabled)
    }

    fn lastfm_session(&self) -> Option<String> {
        self.inner.lock().unwrap().lastfm.as_ref().map(|c| c.secret.clone())
    }

    fn listenbrainz_token(&self) -> Option<String> {
        self.inner.lock().unwrap().listenbrainz.as_ref().map(|c| c.secret.clone())
    }

    pub async fn handle_intent(&self, intent: ScrobbleIntent) {
        match intent {
            ScrobbleIntent::NowPlaying(item) => {
                let (lastfm, listenbrainz) = self.enabled_flags();
                if lastfm {
                    if let Some(session) = self.lastfm_session() {
                        if let Err(error) = self.lastfm.now_playing(&session, &item).await {
                            tracing::debug!("last.fm now-playing failed: {error}");
                        }
                    }
                }
                if listenbrainz {
                    if let Some(token) = self.listenbrainz_token() {
                        let _ = self.listenbrainz.playing_now(&token, &item).await;
                    }
                }
            }
            ScrobbleIntent::Scrobble { item, started_at } => {
                let (lastfm, listenbrainz) = self.enabled_flags();
                let entry = QueuedScrobble { item, started_at };
                let mut changed = false;
                {
                    let mut inner = self.inner.lock().unwrap();
                    if lastfm {
                        inner.lastfm_queue.push(entry.clone());
                        changed = true;
                    }
                    if listenbrainz {
                        inner.listenbrainz_queue.push(entry);
                        changed = true;
                    }
                }
                if changed {
                    self.emit_status();
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app scrobble::tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/scrobble/mod.rs
git commit -m "feat(app): add the ScrobblerService core"
```

---

### Task 8: Worker flush, backoff, and auth-failure handling

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/mod.rs`

**Interfaces:**
- Consumes: Task 7 plus `lastfm::LastFmError`, `listenbrainz::ListenBrainzError`.
- Produces: `ScrobblerService::flush_queues()` (async); `ScrobblerService::start(self: &Arc<Self>)` (spawns the tokio worker via `tauri::async_runtime::spawn`).

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `mod.rs`:

```rust
    #[tokio::test]
    async fn retryable_failure_keeps_the_scrobble_queued() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 16 })),
            )
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        let status = service.status();
        assert_eq!(status.lastfm.queued, 1, "16 must stay queued");
        assert!(!status.lastfm.needs_reconnect);
        assert!(status.lastfm.last_error.is_some());
    }

    #[tokio::test]
    async fn a_successful_flush_drains_the_queue() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "scrobbles": {} })),
            )
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        assert_eq!(service.status().lastfm.queued, 0);
    }

    #[tokio::test]
    async fn an_invalid_session_marks_needs_reconnect_and_notifies_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 9 })))
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        service.set_auth_error_sink(Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        service.flush_queues().await;
        let status = service.status();
        assert!(status.lastfm.needs_reconnect);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "toast must fire once");
        assert!(!status.listenbrainz.needs_reconnect, "other service untouched");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app scrobble::tests`
Expected: FAIL — no method `flush_queues`.

- [ ] **Step 3: Implement the flush worker**

Add to the imports at the top of `mod.rs`:

```rust
use lastfm::LastFmError;
use listenbrainz::ListenBrainzError;
```

(alongside the existing `use lastfm::LastFmClient;` / `use listenbrainz::ListenBrainzClient;` — merge them into `use lastfm::{LastFmClient, LastFmError};` and `use listenbrainz::{ListenBrainzClient, ListenBrainzError};`).

Then add these methods inside `impl ScrobblerService`:

```rust
    pub fn start(self: &Arc<Self>) {
        let Some(mut rx) = self.rx.lock().unwrap().take() else {
            return;
        };
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    intent = rx.recv() => {
                        match intent {
                            Some(intent) => this.handle_intent(intent).await,
                            None => break,
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                }
                this.flush_queues().await;
            }
        });
    }

    fn lastfm_retryable(&self, message: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.lastfm_backoff.fail();
            inner.lastfm_error = Some(message.to_string());
        }
        self.emit_status();
    }

    fn lastfm_auth_failed(&self, message: &str) {
        let newly = {
            let mut inner = self.inner.lock().unwrap();
            let newly = !inner.lastfm_needs_reconnect;
            inner.lastfm_needs_reconnect = true;
            inner.lastfm_error = Some(message.to_string());
            newly
        };
        if newly {
            self.notify_auth_error(LASTFM_SERVICE);
        } else {
            self.emit_status();
        }
    }

    fn listenbrainz_retryable(&self, message: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.listenbrainz_backoff.fail();
            inner.listenbrainz_error = Some(message.to_string());
        }
        self.emit_status();
    }

    fn listenbrainz_auth_failed(&self, message: &str) {
        let newly = {
            let mut inner = self.inner.lock().unwrap();
            let newly = !inner.listenbrainz_needs_reconnect;
            inner.listenbrainz_needs_reconnect = true;
            inner.listenbrainz_error = Some(message.to_string());
            newly
        };
        if newly {
            self.notify_auth_error(LISTENBRAINZ_SERVICE);
        } else {
            self.emit_status();
        }
    }

    pub async fn flush_queues(&self) {
        self.flush_lastfm().await;
        self.flush_listenbrainz().await;
    }

    async fn flush_lastfm(&self) {
        let (session, batch) = {
            let inner = self.inner.lock().unwrap();
            if !inner.lastfm_enabled
                || inner.lastfm_needs_reconnect
                || inner.lastfm.is_none()
                || inner.lastfm_queue.is_empty()
                || !inner.lastfm_backoff.ready()
            {
                return;
            }
            let session = inner.lastfm.as_ref().map(|c| c.secret.clone()).unwrap();
            (session, inner.lastfm_queue.front_batch(50))
        };
        if batch.is_empty() {
            return;
        }
        let plays: Vec<(oxitide_audio::queue::QueueItem, i64)> =
            batch.iter().map(|entry| (entry.item.clone(), entry.started_at)).collect();
        match self.lastfm.scrobble(&session, &plays).await {
            Ok(()) => {
                {
                    let mut inner = self.inner.lock().unwrap();
                    inner.lastfm_queue.remove_front(batch.len());
                    inner.lastfm_backoff.reset();
                    inner.lastfm_error = None;
                }
                self.emit_status();
            }
            Err(LastFmError::Retryable) => {
                self.lastfm_retryable("last.fm is temporarily unavailable")
            }
            Err(LastFmError::Auth) | Err(LastFmError::NotAuthorized) => {
                self.lastfm_auth_failed("last.fm needs reconnecting")
            }
            Err(LastFmError::Permanent) => {
                tracing::warn!("last.fm rejected {} scrobble(s); dropping", batch.len());
                {
                    let mut inner = self.inner.lock().unwrap();
                    inner.lastfm_queue.remove_front(batch.len());
                }
                self.emit_status();
            }
        }
    }

    async fn flush_listenbrainz(&self) {
        let token = {
            let inner = self.inner.lock().unwrap();
            if !inner.listenbrainz_enabled
                || inner.listenbrainz_needs_reconnect
                || inner.listenbrainz.is_none()
                || inner.listenbrainz_queue.is_empty()
                || !inner.listenbrainz_backoff.ready()
            {
                return;
            }
            inner.listenbrainz.as_ref().map(|c| c.secret.clone()).unwrap()
        };
        let mut sent = 0;
        while sent < 50 {
            let entry = {
                let inner = self.inner.lock().unwrap();
                inner.listenbrainz_queue.front_batch(1).into_iter().next()
            };
            let Some(entry) = entry else { break };
            match self
                .listenbrainz
                .submit_listen(&token, &entry.item, entry.started_at)
                .await
            {
                Ok(()) => {
                    let mut inner = self.inner.lock().unwrap();
                    inner.listenbrainz_queue.remove_front(1);
                    inner.listenbrainz_backoff.reset();
                    inner.listenbrainz_error = None;
                    sent += 1;
                }
                Err(ListenBrainzError::Retryable) => {
                    self.listenbrainz_retryable("listenbrainz is temporarily unavailable");
                    break;
                }
                Err(ListenBrainzError::Auth) => {
                    self.listenbrainz_auth_failed("listenbrainz needs reconnecting");
                    break;
                }
                Err(ListenBrainzError::Permanent) => {
                    tracing::warn!("listenbrainz rejected a scrobble; dropping it");
                    let mut inner = self.inner.lock().unwrap();
                    inner.listenbrainz_queue.remove_front(1);
                    sent += 1;
                }
            }
        }
        if sent > 0 {
            self.emit_status();
        }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app scrobble::tests`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/scrobble/mod.rs
git commit -m "feat(app): flush scrobble queues with backoff and auth handling"
```

---

### Task 9: Auth flows (Last.fm and ListenBrainz)

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/mod.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: `lastfm_begin_auth() -> Result<String, String>`, `lastfm_complete_auth() -> Result<ScrobbleStatusDto, String>`, `lastfm_cancel_auth()`, `lastfm_disconnect()`, `listenbrainz_connect(token: String) -> Result<ScrobbleStatusDto, String>`, `listenbrainz_disconnect()`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    #[tokio::test]
    async fn lastfm_complete_auth_saves_the_session() {
        use wiremock::matchers::{method as http_method, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(query_param("method", "auth.getToken"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "token": "tok" })),
            )
            .mount(&server)
            .await;
        Mock::given(http_method("GET"))
            .and(query_param("method", "auth.getSession"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "session": { "name": "v", "key": "sk" } }),
            ))
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store.clone(),
            "key".into(),
            "secret".into(),
            false,
            false,
        );
        let url = service.lastfm_begin_auth().await.unwrap();
        assert!(url.contains("api_key=key"), "{url}");
        assert!(url.contains("token=tok"), "{url}");
        let status = service.lastfm_complete_auth().await.unwrap();
        assert!(status.lastfm.connected);
        assert!(status.lastfm.enabled, "connecting enables the service");
        assert_eq!(status.lastfm.username.as_deref(), Some("v"));
        assert_eq!(store.load(LASTFM_SERVICE).unwrap().unwrap().secret, "sk");
    }

    #[tokio::test]
    async fn lastfm_disconnect_clears_the_credential() {
        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = service(store.clone());
        service.lastfm_disconnect();
        assert!(!service.status().lastfm.connected);
        assert!(store.load(LASTFM_SERVICE).unwrap().is_none());
    }

    #[tokio::test]
    async fn listenbrainz_connect_validates_and_stores() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(path("/validate-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "valid": true, "user_name": "v" })),
            )
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            server.uri(),
            store.clone(),
            "key".into(),
            "secret".into(),
            false,
            false,
        );
        let status = service.listenbrainz_connect("tok".into()).await.unwrap();
        assert!(status.listenbrainz.connected);
        assert!(status.listenbrainz.enabled);
        assert_eq!(status.listenbrainz.username.as_deref(), Some("v"));
        assert_eq!(store.load(LISTENBRAINZ_SERVICE).unwrap().unwrap().secret, "tok");
    }

    #[tokio::test]
    async fn listenbrainz_connect_rejects_an_invalid_token() {
        use wiremock::matchers::method as http_method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let service = ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            server.uri(),
            Arc::new(MemoryCredentialStore::default()),
            "key".into(),
            "secret".into(),
            false,
            false,
        );
        assert!(service.listenbrainz_connect("bad".into()).await.is_err());
        assert!(!service.status().listenbrainz.connected);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app scrobble::tests`
Expected: FAIL — no method `lastfm_begin_auth`.

- [ ] **Step 3: Implement the auth flows**

Add these methods inside `impl ScrobblerService`:

```rust
    pub async fn lastfm_begin_auth(&self) -> Result<String, String> {
        if !self.lastfm_configured {
            return Err("Last.fm API key not configured".to_string());
        }
        let token = self.lastfm.get_token().await.map_err(|e| e.to_string())?;
        self.inner.lock().unwrap().lastfm_pending_token = Some(token.clone());
        Ok(self.lastfm.auth_url(&token))
    }

    pub async fn lastfm_complete_auth(&self) -> Result<ScrobbleStatusDto, String> {
        let token = self
            .inner
            .lock()
            .unwrap()
            .lastfm_pending_token
            .clone()
            .ok_or_else(|| "no authorization in progress".to_string())?;
        let session = match self.lastfm.get_session(&token).await {
            Ok(session) => session,
            Err(LastFmError::NotAuthorized) => {
                return Err("authorization is not complete yet".to_string())
            }
            Err(error) => return Err(error.to_string()),
        };
        let credential =
            StoredCredential { username: session.username, secret: session.key };
        self.store.save(LASTFM_SERVICE, &credential)?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.lastfm = Some(credential);
            inner.lastfm_pending_token = None;
            inner.lastfm_enabled = true;
            inner.lastfm_needs_reconnect = false;
            inner.lastfm_error = None;
        }
        self.emit_status();
        Ok(self.status())
    }

    pub fn lastfm_cancel_auth(&self) {
        self.inner.lock().unwrap().lastfm_pending_token = None;
    }

    pub fn lastfm_disconnect(&self) {
        let _ = self.store.clear(LASTFM_SERVICE);
        {
            let mut inner = self.inner.lock().unwrap();
            inner.lastfm = None;
            inner.lastfm_enabled = false;
            inner.lastfm_queue.clear();
            inner.lastfm_error = None;
            inner.lastfm_needs_reconnect = false;
            inner.lastfm_pending_token = None;
        }
        self.emit_status();
    }

    pub async fn listenbrainz_connect(
        &self,
        token: String,
    ) -> Result<ScrobbleStatusDto, String> {
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err("token is empty".to_string());
        }
        let username = self.listenbrainz.validate(&token).await.map_err(|e| e.to_string())?;
        let credential = StoredCredential { username, secret: token };
        self.store.save(LISTENBRAINZ_SERVICE, &credential)?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.listenbrainz = Some(credential);
            inner.listenbrainz_enabled = true;
            inner.listenbrainz_needs_reconnect = false;
            inner.listenbrainz_error = None;
        }
        self.emit_status();
        Ok(self.status())
    }

    pub fn listenbrainz_disconnect(&self) {
        let _ = self.store.clear(LISTENBRAINZ_SERVICE);
        {
            let mut inner = self.inner.lock().unwrap();
            inner.listenbrainz = None;
            inner.listenbrainz_enabled = false;
            inner.listenbrainz_queue.clear();
            inner.listenbrainz_error = None;
            inner.listenbrainz_needs_reconnect = false;
        }
        self.emit_status();
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app scrobble::tests`
Expected: PASS (11 tests).

- [ ] **Step 5: Run clippy and the full crate tests**

Run: `cargo clippy -p oxitide-app --all-targets -- -D warnings`
Then: `cargo test -p oxitide-app`
Expected: clippy clean; all tests green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/scrobble/mod.rs
git commit -m "feat(app): add the scrobble auth flows"
```

### Task 10: AppState + main.rs wiring

**Files:**
- Modify: `crates/oxitide-app/src/state.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `crates/oxitide-app/src/commands.rs` (engine subscription only)

**Interfaces:**
- Consumes: `scrobble::{ScrobblerService, credentials::KeyringCredentialStore}`.
- Produces: `AppState::set_scrobbler(Arc<ScrobblerService>)`, `AppState::scrobbler() -> Option<Arc<ScrobblerService>>`.

- [ ] **Step 1: Add state accessors with a test**

In `state.rs`, add to the imports: `use crate::scrobble::ScrobblerService;`. Add the field to `AppState` beside `lyrics`:

```rust
    pub scrobbler: Mutex<Option<Arc<ScrobblerService>>>,
```

Add these methods after the lyrics accessors:

```rust
    pub fn set_scrobbler(&self, service: Arc<ScrobblerService>) {
        *self.scrobbler.lock().unwrap() = Some(service);
    }

    pub fn scrobbler(&self) -> Option<Arc<ScrobblerService>> {
        self.scrobbler.lock().unwrap().clone()
    }
```

Add to `mod tests` in `state.rs`:

```rust
    #[test]
    fn scrobbler_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.scrobbler().is_none());
        let service = crate::scrobble::ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            std::sync::Arc::new(crate::scrobble::credentials::MemoryCredentialStore::default()),
            String::new(),
            String::new(),
            false,
            false,
        );
        state.set_scrobbler(service);
        assert!(state.scrobbler().is_some());
    }
```

- [ ] **Step 2: Construct the service in `main()`**

In `main.rs`, in `main()`, replace the existing `state.set_config(config);` line with:

```rust
    let scrobble_http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let (lastfm_key, lastfm_secret) = config.scrobbling.lastfm_credentials();
    let scrobbler = scrobble::ScrobblerService::new(
        scrobble_http,
        "https://ws.audioscrobbler.com/2.0/",
        "https://api.listenbrainz.org/1/",
        std::sync::Arc::new(scrobble::credentials::KeyringCredentialStore),
        lastfm_key,
        lastfm_secret,
        config.scrobbling.lastfm_enabled,
        config.scrobbling.listenbrainz_enabled,
    );
    state.set_scrobbler(scrobbler.clone());
    state.set_config(config);
```

In the `.setup(...)` closure, after `app.state::<AppState>().set_handle(app.handle().clone());`, add:

```rust
            if let Some(scrobbler) = app.state::<AppState>().scrobbler() {
                let handle = app.handle().clone();
                scrobbler.set_status_sink(std::sync::Arc::new(move |status| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://status", status);
                }));
                let handle = app.handle().clone();
                scrobbler.set_auth_error_sink(std::sync::Arc::new(move |service| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://auth-error", service);
                }));
                scrobbler.start();
            }
```

- [ ] **Step 3: Subscribe the engine to the scrobbler in `build_player`**

In `commands.rs::build_player`, after the existing `engine.subscribe(...)` call, add:

```rust
    let scrobble_handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        if let Some(scrobbler) = scrobble_handle.state::<AppState>().scrobbler() {
            scrobbler.on_player_event(&event);
        }
    }));
```

- [ ] **Step 4: Verify it builds and the state test passes**

Run: `cargo test -p oxitide-app state::tests`
Then: `cargo clippy -p oxitide-app --all-targets -- -D warnings`
Expected: build clean; `scrobbler_starts_unset_and_is_set_once` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/state.rs crates/oxitide-app/src/main.rs crates/oxitide-app/src/commands.rs
git commit -m "feat(app): wire the ScrobblerService into startup and the engine"
```

---

### Task 11: Tauri commands

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs`

**Interfaces:**
- Consumes: `AppState::scrobbler`, `scrobble::ScrobbleStatusDto`.
- Produces: `get_scrobble_status`, `lastfm_begin_auth`, `lastfm_complete_auth`, `lastfm_cancel_auth`, `lastfm_disconnect`, `listenbrainz_connect`, `listenbrainz_disconnect`, `set_scrobble_enabled`.

- [ ] **Step 1: Add the commands**

Append to `commands.rs`:

```rust
fn require_scrobbler(
    state: &State<'_, AppState>,
) -> Result<std::sync::Arc<crate::scrobble::ScrobblerService>, String> {
    state.scrobbler().ok_or_else(|| "scrobbler unavailable".to_string())
}

#[tauri::command]
pub fn get_scrobble_status(
    state: State<'_, AppState>,
) -> Result<crate::scrobble::ScrobbleStatusDto, String> {
    Ok(require_scrobbler(&state)?.status())
}

#[tauri::command]
pub async fn lastfm_begin_auth(state: State<'_, AppState>) -> Result<String, String> {
    require_scrobbler(&state)?.lastfm_begin_auth().await
}

#[tauri::command]
pub async fn lastfm_complete_auth(
    state: State<'_, AppState>,
) -> Result<crate::scrobble::ScrobbleStatusDto, String> {
    require_scrobbler(&state)?.lastfm_complete_auth().await
}

#[tauri::command]
pub fn lastfm_cancel_auth(state: State<'_, AppState>) -> Result<(), String> {
    require_scrobbler(&state)?.lastfm_cancel_auth();
    Ok(())
}

#[tauri::command]
pub fn lastfm_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    require_scrobbler(&state)?.lastfm_disconnect();
    Ok(())
}

#[tauri::command]
pub async fn listenbrainz_connect(
    state: State<'_, AppState>,
    token: String,
) -> Result<crate::scrobble::ScrobbleStatusDto, String> {
    require_scrobbler(&state)?.listenbrainz_connect(token).await
}

#[tauri::command]
pub fn listenbrainz_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    require_scrobbler(&state)?.listenbrainz_disconnect();
    Ok(())
}

#[tauri::command]
pub fn set_scrobble_enabled(
    state: State<'_, AppState>,
    service: String,
    enabled: bool,
) -> Result<(), String> {
    let scrobbler = require_scrobbler(&state)?;
    if service != crate::scrobble::LASTFM_SERVICE
        && service != crate::scrobble::LISTENBRAINZ_SERVICE
    {
        return Err("unknown scrobble service".to_string());
    }
    scrobbler.set_enabled(&service, enabled);
    let mut config = state.config();
    if service == crate::scrobble::LASTFM_SERVICE {
        config.scrobbling.lastfm_enabled = enabled;
    } else {
        config.scrobbling.listenbrainz_enabled = enabled;
    }
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

- [ ] **Step 2: Register the commands in `generate_handler![]`**

In `main.rs`, add these entries to the `generate_handler![...]` list (after `commands::get_lyrics,`):

```rust
            commands::get_scrobble_status,
            commands::lastfm_begin_auth,
            commands::lastfm_complete_auth,
            commands::lastfm_cancel_auth,
            commands::lastfm_disconnect,
            commands::listenbrainz_connect,
            commands::listenbrainz_disconnect,
            commands::set_scrobble_enabled,
```

- [ ] **Step 3: Verify the workspace builds**

Run: `cargo build -p oxitide-app`
Expected: success.

- [ ] **Step 4: Run clippy and the workspace tests**

Run: `cargo clippy --workspace -- -D warnings`
Then: `cargo test --workspace`
Expected: clippy clean; all tests green.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose the scrobble commands"
```

---

### Task 12: UI API contract

**Files:**
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Consumes: the Task 11 commands.
- Produces: `ScrobbleServiceId`, `ScrobbleServiceStatus`, `ScrobbleStatusDto`; `SCROBBLE_STATUS_EVENT`, `SCROBBLE_AUTH_ERROR_EVENT`; `api.getScrobbleStatus`, `api.lastfmBeginAuth`, `api.lastfmCompleteAuth`, `api.lastfmCancelAuth`, `api.lastfmDisconnect`, `api.listenbrainzConnect`, `api.listenbrainzDisconnect`, `api.setScrobbleEnabled`; `Config.scrobbling`.

- [ ] **Step 1: Add the types**

In `ui/src/lib/api.ts`, after the `LyricsDto` interface, add:

```ts
export type ScrobbleServiceId = "lastfm" | "listenbrainz";
export interface ScrobbleServiceStatus {
  id: ScrobbleServiceId;
  enabled: boolean;
  connected: boolean;
  username: string | null;
  queued: number;
  needs_reconnect: boolean;
  last_error: string | null;
  configured: boolean;
}
export interface ScrobbleStatusDto {
  lastfm: ScrobbleServiceStatus;
  listenbrainz: ScrobbleServiceStatus;
}
```

Add `scrobbling` to the `Config` interface (before the closing brace):

```ts
  scrobbling: {
    lastfm_enabled: boolean;
    listenbrainz_enabled: boolean;
    lastfm_api_key: string;
    lastfm_api_secret: string;
  };
```

- [ ] **Step 2: Add the event constants**

Beside `AUTH_SUCCESS_EVENT`:

```ts
export const SCROBBLE_STATUS_EVENT = "scrobble://status";
export const SCROBBLE_AUTH_ERROR_EVENT = "scrobble://auth-error";
```

- [ ] **Step 3: Add the invoke methods**

In the `api` object, after `getLyrics`:

```ts
  getScrobbleStatus: () => invoke<ScrobbleStatusDto>("get_scrobble_status"),
  lastfmBeginAuth: () => invoke<string>("lastfm_begin_auth"),
  lastfmCompleteAuth: () => invoke<ScrobbleStatusDto>("lastfm_complete_auth"),
  lastfmCancelAuth: () => invoke<void>("lastfm_cancel_auth"),
  lastfmDisconnect: () => invoke<void>("lastfm_disconnect"),
  listenbrainzConnect: (token: string) =>
    invoke<ScrobbleStatusDto>("listenbrainz_connect", { token }),
  listenbrainzDisconnect: () => invoke<void>("listenbrainz_disconnect"),
  setScrobbleEnabled: (service: ScrobbleServiceId, enabled: boolean) =>
    invoke<void>("set_scrobble_enabled", { service, enabled }),
```

- [ ] **Step 4: Type-check**

Run: `npm --prefix ui run check`
Expected: no new errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/api.ts
git commit -m "feat(ui): add the scrobble API contract"
```

---

### Task 13: Pure scrobble UI helpers

**Files:**
- Create: `ui/src/lib/scrobble.ts`
- Create: `ui/src/lib/scrobble.test.ts`

**Interfaces:**
- Consumes: `ScrobbleServiceStatus` from `./api`.
- Produces: `serviceLabel(status)`, `canToggle(status)`, `queuedLabel(count)`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/scrobble.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { ScrobbleServiceStatus } from "./api";
import { canToggle, queuedLabel, serviceLabel } from "./scrobble";

function status(patch: Partial<ScrobbleServiceStatus>): ScrobbleServiceStatus {
  return {
    id: "lastfm",
    enabled: false,
    connected: false,
    username: null,
    queued: 0,
    needs_reconnect: false,
    last_error: null,
    configured: true,
    ...patch,
  };
}

describe("scrobble helpers", () => {
  it("reports a missing key", () => {
    expect(serviceLabel(status({ configured: false }))).toBe("API key not configured");
  });

  it("prioritizes reconnect over connected", () => {
    expect(
      serviceLabel(status({ connected: true, username: "v", needs_reconnect: true })),
    ).toBe("Reconnect needed");
  });

  it("names the connected user", () => {
    expect(serviceLabel(status({ connected: true, username: "v" }))).toBe("Connected as v");
  });

  it("defaults to not connected", () => {
    expect(serviceLabel(status({}))).toBe("Not connected");
  });

  it("only allows toggling a connected service", () => {
    expect(canToggle(status({ connected: false }))).toBe(false);
    expect(canToggle(status({ connected: true }))).toBe(true);
  });

  it("describes the queue only when non-empty", () => {
    expect(queuedLabel(0)).toBe("");
    expect(queuedLabel(3)).toBe("3 queued");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/scrobble.test.ts`
Expected: FAIL — cannot resolve `./scrobble`.

- [ ] **Step 3: Implement the helpers**

`ui/src/lib/scrobble.ts`:

```ts
import type { ScrobbleServiceStatus } from "./api";

export function serviceLabel(
  status: Pick<ScrobbleServiceStatus, "configured" | "connected" | "username" | "needs_reconnect">,
): string {
  if (!status.configured) return "API key not configured";
  if (status.needs_reconnect) return "Reconnect needed";
  if (status.connected) return `Connected as ${status.username ?? "unknown"}`;
  return "Not connected";
}

export function canToggle(status: Pick<ScrobbleServiceStatus, "connected">): boolean {
  return status.connected;
}

export function queuedLabel(count: number): string {
  return count > 0 ? `${count} queued` : "";
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/scrobble.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/scrobble.ts ui/src/lib/scrobble.test.ts
git commit -m "feat(ui): add the pure scrobble helpers"
```

### Task 14: Connections tab rows and the auth-error toast

**Files:**
- Modify: `ui/src/views/Settings.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: Task 12 API, Task 13 helpers, `SCROBBLE_STATUS_EVENT`, `SCROBBLE_AUTH_ERROR_EVENT`.
- Produces: live scrobbling rows; one toast on auth failure.

- [ ] **Step 1: Add the Settings script**

In `ui/src/views/Settings.svelte`, add to the imports:

```ts
  import { listen } from "@tauri-apps/api/event";
  import {
    SCROBBLE_STATUS_EVENT,
    type ScrobbleServiceId,
    type ScrobbleStatusDto,
  } from "../lib/api";
  import { canToggle, queuedLabel, serviceLabel } from "../lib/scrobble";
```

After the other `$state` declarations, add:

```ts
  let scrobble = $state<ScrobbleStatusDto | null>(null);
  let lastfmAuth = $state<"idle" | "waiting">("idle");
  let lastfmAuthNote = $state("");
  let lbToken = $state("");
  let lbBusy = $state(false);
  let lbError = $state("");

  async function refreshScrobble() {
    try {
      scrobble = await api.getScrobbleStatus();
    } catch (e) {
      apply(e);
    }
  }

  $effect(() => {
    refreshScrobble();
  });

  $effect(() => {
    const unlisten = listen<ScrobbleStatusDto>(SCROBBLE_STATUS_EVENT, (event) => {
      scrobble = event.payload;
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  function toggleService(service: ScrobbleServiceId, enabled: boolean) {
    if (!scrobble) return;
    scrobble = { ...scrobble, [service]: { ...scrobble[service], enabled } };
    api.setScrobbleEnabled(service, enabled).catch(apply);
  }

  async function connectLastfm() {
    lastfmAuthNote = "";
    try {
      const url = await api.lastfmBeginAuth();
      lastfmAuth = "waiting";
      await openUrl(url);
      void pollLastfm();
    } catch (e) {
      lastfmAuth = "idle";
      lastfmAuthNote = String(e);
    }
  }

  async function pollLastfm() {
    for (let attempt = 0; attempt < 40 && lastfmAuth === "waiting"; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 3000));
      if (lastfmAuth !== "waiting") return;
      try {
        scrobble = await api.lastfmCompleteAuth();
        lastfmAuth = "idle";
        return;
      } catch (e) {
        const message = String(e);
        if (!message.includes("not complete")) {
          lastfmAuth = "idle";
          lastfmAuthNote = message;
          return;
        }
      }
    }
    if (lastfmAuth === "waiting") {
      lastfmAuth = "idle";
      lastfmAuthNote = "Authorization timed out — try again.";
    }
  }

  function cancelLastfm() {
    lastfmAuth = "idle";
    api.lastfmCancelAuth().catch(apply);
  }

  function disconnectLastfm() {
    api.lastfmDisconnect().then(refreshScrobble).catch(apply);
  }

  async function connectListenbrainz() {
    lbError = "";
    lbBusy = true;
    try {
      scrobble = await api.listenbrainzConnect(lbToken);
      lbToken = "";
    } catch (e) {
      lbError = String(e);
    } finally {
      lbBusy = false;
    }
  }

  function disconnectListenbrainz() {
    api.listenbrainzDisconnect().then(refreshScrobble).catch(apply);
  }
```

- [ ] **Step 2: Replace the scrobbling placeholder rows**

In the `connections` panel, replace this block:

```svelte
      <h3>Scrobbling</h3>
      {@render planned("Last.fm", "Scrobble plays and publish now-playing to Last.fm.")}
      {@render planned("ListenBrainz", "Scrobble plays and publish now-playing to ListenBrainz.")}
```

with:

```svelte
      <h3>Scrobbling</h3>
      {#if scrobble}
        <div class="scrobble">
          <div class="scrobble-main">
            <label class="scrobble-head">
              <input
                type="checkbox"
                checked={scrobble.lastfm.enabled}
                disabled={!canToggle(scrobble.lastfm)}
                onchange={(e) => toggleService("lastfm", (e.target as HTMLInputElement).checked)}
              />
              <span>Last.fm</span>
            </label>
            <span class="muted">{serviceLabel(scrobble.lastfm)}</span>
            {#if queuedLabel(scrobble.lastfm.queued)}
              <span class="muted">{queuedLabel(scrobble.lastfm.queued)}</span>
            {/if}
            {#if scrobble.lastfm.last_error}
              <span class="muted">{scrobble.lastfm.last_error}</span>
            {/if}
            {#if lastfmAuthNote}<span class="muted">{lastfmAuthNote}</span>{/if}
          </div>
          {#if lastfmAuth === "waiting"}
            <button onclick={() => void pollLastfm()}>I've authorized</button>
            <button onclick={cancelLastfm}>Cancel</button>
          {:else if scrobble.lastfm.connected}
            <button onclick={disconnectLastfm}>Disconnect</button>
          {:else}
            <button disabled={!scrobble.lastfm.configured} onclick={() => void connectLastfm()}>
              Connect
            </button>
          {/if}
        </div>

        <div class="scrobble">
          <div class="scrobble-main">
            <label class="scrobble-head">
              <input
                type="checkbox"
                checked={scrobble.listenbrainz.enabled}
                disabled={!canToggle(scrobble.listenbrainz)}
                onchange={(e) =>
                  toggleService("listenbrainz", (e.target as HTMLInputElement).checked)}
              />
              <span>ListenBrainz</span>
            </label>
            <span class="muted">{serviceLabel(scrobble.listenbrainz)}</span>
            {#if queuedLabel(scrobble.listenbrainz.queued)}
              <span class="muted">{queuedLabel(scrobble.listenbrainz.queued)}</span>
            {/if}
            {#if scrobble.listenbrainz.last_error}
              <span class="muted">{scrobble.listenbrainz.last_error}</span>
            {/if}
            {#if lbError}<span class="muted">{lbError}</span>{/if}
          </div>
          {#if scrobble.listenbrainz.connected}
            <button onclick={disconnectListenbrainz}>Disconnect</button>
          {:else}
            <input type="password" placeholder="ListenBrainz token" bind:value={lbToken} />
            <button disabled={lbBusy || !lbToken.trim()} onclick={() => void connectListenbrainz()}>
              Connect
            </button>
          {/if}
        </div>
      {:else}
        <p class="muted">Loading scrobbling status…</p>
      {/if}
      <p class="muted">
        Get a ListenBrainz token from
        <button
          class="link"
          type="button"
          onclick={() => openCredit("https://listenbrainz.org/profile/")}
        >your profile</button>.
      </p>
```

Also update the Connections intro paragraph that currently reads
`Planned integrations — nothing to configure yet.` to
`Scrobbling is configurable below; the remaining integrations are planned.`

- [ ] **Step 3: Add row styles**

Add to the `<style>` block in `Settings.svelte`:

```css
  .scrobble {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 8px 0;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  .scrobble-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .scrobble-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
```

- [ ] **Step 4: Add the auth-error toast in `App.svelte`**

Add `SCROBBLE_AUTH_ERROR_EVENT` to the existing `import { api, type QueueItem } from "./lib/api";` line (becomes `import { api, SCROBBLE_AUTH_ERROR_EVENT, type QueueItem } from "./lib/api";`). Add this effect beside the `session_expired` listener effect:

```ts
  $effect(() => {
    const unlisten = listen<string>(SCROBBLE_AUTH_ERROR_EVENT, (event) => {
      const name = event.payload === "lastfm" ? "Last.fm" : "ListenBrainz";
      toasts.show(`${name} scrobbling needs reconnecting.`, { severity: "error" });
    });
    return () => {
      unlisten.then((off) => off());
    };
  });
```

- [ ] **Step 5: Type-check and run the UI tests**

Run: `npm --prefix ui run check`
Then: `npm --prefix ui test`
Expected: svelte-check clean; all vitest suites pass.

- [ ] **Step 6: Commit**

```bash
git add ui/src/views/Settings.svelte ui/src/App.svelte
git commit -m "feat(ui): add the Connections scrobbling rows"
```

---

### Task 15: Manual smoke checklist and full verification

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-29-track5-scrobbling-smoke.md`

**Interfaces:**
- Consumes: the whole feature.
- Produces: a recorded checklist; a final green workspace.

- [ ] **Step 1: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-09-29-track5-scrobbling-smoke.md`:

```markdown
# Track 5 scrobbling — manual smoke

Prereqs: a real Last.fm app key/secret (export `OXITIDE_LASTFM_API_KEY` and
`OXITIDE_LASTFM_API_SECRET`), a Last.fm account, a ListenBrainz account + token,
and a real TIDAL login. Run `npm --prefix ui run app:dev`.

- [ ] Last.fm: Settings → Connections → Connect opens the browser; authorizing
      flips the row to "Connected as <user>" and the toggle is enabled.
- [ ] ListenBrainz: pasting the token and clicking Connect shows the username;
      a bogus token shows an inline error and stays disconnected.
- [ ] Play a track longer than 30 s and let it pass half; the scrobble appears on
      both services' profiles within a minute.
- [ ] Now-playing shows on Last.fm while the track plays, and refreshes after a
      pause longer than 30 s.
- [ ] Turn Last.fm off; only ListenBrainz receives the next scrobble. Turn it back on.
- [ ] Disconnect while offline mid-track; go back online; the queued count drains
      (session-only queue — confirm it does NOT survive an app restart).
- [ ] Revoke/invalidate the ListenBrainz token, then play; one error toast appears,
      the row says "Reconnect needed", and Last.fm keeps scrobbling.
- [ ] A track under 30 s never scrobbles; skipping before half never scrobbles;
      `repeat=one` scrobbles each loop.
- [ ] Disconnect clears the row back to Connect and the keyring entry is gone.
- [ ] Both modes (Atmosphere/Instrument) and both densities render the rows;
      no unicode glyphs appear.
- [ ] Window resized to minimum width keeps the rows usable.
```

- [ ] **Step 2: Run the full verification**

Run: `cargo test --workspace`
Then: `cargo clippy --workspace -- -D warnings`
Then: `npm --prefix ui run check`
Then: `npm --prefix ui test`
Expected: all green; zero clippy warnings; svelte-check clean.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/notes/2026-09-29-track5-scrobbling-smoke.md
git commit -m "test: record the scrobbling smoke checklist"
```

---

## Self-Review

**Spec coverage**

- §1/§2 outcome and scope → Tasks 7–15.
- §3 S-D1 independent toggles → Tasks 7, 11, 14.
- §3 S-D2 Rust service → Tasks 2, 7–9.
- §3 S-D3 env/config key → Tasks 1, 10.
- §3 S-D4/S-D9 memory-only queue → Task 4 (`ScrobbleQueue` is in memory only; no serialization).
- §3 S-D5 per-service toggles only, no privacy control → Tasks 7, 14 (no separate pause control exists).
- §3 S-D6 status + one toast → Tasks 7–8 (sink + `notify_auth_error` once), 14 (listener).
- §3 S-D7 scrobble rule → Task 3.
- §3 S-D8 auth + keyring → Tasks 2, 9.
- §3 S-D10 presentation → Tasks 13–14.
- §5 components/flow → Tasks 2–10.
- §6 event semantics → Task 3.
- §7 auth → Tasks 5, 6, 9.
- §8 queue/retry/errors → Tasks 5, 6, 8.
- §9 UI → Tasks 12–14.
- §10 performance → inherent: no polling when idle (Task 8 worker sleeps on the channel + 30 s tick); tracker is integer-only (Task 3); no disk writes for the queue (Task 4).
- §11 testing → inline wiremock + pure tests across Tasks 2–9, vitest in Task 13, smoke in Task 15.
- §12 risks → covered by fixtures; §13 deferred → out of scope, no task (correct).

**Type/name consistency**

- `ScrobbleIntent` variants, `ScrobbleTracker::on_event`, `QueuedScrobble`, `ScrobbleQueue` methods, `LastFmClient`/`ListenBrainzClient` method names, `ScrobbleStatusDto`/`ScrobbleServiceStatus` fields, `LASTFM_SERVICE`/`LISTENBRAINZ_SERVICE`, and the command names are used identically in every task.
- IPC field names stay snake_case (project convention); invoke argument `token` matches the Rust `token: String`.

**Placeholder scan:** none — every step carries real code or an exact command.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-29-track5-scrobbling.md`.




