# Track 10.1 Session Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** OxiTide remembers the queue, current track, position, shuffle/repeat, and playing-playlist marker across restarts, restoring all of it **paused** with no audio, network, or output activity until the user presses play.

**Architecture:** A new `crates/oxitide-app/src/session.rs` owns a versioned `session.json` under `%APPDATA%\OxiTide\` (atomic tmp+rename, malformed files renamed `.bak`). A `SessionSaver` thread turns engine events into debounced writes and is flushed on exit. The audio engine gains `current_index` on `PlaybackState` and one `RestoreSession` command that loads the saved item order exactly (a plain `SetShuffle` would re-shuffle the tail) and stays paused, holding the saved position as a non-auto-start seek. The UI calls `get_restored_session` after either login path and applies it through the extended `player.restore()`.

**Tech Stack:** Rust (Tauri 2, serde, serde_json, std::sync::mpsc), Svelte 5 runes, TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-04-oxitide-track10-qol-design.md` (§5.1–§5.3, §6.1)

**Spec refinement (flagged for review):** the spec's `start_paused` flag on `PlayTracks`/`PlayShuffled` is implemented as a dedicated `EngineCmd::RestoreSession` instead: one variant covers paused start, exact saved order, shuffle/repeat, and position atomically, where flags on two existing variants could not preserve exact order (restoring shuffle via `SetShuffle` re-shuffles the tail).

## Global Constraints

- Windows-first; no CI. Every check is manual, run by the implementer.
- Windows gates after every Rust/UI task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- HTTP tests use `wiremock` only; this plan adds no HTTP. Engine tests use `NullOutput` and fake resolvers/fetchers only — never a real device.
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case. Payload field names follow the Rust DTO (snake_case here, matching `QueueItem`/`PlaybackState`); keep `ui/src/lib/api.ts` in sync by hand.
- Session data is local user data; never log titles, tokens, or file contents. Warnings name paths and reasons only.
- Do not edit `AGENTS.md` in this plan — the track completion note owns AGENTS.md updates (spec §10).
- UI work here is logic only (no new components); the semantic-token rule does not arise.
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green run instead.

## Review Focus

- **Malformed or future-version `session.json` must never block startup.** A corrupt file is renamed `.bak` and treated as absent; the app boots empty. Pinned by `malformed_session_is_backed_up_and_ignored`, `future_version_is_backed_up_and_ignored` (Task 1), and smoke step 7.
- **Restore must not play, fetch, or open the output until the user presses play.** Pinned by `restore_session_starts_paused_and_fetches_nothing` (Task 2, CountingFetcher assertion) and smoke step 1.
- **The restored shuffle order must be exactly what was saved** (no tail re-shuffle). Pinned by `restore_keeps_exact_order_and_starts_paused` (Task 2 queue test) and the engine restore order assertion.
- **Restoring must not double-count playlist recency** (the marker returns without counting a new play). Pinned by the `createPlaySource` restore test (Task 5).
- **Logout must clear the session file** so the next login cannot inherit a previous queue. Pinned by `saver_clear_removes_the_file` (Task 3) plus the `auth_logout` wiring (Task 4) and smoke step 4.

---

## File Structure

**Rust**

- Create `crates/oxitide-app/src/session.rs` — `Session`, `SessionItem`, path/load/save/clear, `SessionSaver` + tests.
- Modify `crates/oxitide-audio/src/queue.rs` — `Queue::restore` + tests.
- Modify `crates/oxitide-audio/src/engine.rs` — `PlaybackState.current_index`, `EngineCmd::RestoreSession` + handling + tests.
- Modify `crates/oxitide-app/src/media_controls.rs` — test `PlaybackState` literal gains `current_index`.
- Modify `crates/oxitide-app/src/scrobble/tracker.rs` — test `PlaybackState` literal gains `current_index`.
- Modify `crates/oxitide-app/src/state.rs` — `session_saver` accessor.
- Modify `crates/oxitide-app/src/commands.rs` — `RestoredSessionDto`, `get_restored_session`, `restore_session`, `set_play_source`, `clear_session`; logout clear; `build_player` records events.
- Modify `crates/oxitide-app/src/main.rs` — `mod session;`, start the saver, register commands, flush on `RunEvent::Exit`.

**UI**

- Modify `ui/src/lib/api.ts` — `RestoredSession`, `PlaybackState.current_index`, three wrappers.
- Modify `ui/src/lib/player.svelte.ts` — `PlayerState.current_index`, `restore()` rewrite.
- Modify `ui/src/lib/player.restore.test.ts` — new signature tests.
- Modify `ui/src/lib/playSource.svelte.ts` — `restore()` + optional persist callback.
- Modify `ui/src/lib/playSource.test.ts` — restore/persist tests.
- Modify `ui/src/App.svelte` — both login paths restore; expired snapshot carries shuffle/repeat/playSource.

**Docs**

- Create `docs/superpowers/plans/notes/2026-10-04-track10-1-session-restore-smoke.md` — human GUI checklist.

---

### Task 1: Session store

**Files:**
- Create: `crates/oxitide-app/src/session.rs`
- Modify: `crates/oxitide-app/src/main.rs:15` (module list)

