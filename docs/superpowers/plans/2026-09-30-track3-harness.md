# Track 3 Phase A — Performance Harness & Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Track 3 measurement harness (Rust milestones + audio hooks + UI frame sampler + scripted runs + budget gate) and record the baseline the rest of the wave is measured against.

**Architecture:** An opt-in perf session is created at process start from two env vars (`OXITIDE_PERF_SCENARIO`, `OXITIDE_PERF_DIR`) and writes flushed JSONL lines (marks, audio events, TTFS, memory samples, UI frame metrics) to `.superpowers/perf/runs/`. The audio crate exposes a plain `AudioPerfEvent` hook so it stays backend-independent; `oxitide-app` installs the hook and serves three Tauri commands (`perf_mark`, `get_perf_snapshot`, `perf_submit_ui_metrics`). The UI samples rAF frame times and long tasks only while a session is active and shows a HUD; a Node budget checker (`scripts/perf/check.mjs`) aggregates runs and gates against `scripts/perf/budgets.json`. Normal runs record nothing.

**Tech Stack:** Rust stable (edition per workspace), Tauri 2, `serde_json`, `windows` crate (process metrics), Svelte 5 runes, vitest, Node 20+ (`node --test`, `fetch`, global `WebSocket` for CDP), PowerShell 5.1.

**Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track3-performance-design.md`

## Global Constraints

- **Behavior-preserving (T3-D5):** every perf path is inert unless `perf::session()` returns `Some`; no visible UI or animation change in normal runs; no playback-semantics change.
- **Budgets** are absolute p95 over ≥5 measured runs; `scripts/perf/budgets.json` mirrors the spec table and is the gate.
- **Commands:** any new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs`; `invoke` sends camelCase args, Rust params stay snake_case.
- **Tests:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`; `npm --prefix ui run check`; `npm --prefix ui run test`. HTTP tests never hit live TIDAL (wiremock only). Never log token values.
- **No new runtime dependencies.** Windows-only code is `#[cfg(windows)]` with a safe non-Windows fallback.
- **JSONL discipline:** every line is written with a single `write_all` + `flush`; a killed process may truncate only the final line, never earlier ones.
- **Run artifacts:** raw runs under `.superpowers/perf/runs/` (gitignored); committed summaries under `docs/superpowers/plans/notes/`.

## Review Focus

- **Normal runs (no session):** zero files written, no HUD, no rAF/observer work, no measurable overhead — every hook checks `perf::session()` first.
- **Missing/unwritable perf dir:** one `tracing::warn`, session disabled, app starts and plays normally — `create_in` failure must never propagate to `main`.
- **Kill -9 mid-run:** JSONL remains parseable up to the last complete line (single write + flush per line, no multi-line records).
- **Clock anomalies:** `GetProcessTimes` failure or a process-start time after `main()` start must not panic; marks still carry monotonic `t_ms`.
- **Driver brittleness:** a missing Vite port, no CDP target, or an app that fails to exit must produce a non-zero script exit with a clear message within a bounded time — never a hang.

---

### Task 1: Audio perf hook

**Files:**
- Create: `crates/oxitide-audio/src/perf.rs`
- Modify: `crates/oxitide-audio/src/lib.rs` (add `pub mod perf;` after `pub mod output;`)
- Modify: `crates/oxitide-audio/src/engine.rs`
- Test: `crates/oxitide-audio/src/engine.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing new.
- Produces: `oxitide_audio::perf::AudioPerfEvent` (enum below), `oxitide_audio::perf::PerfHook = Arc<dyn Fn(AudioPerfEvent) + Send + Sync>`, `Engine::set_perf_hook(&self, hook: PerfHook)`. Task 2 forwards these events into the JSONL session; Task 3 installs the hook in `build_player`.

- [ ] **Step 1: Write the failing test**

Add to `crates/oxitide-audio/src/engine.rs`'s `mod tests` (near `set_output_device_is_passed_to_open`, which shows the `build_engine` + `wait_for` patterns):

```rust
#[test]
fn perf_hook_receives_decode_cache_and_first_pcm_events() {
    let (plan, files) = fixture_plan();
    let events: Arc<Mutex<Vec<crate::perf::AudioPerfEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let engine = build_engine(
        Arc::new(FakeResolver { plan }),
        Arc::new(crate::fetch::MapFetcher { files }),
        Box::new(NullOutput::new()),
        Quality::Lossless,
    );
    engine.set_perf_hook(Arc::new(move |event| sink.lock().unwrap().push(event)));
    engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

    wait_for(|| {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, crate::perf::AudioPerfEvent::FirstPcm { track_id: 1 }))
    });
    let events = events.lock().unwrap().clone();
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::perf::AudioPerfEvent::DecodeStart { track_id: 1 })));
    // The fixture plan has no manifest hash, so it cannot be cached.
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::perf::AudioPerfEvent::CacheMiss { track_id: 1 })));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxitide-audio perf_hook_receives_decode_cache_and_first_pcm_events`
Expected: FAIL — `crate::perf` unresolved / `set_perf_hook` not found.

- [ ] **Step 3: Create `crates/oxitide-audio/src/perf.rs`**

```rust
//! Performance events emitted by the audio engine. The app installs a hook to
//! timestamp them; the audio crate stays independent of any metrics backend.

use std::sync::Arc;

/// A milestone the engine reports to an installed [`PerfHook`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioPerfEvent {
    /// The manifest resolved (playback info fetched and parsed).
    DecodeStart { track_id: u64 },
    /// A usable disk-cache entry was found for the track.
    CacheHit { track_id: u64 },
    /// No usable cache entry: no cache key, no entry, or a failed hit.
    CacheMiss { track_id: u64 },
    /// The first PCM buffer of a track was written to the output.
    FirstPcm { track_id: u64 },
    /// A different track started after one ended; `gap_ms` is the time since
    /// the previous track's last PCM write.
    Advance { track_id: u64, gap_ms: u64 },
}

/// Called on the worker/decode threads; must be cheap and non-blocking.
pub type PerfHook = Arc<dyn Fn(AudioPerfEvent) + Send + Sync>;
```

- [ ] **Step 4: Wire the hook into `engine.rs`**

Add the import near the other `use crate::` lines at the top of `engine.rs`:

```rust
use crate::perf::{AudioPerfEvent, PerfHook};
```

Add the field to `struct Inner` (next to `listeners: Mutex<Vec<Listener>>,`):

```rust
    perf_hook: Mutex<Option<PerfHook>>,
```

In `Engine::new`'s `Inner { ... }` initializer (next to `listeners: Mutex::new(Vec::new()),`) add:

```rust
            perf_hook: Mutex::new(None),
```

Add the method on `impl Engine` (next to `set_output_device`):

```rust
    /// Install a perf hook. Events are delivered on engine threads and must be
    /// cheap; the engine never waits on the hook.
    pub fn set_perf_hook(&self, hook: PerfHook) {
        *self.inner.perf_hook.lock().unwrap() = Some(hook);
    }
```

Add the free helper next to `emit`:

```rust
fn fire_perf(inner: &Inner, event: AudioPerfEvent) {
    let hook = inner.perf_hook.lock().unwrap().clone();
    if let Some(hook) = hook {
        hook(event);
    }
}
```

In `decode_track`, after the HE-AAC rejection block and before `let cache_key = ...`, emit the decode start:

```rust
    fire_perf(inner, AudioPerfEvent::DecodeStart { track_id: item.track_id });
```

Replace the cache lookup block (`if let Some(key) = cache_key { ... }`) so hits and misses are reported (hit on lookup, miss on absent key, absent entry, or failed hit):

```rust
    match cache_key {
        Some(key) => match inner.cache.lookup(&key) {
            Some(path) => {
                fire_perf(inner, AudioPerfEvent::CacheHit { track_id: item.track_id });
                inner.cache.protect(&key);
                let result =
                    decode_from_file(&inner, &item, mode, generation, seek_ms, &tx, &path, &plan);
                inner.cache.unprotect(&key);
                match result {
                    Ok(()) => return,
                    Err(e) => {
                        if inner.generation.load(Ordering::SeqCst) != generation {
                            let _ = tx.send(Err(e));
                            return;
                        }
                        tracing::warn!(
                            "cache entry {} failed to decode ({e}); refetching",
                            key.file_name()
                        );
                        inner.cache.remove(&key);
                        fire_perf(inner, AudioPerfEvent::CacheMiss { track_id: item.track_id });
                    }
                }
            }
            None => fire_perf(inner, AudioPerfEvent::CacheMiss { track_id: item.track_id }),
        },
        None => fire_perf(inner, AudioPerfEvent::CacheMiss { track_id: item.track_id }),
    }
    decode_from_network(inner, item, mode, generation, seek_ms, tx, buffer, plan, cache_key);