**Interfaces:**
- Consumes: `oxitide_audio::queue::{QueueItem, RepeatMode}`.
- Produces: `session::{SESSION_VERSION, Session, SessionItem, session_path, load, save, clear}` — used by Tasks 3 and 4.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-app/src/session.rs` with only this test module (the implementation comes in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-session-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn item(id: u64) -> SessionItem {
        SessionItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 3,
            cover: None,
            explicit: None,
            ai: None,
        }
    }

    fn session() -> Session {
        Session {
            version: SESSION_VERSION,
            queue: vec![item(1), item(2), item(3)],
            current_index: Some(1),
            position_ms: 4200,
            shuffle: true,
            repeat: RepeatMode::All,
            play_source: Some("pl-1".into()),
            saved_at: 1_700_000_000,
        }
    }

    #[test]
    fn round_trip_preserves_everything_including_duplicates() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let mut original = session();
        original.queue = vec![item(1), item(2), item(1), item(3)];
        save(&path, &original).unwrap();
        assert_eq!(load(&path), Some(original));
        assert!(!dir.join("session.json.tmp").exists(), "temp file left behind");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_file_is_none() {
        let dir = temp_dir();
        assert_eq!(load(&dir.join("session.json")), None);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: a corrupt file must never reach startup.
    #[test]
    fn malformed_session_is_backed_up_and_ignored() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        fs::write(&path, "not json at all").unwrap();
        assert_eq!(load(&path), None);
        assert!(!path.exists());
        assert!(dir.join("session.json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: files written by a future OxiTide must not be trusted.
    #[test]
    fn future_version_is_backed_up_and_ignored() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        fs::write(&path, r#"{"version":99,"queue":[],"current_index":null,"position_ms":0,"shuffle":false,"repeat":"off","play_source":null,"saved_at":0}"#).unwrap();
        assert_eq!(load(&path), None);
        assert!(dir.join("session.json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_removes_the_file_and_tolerates_absence() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        save(&path, &session()).unwrap();
        clear(&path);
        assert!(!path.exists());
        clear(&path); // no panic on a missing file
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unwritable_path_returns_an_error_not_a_panic() {
        let dir = temp_dir();
        let blocker = dir.join("blocked");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("session.json");
        assert!(save(&path, &session()).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
```

- [ ] **Step 2: Register the module and verify the tests fail**

Add `mod session;` to `crates/oxitide-app/src/main.rs` after `mod scrobble;` (line 14).

Run: `cargo test -p oxitide-app session::`
Expected: compile failure — `Session`, `SessionItem`, `save`, `load`, `clear`, `SESSION_VERSION` do not exist yet.

- [ ] **Step 3: Write the implementation**

Add above the test module in `crates/oxitide-app/src/session.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use oxitide_audio::queue::{QueueItem, RepeatMode};
use serde::{Deserialize, Serialize};

pub const SESSION_VERSION: u32 = 1;

/// One queue item as persisted. `explicit`/`ai` are reserved for the Track 10.5
/// content filters; they are `None` until that plan populates them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionItem {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: u32,
    pub cover: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai: Option<bool>,
}

impl Default for SessionItem {
    fn default() -> Self {
        Self {
            track_id: 0,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration_secs: 0,
            cover: None,
            explicit: None,
            ai: None,
        }
    }
}

impl From<&QueueItem> for SessionItem {
    fn from(item: &QueueItem) -> Self {
        Self {
            track_id: item.track_id,
            title: item.title.clone(),
            artist: item.artist.clone(),
            album: item.album.clone(),
            duration_secs: item.duration_secs,
            cover: item.cover.clone(),
            explicit: None,
            ai: None,
        }
    }
}

impl From<SessionItem> for QueueItem {
    fn from(item: SessionItem) -> Self {
        Self {
            track_id: item.track_id,
            title: item.title,
            artist: item.artist,
            album: item.album,
            duration_secs: item.duration_secs,
            cover: item.cover,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub version: u32,
    pub queue: Vec<SessionItem>,
    pub current_index: Option<usize>,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub play_source: Option<String>,
    pub saved_at: u64,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            version: SESSION_VERSION,
            queue: Vec::new(),
            current_index: None,
            position_ms: 0,
            shuffle: false,
            repeat: RepeatMode::Off,
            play_source: None,
            saved_at: 0,
        }
    }
}

/// `%APPDATA%\OxiTide\session.json`, beside `config.toml`.
pub fn session_path() -> Option<PathBuf> {
    crate::config::config_path().map(|path| path.with_file_name("session.json"))
}

/// Read `path`; a missing file yields `None`, and a malformed or
/// future-version file is renamed `.bak` (so it cannot brick startup) and
/// yields `None`.
pub fn load(path: &Path) -> Option<Session> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Session>(&text) {
        Ok(session) if session.version == SESSION_VERSION => Some(session),
        Ok(session) => {
            tracing::warn!("unsupported session version {}; backing up", session.version);
            backup(path);
            None
        }
        Err(e) => {
            tracing::warn!("malformed session at {}: {e}", path.display());
            backup(path);
            None
        }
    }
}

/// Atomic write (temp file + rename), the `config.rs` pattern.
pub fn save(path: &Path, session: &Session) -> Result<(), String> {
    let text = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove the session file; a missing file is not an error.
pub fn clear(path: &Path) {
    if let Err(e) = fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("failed to clear session: {e}");
        }
    }
}

fn backup(path: &Path) {
    let backup = path.with_extension("json.bak");
    let _ = fs::rename(path, &backup);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app session::`
Expected: PASS (6 tests).

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/session.rs crates/oxitide-app/src/main.rs
git commit -m "feat(session): add the session store for queue persistence"
```

---

### Task 2: Paused restore in the engine

**Files:**
- Modify: `crates/oxitide-audio/src/queue.rs` (add `Queue::restore` + tests)
- Modify: `crates/oxitide-audio/src/engine.rs` (`PlaybackState.current_index`, `EngineCmd::RestoreSession`, handling + tests)
- Modify: `crates/oxitide-app/src/media_controls.rs:290-305` (test literal)
- Modify: `crates/oxitide-app/src/scrobble/tracker.rs:254-271` (test literal)

**Interfaces:**
- Consumes: `Queue`, `PlaybackState`, `EngineCmd`, `PendingSeek`.
- Produces: `Queue::restore(&mut self, Vec<QueueItem>, usize, bool, RepeatMode)`; `PlaybackState.current_index: Option<usize>`; `EngineCmd::RestoreSession { items, start, position_ms, shuffle, repeat }` — used by Task 4 and mirrored in `ui/src/lib/api.ts` (Task 5).

- [ ] **Step 1: Write the failing queue tests**

Add to `crates/oxitide-audio/src/queue.rs` inside `mod tests`:

```rust
    /// Review Focus: restoring a shuffled session must not re-shuffle it.
    #[test]
    fn restore_keeps_exact_order_and_starts_paused() {
        let mut q = Queue::default();
        q.restore((1..=3).map(item).collect(), 1, true, RepeatMode::All);
        assert_eq!(q.current_index(), Some(1));
        assert!(!q.is_playing());
        assert!(q.shuffle());
        assert_eq!(q.repeat(), RepeatMode::All);
        assert_eq!(
            q.items().iter().map(|i| i.track_id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn restore_clamps_the_start_index_and_handles_empty() {
        let mut q = Queue::default();
        q.restore((1..=2).map(item).collect(), 99, false, RepeatMode::Off);
        assert_eq!(q.current_index(), Some(1));
        q.restore(Vec::new(), 0, false, RepeatMode::Off);
        assert_eq!(q.current_index(), None);
        assert!(!q.is_playing());
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p oxitide-audio restore_keeps_exact_order -v`
Expected: FAIL — no method named `restore`.

- [ ] **Step 3: Implement `Queue::restore`**

Add after `play_context_shuffled_with` in `crates/oxitide-audio/src/queue.rs`:

```rust
    /// Restore a saved context exactly as persisted: the item order is kept
    /// (even with shuffle on — unlike `set_shuffle`, which re-shuffles the
    /// tail), the start index is clamped, and playback starts paused.
    pub fn restore(
        &mut self,
        items: Vec<QueueItem>,
        start: usize,
        shuffle: bool,
        repeat: RepeatMode,
    ) {
        self.items = items;
        self.shuffle = shuffle;
        self.repeat = repeat;
        if self.items.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(start.min(self.items.len() - 1));
            self.playing = false;
        }
    }
```

- [ ] **Step 4: Run the queue tests to verify they pass**

Run: `cargo test -p oxitide-audio restore_`
Expected: PASS (2 tests).

- [ ] **Step 5: Write the failing engine test**

Add to `crates/oxitide-audio/src/engine.rs` inside `mod tests` (helpers `item`, `engine_with`, `build_engine`, `fixture_plan`, `CountingFetcher` already exist):

```rust
    #[test]
    fn restore_session_starts_paused_and_fetches_nothing() {
        let (plan, files) = fixture_plan();
        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = Arc::new(CountingFetcher { files, calls: calls.clone() });
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            fetcher,
            Box::new(NullOutput::new()),
            Quality::Lossless,
        );
        engine
            .handle(EngineCmd::RestoreSession {
                items: vec![item(1), item(2), item(3)],
                start: 1,
                position_ms: 4200,
                shuffle: true,
                repeat: RepeatMode::All,
            })
            .unwrap();
        let state = engine.state();
        assert!(!state.playing, "restore must not start playback");
        assert_eq!(state.current_index, Some(1));
        assert_eq!(state.position_ms, 4200);
        assert!(state.shuffle);
        assert_eq!(state.repeat, RepeatMode::All);
        assert_eq!(
            state.queue.iter().map(|i| i.track_id).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "restore must keep the saved order even with shuffle on"
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "restore must not fetch media");
    }

    #[test]
    fn restore_session_resumes_at_the_saved_position() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        engine
            .handle(EngineCmd::RestoreSession {
                items: vec![item(1), item(2)],
                start: 0,
                position_ms: 1200,
                shuffle: false,
                repeat: RepeatMode::Off,
            })
            .unwrap();
        engine.handle(EngineCmd::PlayPause).unwrap();
        let state = engine.state();
        assert!(state.playing);
        assert_eq!(state.current_index, Some(0));
    }
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test -p oxitide-audio restore_session -v`
Expected: FAIL — no variant `RestoreSession`.

- [ ] **Step 7: Implement the engine changes**

In `crates/oxitide-audio/src/engine.rs`:

1. Add `current_index` to `PlaybackState` after `current` (line 39):

```rust
    pub current: Option<QueueItem>,
    pub current_index: Option<usize>,
```

2. In `build_state` (line 143-158) add after `current: q.current().cloned(),`:

```rust
        current_index: q.current_index(),
```

3. Add the variant to `EngineCmd` after `PlayShuffled` (line 65):

```rust
    /// Replace the queue with a persisted session and start paused: no output
    /// is opened and no media is fetched until the user presses play. The
    /// saved queue order is preserved even when `shuffle` is true.
    RestoreSession {
        items: Vec<QueueItem>,
        start: usize,
        position_ms: u64,
        shuffle: bool,
        repeat: RepeatMode,
    },
```

4. Add the handler arm in `handle` after the `PlayShuffled` arm (line 391-393):

```rust
            EngineCmd::RestoreSession { items, start, position_ms, shuffle, repeat } => {
                *self.inner.stream_meta.lock().unwrap() = None;
                let item = self.mutate_playing_queue(|q| {
                    q.restore(items, start, shuffle, repeat);
                    q.current().cloned()
                });
                self.inner.paused.store(true, Ordering::SeqCst);
                self.inner.position_ms.store(
                    if item.is_some() { position_ms } else { 0 },
                    Ordering::SeqCst,
                );
                self.inner.position_track.store(
                    item.as_ref().map(|i| i.track_id).unwrap_or(0),
                    Ordering::SeqCst,
                );
                // A non-auto-start seek keeps the worker idle at restore while
                // letting the first play resume from the saved position.
                *self.inner.seek_target.lock().unwrap() = item.as_ref().map(|i| PendingSeek {
                    track_id: i.track_id,
                    position_ms,
                    auto_start: false,
                });
                self.inner.notify_worker();
                self.emit(EngineEvent::NowPlaying(item));
                self.emit(EngineEvent::State(self.state()));
            }
```

5. Add `current_index: None,` to the two `PlaybackState` literals in test helpers: `crates/oxitide-app/src/media_controls.rs:290` and `crates/oxitide-app/src/scrobble/tracker.rs:255`.

- [ ] **Step 8: Run the engine tests and the full Rust gates**

Run: `cargo test -p oxitide-audio restore_; if ($?) { cargo test --workspace }; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green, including the new engine tests and the updated literals.

- [ ] **Step 9: Commit**

```bash
git add crates/oxitide-audio/src/queue.rs crates/oxitide-audio/src/engine.rs crates/oxitide-app/src/media_controls.rs crates/oxitide-app/src/scrobble/tracker.rs
git commit -m "feat(audio): add paused session restore to the engine"
```

---

### Task 3: SessionSaver

**Files:**
- Modify: `crates/oxitide-app/src/session.rs` (add saver + tests)

**Interfaces:**
- Consumes: `EngineEvent`, `PlaybackState` (Task 2), `session::{Session, save, clear}` (Task 1).
- Produces: `session::{SessionSaver, SESSION_DEBOUNCE}` with `start(path, debounce)`, `record(&EngineEvent)`, `set_play_source(Option<String>)`, `flush_now()`, `clear()` — used by Task 4.

- [ ] **Step 1: Write the failing saver tests**

Add to the `mod tests` in `crates/oxitide-app/src/session.rs` (plus the helper and imports shown):

```rust
    use oxitide_audio::engine::{EngineEvent, PlaybackState};
    use oxitide_tidal::stream::Quality;

    fn playback_state(ids: &[u64], current: Option<usize>, position_ms: u64) -> PlaybackState {
        let items: Vec<QueueItem> = ids
            .iter()
            .map(|id| QueueItem {
                track_id: *id,
                title: format!("T{id}"),
                artist: "A".into(),
                album: "B".into(),
                duration_secs: 3,
                cover: None,
            })
            .collect();
        PlaybackState {
            playing: false,
            buffering: false,
            current: current.and_then(|i| items.get(i).cloned()),
            current_index: current,
            position_ms,
            duration_ms: 3000,
            volume: 1.0,
            shuffle: true,
            repeat: RepeatMode::All,
            preferred_quality: Quality::Lossless,
            output_mode: oxitide_audio::output::OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            queue: items,
            stream: None,
        }
    }

    #[test]
    fn saver_flushes_state_and_position_on_demand() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let saver = SessionSaver::start(path.clone(), Duration::from_secs(60));
        saver.record(&EngineEvent::State(playback_state(&[1, 2, 3], Some(1), 1000)));
        saver.record(&EngineEvent::Position { position_ms: 4200, duration_ms: 3000 });
        saver.flush_now();
        let session = load(&path).expect("session written");
        assert_eq!(
            session.queue.iter().map(|i| i.track_id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(session.current_index, Some(1));
        assert_eq!(session.position_ms, 4200);
        assert!(session.shuffle);
        assert_eq!(session.repeat, RepeatMode::All);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saver_persists_the_play_source() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let saver = SessionSaver::start(path.clone(), Duration::from_secs(60));
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 0)));
        saver.set_play_source(Some("pl-1".into()));
        saver.flush_now();
        assert_eq!(load(&path).unwrap().play_source.as_deref(), Some("pl-1"));
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: clearing must survive the engine's trailing empty state.
    #[test]
    fn saver_clear_removes_the_file() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let saver = SessionSaver::start(path.clone(), Duration::from_secs(60));
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 0)));
        saver.flush_now();
        assert!(path.exists());
        saver.clear();
        saver.flush_now();
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saver_debounces_position_updates() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let saver = SessionSaver::start(path.clone(), Duration::from_millis(50));
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 100)));
        saver.flush_now();
        saver.record(&EngineEvent::Position { position_ms: 9000, duration_ms: 3000 });
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(load(&path).unwrap().position_ms, 9000);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saver_survives_an_unwritable_path() {
        let dir = temp_dir();
        let blocker = dir.join("blocked");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("session.json");
        let saver = SessionSaver::start(path, Duration::from_secs(60));
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 0)));
        saver.flush_now(); // must log, not panic
        let _ = fs::remove_dir_all(dir);
    }
```

Also add `use std::time::Duration;` to the test module's imports.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p oxitide-app saver_ -v`
Expected: FAIL — `SessionSaver` does not exist.

- [ ] **Step 3: Implement the saver**

Add above the test module in `crates/oxitide-app/src/session.rs`:

```rust
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxitide_audio::engine::{EngineEvent, PlaybackState};

/// State changes flush after this window; position ticks only mark dirty.
pub const SESSION_DEBOUNCE: Duration = Duration::from_secs(3);

enum SaverMsg {
    Event(EngineEvent),
    SetPlaySource(Option<String>),
    Flush(Sender<()>),
    Clear,
}

/// Writes `session.json` from engine events on a dedicated thread so the event
/// listener never does disk I/O. `flush_now` makes an exit write synchronous.
pub struct SessionSaver {
    tx: Sender<SaverMsg>,
}

impl SessionSaver {
    pub fn start(path: PathBuf, debounce: Duration) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        thread::Builder::new()
            .name("oxitide-session".into())
            .spawn(move || saver_main(path, debounce, rx))
            .expect("spawn session saver");
        Self { tx }
    }

    pub fn record(&self, event: &EngineEvent) {
        let _ = self.tx.send(SaverMsg::Event(event.clone()));
    }

    pub fn set_play_source(&self, id: Option<String>) {
        let _ = self.tx.send(SaverMsg::SetPlaySource(id));
    }

    /// Best-effort synchronous flush used on exit; waits briefly for the saver.
    pub fn flush_now(&self) {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        if self.tx.send(SaverMsg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv_timeout(Duration::from_secs(2));
        }
    }

    pub fn clear(&self) {
        let _ = self.tx.send(SaverMsg::Clear);
    }
}

fn session_from_state(state: &PlaybackState, play_source: Option<String>, saved_at: u64) -> Session {
    Session {
        version: SESSION_VERSION,
        queue: state.queue.iter().map(SessionItem::from).collect(),
        current_index: state.current_index,
        position_ms: state.position_ms,
        shuffle: state.shuffle,
        repeat: state.repeat,
        play_source,
        saved_at,
    }
}

fn saver_main(path: PathBuf, debounce: Duration, rx: Receiver<SaverMsg>) {
    let mut snapshot: Option<PlaybackState> = None;
    let mut play_source: Option<String> = None;
    let mut dirty = false;
    let mut urgent = false;
    let mut last_flush = Instant::now();
    loop {
        let wait = if dirty && !urgent {
            debounce
                .saturating_sub(last_flush.elapsed())
                .min(Duration::from_millis(100))
        } else {
            Duration::from_millis(100)
        };
        match rx.recv_timeout(wait) {
            Ok(SaverMsg::Event(EngineEvent::State(state))) => {
                snapshot = Some(state);
                dirty = true;
                urgent = true;
            }
            Ok(SaverMsg::Event(EngineEvent::Position { position_ms, .. })) => {
                if let Some(state) = snapshot.as_mut() {
                    state.position_ms = position_ms;
                    dirty = true;
                }
            }
            Ok(SaverMsg::Event(_)) => {}
            Ok(SaverMsg::SetPlaySource(id)) => {
                play_source = id;
                dirty = true;
                urgent = true;
            }
            Ok(SaverMsg::Flush(ack)) => {
                if dirty {
                    write_snapshot(&path, snapshot.as_ref(), &play_source);
                    dirty = false;
                    urgent = false;
                    last_flush = Instant::now();
                }
                let _ = ack.send(());
            }
            Ok(SaverMsg::Clear) => {
                clear(&path);
                snapshot = None;
                play_source = None;
                dirty = false;
                urgent = false;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if dirty && (urgent || last_flush.elapsed() >= debounce) {
            write_snapshot(&path, snapshot.as_ref(), &play_source);
            dirty = false;
            urgent = false;
            last_flush = Instant::now();
        }
    }
}

fn write_snapshot(path: &Path, snapshot: Option<&PlaybackState>, play_source: &Option<String>) {
    let Some(state) = snapshot else { return };
    let saved_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let session = session_from_state(state, play_source.clone(), saved_at);
    if let Err(e) = save(path, &session) {
        tracing::warn!("failed to save session: {e}");
    }
}
```

- [ ] **Step 4: Run the saver tests to verify they pass**

Run: `cargo test -p oxitide-app saver_`
Expected: PASS (5 tests).

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green (this also compiles the saver against the Task 2 engine changes).

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/session.rs
git commit -m "feat(session): persist playback state from engine events"
```

---

### Task 4: Commands and wiring

**Files:**
- Modify: `crates/oxitide-app/src/state.rs` (accessor)
- Modify: `crates/oxitide-app/src/commands.rs` (DTO + 4 commands, logout clear, `build_player` record)
- Modify: `crates/oxitide-app/src/main.rs` (start saver, register commands, exit flush)

**Interfaces:**
- Consumes: `session::{SessionSaver, load, session_path}` (Tasks 1/3); `EngineCmd::RestoreSession` (Task 2).
- Produces (IPC): `get_restored_session() -> Option<RestoredSessionDto>`, `restore_session(items, start, positionMs, shuffle, repeat)`, `set_play_source(id)`, `clear_session()` — consumed by Task 5.

- [ ] **Step 1: Add the `AppState` accessor**

In `crates/oxitide-app/src/state.rs`, add the field to `AppState` after `scrobbler`:

```rust
    /// Persists the playback session across restarts; `None` when the app has
    /// no writable data directory.
    pub session_saver: Mutex<Option<Arc<crate::session::SessionSaver>>>,
```

and the accessors after `scrobbler()`:

```rust
    pub fn set_session_saver(&self, saver: Arc<crate::session::SessionSaver>) {
        *self.session_saver.lock().unwrap() = Some(saver);
    }

    pub fn session_saver(&self) -> Option<Arc<crate::session::SessionSaver>> {
        self.session_saver.lock().unwrap().clone()
    }
```

Add a state test alongside the existing ones:

```rust
    #[test]
    fn session_saver_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.session_saver().is_none());
        let dir = std::env::temp_dir().join(format!(
            "oxitide-state-saver-{}",
            std::process::id()
        ));
        let saver = Arc::new(crate::session::SessionSaver::start(
            dir.join("session.json"),
            std::time::Duration::from_secs(60),
        ));
        state.set_session_saver(saver);
        assert!(state.session_saver().is_some());
    }