```

In `run_session`, declare the tracking state just before `let mut stream = start_track(...)`:

```rust
    let mut first_write_of_track = true;
    let mut last_write: Option<(u64, Instant)> = None;
```

Reset it at the top of the outer loop, right after `let track_id = stream.item.track_id;`:

```rust
        first_write_of_track = true;
```

Inside the `Ok(Ok(StreamEvent::Samples(buf)))` arm, after the `frames += buf.frames(channels);` line, add:

```rust
                    if first_write_of_track {
                        first_write_of_track = false;
                        fire_perf(inner, AudioPerfEvent::FirstPcm { track_id });
                        if let Some((previous, at)) = last_write {
                            if previous != track_id {
                                fire_perf(
                                    inner,
                                    AudioPerfEvent::Advance {
                                        track_id,
                                        gap_ms: at.elapsed().as_millis() as u64,
                                    },
                                );
                            }
                        }
                    }
                    last_write = Some((track_id, Instant::now()));
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio perf_hook_receives_decode_cache_and_first_pcm_events`
Expected: PASS.

- [ ] **Step 6: Run the full audio suite and clippy**

Run: `cargo test -p oxitide-audio; if ($?) { cargo clippy -p oxitide-audio -- -D warnings }`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/oxitide-audio/src/perf.rs crates/oxitide-audio/src/lib.rs crates/oxitide-audio/src/engine.rs
git commit -m "feat(audio): add an opt-in perf event hook"
```

---

### Task 2: App perf core module

**Files:**
- Create: `crates/oxitide-app/src/perf.rs`
- Modify: `crates/oxitide-app/Cargo.toml` (windows features)
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `oxitide_audio::perf::AudioPerfEvent` (Task 1).
- Produces (used by Task 3 and the UI):
  - `perf::install() -> Option<Arc<PerfSession>>`
  - `perf::session() -> Option<&'static Arc<PerfSession>>`
  - `perf::mark(name: &str)`, `perf::note_play_command()`, `perf::audio_event(event: AudioPerfEvent)`, `perf::ui_metrics(serde_json::Value)`
  - `perf::snapshot() -> PerfSnapshot` (serde `Serialize`, camelCase-free snake_case fields)
  - `PerfSession::create_in(dir: &Path, scenario: &str) -> std::io::Result<Arc<PerfSession>>`

- [ ] **Step 1: Add the windows features to `crates/oxitide-app/Cargo.toml`**

In `[target.'cfg(windows)'.dependencies]` add to the `features` list:

```toml
    "Win32_System_Threading",
    "Win32_System_ProcessStatus",
    "Win32_System_Diagnostics_ToolHelp",
```

- [ ] **Step 2: Write the failing tests**