```

- [ ] **Step 2: Run the state test**

Run: `cargo test -p oxitide-app session_saver_starts_unset -v`
Expected: FAIL to compile until the field/accessor exist; then PASS.

- [ ] **Step 3: Add the commands**

In `crates/oxitide-app/src/commands.rs`, add next to `get_playback_state` (line 1269):

```rust
/// The persisted session as the UI should apply it, or `None` when no usable
/// file exists. Playback is applied paused by `restore_session`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestoredSessionDto {
    pub queue: Vec<QueueItem>,
    pub current_index: usize,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub play_source: Option<String>,
}

#[tauri::command]
pub fn get_restored_session() -> Result<Option<RestoredSessionDto>, String> {
    let Some(path) = crate::session::session_path() else {
        return Ok(None);
    };
    Ok(crate::session::load(&path).map(|session| RestoredSessionDto {
        current_index: session.current_index.unwrap_or(0),
        position_ms: session.position_ms,
        shuffle: session.shuffle,
        repeat: session.repeat,
        play_source: session.play_source,
        queue: session.queue.into_iter().map(QueueItem::from).collect(),
    }))
}

#[tauri::command]
pub fn restore_session(
    state: State<'_, AppState>,
    items: Vec<QueueItem>,
    start: usize,
    position_ms: u64,
    shuffle: bool,
    repeat: RepeatMode,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::RestoreSession { items, start, position_ms, shuffle, repeat })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_play_source(state: State<'_, AppState>, id: Option<String>) -> Result<(), String> {
    if let Some(saver) = state.session_saver() {
        saver.set_play_source(id);
    }
    Ok(())
}

#[tauri::command]
pub fn clear_session(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(saver) = state.session_saver() {
        saver.clear();
    }
    Ok(())
}
```

- [ ] **Step 4: Wire recording, logout, startup, and exit**

1. In `build_player` (commands.rs:433-453), inside the first `engine.subscribe` closure, record before the emit:

```rust
    engine.subscribe(Arc::new(move |event| {
        if let Some(saver) = handle.state::<AppState>().session_saver() {
            saver.record(&event);
        }
        if let EngineEvent::Error { code, .. } = &event {
```

2. In `auth_logout` (commands.rs:227-239), before `state.logout()`:

```rust
    if let Some(saver) = state.session_saver() {
        saver.clear();
    }
```

3. In `main.rs`, after `state.set_config(config)` (line 185):

```rust
    if let Some(path) = session::session_path() {
        state.set_session_saver(std::sync::Arc::new(session::SessionSaver::start(
            path,
            session::SESSION_DEBOUNCE,
        )));
    }
```

4. In `main.rs` `RunEvent::Exit` (line 394-403), before the cache handling:

```rust
                if let Some(saver) = state.session_saver() {
                    saver.flush_now();
                }
```

5. Register the four commands in `generate_handler!` after `commands::get_playback_state` (line 350):

```rust
            commands::get_restored_session,
            commands::restore_session,
            commands::set_play_source,
            commands::clear_session,
```

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green. (`cargo build -p oxitide-app` is implied by the workspace tests but run `cargo build -p oxitide-app` if anything looks cached.)

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(session): add restore and play-source commands"
```

---

### Task 5: UI restore path

**Files:**
- Modify: `ui/src/lib/api.ts` (types + 3 wrappers)
- Modify: `ui/src/lib/player.svelte.ts` (`restore` rewrite, `current_index`)
- Modify: `ui/src/lib/player.restore.test.ts` (rewrite)
- Modify: `ui/src/lib/playSource.svelte.ts` (`restore`, persist)
- Modify: `ui/src/lib/playSource.test.ts` (add tests)
- Modify: `ui/src/App.svelte` (both login paths)

**Interfaces:**
- Consumes: IPC from Task 4 (`get_restored_session`, `restore_session`, `set_play_source`); `PlaybackState.current_index` (Task 2).
- Produces: `player.restore(queue, index, positionMs, shuffle, repeat)`; `playSource.restore(id)`; `api.getRestoredSession/restoreSession/setPlaySource`.

- [ ] **Step 1: Write the failing UI tests**

Replace `ui/src/lib/player.restore.test.ts` with:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  restoreSession: vi.fn(),
  getPlaybackState: vi.fn(),
}));

vi.mock("./api", () => ({ api: mocks }));

import { createPlayer, emptyState } from "./player.svelte";

const queue = [
  { track_id: 1, title: "A", artist: "X", album: "", duration_secs: 10, cover: null },
  { track_id: 2, title: "B", artist: "Y", album: "", duration_secs: 10, cover: null },
];

beforeEach(() => {
  mocks.restoreSession.mockReset().mockResolvedValue(undefined);
  mocks.getPlaybackState.mockReset().mockResolvedValue({
    ...emptyState(),
    current: queue[1],
    queue,
    current_index: 1,
  });
});

describe("player.restore", () => {
  it("restores the saved session paused, with shuffle and repeat", async () => {
    const player = createPlayer();
    await player.restore(queue, 1, 1500, true, "all");

    expect(mocks.restoreSession).toHaveBeenCalledWith(queue, 1, 1500, true, "all");
    expect(mocks.getPlaybackState).toHaveBeenCalled();
  });

  it("does nothing for an empty queue", async () => {
    const player = createPlayer();
    await player.restore([], 0, 0, false, "off");

    expect(mocks.restoreSession).not.toHaveBeenCalled();
    expect(mocks.getPlaybackState).not.toHaveBeenCalled();
  });
});
```

Add to `ui/src/lib/playSource.test.ts` inside `describe("createPlaySource")`:

```ts
  /// Review Focus: restoring must not count as a new play in the recency log.
  it("restore re-applies the source without reporting a play", () => {
    const onPlay = vi.fn();
    const source = createPlaySource(onPlay);
    source.restore("p9");
    expect(source.playlistId).toBe("p9");
    expect(onPlay).not.toHaveBeenCalled();
  });

  it("persists every source change, including clearing", () => {
    const persist = vi.fn();
    const source = createPlaySource(undefined, persist);
    source.set("p1");
    source.set(null);
    expect(persist.mock.calls).toEqual([["p1"], [null]]);
  });
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npm --prefix ui test -- src/lib/player.restore.test.ts src/lib/playSource.test.ts`
Expected: FAIL — `restoreSession` not called / `source.restore` is not a function.

- [ ] **Step 3: Add the IPC types and wrappers**

In `ui/src/lib/api.ts`:

1. Add `current_index` to `PlaybackState` after `current` (line 129):

```ts
  playing: boolean; buffering: boolean; current?: QueueItem | null;
  current_index?: number | null;
```

2. Add the `RestoredSession` interface after `PlaybackState`:

```ts
export interface RestoredSession {
  queue: QueueItem[];
  current_index: number;
  position_ms: number;
  shuffle: boolean;
  repeat: RepeatMode;
  play_source: string | null;
}
```

3. Add the wrappers after `getPlaybackState` (line 313):

```ts
  getRestoredSession: () => invoke<RestoredSession | null>("get_restored_session"),
  restoreSession: (
    items: QueueItem[],
    start: number,
    positionMs: number,
    shuffle: boolean,
    repeat: RepeatMode,
  ) => invoke<void>("restore_session", { items, start, positionMs, shuffle, repeat }),
  setPlaySource: (id: string | null) => invoke<void>("set_play_source", { id }),
```

- [ ] **Step 4: Update the player store**

In `ui/src/lib/player.svelte.ts`:

1. Add to `PlayerState` after `current` (line 7):

```ts
  current_index: number | null;
```

2. Add to `emptyState()` after `current: null,`:

```ts
    current_index: null,
```

3. Replace `restore` (lines 155-170) with:

```ts
  /// Rebuild the queue from a saved session: the engine loads the exact order
  /// paused at the saved position, so nothing plays until the user asks.
  async function restore(
    queue: QueueItem[],
    index: number,
    positionMs: number,
    shuffle: boolean,
    repeat: RepeatMode,
  ) {
    if (queue.length === 0) return;
    await api.restoreSession(queue, index, positionMs, shuffle, repeat);
    const snapshot = await api.getPlaybackState();
    applyEvent(state, { type: "state", ...snapshot });
  }
```

- [ ] **Step 5: Update playSource**

Replace the exported factory and singleton in `ui/src/lib/playSource.svelte.ts` (add the `api` import at the top):

```ts
import { api } from "./api";
```

```ts
export function createPlaySource(
  onPlay?: (uuid: string) => void,
  persist?: (id: string | null) => void,
) {
  let playlistId = $state<string | null>(null);
  return {
    get playlistId() { return playlistId; },
    set(id: string | null) {
      playlistId = id;
      persist?.(id);
      if (id !== null) onPlay?.(id);
    },
    /// Re-apply a saved source without recording a new play (session restore).
    restore(id: string | null) {
      playlistId = id;
    },
  };
}

export const playSource = createPlaySource(
  (id) => playlistRecency().record(id),
  (id) => { void api.setPlaySource(id).catch(() => {}); },
);
```

- [ ] **Step 6: Update App.svelte**

1. Imports: add `RepeatMode` to the `./lib/api` type import and add:

```ts
  import { playSource } from "./lib/playSource.svelte";
```

2. Extend `ExpiredSession` (line 55-60):

```ts
  interface ExpiredSession {
    queue: QueueItem[];
    index: number;
    position: number;
    shuffle: boolean;
    repeat: RepeatMode;
    playSource: string | null;
  }
```

3. In the `session_expired` listener snapshot (line 137-144), replace the object with:

```ts
      expired = {
        queue: snapshot.queue,
        index:
          snapshot.current_index ??
          (current
            ? Math.max(0, snapshot.queue.findIndex((item) => item.track_id === current.track_id))
            : 0),
        position: snapshot.position_ms,
        shuffle: snapshot.shuffle,
        repeat: snapshot.repeat,
        playSource: playSource.playlistId,
      };
```

4. Add the shared disk-restore helper above `onLogin` and rewrite `onLogin`:

```ts
  async function restoreSavedSession() {
    try {
      const session = await api.getRestoredSession();
      if (!session) return;
      await player.restore(
        session.queue,
        session.current_index,
        session.position_ms,
        session.shuffle,
        session.repeat,
      );
      playSource.restore(session.play_source);
    } catch (e) {
      error = String(e);
    }
  }

  async function onLogin() {
    authError = "";
    clearPagerCache();
    clearPlaylistCovers();
    error = "";
    const restore = expired;
    expired = null;
    if (restore) {
      try {
        await player.restore(
          restore.queue,
          restore.index,
          restore.position,
          restore.shuffle,
          restore.repeat,
        );
        playSource.restore(restore.playSource);
      } catch (e) {
        error = String(e);
      }
    } else {
      await restoreSavedSession();
    }
    loggedIn = true;
  }
```

5. In the `authRestore` effect (line 276-288), restore before flipping `loggedIn` so the player store never applies a pre-restore snapshot last:

```ts
  $effect(() => {
    api
      .authRestore()
      .then(async (session) => {
        if (session) {
          await restoreSavedSession();
          loggedIn = true;
        }
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        booting = false;
      });
  });
```

- [ ] **Step 7: Run the UI gates**

Run: `npm --prefix ui test -- src/lib/player.restore.test.ts src/lib/playSource.test.ts; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 8: Run the full Windows gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/player.svelte.ts ui/src/lib/player.restore.test.ts ui/src/lib/playSource.svelte.ts ui/src/lib/playSource.test.ts ui/src/App.svelte
git commit -m "feat(ui): restore the saved session on login"
```

---

### Task 6: Smoke checklist and verification

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-04-track10-1-session-restore-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the human verification record for this plan.

- [ ] **Step 1: Write the checklist**

Create `docs/superpowers/plans/notes/2026-10-04-track10-1-session-restore-smoke.md`:

```markdown
# Track 10.1 session-restore smoke checklist (2026-10-04)

Human check with a real account. Run `npm --prefix ui run app:dev` (Vite + Tauri).
Mark each item pass/fail with a note; failures become plan fixes, not silent exceptions.

1. Queue in, app out
   - Play a playlist (the sidebar row shows the playing marker), pause mid-track,
     note the position, close the window (X).
   - Relaunch, log in if asked.
   - PASS = queue rail shows the same items in the same order, the player bar shows
     the same track at the same position, shuffle/repeat match what was set, the
     playlist marker is still lit, and playback is paused.
2. Resume
   - Press play. PASS = playback resumes at the saved position (no restart from 0,
     no blip before the press).
3. Edits
   - Reorder or remove a queue item, then quit and relaunch.
     PASS = the edited order is restored.
4. Logout
   - Log out. PASS = queue empties and `%APPDATA%\OxiTide\session.json` is gone.
   - Log back in. PASS = the app starts with an empty queue, not the old one.
5. Corrupt file
   - Quit. Overwrite `session.json` with `garbage`. Launch.
     PASS = app boots to an empty queue, no error dialog, and `session.json.bak` exists.
6. Mini player
   - With `start_in_mini` on and a saved queue, launch. PASS = mini player shows the
     restored track paused; exit mini shows the full queue.
7. Empty queue
   - Clear the queue, quit, relaunch. PASS = empty state, no errors.

Record results below.

- [ ] 1 queue in/out
- [ ] 2 resume position
- [ ] 3 edits survive
- [ ] 4 logout clears + fresh login empty
- [ ] 5 corrupt file boots empty
- [ ] 6 mini player restore
- [ ] 7 empty queue
```

- [ ] **Step 2: Run the manual smoke**

Run: `npm --prefix ui run app:dev` and work the checklist. Record results in the file (tick the boxes; add a note for any failure).

- [ ] **Step 3: Run the final gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-04-track10-1-session-restore-smoke.md
git commit -m "docs: add the Track 10.1 session-restore smoke checklist"
```

---

## Self-Review

- **Spec coverage (spec §5.1–§5.3, §6.1):** `session.json` fields/version/atomic writes/malformed-`.bak` → Task 1; debounced save + exit flush + no position-path I/O → Task 3; `current_index` → Task 2; paused restore without blip → Tasks 2/5; logout clear → Task 4; play-source marker persistence → Tasks 3/4/5; expired-snapshot preference yielding to (now extended) restore → Task 5; commands/`generate_handler!` → Task 4. Content-filter flags have their fields reserved (`SessionItem.explicit/ai`) and are populated in Plan 10.5. The spec's `start_paused` refinement is declared in the header.
- **Placeholder scan:** no TBD/TODO; every code step carries complete code.
- **Type consistency:** `Session.current_index: Option<usize>` → `RestoredSessionDto.current_index: usize` → `RestoredSession.current_index: number`; `Queue::restore` matches `EngineCmd::RestoreSession` argument order; `player.restore(queue, index, positionMs, shuffle, repeat)` matches the call in both App paths and the test; `SessionSaver` method names are identical in Rust, tests, and the commands wiring.
- **Review Focus:** each line maps to a named test or smoke step (listed at the top).