Create `crates/oxitide-app/src/perf.rs` with the module and its tests (implementation stubs first are not needed — write both in Step 4; this step writes the test bodies at the bottom of the file):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_audio::perf::AudioPerfEvent;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("oxitide-perf-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn run_id_contains_the_scenario() {
        let session = PerfSession::create_in(&temp_dir("runid"), "S1").unwrap();
        assert!(session.run_id.starts_with("S1-"));
    }

    #[test]
    fn marks_are_flushed_as_parseable_jsonl() {
        let dir = temp_dir("marks");
        let session = PerfSession::create_in(&dir, "S1").unwrap();
        session.mark("config_loaded");
        session.mark("window_shown");
        let text = std::fs::read_to_string(session.jsonl_path()).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("every line must be valid JSON"))
            .collect();
        assert_eq!(lines[0]["kind"], "session");
        assert_eq!(lines[1]["kind"], "mark");
        assert_eq!(lines[1]["name"], "config_loaded");
        assert_eq!(lines[2]["name"], "window_shown");
        assert!(lines[1]["t_ms"].as_u64().is_some());
    }

    #[test]
    fn first_pcm_after_a_play_command_records_ttfs() {
        let session = PerfSession::create_in(&temp_dir("ttfs"), "S3-hit").unwrap();
        session.note_play_command();
        std::thread::sleep(std::time::Duration::from_millis(5));
        session.audio_event(AudioPerfEvent::FirstPcm { track_id: 7 });
        let snapshot = session.snapshot();
        let ttfs = snapshot.last_ttfs_ms.expect("ttfs recorded");
        assert!(ttfs >= 5, "ttfs should include the sleep, got {ttfs}");
    }

    #[test]
    fn audio_events_count_cache_hits_and_misses() {
        let session = PerfSession::create_in(&temp_dir("counters"), "S8").unwrap();
        session.audio_event(AudioPerfEvent::CacheHit { track_id: 1 });
        session.audio_event(AudioPerfEvent::CacheMiss { track_id: 2 });
        session.audio_event(AudioPerfEvent::CacheMiss { track_id: 3 });
        let snapshot = session.snapshot();
        assert_eq!(snapshot.cache_hits, 1);
        assert_eq!(snapshot.cache_misses, 2);
    }

    #[test]
    fn create_in_fails_when_the_path_is_a_file() {
        let file = temp_dir("file");
        std::fs::write(&file, b"not a dir").unwrap();
        assert!(PerfSession::create_in(&file, "S1").is_err());
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn snapshot_without_a_session_is_inactive() {
        let snapshot = snapshot_for(None);
        assert!(!snapshot.active);
        assert!(snapshot.marks.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn process_tree_memory_is_positive() {
        let mb = process_tree_memory_mb().expect("measurement available on Windows");
        assert!(mb > 0, "expected a positive working set, got {mb}");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p oxitide-app perf::`
Expected: FAIL — module does not compile yet.

- [ ] **Step 4: Implement `crates/oxitide-app/src/perf.rs`**

```rust
//! Performance harness core: process-start milestones, audio/cache counters,
//! memory samples, and the JSONL report sink for a perf session.
//!
//! Inactive unless `OXITIDE_PERF_SCENARIO` is set at launch. In normal runs
//! nothing is written and the per-event cost is a single `OnceLock` read.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxitide_audio::perf::AudioPerfEvent;
use serde::Serialize;

pub const SCENARIO_ENV: &str = "OXITIDE_PERF_SCENARIO";
pub const DIR_ENV: &str = "OXITIDE_PERF_DIR";

static SESSION: OnceLock<Arc<PerfSession>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
pub struct PerfMark {
    pub name: String,
    /// Milliseconds since process creation (monotonic within the run).
    pub t_ms: u64,
    /// Wall-clock time of the mark.
    pub unix_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerfSnapshot {
    pub active: bool,
    pub run_id: String,
    pub scenario: String,
    pub marks: Vec<PerfMark>,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_ttfs_ms: Option<u64>,
    pub memory_mb: Option<u64>,
}

#[derive(Default)]
struct Counters {
    cache_hits: u64,
    cache_misses: u64,
    last_ttfs_ms: Option<u64>,
}

pub struct PerfSession {
    run_id: String,
    scenario: String,
    jsonl_path: PathBuf,
    start: Instant,
    process_start_unix_ms: u128,
    out: Mutex<BufWriter<File>>,
    marks: Mutex<Vec<PerfMark>>,
    counters: Mutex<Counters>,
    play_pending: Mutex<Option<Instant>>,
}

impl PerfSession {
    /// Create the run file under `dir`. Fails cleanly when `dir` cannot be
    /// created or the run file cannot be opened.
    pub fn create_in(dir: &Path, scenario: &str) -> std::io::Result<Arc<PerfSession>> {
        std::fs::create_dir_all(dir)?;
        let unix_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let run_id = format!("{scenario}-{unix_secs}");
        let jsonl_path = dir.join(format!("{run_id}.jsonl"));
        let file = OpenOptions::new().create(true).append(true).open(&jsonl_path)?;
        let session = Arc::new(PerfSession {
            run_id,
            scenario: scenario.to_string(),
            jsonl_path,
            start: Instant::now(),
            process_start_unix_ms: process_creation_unix_ms()
                .unwrap_or_else(now_unix_ms),
            out: Mutex::new(BufWriter::new(file)),
            marks: Mutex::new(Vec::new()),
            counters: Mutex::new(Counters::default()),
            play_pending: Mutex::new(None),
        });
        session.write(serde_json::json!({
            "kind": "session",
            "run_id": session.run_id,
            "scenario": session.scenario,
            "unix_ms": session.process_start_unix_ms,
        }));
        Ok(session)
    }

    pub fn jsonl_path(&self) -> &Path {
        &self.jsonl_path
    }

    fn write(&self, mut value: serde_json::Value) {
        if let serde_json::Value::Object(map) = &mut value {
            map.insert("run_id".into(), serde_json::Value::String(self.run_id.clone()));
        }
        let mut out = self.out.lock().unwrap();
        let mut line = value.to_string();
        line.push('\n');
        if let Err(e) = out.write_all(line.as_bytes()).and_then(|()| out.flush()) {
            tracing::warn!("perf write failed: {e}");
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis().min(u64::MAX as u128) as u64
    }

    pub fn mark(&self, name: &str) {
        let t_ms = self.elapsed_ms();
        let unix_ms = self.process_start_unix_ms + t_ms as u128;
        let mark = PerfMark { name: name.to_string(), t_ms, unix_ms };
        self.marks.lock().unwrap().push(mark.clone());
        self.write(serde_json::json!({
            "kind": "mark",
            "name": mark.name,
            "t_ms": mark.t_ms,
            "unix_ms": mark.unix_ms,
        }));
    }

    /// Called by the play commands so the next `FirstPcm` yields a TTFS.
    pub fn note_play_command(&self) {
        *self.play_pending.lock().unwrap() = Some(Instant::now());
    }

    pub fn audio_event(&self, event: AudioPerfEvent) {
        let mut counters = self.counters.lock().unwrap();
        match &event {
            AudioPerfEvent::CacheHit { .. } => counters.cache_hits += 1,
            AudioPerfEvent::CacheMiss { .. } => counters.cache_misses += 1,
            AudioPerfEvent::FirstPcm { .. } => {
                if let Some(started) = self.play_pending.lock().unwrap().take() {
                    // A pending play that never produced audio (pause toggles,
                    // errors) must not pair with a much later first PCM.
                    if started.elapsed() <= Duration::from_secs(30) {
                        let ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                        counters.last_ttfs_ms = Some(ms);
                        self.write(serde_json::json!({
                            "kind": "ttfs",
                            "ms": ms,
                            "t_ms": self.elapsed_ms(),
                        }));
                    }
                }
            }
            _ => {}
        }
        let (kind, track_id, gap_ms) = match &event {
            AudioPerfEvent::DecodeStart { track_id } => ("decode_start", *track_id, None),
            AudioPerfEvent::CacheHit { track_id } => ("cache_hit", *track_id, None),
            AudioPerfEvent::CacheMiss { track_id } => ("cache_miss", *track_id, None),
            AudioPerfEvent::FirstPcm { track_id } => ("first_pcm", *track_id, None),
            AudioPerfEvent::Advance { track_id, gap_ms } => ("advance", *track_id, Some(*gap_ms)),
        };
        self.write(serde_json::json!({
            "kind": "audio",
            "event": kind,
            "track_id": track_id,
            "gap_ms": gap_ms,
            "t_ms": self.elapsed_ms(),
        }));
    }

    pub fn ui_metrics(&self, metrics: serde_json::Value) {
        self.write(serde_json::json!({
            "kind": "ui_metrics",
            "t_ms": self.elapsed_ms(),
            "metrics": metrics,
        }));
    }

    pub fn write_memory_sample(&self) {
        if let Some(mb) = process_tree_memory_mb() {
            self.write(serde_json::json!({
                "kind": "memory",
                "mb": mb,
                "t_ms": self.elapsed_ms(),
            }));
        }
    }

    pub fn snapshot(&self) -> PerfSnapshot {
        let counters = self.counters.lock().unwrap();
        PerfSnapshot {
            active: true,
            run_id: self.run_id.clone(),
            scenario: self.scenario.clone(),
            marks: self.marks.lock().unwrap().clone(),
            cache_hits: counters.cache_hits,
            cache_misses: counters.cache_misses,
            last_ttfs_ms: counters.last_ttfs_ms,
            memory_mb: process_tree_memory_mb(),
        }
    }
}

/// Start the session from the environment. `None` (with no side effects) when
/// `OXITIDE_PERF_SCENARIO` is unset; a warning and `None` when the directory is
/// unusable.
pub fn install() -> Option<Arc<PerfSession>> {
    let scenario = std::env::var(SCENARIO_ENV)
        .ok()
        .filter(|s| !s.trim().is_empty())?;
    let dir = std::env::var(DIR_ENV)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(default_dir)?;
    match PerfSession::create_in(&dir, &scenario) {
        Ok(session) => {
            tracing::info!("perf session {} -> {}", session.run_id, dir.display());
            std::thread::Builder::new()
                .name("oxitide-perf-mem".into())
                .spawn({
                    let session = session.clone();
                    move || loop {
                        std::thread::sleep(Duration::from_secs(2));
                        session.write_memory_sample();
                    }
                })
                .ok();
            let _ = SESSION.set(session.clone());
            Some(session)
        }
        Err(e) => {
            tracing::warn!("perf session disabled: {e}");
            None
        }
    }
}

pub fn session() -> Option<&'static Arc<PerfSession>> {
    SESSION.get()
}

pub fn mark(name: &str) {
    if let Some(session) = session() {
        session.mark(name);
    }
}

pub fn note_play_command() {
    if let Some(session) = session() {
        session.note_play_command();
    }
}

pub fn audio_event(event: AudioPerfEvent) {
    if let Some(session) = session() {
        session.audio_event(event);
    }
}

pub fn ui_metrics(metrics: serde_json::Value) {
    if let Some(session) = session() {
        session.ui_metrics(metrics);
    }
}

pub fn snapshot() -> PerfSnapshot {
    snapshot_for(session())
}

fn snapshot_for(session: Option<&Arc<PerfSession>>) -> PerfSnapshot {
    session.map(|s| s.snapshot()).unwrap_or(PerfSnapshot {
        active: false,
        run_id: String::new(),
        scenario: String::new(),
        marks: Vec::new(),
        cache_hits: 0,
        cache_misses: 0,
        last_ttfs_ms: None,
        memory_mb: None,
    })
}

#[cfg(debug_assertions)]
fn default_dir() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Some(root.join(".superpowers/perf/runs"))
}

#[cfg(not(debug_assertions))]
fn default_dir() -> Option<PathBuf> {
    None
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Windows: the process creation time in unix milliseconds from
/// `GetProcessTimes`; `None` when unavailable.
#[cfg(windows)]
fn process_creation_unix_ms() -> Option<u128> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut creation, &mut exit, &mut kernel, &mut user)
            .ok()?;
    }
    let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
    // FILETIME counts 100 ns intervals since 1601-01-01; unix epoch offset:
    const UNIX_EPOCH_FILETIME_TICKS: u64 = 116_444_736_000_000_000;
    if ticks < UNIX_EPOCH_FILETIME_TICKS {
        return None;
    }
    Some(((ticks - UNIX_EPOCH_FILETIME_TICKS) / 10_000) as u128)
}

#[cfg(not(windows))]
fn process_creation_unix_ms() -> Option<u128> {
    None
}

/// Working set of this process plus all descendant processes (the WebView2
/// helpers), in MiB. Windows only; `None` elsewhere.
#[cfg(windows)]
fn process_tree_memory_mb() -> Option<u64> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    let self_pid = std::process::id();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut parents: Vec<(u32, u32)> = Vec::new();
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                parents.push((entry.th32ProcessID, entry.th32ParentProcessID));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    let is_descendant = |pid: u32| {
        let mut current = pid;
        for _ in 0..8 {
            if current == self_pid {
                return true;
            }
            match parents.iter().find(|(p, _)| *p == current) {
                Some((_, parent)) if *parent != 0 => current = *parent,
                _ => return false,
            }
        }
        false
    };
    let mut total: u64 = 0;
    for (pid, _) in parents {
        if !is_descendant(pid) {
            continue;
        }
        let handle: HANDLE =
            match unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) } {
                Ok(handle) => handle,
                Err(_) => continue,
            };
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let ok = unsafe {
            GetProcessMemoryInfo(
                handle,
                &mut counters,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
        };
        let _ = unsafe { CloseHandle(handle) };
        if ok.is_ok() {
            total += counters.WorkingSetSize as u64;
        }
    }
    Some(total / (1024 * 1024))
}

#[cfg(not(windows))]
fn process_tree_memory_mb() -> Option<u64> {
    None
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p oxitide-app perf::`
Expected: PASS (skip the Windows-only test on other platforms). If `windows` API paths differ in 0.62, fix the `use` paths — `cargo check` output names the correct module path.

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/oxitide-app/src/perf.rs crates/oxitide-app/Cargo.toml
git commit -m "feat(app): add the perf session core and JSONL sink"
```

---

### Task 3: App wiring and perf commands

**Files:**
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `crates/oxitide-app/src/commands.rs`

**Interfaces:**
- Consumes: `perf::install/mark/note_play_command/audio_event/snapshot/ui_metrics` (Task 2), `Engine::set_perf_hook` (Task 1).
- Produces (UI contract, Task 5):
  - `perf_mark(name: string)` → `void`
  - `get_perf_snapshot()` → `PerfSnapshot`
  - `perf_submit_ui_metrics(metrics: object)` → `void`

- [ ] **Step 1: Wire `main.rs`**

Add `mod perf;` to the module list (alphabetical, after `mod palette;`). Make the first statement of `fn main()`:

```rust
    perf::install();
```

Add marks at the milestones (each is a one-line call at the point named):

- after `let config = config::Config::load();` → `perf::mark("config_loaded");`
- after `state.set_cache(player::build_cache(&config));` → `perf::mark("cache_ready");`
- after `state.set_scrobbler(scrobbler.clone());` → `perf::mark("services_ready");`
- inside `setup`, after `window.show()` succeeds (before `Ok(())`) → `perf::mark("window_shown");`

Add to `generate_handler![...]` (end of the list, before `commands::analyze_cover`):

```rust
            commands::perf_mark,
            commands::get_perf_snapshot,
            commands::perf_submit_ui_metrics,
```

- [ ] **Step 2: Wire `commands.rs`**

In `auth_restore`, in the `Some((session, tokens))` arm after `state.set_player(replace_player(...)?);`:

```rust
            crate::perf::mark("auth_restored");
```

At the end of `build_player`, just before `Ok(Player { engine })`:

```rust
    engine.set_perf_hook(std::sync::Arc::new(crate::perf::audio_event));
    crate::perf::mark("player_built");
```

In `play_tracks` and `play_pause`, before `player.engine.handle(cmd)`:

```rust
    crate::perf::note_play_command();
```

Add the three commands (place near `get_playback_state`):

```rust
#[tauri::command]
pub fn perf_mark(name: String) {
    crate::perf::mark(&name);
}

#[tauri::command]
pub fn get_perf_snapshot() -> crate::perf::PerfSnapshot {
    crate::perf::snapshot()
}

#[tauri::command]
pub fn perf_submit_ui_metrics(metrics: serde_json::Value) {
    crate::perf::ui_metrics(metrics);
}
```

- [ ] **Step 3: Verify compile + tests + clippy**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/oxitide-app/src/main.rs crates/oxitide-app/src/commands.rs
git commit -m "feat(app): expose perf marks, snapshot, and UI metrics commands"
```

---

### Task 4: UI perf helpers (pure)

**Files:**
- Create: `ui/src/lib/perf.ts`
- Test: `ui/src/lib/perf.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `percentile(values, p)`, `FrameSampler`, `LongTaskLog`, `formatHudLines(snapshotLike, frames, longTasks)` — used by Task 5.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/perf.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { FrameSampler, LongTaskLog, formatHudLines, percentile } from "./perf";

describe("percentile", () => {
  it("returns the nearest-rank percentile", () => {
    expect(percentile([5, 1, 3, 2, 4], 50)).toBe(3);
    expect(percentile([1, 2, 3, 4], 100)).toBe(4);
    expect(percentile([1, 2, 3, 4], 1)).toBe(1);
  });

  it("returns 0 for an empty sample", () => {
    expect(percentile([], 95)).toBe(0);
  });
});

describe("FrameSampler", () => {
  it("summarizes recorded frame times", () => {
    const sampler = new FrameSampler(3);
    sampler.record(10);
    sampler.record(20);
    sampler.record(30);
    sampler.record(40);
    const summary = sampler.summary();
    expect(summary.count).toBe(3);
    expect(summary.max).toBe(40);
    expect(summary.p95).toBe(40);
  });

  it("is empty before any frames", () => {
    expect(new FrameSampler().summary()).toEqual({ count: 0, p50: 0, p95: 0, p99: 0, max: 0 });
  });
});

describe("LongTaskLog", () => {
  it("counts and tracks the maximum", () => {
    const log = new LongTaskLog();
    log.record(60);
    log.record(120);
    expect(log.summary()).toEqual({ count: 2, max_ms: 120 });
  });
});

describe("formatHudLines", () => {
  it("renders frame and long-task lines", () => {
    const lines = formatHudLines(
      { run_id: "S5-instrument-1", scenario: "S5-instrument", memory_mb: 250, last_ttfs_ms: 420, cache_hits: 3, cache_misses: 1 },
      { count: 10, p50: 4, p95: 12, p99: 30, max: 44 },
      { count: 1, max_ms: 60 },
    );
    expect(lines[0]).toContain("S5-instrument");
    expect(lines.join("\n")).toContain("p95 12.0");
    expect(lines.join("\n")).toContain("memory 250 MB");
    expect(lines.join("\n")).toContain("TTFS 420 ms");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/perf.test.ts`
Expected: FAIL — `./perf` not found.

- [ ] **Step 3: Implement `ui/src/lib/perf.ts`**

```ts
export function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(sorted.length - 1, Math.max(0, rank - 1))];
}

export interface FrameSummary {
  count: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

export class FrameSampler {
  private values: number[] = [];

  constructor(private cap = 5000) {}

  record(ms: number) {
    this.values.push(ms);
    if (this.values.length > this.cap) this.values.shift();
  }

  reset() {
    this.values = [];
  }

  summary(): FrameSummary {
    if (this.values.length === 0) return { count: 0, p50: 0, p95: 0, p99: 0, max: 0 };
    return {
      count: this.values.length,
      p50: percentile(this.values, 50),
      p95: percentile(this.values, 95),
      p99: percentile(this.values, 99),
      max: Math.max(...this.values),
    };
  }
}

export interface LongTaskSummary {
  count: number;
  max_ms: number;
}

export class LongTaskLog {
  private count = 0;
  private max = 0;

  record(duration: number) {
    this.count += 1;
    this.max = Math.max(this.max, duration);
  }

  reset() {
    this.count = 0;
    this.max = 0;
  }

  summary(): LongTaskSummary {
    return { count: this.count, max_ms: this.max };
  }
}

export interface HudSnapshotLike {
  run_id: string;
  scenario: string;
  memory_mb: number | null;
  last_ttfs_ms: number | null;
  cache_hits: number;
  cache_misses: number;
}

export function formatHudLines(
  snapshot: HudSnapshotLike,
  frames: FrameSummary,
  longTasks: LongTaskSummary,
): string[] {
  const fixed = (v: number) => v.toFixed(1);
  return [
    `${snapshot.scenario}  ${snapshot.run_id}`,
    `frame p50 ${fixed(frames.p50)} p95 ${fixed(frames.p95)} p99 ${fixed(frames.p99)} max ${fixed(frames.max)} (n=${frames.count})`,
    `longtasks ${longTasks.count} max ${fixed(longTasks.max_ms)} ms`,
    `memory ${snapshot.memory_mb ?? "?"} MB  TTFS ${snapshot.last_ttfs_ms ?? "?"} ms`,
    `cache hit ${snapshot.cache_hits} miss ${snapshot.cache_misses}`,
  ];
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/perf.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/perf.ts ui/src/lib/perf.test.ts
git commit -m "feat(ui): add pure perf sampling helpers"
```

---

### Task 5: UI perf store, HUD, and automation bridge

**Files:**
- Modify: `ui/src/lib/api.ts`
- Create: `ui/src/lib/perf.svelte.ts`
- Create: `ui/src/lib/components/PerfHud.svelte`
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/lib/components/TrackTable.svelte` (non-visual data attributes only)

**Interfaces:**
- Consumes: `api.getPerfSnapshot/perfMark/perfSubmitUiMetrics` below; Task 4 helpers.
- Produces: `perf` singleton (`active`, `snapshot`, `init()`, `mark(name)`, `toggleHud()`); `window.__perf` bridge used by the Task 7 driver:
  - `goto(name)`, `scrollTracks(px)`, `setFilter(text)`.

- [ ] **Step 1: Add the API contract to `ui/src/lib/api.ts`**

Add interfaces near the other DTOs:

```ts
export interface PerfMark { name: string; t_ms: number; unix_ms: number }
export interface PerfSnapshot {
  active: boolean;
  run_id: string;
  scenario: string;
  marks: PerfMark[];
  cache_hits: number;
  cache_misses: number;
  last_ttfs_ms: number | null;
  memory_mb: number | null;
}
```

Add to the `api` object (near `getAudioStatus`):

```ts
  getPerfSnapshot: () => invoke<PerfSnapshot>("get_perf_snapshot"),
  perfMark: (name: string) => invoke<void>("perf_mark", { name }),
  perfSubmitUiMetrics: (metrics: Record<string, unknown>) =>
    invoke<void>("perf_submit_ui_metrics", { metrics }),
```

- [ ] **Step 2: Create `ui/src/lib/perf.svelte.ts`**

```ts
import { api, type PerfSnapshot } from "./api";
import { appearance } from "./appearance.svelte";
import { FrameSampler, LongTaskLog } from "./perf";

export function createPerf() {
  let active = $state(false);
  let hudVisible = $state(true);
  let snapshot = $state<PerfSnapshot | null>(null);
  const frames = new FrameSampler();
  const longTasks = new LongTaskLog();
  let started = false;
  let raf = 0;
  let lastFrame = 0;
  const timers: number[] = [];
  let observer: PerformanceObserver | null = null;

  async function refresh() {
    try {
      snapshot = await api.getPerfSnapshot();
    } catch {
      /* session may have ended */
    }
  }

  async function submit() {
    if (!active) return;
    try {
      await api.perfSubmitUiMetrics({
        scenario: snapshot?.scenario ?? "",
        mode: appearance.mode,
        frames: frames.summary(),
        long_tasks: longTasks.summary(),
      });
    } catch {
      /* never disturb the app for metrics */
    }
  }

  async function init() {
    if (started) return;
    started = true;
    try {
      snapshot = await api.getPerfSnapshot();
    } catch {
      return;
    }
    if (!snapshot.active) return;
    active = true;
    lastFrame = performance.now();
    const tick = (now: number) => {
      frames.record(now - lastFrame);
      lastFrame = now;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    requestAnimationFrame(() => mark("first_paint"));
    try {
      observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) longTasks.record(entry.duration);
      });
      observer.observe({ entryTypes: ["longtask"] });
    } catch {
      observer = null;
    }
    timers.push(window.setInterval(() => void refresh(), 2000));
    timers.push(window.setInterval(() => void submit(), 5000));
  }

  function mark(name: string) {
    void api.perfMark(name).catch(() => {});
  }

  function toggleHud() {
    hudVisible = !hudVisible;
  }

  function dispose() {
    cancelAnimationFrame(raf);
    observer?.disconnect();
    for (const timer of timers) clearInterval(timer);
  }

  return {
    get active() { return active; },
    get hudVisible() { return hudVisible; },
    get snapshot() { return snapshot; },
    get frames() { return frames; },
    get longTasks() { return longTasks; },
    init,
    mark,
    toggleHud,
    dispose,
  };
}

export const perf = createPerf();
```

- [ ] **Step 3: Create `ui/src/lib/components/PerfHud.svelte`**

```svelte
<script lang="ts">
  import { perf } from "../perf.svelte";
  import { formatHudLines } from "../perf";

  function onKeydown(event: KeyboardEvent) {
    if (!perf.active) return;
    if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === "h") {
      event.preventDefault();
      perf.toggleHud();
    }
  }

  const lines = $derived(
    perf.snapshot
      ? formatHudLines(perf.snapshot, perf.frames.summary(), perf.longTasks.summary())
      : [],
  );
</script>

<svelte:window onkeydown={onKeydown} />

{#if perf.active && perf.hudVisible}
  <aside class="perf-hud" aria-label="Performance HUD">
    {#each lines as line}<div>{line}</div>{/each}
  </aside>
{/if}

<style>
  .perf-hud {
    position: fixed;
    right: 12px;
    bottom: 12px;
    z-index: 9999;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgb(0 0 0 / 0.78);
    color: #fff;
    font: 11px/1.5 monospace;
    pointer-events: none;
    white-space: pre;
  }
</style>
```

- [ ] **Step 4: Wire `App.svelte`**

Imports:

```ts
  import PerfHud from "./lib/components/PerfHud.svelte";
  import { perf } from "./lib/perf.svelte";
```

Add near the other init effects (after the `appearance.init` effect):

```ts
  $effect(() => {
    void perf.init();
  });

  $effect(() => {
    if (!booting) perf.mark("ui_ready");
  });
```

Add the automation bridge effect (after the perf init effect):

```ts
  $effect(() => {
    if (!perf.active) return;
    const bridge = {
      goto: (name: string) => nav.goto({ name } as never),
      scrollTracks: (px: number) =>
        document.querySelector<HTMLElement>("[data-perf-scroll]")?.scrollBy({ top: px }),
      setFilter: (text: string) => {
        const input = document.querySelector<HTMLInputElement>("[data-perf-filter]");
        if (!input) return false;
        input.value = text;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        return true;
      },
    };
    (window as unknown as { __perf?: typeof bridge }).__perf = bridge;
    return () => {
      delete (window as unknown as { __perf?: unknown }).__perf;
    };
  });
```

Render the HUD next to the other global overlays (next to `<ToastHost {toasts} />`):

```svelte
<PerfHud />
```

- [ ] **Step 5: Add the non-visual hooks to `TrackTable.svelte`**

- On the element with `onscroll={onScroll}` in `TrackTable.svelte` (currently around line 160), add the attribute `data-perf-scroll` (no styling or behavior effect).
- On the filter `<input class="filter" placeholder="Filter tracks" ...>` (currently around lines 137–139), add the attribute `data-perf-filter`.

- [ ] **Step 6: Verify the UI build**

Run: `npm --prefix ui run check; if ($?) { npm --prefix ui test }`
Expected: clean check, all tests pass.

- [ ] **Step 7: Manual smoke of HUD/bridge (dev only)**

Run: `$env:OXITIDE_PERF_SCENARIO = "S1-dev"; $env:OXITIDE_PERF_DIR = "$PWD\.superpowers\perf\runs"; npm --prefix ui run app:dev`
Expected: app opens, HUD is visible in the corner, a `S1-dev-*.jsonl` file appears with `mark` lines for `config_loaded`, `cache_ready`, `services_ready`, `window_shown`, `first_paint`, `ui_ready`. Close the app; remove the test env vars.

- [ ] **Step 8: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/perf.svelte.ts ui/src/lib/components/PerfHud.svelte ui/src/App.svelte ui/src/lib/components/TrackTable.svelte
git commit -m "feat(ui): add the perf store, HUD, and automation bridge"
```

---

### Task 6: Budgets and the check gate

**Files:**
- Create: `scripts/perf/budgets.json`
- Create: `scripts/perf/check.mjs`
- Test: `scripts/perf/check.test.mjs`
- Modify: `ui/package.json` (script alias)

**Interfaces:**
- Consumes: JSONL run files (Task 2 format).
- Produces: `check.mjs` CLI — `node scripts/perf/check.mjs [--dir <runs>] [--scenario <id>]`; exits 1 on any failed budget, 0 otherwise; prints one line per budget. Exported functions `readRuns`, `metricsFor`, `evaluate` for the test.

- [ ] **Step 1: Write the failing test**

Create `scripts/perf/check.test.mjs`:

```js
import { test } from "node:test";
import assert from "node:assert/strict";
import { evaluate, metricsFor, readRuns } from "./check.mjs";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

function runFile(lines) {
  const dir = mkdtempSync(join(tmpdir(), "oxitide-perf-check-"));
  const file = join(dir, "S1-1.jsonl");
  writeFileSync(file, lines.map((l) => JSON.stringify(l)).join("\n") + "\n");
  return file;
}

test("readRuns groups lines by run id", () => {
  const file = runFile([
    { kind: "session", run_id: "S1-1", scenario: "S1" },
    { kind: "mark", name: "first_paint", t_ms: 900, unix_ms: 1 },
    { kind: "mark", name: "ui_ready", t_ms: 2000, unix_ms: 2 },
  ]);
  const runs = readRuns([file]);
  assert.equal(runs.length, 1);
  assert.equal(runs[0].id, "S1-1");
  assert.equal(runs[0].lines.length, 3);
});

test("metricsFor derives mark, ttfs, frame, and memory metrics", () => {
  const file = runFile([
    { kind: "session", run_id: "S3-hit-1", scenario: "S3-hit" },
    { kind: "mark", name: "first_paint", t_ms: 800, unix_ms: 1 },
    { kind: "ttfs", ms: 250, t_ms: 900 },
    { kind: "audio", event: "cache_hit", track_id: 1, t_ms: 890 },
    { kind: "audio", event: "advance", track_id: 2, gap_ms: 40, t_ms: 2000 },
    { kind: "memory", mb: 300, t_ms: 100 },
    { kind: "memory", mb: 340, t_ms: 5000 },
    { kind: "ui_metrics", t_ms: 5100, metrics: { frames: { count: 4, p50: 4, p95: 10, p99: 12, max: 20 }, long_tasks: { count: 0, max_ms: 0 }, mode: "instrument" } },
  ]);
  const [run] = readRuns([file]);
  const metrics = metricsFor(run);
  assert.equal(metrics.first_paint_ms, 800);
  assert.equal(metrics.ttfs_ms, 250);
  assert.equal(metrics.cache_hits, 1);
  assert.equal(metrics.advance_max_gap_ms, 40);
  assert.equal(metrics.memory_max_mb, 340);
  assert.equal(metrics.memory_drift_mb, 40);
  assert.equal(metrics["frame_p95_ms:instrument"], 10);
});

test("evaluate flags a budget breach and passes within budget", () => {
  const config = {
    S1: [{ metric: "first_paint_ms", max: 1500 }],
  };
  const pass = evaluate([{ scenario: "S1", metrics: { first_paint_ms: 900 } }], config);
  assert.equal(pass.ok, true);
  const fail = evaluate([{ scenario: "S1", metrics: { first_paint_ms: 1600 } }], config);
  assert.equal(fail.ok, false);
  const unmeasured = evaluate([{ scenario: "S1", metrics: {} }], config);
  assert.equal(unmeasured.ok, true);
  assert.equal(unmeasured.results[0].status, "unmeasured");
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `node --test scripts/perf/check.test.mjs`
Expected: FAIL — `./check.mjs` not found.

- [ ] **Step 3: Implement `scripts/perf/check.mjs`**

```js
#!/usr/bin/env node
// Budget gate for the Track 3 harness. Reads perf JSONL runs, aggregates
// metrics per scenario, compares with budgets.json, and exits non-zero on any
// failing budget. Usage:
//   node scripts/perf/check.mjs [--dir <runs-dir>] [--scenario <id>]
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

export function readRuns(files) {
  const runs = new Map();
  for (const file of files) {
    let text;
    try {
      text = readFileSync(file, "utf8");
    } catch {
      continue;
    }
    let currentRunId = null;
    for (const raw of text.split("\n")) {
      if (!raw.trim()) continue;
      let line;
      try {
        line = JSON.parse(raw);
      } catch {
        continue; // a killed process may leave one truncated final line
      }
      if (line.run_id) currentRunId = line.run_id;
      const key = currentRunId ?? file;
      if (!runs.has(key)) {
        runs.set(key, { id: key, scenario: line.scenario ?? "unknown", lines: [], file });
      }
      const run = runs.get(key);
      if (line.scenario) run.scenario = line.scenario;
      run.lines.push(line);
    }
  }
  return [...runs.values()];
}

export function metricsFor(run) {
  const metrics = { scenario: run.scenario };
  const frames = new Map(); // mode -> number[]
  const longTasks = [];
  const memories = []; // {t_ms, mb}
  let advanceMax = 0;
  for (const line of run.lines) {
    if (line.kind === "mark") metrics[`${line.name}_ms`] = line.t_ms;
    if (line.kind === "ttfs") metrics.ttfs_ms = line.ms;
    if (line.kind === "memory") memories.push({ t: line.t_ms, mb: line.mb });
    if (line.kind === "audio") {
      if (line.event === "cache_hit") metrics.cache_hits = (metrics.cache_hits ?? 0) + 1;
      if (line.event === "cache_miss") metrics.cache_misses = (metrics.cache_misses ?? 0) + 1;
      if (line.event === "advance") advanceMax = Math.max(advanceMax, line.gap_ms ?? 0);
    }
    if (line.kind === "ui_metrics") {
      const mode = line.metrics?.mode ?? "instrument";
      if (line.metrics?.frames?.count) {
        if (!frames.has(mode)) frames.set(mode, []);
        frames.get(mode).push(line.metrics.frames);
      }
      if (line.metrics?.long_tasks) longTasks.push(line.metrics.long_tasks);
    }
  }
  if (advanceMax > 0) metrics.advance_max_gap_ms = advanceMax;
  if (memories.length > 0) {
    metrics.memory_max_mb = Math.max(...memories.map((m) => m.mb));
    const first = memories[0].mb;
    const last = memories[memories.length - 1].mb;
    metrics.memory_drift_mb = last - first;
  }
  for (const [mode, samples] of frames) {
    metrics[`frame_p95_ms:${mode}`] = Math.max(...samples.map((s) => s.p95));
  }
  if (longTasks.length > 0) {
    metrics.long_task_max_ms = Math.max(...longTasks.map((t) => t.max_ms));
  }
  const totalCache = (metrics.cache_hits ?? 0) + (metrics.cache_misses ?? 0);
  if (totalCache > 0) metrics.cache_hit_rate = (metrics.cache_hits ?? 0) / totalCache;
  return metrics;
}

function p95(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(0.95 * sorted.length) - 1)];
}

export function aggregate(runs) {
  const byScenario = new Map();
  for (const run of runs) {
    if (!byScenario.has(run.scenario)) byScenario.set(run.scenario, []);
    byScenario.get(run.scenario).push(metricsFor(run));
  }
  const aggregated = new Map();
  for (const [scenario, samples] of byScenario) {
    const out = { scenario, samples: samples.length };
    const keys = new Set(samples.flatMap((s) => Object.keys(s)));
    for (const key of keys) {
      if (key === "scenario") continue;
      const values = samples.map((s) => s[key]).filter((v) => typeof v === "number");
      if (values.length === 0) continue;
      out[key] = p95(values);
      out[`${key}_mean`] = values.reduce((a, b) => a + b, 0) / values.length;
    }
    aggregated.set(scenario, out);
  }
  return aggregated;
}

export function evaluate(aggregatedRuns, config) {
  const results = [];
  for (const run of aggregatedRuns) {
    const budgets = config[run.scenario];
    if (!budgets) continue;
    for (const budget of budgets) {
      const value = run.metrics[budget.metric];
      const status =
        value === undefined ? "unmeasured" : value <= budget.max ? "pass" : "fail";
      results.push({ scenario: run.scenario, metric: budget.metric, value, max: budget.max, status });
    }
  }
  return { ok: results.every((r) => r.status !== "fail"), results };
}

function main() {
  const args = process.argv.slice(2);
  const dirFlag = args.indexOf("--dir");
  const scenarioFlag = args.indexOf("--scenario");
  const dir = dirFlag >= 0 ? args[dirFlag + 1] : join(HERE, "../../.superpowers/perf/runs");
  const onlyScenario = scenarioFlag >= 0 ? args[scenarioFlag + 1] : null;
  const config = JSON.parse(readFileSync(join(HERE, "budgets.json"), "utf8"));
  let files;
  try {
    files = readdirSync(dir).filter((f) => f.endsWith(".jsonl")).map((f) => join(dir, f));
  } catch (e) {
    console.error(`perf runs directory unavailable: ${dir} (${e.message})`);
    process.exit(1);
  }
  let runs = readRuns(files);
  if (onlyScenario) runs = runs.filter((r) => r.scenario === onlyScenario);
  const aggregated = [...aggregate(runs).values()].map((m) => ({ scenario: m.scenario, metrics: m }));
  const { ok, results } = evaluate(aggregated, config);
  if (results.length === 0) {
    console.error(`no budgets matched ${runs.length} run(s) in ${dir}`);
    process.exit(1);
  }
  for (const result of results) {
    const value = result.value === undefined ? "n/a" : result.value.toFixed(1);
    console.log(
      `${result.status.toUpperCase().padEnd(10)} ${result.scenario.padEnd(16)} ${result.metric.padEnd(28)} ${value}  (max ${result.max})`,
    );
  }
  process.exit(ok ? 0 : 1);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
```

- [ ] **Step 4: Create `scripts/perf/budgets.json`**

```json
{
  "S1": [
    { "metric": "first_paint_ms", "max": 1500 },
    { "metric": "ui_ready_ms", "max": 2500 }
  ],
  "S2": [{ "metric": "first_paint_ms", "max": 1000 }],
  "S3-miss": [{ "metric": "ttfs_ms", "max": 2500 }],
  "S3-hit": [{ "metric": "ttfs_ms", "max": 300 }],
  "S4": [{ "metric": "advance_max_gap_ms", "max": 100 }],
  "S5-instrument": [
    { "metric": "frame_p95_ms:instrument", "max": 16.7 },
    { "metric": "long_task_max_ms", "max": 100 }
  ],
  "S5-atmosphere": [
    { "metric": "frame_p95_ms:atmosphere", "max": 20 },
    { "metric": "long_task_max_ms", "max": 100 }
  ],
  "S6": [{ "metric": "frame_p95_ms:instrument", "max": 16.7 }],
  "S7": [],
  "S8": [{ "metric": "cache_hit_rate", "max": 1.0 }],
  "S9": [{ "metric": "memory_drift_mb", "max": 50 }]
}
```

Notes for the executor: `S7`'s 250 ms queue-build budget is measured by the driver report (Task 7), not JSONL metrics, and is evaluated in the baseline note; `S8`'s threshold is expressed as a rate with an auxiliary floor the baseline note records (`cache_hit_rate >= 0.9` — the `evaluate` function only supports `max`, so add `"min"` support in Step 3 rather than encoding it here).

- [ ] **Step 5: Add `min` budget support (test-first)**

Add to `check.test.mjs`:

```js
test("evaluate supports a minimum floor", () => {
  const config = { S8: [{ metric: "cache_hit_rate", min: 0.9 }] };
  const pass = evaluate([{ scenario: "S8", metrics: { cache_hit_rate: 0.95 } }], config);
  assert.equal(pass.ok, true);
  const fail = evaluate([{ scenario: "S8", metrics: { cache_hit_rate: 0.5 } }], config);
  assert.equal(fail.ok, false);
});
```

Run it to see it fail, then update `evaluate`:

```js
      const maxOk = budget.max === undefined || (value !== undefined && value <= budget.max);
      const minOk = budget.min === undefined || (value !== undefined && value >= budget.min);
      const status =
        value === undefined ? "unmeasured" : maxOk && minOk ? "pass" : "fail";
      results.push({
        scenario: run.scenario,
        metric: budget.metric,
        value,
        max: budget.max ?? budget.min,
        status,
      });
```

and change the `S8` entry in `budgets.json` to `[{ "metric": "cache_hit_rate", "min": 0.9 }]`.

- [ ] **Step 6: Run the gate tests**

Run: `node --test scripts/perf/check.test.mjs`
Expected: PASS.

- [ ] **Step 7: Add the npm alias**

In `ui/package.json` scripts, after `"test"`:

```json
    "perf:check": "node ../scripts/perf/check.mjs",
```

- [ ] **Step 8: Commit**

```bash
git add scripts/perf/budgets.json scripts/perf/check.mjs scripts/perf/check.test.mjs ui/package.json
git commit -m "feat(perf): add the budget gate and thresholds"
```

---

### Task 7: Scenario runner and CDP driver

**Files:**
- Create: `scripts/perf/run-scenario.ps1`
- Create: `scripts/perf/drive.mjs`

**Interfaces:**
- Consumes: `window.__perf` bridge (Task 5), `check.mjs` (Task 6), env contract `OXITIDE_PERF_SCENARIO` / `OXITIDE_PERF_DIR` (Task 2).
- Produces: `run-scenario.ps1 -Scenario <id> [-Runs N] [-Manual]` used by Task 8.

- [ ] **Step 1: Implement `scripts/perf/drive.mjs`**

```js
#!/usr/bin/env node
// Drives a scripted perf scenario through the WebView2 CDP endpoint.
// Requires Node 22+ (global WebSocket). Exits non-zero on failure.
import { setTimeout as delay } from "node:timers/promises";

const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : fallback;
};
const scenario = flag("--scenario");
const port = Number(flag("--port", "9222"));
const timeoutMs = Number(flag("--timeout", "120000"));

if (!scenario) {
  console.error("usage: node drive.mjs --scenario <id> [--port 9222] [--timeout 120000]");
  process.exit(2);
}

async function connect() {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
      if (page) return page.webSocketDebuggerUrl;
    } catch {
      /* not up yet */
    }
    await delay(500);
  }
  throw new Error(`no CDP page target on port ${port} within 30 s`);
}

const wsUrl = await connect();
const ws = new WebSocket(wsUrl);
const pending = new Map();
let nextId = 1;
ws.addEventListener("message", (event) => {
  const msg = JSON.parse(event.data);
  const resolve = pending.get(msg.id);
  if (resolve) {
    pending.delete(msg.id);
    resolve(msg);
  }
});
await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, { once: true });
  ws.addEventListener("error", reject, { once: true });
});

function send(method, params = {}) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result)));
    ws.send(JSON.stringify({ id, method, params }));
    setTimeout(() => {
      if (pending.delete(id)) reject(new Error(`${method} timed out`));
    }, 30000);
  });
}

async function evaluate(expression) {
  const result = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(`evaluate failed: ${result.exceptionDetails.text}`);
  }
  return result.result.value;
}

const until = async (expression, label, maxMs = 30000) => {
  const deadline = Date.now() + maxMs;
  while (Date.now() < deadline) {
    if (await evaluate(expression)) return;
    await delay(500);
  }
  throw new Error(`timed out waiting for ${label}`);
};

const scenarioFallback = async (id) => {
  if (id.startsWith("S5")) {
    await until("!!window.__perf", "__perf bridge");
    await evaluate('window.__perf.goto("tracks")');
    await until('!!document.querySelector("[data-perf-scroll]")', "track table");
    await delay(3000);
    for (let i = 0; i < 12; i++) {
      await evaluate("window.__perf.scrollTracks(1200)");
      await delay(600);
    }
    await delay(3000);
  } else if (id.startsWith("S6")) {
    await until("!!window.__perf", "__perf bridge");
    await evaluate('window.__perf.goto("tracks")');
    await until('!!document.querySelector("[data-perf-filter]")', "filter input");
    await delay(5000); // let the full library load so filtering is over loaded rows
    for (const query of ["a", "th", "the ", "the e", ""]) {
      await evaluate(`window.__perf.setFilter(${JSON.stringify(query)})`);
      await delay(800);
    }
    await delay(3000);
  } else if (id === "S7") {
    await until("!!window.__TAURI_INTERNALS__", "tauri internals");
    const started = Date.now();
    await evaluate(`(async () => {
      const tracks = await window.__TAURI_INTERNALS__.invoke("get_favorite_tracks");
      await window.__TAURI_INTERNALS__.invoke("play_tracks", { items: tracks, start: 0, shuffle: false });
      return tracks.length;
    })()`).then((count) => console.log(`S7 invoked play_tracks for ${count} tracks`));
    console.log(`S7 command round-trip: ${Date.now() - started} ms`);
    await delay(5000);
  } else {
    // Start-only scenarios (S1/S2 and manual ones): idle while marks accumulate.
    await delay(Number(flag("--idle", "8000")));
  }
};

try {
  await send("Runtime.enable");
} catch {
  /* older targets may not need it */
}

await scenarioFallback(scenario).catch((e) => {
  console.error(`driver failed: ${e.message}`);
  process.exitCode = 1;
});
ws.close();
```

- [ ] **Step 2: Implement `scripts/perf/run-scenario.ps1`**

```powershell
# Runs a scripted or manual perf scenario N times and then evaluates budgets.
param(
    [Parameter(Mandatory = $true)][string]$Scenario,
    [int]$Runs = 5,
    [switch]$Manual,
    [switch]$SkipCheck,
    [string]$Exe = "target/debug/oxitide-app.exe",
    [int]$Port = 9222
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$perfDir = Join-Path $root ".superpowers\perf\runs"
New-Item -ItemType Directory -Force -Path $perfDir | Out-Null

function Wait-ForPort([int]$p, [int]$seconds) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-NetConnection -ComputerName 127.0.0.1 -Port $p -InformationLevel Quiet -WarningAction SilentlyContinue) { return $true }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

# Debug builds load the Vite dev server; make sure it is up.
if (-not (Test-NetConnection -ComputerName 127.0.0.1 -Port 1420 -InformationLevel Quiet -WarningAction SilentlyContinue)) {
    Write-Host "starting Vite on 1420 ..."
    Start-Process -FilePath "npm" -ArgumentList "--prefix", "ui", "run", "dev" -WorkingDirectory $root -WindowStyle Hidden
    if (-not (Wait-ForPort 1420 60)) { throw "Vite did not start on port 1420" }
}

$env:OXITIDE_PERF_DIR = $perfDir
$env:OXITIDE_PERF_SCENARIO = $Scenario
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$exePath = Join-Path $root $Exe
if (-not (Test-Path -LiteralPath $exePath)) { throw "app binary not found: $exePath (run npm --prefix ui run app:build or cargo build -p oxitide-app)" }

for ($i = 1; $i -le $Runs; $i++) {
    Write-Host "run $i/$Runs ($Scenario)"
    $proc = Start-Process -FilePath $exePath -PassThru
    try {
        if ($Manual) {
            Read-Host "Perform the $Scenario scenario in the app, then press Enter to close it"
        } else {
            node "$root\scripts\perf\drive.mjs" --scenario $Scenario --port $Port
            if ($LASTEXITCODE -ne 0) { Write-Warning "driver reported a failure for run $i" }
        }
    } finally {
        if (-not $proc.HasExited) { & taskkill /PID $proc.Id /T /F | Out-Null }
        $proc.WaitForExit(5000) | Out-Null
        Start-Sleep -Milliseconds 1500
    }
}

if (-not $SkipCheck) {
    node "$root\scripts\perf\check.mjs" --dir $perfDir --scenario $Scenario
    exit $LASTEXITCODE
}
```

- [ ] **Step 3: Dry-run the runner against a start-only scenario**

Run: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 1`
Expected: Vite starts (or is already up), the app launches, a `S1-*.jsonl` appears in `.superpowers/perf/runs/`, the app is killed, and `check.mjs` prints S1 budget lines (PASS or FAIL) and exits 0/1 accordingly. A FAIL at this point is a valid baseline outcome, not a harness failure.

- [ ] **Step 4: Commit**

```bash
git add scripts/perf/run-scenario.ps1 scripts/perf/drive.mjs
git commit -m "feat(perf): add the scenario runner and CDP driver"
```

---

### Task 8: Baseline runs, recalibration, and the baseline note

**Files:**
- Create: `docs/superpowers/plans/notes/<execution-date>-track3-baseline.md` (date the file when run)
- Modify: `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` (Track 3 status line only after the note lands)

**Interfaces:**
- Consumes: everything above.
- Produces: the committed baseline; the one allowed recalibration (T3-D9).

- [ ] **Step 1: Scripted scenarios (S1, S2, S5-instrument, S5-atmosphere)**

For each scenario run 5 times; for S5 set the app to Instrument mode first, then repeat after switching to Atmosphere (the mode is recorded in every `ui_metrics` line):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-instrument -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere -Runs 5
```

Expected: a pass/fail table per scenario; raw runs in `.superpowers/perf/runs/`.

- [ ] **Step 2: Human-in-the-loop scenarios**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-miss -Runs 5 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-hit -Runs 5 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S4 -Runs 5 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S8 -Runs 2 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S9 -Runs 1 -Manual
```

Protocol per scenario (the runner prompts on each run):

- **S3-miss:** clear the cache once before the first run (Settings → clear cache), play the reference track from a fresh album; play only one track.
- **S3-hit:** replay the same track immediately; do not touch the cache between runs.
- **S4:** play a reference album and let it play through 2–3 track transitions without skipping.
- **S8:** play the same 10 Max-quality tracks in a loop twice (album repeat); note the queue actually uses Max.
- **S9:** leave a queue playing/skipping for 2 hours (skip/quality-switch churn is allowed); do not restart the app.

- [ ] **Step 3: Best-effort scripted S6/S7 at the current library size**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S6 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S7 -Runs 3
```

Record the loaded row count from the UI in the note. The 10k budget is marked "unmeasured (fixture pending Phase C)" until the Phase C plan adds a large-list fixture; the numbers collected here are informational.

- [ ] **Step 4: Aggregate the baseline and write the note**

Create `docs/superpowers/plans/notes/<date>-track3-baseline.md` with:

- Header (`Date`, `Build`, `Machine`, `Conditions`: Vite dev build unless noted, instrument unless noted).
- The `check.mjs` output verbatim per scenario.
- One table: metric, budget, measured p95, status.
- The T3-D9 recalibration section: for each budget the owner adjusts, record `old → new, reason`; leave empty if none.
- Notes on invalid runs (network, background load) and the S6/S7 row-count caveat.

- [ ] **Step 5: Owner recalibration (single round)**

Present the baseline table and the proposed adjustments to the owner. Apply only what the owner approves to `budgets.json` and the spec's §4.2 table in the same commit.

- [ ] **Step 6: Update the roadmap Track 3 status**

Change the Track 3 row's status to `Active — Phase A (harness + baseline) complete; Phase B (startup + TTFS) planning next` and add the baseline note path.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/plans/notes docs/superpowers/specs scripts/perf/budgets.json
git commit -m "test: record the Track 3 baseline and lock budgets"
```

---

## Self-Review Output

**Spec coverage:** harness architecture (§5.1 Rust core → Task 2; §5.2 UI sampler/HUD → Tasks 4–5; §5.3 driver → Task 7; §5.4 reports → Tasks 2/6; §5.5 scenarios → Tasks 7–8; §5.6 overhead → Review Focus + session gating); budgets (§4.2 → Task 6 `budgets.json`); audio engine marks (§5.1 → Task 1); app milestones (`config_loaded`…`player_built` → Task 3); recalibration (§4.3 → Task 8); Phase A deliverables (= Tasks 1–8). Deferred by design: S6/S7 10k fixture (Phase C), per-mode budget refinement (T3-D6), AGENTS.md command docs (spec assigns to Phase D).

**Placeholder scan:** no TBD/TODO; the only `<execution-date>` string is an intentional file-name pattern resolved when the baseline note is written (Task 8).

**Type consistency:** `AudioPerfEvent` variants match between Task 1 (definition), Task 1 test, Task 2 match arms; `PerfSnapshot`/`PerfMark` fields match between Task 2 (Rust), Task 5 (TS interfaces), and Tasks 6/7 (JSONL consumers); `perf_mark`/`get_perf_snapshot`/`perf_submit_ui_metrics` command names and camelCase arg (`{ name }`, `{ metrics }`) match Tasks 3, 5, and 6.

**Review Focus tests:** normal-run inertness (Task 2 `snapshot_without_a_session_is_inactive` + gated `session()` reads); unwritable dir (Task 2 `create_in_fails_when_the_path_is_a_file`); truncated JSONL (Task 6 `readRuns` skips the torn line); clock anomalies (Task 2 `process_creation_unix_ms` falls back to `now_unix_ms`, `ticks < epoch` check); driver failure (Task 7 bounded 30 s CDP connect and per-call timeouts, non-zero exit).
