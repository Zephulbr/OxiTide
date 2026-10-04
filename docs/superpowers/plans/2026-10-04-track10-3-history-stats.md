# Track 10.3 History and Stats Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A local, private play log (`%APPDATA%\OxiTide\history.jsonl`) that records finalized plays, powers a History view with Recently-played and Stats tabs, and never touches the network or the 500 ms position path with disk I/O.

**Architecture:** A new `crates/oxitide-app/src/history.rs` owns a pure `HistoryTracker` state machine (fed the same `EngineEvent`s as the scrobbler, finalizing on track transition / queue end / exit), a `HistoryStore` whose writer thread appends JSONL and prunes at the config cap, and pure stats math. `AppState` holds the store; a third `engine.subscribe` in `build_player` feeds it; `RunEvent::Exit` and logout flush the in-flight play. The UI adds a `history` route (`recent`/`stats` tabs) reachable from the sidebar, a custom chronological list (day groups + relative times), and an inline-CSS 30-day bar chart.

**Tech Stack:** Rust (std only — no new crates; `serde_json` JSONL, `std::sync::mpsc` writer thread), Svelte 5 runes, TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-04-oxitide-track10-qol-design.md` (§5.1, §5.3, §6.3, §7, §8; decision QoL-D9)

**Spec refinements (flagged for review):**
1. `HistoryRecord` carries `cover` (the §5.1 field list omits it, but §6.3 requires covers in Recently played; the engine's `QueueItem` already has it).
2. Thresholds are read from QoL-D9 verbatim: a play is recorded at ≥30 s listened, or ≥50% for tracks under 60 s; `completed` is ≥90% listened **or** the last observed position within 1.5 s of the end ("natural end"). Listened time accrues only on positive deltas ≤3 s, mirroring the scrobbler's seek semantics.
3. `get_stats(offset_minutes)` takes the client's UTC offset so the 30-day chart buckets by **local** day without a Rust timezone dependency (no chrono; pure integer civil-date math). Rolling 7/30-day totals stay wall-clock windows.
4. Settings gains minimal privacy/retention controls in the **Storage** tab (enabled toggle, max entries, clear with confirm) — §6.3 did not name a location; §2's "retention and privacy controls" requires them.

## Global Constraints

- Windows-first; no CI. Every check is manual, run by the implementer.
- Gates after every Rust/UI task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- **No new dependencies.** The chart is inline CSS; the Rust date math is pure integer arithmetic. HTTP tests use `wiremock` only; this plan adds no HTTP.
- History writes never run on the engine event thread: the listener only locks the in-memory tracker and sends on a channel; the writer thread owns file I/O.
- History is local user data; never log titles, tokens, or file contents. Warnings name paths and reasons only.
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case (`offsetMinutes` ↔ `offset_minutes`). Keep `ui/src/lib/api.ts` in sync by hand.
- Icons are inlined Lucide paths in `ui/src/lib/components/Icon.svelte`; components consume semantic tokens only; Svelte 5 runes only.
- Do not edit `AGENTS.md` in this plan — the track completion note owns AGENTS.md updates (spec §10).
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green run instead.

## Review Focus

- **A skipped play (below threshold) must never reach the log or stats.** Pinned by `skip_under_thirty_seconds_is_not_recorded`, `short_track_needs_half_listened` (Task 1) and smoke step 2.
- **A finalized play is recorded exactly once** — transition, queue end, and exit must not double-write or drop it. Pinned by `long_play_is_recorded_at_transition`, `queue_end_finalizes`, `repeat_one_records_two_plays` (Task 1), `finalize_now_flushes_the_in_flight_play` (Task 2).
- **The in-flight play appears in Recently played but never in Stats.** Pinned by `in_flight_is_prepended_in_the_page` (Task 2) and `in_flight_is_not_counted_in_stats` (Task 3).
- **Pruning keeps the newest records and a malformed line never bricks the view.** Pinned by `prunes_oldest_beyond_max_entries`, `malformed_lines_are_skipped` (Task 2).
- **Local-day bucketing is correct across offsets and midnight.** Pinned by `daily_buckets_use_the_offset`, `date_from_day_matches_known_dates` (Task 3) and the `historyTime` tests (Task 5).

---

## File Structure

**Rust**

- Create `crates/oxitide-app/src/history.rs` — `HistoryRecord`, `HistoryTracker`, `HistoryStore`, `Stats` + `compute_stats`, `history_path`, tests.
- Modify `crates/oxitide-app/src/config.rs` — `HistoryConfig`, `Config.history`, tests.
- Modify `crates/oxitide-app/src/state.rs` — `history` field + accessors + test.
- Modify `crates/oxitide-app/src/commands.rs` — `HistoryPageDto`, five commands, listener, logout flush.
- Modify `crates/oxitide-app/src/main.rs` — module, store creation, exit flush, `generate_handler!`.

**UI**

- Modify `ui/src/lib/api.ts` — `HistoryEntry`/`HistoryPage`/`Stats` types, `Config.history`, wrappers, `historyToQueueItem`.
- Modify `ui/src/lib/nav.svelte.ts` — `HistoryTab`, route variant, `sameRoute`, `parseRoute`.
- Modify `ui/src/lib/nav.test.ts` — history route tests.
- Modify `ui/src/lib/components/Icon.svelte` — `history` icon.
- Modify `ui/src/lib/components/Sidebar.svelte` — History entry.
- Modify `ui/src/App.svelte` — import + route switch.
- Create `ui/src/lib/historyTime.ts` — relative time, day labels, grouping.
- Create `ui/src/lib/historyTime.test.ts` — tests with local-constructed clocks.
- Create `ui/src/lib/historyChart.ts` + `ui/src/lib/historyChart.test.ts` — chart scaling.
- Create `ui/src/views/History.svelte` — tabs, Recent tab, Stats fetch.
- Create `ui/src/lib/components/HistoryStats.svelte` — stats cards, top lists, 30-day chart.
- Modify `ui/src/views/Settings.svelte` — Storage-tab history controls.

**Docs**

- Create `docs/superpowers/plans/notes/2026-10-04-track10-3-history-stats-smoke.md` — human GUI checklist.

---

### Task 1: History tracker

**Files:**
- Create: `crates/oxitide-app/src/history.rs`
- Modify: `crates/oxitide-app/src/main.rs:15` (module list, after `mod session;`)

**Interfaces:**
- Consumes: `oxitide_audio::engine::EngineEvent`, `oxitide_audio::queue::QueueItem`.
- Produces: `history::{HistoryRecord, HistoryTracker, HISTORY_MIN_MS, HISTORY_SHORT_TRACK_MS, SEEK_MAX_MS, DEFAULT_MAX_ENTRIES}` with `HistoryTracker::{on_event, in_flight, finish}` — used by Tasks 2–4.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-app/src/history.rs` with only this test module (the implementation comes in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_audio::engine::PlaybackState;
    use oxitide_audio::output::OutputMode;
    use oxitide_tidal::stream::Quality;

    fn item(id: u64, duration_secs: u32) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs,
            cover: None,
        }
    }

    fn playback_state(playing: bool) -> PlaybackState {
        PlaybackState {
            playing,
            buffering: false,
            current: None,
            current_index: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            shuffle: false,
            repeat: oxitide_audio::queue::RepeatMode::Off,
            preferred_quality: Quality::Lossless,
            output_mode: OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            queue: Vec::new(),
            stream: None,
        }
    }

    fn now_playing(item: &QueueItem) -> EngineEvent {
        EngineEvent::NowPlaying(Some(item.clone()))
    }

    fn state(playing: bool) -> EngineEvent {
        EngineEvent::State(playback_state(playing))
    }

    fn position(ms: u64, duration_ms: u64) -> EngineEvent {
        EngineEvent::Position { position_ms: ms, duration_ms }
    }

    /// Start a play and feed `seconds` of 1 s position ticks.
    fn play_seconds(tracker: &mut HistoryTracker, item: &QueueItem, seconds: u64) {
        tracker.on_event(&now_playing(item), 100);
        tracker.on_event(&state(true), 100);
        for step in 1..=seconds {
            tracker.on_event(&position(step * 1000, item.duration_secs as u64 * 1000), 100);
        }
    }

    #[test]
    fn skip_under_thirty_seconds_is_not_recorded() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 200), 20);
        assert_eq!(tracker.on_event(&now_playing(&item(2, 200)), 200), None);
    }

    #[test]
    fn long_play_is_recorded_at_transition() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 200), 40);
        let record = tracker.on_event(&now_playing(&item(2, 200)), 200).expect("recorded");
        assert_eq!(record.track_id, 1);
        assert_eq!(record.listened_ms, 40_000);
        assert!(!record.completed);
        assert_eq!(record.started_at, 100);
    }

    #[test]
    fn short_track_needs_half_listened() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 45), 20);
        assert_eq!(tracker.on_event(&now_playing(&item(2, 45)), 200), None);

        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 45), 23);
        let record = tracker.on_event(&now_playing(&item(2, 45)), 200).expect("recorded");
        assert_eq!(record.listened_ms, 23_000);
    }

    #[test]
    fn queue_end_finalizes() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 200), 40);
        let record = tracker.on_event(&EngineEvent::NowPlaying(None), 200).expect("recorded");
        assert_eq!(record.track_id, 1);
    }

    #[test]
    fn pause_does_not_finalize() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 200), 40);
        assert_eq!(tracker.on_event(&state(false), 200), None);
        for step in 41..=60 {
            tracker.on_event(&position(step * 1000, 200_000), 200);
        }
        let record = tracker.on_event(&now_playing(&item(2, 200)), 300).expect("recorded");
        assert_eq!(record.listened_ms, 60_000);
    }

    #[test]
    fn forward_seek_is_not_counted() {
        let mut tracker = HistoryTracker::default();
        tracker.on_event(&now_playing(&item(1, 200)), 100);
        tracker.on_event(&state(true), 100);
        tracker.on_event(&position(30_000, 200_000), 100); // jump: not listened
        for step in 31..=60 {
            tracker.on_event(&position(step * 1000, 200_000), 100);
        }
        let record = tracker.on_event(&now_playing(&item(2, 200)), 200).expect("recorded");
        assert_eq!(record.listened_ms, 30_000, "the 30 s jump must not count");
    }

    #[test]
    fn completed_at_ninety_percent() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 100), 90);
        let record = tracker.on_event(&now_playing(&item(2, 100)), 200).expect("recorded");
        assert!(record.completed);

        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 100), 80);
        let record = tracker.on_event(&now_playing(&item(2, 100)), 200).expect("recorded");
        assert!(!record.completed);
    }

    #[test]
    fn short_track_natural_end_is_completed() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 4), 3);
        let record = tracker.on_event(&EngineEvent::NowPlaying(None), 200).expect("recorded");
        assert!(record.completed, "a track played to its end is completed");
    }

    #[test]
    fn repeat_one_records_two_plays() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 200), 40);
        let first = tracker.on_event(&now_playing(&item(1, 200)), 200).expect("first");
        play_seconds(&mut tracker, &item(1, 200), 40);
        let second = tracker.on_event(&EngineEvent::NowPlaying(None), 400).expect("second");
        assert_eq!(first.track_id, 1);
        assert_eq!(second.track_id, 1);
    }

    #[test]
    fn restore_paused_does_not_start_a_play() {
        let mut tracker = HistoryTracker::default();
        tracker.on_event(&now_playing(&item(1, 200)), 100);
        tracker.on_event(&state(false), 100);
        assert_eq!(tracker.in_flight(), None);
        assert_eq!(tracker.finish(), None);
    }

    #[test]
    fn in_flight_reflects_the_current_play() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 200), 20);
        let record = tracker.in_flight().expect("in flight");
        assert_eq!(record.track_id, 1);
        assert_eq!(record.listened_ms, 20_000);
        assert!(!record.completed);
    }

    #[test]
    fn finish_records_the_current_play() {
        let mut tracker = HistoryTracker::default();
        play_seconds(&mut tracker, &item(1, 200), 40);
        let record = tracker.finish().expect("recorded");
        assert_eq!(record.listened_ms, 40_000);
        assert_eq!(tracker.finish(), None, "finishing twice must not duplicate");
    }
}
```

- [ ] **Step 2: Register the module and verify the tests fail**

Add `mod history;` to `crates/oxitide-app/src/main.rs` after `mod session;` (line 15).

Run: `cargo test -p oxitide-app history::tests -v`
Expected: compile failure — `HistoryTracker`, `HistoryRecord` do not exist.

- [ ] **Step 3: Write the implementation**

Add above the test module in `crates/oxitide-app/src/history.rs`:

```rust
//! Local playback history: a pure listen tracker, a JSONL store with a
//! background writer, and stats derived on read (spec §5.1, §6.3; QoL-D9).
//! No network, no engine changes: it rides the same event feed as scrobbling.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use oxitide_audio::engine::EngineEvent;
use oxitide_audio::queue::QueueItem;
use serde::{Deserialize, Serialize};

/// A play is recorded at 30 s listened, or half the track under a minute.
pub const HISTORY_MIN_MS: u64 = 30_000;
pub const HISTORY_SHORT_TRACK_MS: u64 = 60_000;
/// A play is completed at 90% listened or when the position reached the end.
pub const HISTORY_END_SLACK_MS: u64 = 1_500;
/// Positive position deltas larger than this are seeks, not listening.
pub const SEEK_MAX_MS: u64 = 3_000;
/// Default cap; config `history.max_entries` overrides.
pub const DEFAULT_MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: u32,
    #[serde(default)]
    pub cover: Option<String>,
    /// Unix seconds (UTC) when playback actually started.
    pub started_at: i64,
    pub listened_ms: u64,
    pub completed: bool,
}

/// The play currently accumulating, if playback has started.
#[derive(Debug)]
struct ActivePlay {
    item: QueueItem,
    started_at: i64,
    last_pos_ms: u64,
    duration_ms: u64,
    listened_ms: u64,
}

/// Pure listen state machine: finalizes a play on track transition, queue
/// end, or an explicit `finish` (exit/logout). Paused restores never start.
#[derive(Debug, Default)]
pub struct HistoryTracker {
    current: Option<ActivePlay>,
    pending_start: Option<QueueItem>,
}

impl HistoryTracker {
    /// Feed one engine event; returns a record when a play just finalized.
    pub fn on_event(&mut self, event: &EngineEvent, now: i64) -> Option<HistoryRecord> {
        match event {
            EngineEvent::NowPlaying(Some(item)) => {
                let finalized = self.finalize();
                self.pending_start = Some(item.clone());
                finalized
            }
            EngineEvent::NowPlaying(None) => {
                let finalized = self.finalize();
                self.pending_start = None;
                finalized
            }
            EngineEvent::State(state) => {
                if state.playing {
                    if let Some(item) = self.pending_start.take() {
                        self.current = Some(ActivePlay {
                            item,
                            started_at: now,
                            last_pos_ms: 0,
                            duration_ms: 0,
                            listened_ms: 0,
                        });
                    }
                }
                None
            }
            EngineEvent::Position { position_ms, duration_ms } => {
                if let Some(play) = self.current.as_mut() {
                    let delta = position_ms.saturating_sub(play.last_pos_ms);
                    if delta > 0 && delta <= SEEK_MAX_MS {
                        play.listened_ms += delta;
                    }
                    play.last_pos_ms = *position_ms;
                    if *duration_ms > 0 {
                        play.duration_ms = *duration_ms;
                    }
                }
                None
            }
            EngineEvent::Error { .. } => None,
        }
    }

    /// The current play as a record for the first history page.
    pub fn in_flight(&self) -> Option<HistoryRecord> {
        let play = self.current.as_ref()?;
        Some(record_from(&play.item, play.started_at, play.listened_ms, false))
    }

    /// Finalize the current play (exit / logout). No-op without one.
    pub fn finish(&mut self) -> Option<HistoryRecord> {
        self.finalize()
    }

    fn finalize(&mut self) -> Option<HistoryRecord> {
        let play = self.current.take()?;
        let duration_ms = if play.duration_ms > 0 {
            play.duration_ms
        } else {
            play.item.duration_secs as u64 * 1000
        };
        if !should_record(play.listened_ms, duration_ms) {
            return None;
        }
        let completed = is_completed(play.listened_ms, duration_ms, play.last_pos_ms);
        Some(record_from(&play.item, play.started_at, play.listened_ms, completed))
    }
}

/// Recorded at 30 s listened, or half the track under a minute (QoL-D9).
fn should_record(listened_ms: u64, duration_ms: u64) -> bool {
    if listened_ms >= HISTORY_MIN_MS {
        return true;
    }
    duration_ms > 0 && duration_ms < HISTORY_SHORT_TRACK_MS && listened_ms * 2 >= duration_ms
}

/// Completed at 90% listened or a position within one tick of the end.
fn is_completed(listened_ms: u64, duration_ms: u64, last_pos_ms: u64) -> bool {
    if duration_ms == 0 {
        return false;
    }
    listened_ms * 10 >= duration_ms * 9
        || last_pos_ms.saturating_add(HISTORY_END_SLACK_MS) >= duration_ms
}

fn record_from(item: &QueueItem, started_at: i64, listened_ms: u64, completed: bool) -> HistoryRecord {
    HistoryRecord {
        track_id: item.track_id,
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_secs: item.duration_secs,
        cover: item.cover.clone(),
        started_at,
        listened_ms,
        completed,
    }
}

/// Unix seconds (UTC).
fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `%APPDATA%\OxiTide\history.jsonl`, beside `config.toml`.
pub fn history_path() -> Option<PathBuf> {
    crate::config::config_path().map(|path| path.with_file_name("history.jsonl"))
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app history::tests`
Expected: PASS (12 tests).

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/history.rs crates/oxitide-app/src/main.rs
git commit -m "feat(history): add the local listen tracker"
```

---

### Task 2: History store

**Files:**
- Modify: `crates/oxitide-app/src/history.rs` (store + tests)

**Interfaces:**
- Consumes: `HistoryTracker`, `HistoryRecord` (Task 1).
- Produces: `history::HistoryStore` with `start(path, max_entries, enabled)`, `record(&EngineEvent)`, `finalize_now()`, `page(offset, limit) -> (Vec<HistoryRecord>, usize)`, `clear()`, `set_enabled(bool)`, `set_max_entries(usize)` — used by Tasks 3–4.

- [ ] **Step 1: Write the failing store tests**

Add to `mod tests` in `crates/oxitide-app/src/history.rs` (imports included):

```rust
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-history-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn store(dir: &Path, max_entries: usize, enabled: bool) -> HistoryStore {
        HistoryStore::start(dir.join("history.jsonl"), max_entries, enabled)
    }

    /// Record `seconds` of a play, then finalize it with a transition.
    fn record_play(store: &HistoryStore, id: u64, seconds: u64) {
        let track = item(id, 200);
        store.record(&now_playing(&track));
        store.record(&state(true));
        for step in 1..=seconds {
            store.record(&position(step * 1000, 200_000));
        }
        store.record(&now_playing(&item(id + 1000, 200)));
    }

    #[test]
    fn finalized_plays_round_trip_newest_first() {
        let dir = temp_dir();
        let store = store(&dir, 100, true);
        record_play(&store, 1, 40);
        record_play(&store, 2, 40);
        store.record(&EngineEvent::NowPlaying(None));

        let (items, total) = store.page(0, 10);
        assert_eq!(total, 2);
        assert_eq!(items.iter().map(|r| r.track_id).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!(items[0].listened_ms, 40_000);
        let lines = fs::read_to_string(dir.join("history.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 2);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: the in-flight play shows in the page (not yet in stats).
    #[test]
    fn in_flight_is_prepended_in_the_page() {
        let dir = temp_dir();
        let store = store(&dir, 100, true);
        let track = item(1, 200);
        store.record(&now_playing(&track));
        store.record(&state(true));
        for step in 1..=40 {
            store.record(&position(step * 1000, 200_000));
        }

        let (items, total) = store.page(0, 10);
        assert_eq!(total, 1);
        assert_eq!(items[0].track_id, 1);
        assert_eq!(items[0].listened_ms, 40_000);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let dir = temp_dir();
        let path = dir.join("history.jsonl");
        fs::write(
            &path,
            "not json\n{\"track_id\":1,\"title\":\"T1\",\"artist\":\"A\",\"album\":\"B\",\"duration_secs\":200,\"started_at\":1,\"listened_ms\":40000,\"completed\":true}\n\n",
        )
        .unwrap();
        let store = store(&dir, 100, true);
        let (items, total) = store.page(0, 10);
        assert_eq!(total, 1);
        assert_eq!(items[0].track_id, 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn prunes_oldest_beyond_max_entries() {
        let dir = temp_dir();
        let store = store(&dir, 2, true);
        record_play(&store, 1, 40);
        record_play(&store, 2, 40);
        record_play(&store, 3, 40);
        store.record(&EngineEvent::NowPlaying(None));

        let (items, total) = store.page(0, 10);
        assert_eq!(total, 2);
        assert_eq!(items.iter().map(|r| r.track_id).collect::<Vec<_>>(), vec![3, 2]);
        let lines = fs::read_to_string(dir.join("history.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 2, "the file must be pruned too");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_removes_the_file_and_memory() {
        let dir = temp_dir();
        let store = store(&dir, 100, true);
        record_play(&store, 1, 40);
        store.record(&EngineEvent::NowPlaying(None));
        assert!(dir.join("history.jsonl").exists());

        store.clear();

        assert_eq!(store.page(0, 10).1, 0);
        assert!(!dir.join("history.jsonl").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn disabled_store_records_nothing() {
        let dir = temp_dir();
        let store = store(&dir, 100, false);
        record_play(&store, 1, 40);
        store.record(&EngineEvent::NowPlaying(None));
        assert_eq!(store.page(0, 10).1, 0);

        store.set_enabled(true);
        record_play(&store, 2, 40);
        store.record(&EngineEvent::NowPlaying(None));
        assert_eq!(store.page(0, 10).1, 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: exit must flush the in-flight play.
    #[test]
    fn finalize_now_flushes_the_in_flight_play() {
        let dir = temp_dir();
        let store = store(&dir, 100, true);
        let track = item(1, 200);
        store.record(&now_playing(&track));
        store.record(&state(true));
        for step in 1..=40 {
            store.record(&position(step * 1000, 200_000));
        }

        store.finalize_now();

        let (items, total) = store.page(0, 10);
        assert_eq!(total, 1);
        assert_eq!(items[0].track_id, 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn page_paginates() {
        let dir = temp_dir();
        let store = store(&dir, 100, true);
        record_play(&store, 1, 40);
        record_play(&store, 2, 40);
        record_play(&store, 3, 40);
        store.record(&EngineEvent::NowPlaying(None));

        let (items, total) = store.page(1, 1);
        assert_eq!(total, 3);
        assert_eq!(items.iter().map(|r| r.track_id).collect::<Vec<_>>(), vec![2]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn set_max_entries_applies_to_later_appends() {
        let dir = temp_dir();
        let store = store(&dir, 100, true);
        store.set_max_entries(1);
        record_play(&store, 1, 40);
        record_play(&store, 2, 40);
        store.record(&EngineEvent::NowPlaying(None));
        assert_eq!(store.page(0, 10).1, 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unwritable_path_does_not_panic() {
        let dir = temp_dir();
        let blocker = dir.join("blocked");
        fs::write(&blocker, "file").unwrap();
        let store = HistoryStore::start(blocker.join("history.jsonl"), 100, true);
        record_play(&store, 1, 40);
        store.record(&EngineEvent::NowPlaying(None));
        store.finalize_now();
        let _ = fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p oxitide-app history::tests -v`
Expected: compile failure — `HistoryStore` does not exist.

- [ ] **Step 3: Implement the store**

Add the imports to the module's `use` block:

```rust
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
```

Add above the test module in `crates/oxitide-app/src/history.rs`:

```rust
enum WriterMsg {
    Append(HistoryRecord),
    Clear,
    Flush(Sender<()>),
}

/// The history log and its tracker. `record` is cheap (in-memory only); a
/// dedicated thread owns every file write, so the engine event path never
/// blocks on disk.
pub struct HistoryStore {
    tracker: Mutex<HistoryTracker>,
    records: Arc<Mutex<VecDeque<HistoryRecord>>>, // newest first
    tx: Sender<WriterMsg>,
    enabled: AtomicBool,
    max_entries: Arc<AtomicUsize>,
}

impl HistoryStore {
    pub fn start(path: PathBuf, max_entries: usize, enabled: bool) -> Self {
        let max_entries = Arc::new(AtomicUsize::new(max_entries.max(1)));
        let (loaded, trimmed) = load_records(&path, max_entries.load(Ordering::SeqCst));
        let records = Arc::new(Mutex::new(loaded));
        let (tx, rx) = std::sync::mpsc::channel();
        let writer_records = records.clone();
        let writer_max = max_entries.clone();
        thread::Builder::new()
            .name("oxitide-history".into())
            .spawn(move || writer_main(path, writer_records, writer_max, trimmed, rx))
            .expect("spawn history writer");
        Self {
            tracker: Mutex::new(HistoryTracker::default()),
            records,
            tx,
            enabled: AtomicBool::new(enabled),
            max_entries,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn set_max_entries(&self, max_entries: usize) {
        self.max_entries.store(max_entries.max(1), Ordering::SeqCst);
    }

    /// Feed one engine event. In-memory only; the writer thread persists any
    /// finalized record.
    pub fn record(&self, event: &EngineEvent) {
        if !self.enabled.load(Ordering::SeqCst) {
            return;
        }
        let finalized = {
            let mut tracker = self.tracker.lock().unwrap();
            tracker.on_event(event, now_epoch())
        };
        if let Some(record) = finalized {
            let _ = self.tx.send(WriterMsg::Append(record));
        }
    }

    /// Finalize the in-flight play and wait for the writer to drain (exit and
    /// logout). Best effort: never blocks longer than two seconds.
    pub fn finalize_now(&self) {
        let finalized = {
            let mut tracker = self.tracker.lock().unwrap();
            tracker.finish()
        };
        if let Some(record) = finalized {
            let _ = self.tx.send(WriterMsg::Append(record));
        }
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        if self.tx.send(WriterMsg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv_timeout(Duration::from_secs(2));
        }
    }

    /// Newest-first page; the in-flight play is prepended (spec §6.3) and is
    /// included in `total`.
    pub fn page(&self, offset: usize, limit: usize) -> (Vec<HistoryRecord>, usize) {
        let inflight = self.tracker.lock().unwrap().in_flight();
        let records = self.records.lock().unwrap();
        let total = records.len() + usize::from(inflight.is_some());
        let items = inflight
            .into_iter()
            .chain(records.iter().cloned())
            .skip(offset)
            .take(limit)
            .collect();
        (items, total)
    }

    /// Empty the log and remove the file. The in-flight play is untouched.
    pub fn clear(&self) {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        if self.tx.send(WriterMsg::Clear).is_ok() && self.tx.send(WriterMsg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv_timeout(Duration::from_secs(2));
        }
    }
}

/// Read the log into memory (newest first); malformed lines are skipped so one
/// bad line never loses the rest. Returns whether the cap trimmed the file.
fn load_records(path: &Path, max_entries: usize) -> (VecDeque<HistoryRecord>, bool) {
    let Ok(text) = fs::read_to_string(path) else {
        return (VecDeque::new(), false);
    };
    let mut records = VecDeque::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<HistoryRecord>(line) {
            Ok(record) => records.push_front(record),
            Err(e) => tracing::warn!("skipping malformed history line: {e}"),
        }
    }
    let mut trimmed = false;
    while records.len() > max_entries.max(1) {
        records.pop_back();
        trimmed = true;
    }
    (records, trimmed)
}

fn writer_main(
    path: PathBuf,
    records: Arc<Mutex<VecDeque<HistoryRecord>>>,
    max_entries: Arc<AtomicUsize>,
    trimmed: bool,
    rx: Receiver<WriterMsg>,
) {
    if trimmed {
        let snapshot = records.lock().unwrap().clone();
        rewrite(&path, &snapshot);
    }
    loop {
        match rx.recv() {
            Ok(WriterMsg::Append(record)) => {
                let max = max_entries.load(Ordering::SeqCst).max(1);
                let (line, snapshot) = {
                    let mut records = records.lock().unwrap();
                    records.push_front(record);
                    let mut trimmed = false;
                    while records.len() > max {
                        records.pop_back();
                        trimmed = true;
                    }
                    let line = serde_json::to_string(records.front().unwrap()).unwrap_or_default();
                    (line, if trimmed { Some(records.clone()) } else { None })
                };
                if !line.is_empty() {
                    if let Err(e) = append_line(&path, &line) {
                        tracing::warn!("history append failed: {e}");
                    }
                }
                if let Some(snapshot) = snapshot {
                    rewrite(&path, &snapshot);
                }
            }
            Ok(WriterMsg::Clear) => {
                records.lock().unwrap().clear();
                match fs::remove_file(&path) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        tracing::warn!("history clear failed: {e}");
                    }
                    _ => {}
                }
            }
            Ok(WriterMsg::Flush(ack)) => {
                let _ = ack.send(());
            }
            Err(_) => break,
        }
    }
}

fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()
}

/// Atomic rewrite (temp file + rename), oldest first.
fn rewrite(path: &Path, records: &VecDeque<HistoryRecord>) {
    let mut text = String::new();
    for record in records.iter().rev() {
        if let Ok(line) = serde_json::to_string(record) {
            text.push_str(&line);
            text.push('\n');
        }
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("jsonl.tmp");
    if fs::write(&tmp, text).and_then(|_| fs::rename(&tmp, path)).is_err() {
        tracing::warn!("failed to rewrite history at {}", path.display());
    }
}
```

- [ ] **Step 4: Run the store tests to verify they pass**

Run: `cargo test -p oxitide-app history::tests`
Expected: PASS (12 tracker + 10 store tests).

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/history.rs
git commit -m "feat(history): persist finalized plays to jsonl"
```

---

### Task 3: Stats math

**Files:**
- Modify: `crates/oxitide-app/src/history.rs` (Stats types, `compute_stats`, `HistoryStore::stats`, tests)

**Interfaces:**
- Consumes: `HistoryRecord`, `HistoryStore` (Tasks 1–2).
- Produces: `history::{Stats, TopEntry, TopTrack, DailyEntry, compute_stats}` and `HistoryStore::stats(offset_minutes) -> Stats` — used by Task 4's `get_stats`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/oxitide-app/src/history.rs`:

```rust
    use std::collections::VecDeque;

    fn record(id: u64, artist: &str, album: &str, started_at: i64, listened_ms: u64) -> HistoryRecord {
        HistoryRecord {
            track_id: id,
            title: format!("T{id}"),
            artist: artist.into(),
            album: album.into(),
            duration_secs: 200,
            cover: None,
            started_at,
            listened_ms,
            completed: true,
        }
    }

    fn stats_of(records: Vec<HistoryRecord>, now: i64, offset_minutes: i32) -> Stats {
        compute_stats(&VecDeque::from(records), now, offset_minutes)
    }

    #[test]
    fn totals_and_windows() {
        let now = 2_000_000_000;
        let stats = stats_of(
            vec![
                record(1, "A", "X", now - 86_400, 60_000),      // 1 day ago
                record(2, "A", "X", now - 10 * 86_400, 30_000), // 10 days ago
                record(3, "B", "Y", now - 40 * 86_400, 10_000), // 40 days ago
            ],
            now,
            0,
        );
        assert_eq!(stats.total_listened_ms, 100_000);
        assert_eq!(stats.listened_ms_7d, 60_000);
        assert_eq!(stats.listened_ms_30d, 90_000);
        assert_eq!(stats.total_plays, 3);
        assert_eq!(stats.plays_7d, 1);
        assert_eq!(stats.plays_30d, 2);
    }

    #[test]
    fn top_lists_sort_by_plays_then_time() {
        let now = 2_000_000_000;
        let stats = stats_of(
            vec![
                record(1, "A", "X", now, 10_000),
                record(2, "A", "Y", now, 10_000),
                record(3, "B", "Y", now, 10_000),
                record(4, "C", "Z", now, 90_000),
            ],
            now,
            0,
        );
        let artists: Vec<_> = stats.top_artists.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(artists, vec!["A", "B", "C"]);
        let albums: Vec<_> = stats.top_albums.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(albums, vec!["Y", "X", "Z"]);
        assert_eq!(stats.top_tracks[0].track_id, 1);
    }

    #[test]
    fn daily_buckets_use_the_offset() {
        // 2026-10-04T12:00:00Z.
        let now = 1_791_115_200;
        // 2026-10-03T23:30:00Z: UTC+1 (offset -60) is already Oct 4.
        let late = record(1, "A", "X", now - 12 * 3600 - 1800, 60_000);

        let utc = stats_of(vec![late.clone()], now, 0);
        assert_eq!(utc.daily.last().unwrap().date, "2026-10-04");
        assert_eq!(utc.daily.last().unwrap().plays, 0);
        assert_eq!(utc.daily[utc.daily.len() - 2].plays, 1);

        let ahead = stats_of(vec![late], now, -60);
        assert_eq!(ahead.daily.last().unwrap().date, "2026-10-04");
        assert_eq!(ahead.daily.last().unwrap().plays, 1);
    }

    #[test]
    fn date_from_day_matches_known_dates() {
        assert_eq!(date_from_day(0), "1970-01-01");
        assert_eq!(date_from_day(-1), "1969-12-31");
        assert_eq!(date_from_day(20_730), "2026-10-04");
    }

    /// PM-D1 gate: stats stay well under 50 ms at the 10k cap.
    #[test]
    fn stats_over_ten_thousand_records_stays_under_50ms() {
        let now = 2_000_000_000;
        let records: Vec<HistoryRecord> = (0..10_000)
            .map(|i| record(i, "Artist", "Album", now - (i as i64 % 100) * 86_400, 60_000))
            .collect();
        let started = std::time::Instant::now();
        let stats = stats_of(records, now, 0);
        assert_eq!(stats.total_plays, 10_000);
        assert!(
            started.elapsed() < Duration::from_millis(50),
            "stats took {:?}",
            started.elapsed()
        );
    }

    /// Review Focus: the in-flight play is in the page but not the stats.
    #[test]
    fn in_flight_is_not_counted_in_stats() {
        let dir = temp_dir();
        let store = store(&dir, 100, true);
        let track = item(1, 200);
        store.record(&now_playing(&track));
        store.record(&state(true));
        for step in 1..=40 {
            store.record(&position(step * 1000, 200_000));
        }
        assert_eq!(store.stats(0).total_plays, 0);
        let _ = fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p oxitide-app history::tests -v`
Expected: compile failure — `Stats`, `compute_stats`, `date_from_day`, `store.stats` do not exist.

- [ ] **Step 3: Implement the stats**

Add the method inside the existing `impl HistoryStore` (after `page`):

```rust
    /// Derived on read; the in-flight play is deliberately excluded (spec §6.3).
    pub fn stats(&self, offset_minutes: i32) -> Stats {
        let records = self.records.lock().unwrap();
        compute_stats(&records, now_epoch(), offset_minutes)
    }
```

Add below `rewrite` in `crates/oxitide-app/src/history.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Stats {
    pub total_listened_ms: u64,
    pub listened_ms_7d: u64,
    pub listened_ms_30d: u64,
    pub total_plays: u64,
    pub plays_7d: u64,
    pub plays_30d: u64,
    pub top_artists: Vec<TopEntry>,
    pub top_tracks: Vec<TopTrack>,
    pub top_albums: Vec<TopEntry>,
    pub daily: Vec<DailyEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TopEntry {
    pub name: String,
    pub plays: u64,
    pub listened_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TopTrack {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub plays: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DailyEntry {
    /// Local calendar date (`YYYY-MM-DD`).
    pub date: String,
    pub plays: u64,
    pub listened_ms: u64,
}

const TOP_LIMIT: usize = 10;
const DAILY_DAYS: i64 = 30;

/// Aggregate the log on read. `offset_minutes` is the client's
/// `Date.getTimezoneOffset()` so the daily chart buckets by local day.
pub fn compute_stats(records: &VecDeque<HistoryRecord>, now: i64, offset_minutes: i32) -> Stats {
    let mut stats = Stats::default();
    let week_cutoff = now - 7 * 86_400;
    let month_cutoff = now - 30 * 86_400;
    let shift = offset_minutes as i64 * 60;
    let today = (now - shift).div_euclid(86_400);

    let mut artists: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();
    let mut albums: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();
    let mut tracks: std::collections::HashMap<u64, TopTrack> = std::collections::HashMap::new();

    for record in records {
        stats.total_listened_ms += record.listened_ms;
        stats.total_plays += 1;
        if record.started_at >= week_cutoff {
            stats.listened_ms_7d += record.listened_ms;
            stats.plays_7d += 1;
        }
        if record.started_at >= month_cutoff {
            stats.listened_ms_30d += record.listened_ms;
            stats.plays_30d += 1;
        }
        if !record.artist.is_empty() {
            let entry = artists.entry(record.artist.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += record.listened_ms;
        }
        if !record.album.is_empty() {
            let entry = albums.entry(record.album.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += record.listened_ms;
        }
        let track = tracks.entry(record.track_id).or_insert_with(|| TopTrack {
            track_id: record.track_id,
            title: record.title.clone(),
            artist: record.artist.clone(),
            plays: 0,
        });
        track.plays += 1;
    }

    stats.top_artists = top_entries(artists);
    stats.top_albums = top_entries(albums);
    let mut top_tracks: Vec<TopTrack> = tracks.into_values().collect();
    top_tracks.sort_by(|a, b| b.plays.cmp(&a.plays).then_with(|| a.title.cmp(&b.title)));
    top_tracks.truncate(TOP_LIMIT);
    stats.top_tracks = top_tracks;

    stats.daily = (0..DAILY_DAYS)
        .map(|i| {
            let day = today - (DAILY_DAYS - 1) + i;
            DailyEntry { date: date_from_day(day), plays: 0, listened_ms: 0 }
        })
        .collect();
    for record in records {
        let day = (record.started_at - shift).div_euclid(86_400);
        if day >= today - (DAILY_DAYS - 1) && day <= today {
            let index = (day - (today - (DAILY_DAYS - 1))) as usize;
            stats.daily[index].plays += 1;
            stats.daily[index].listened_ms += record.listened_ms;
        }
    }
    stats
}

fn top_entries(map: std::collections::HashMap<String, (u64, u64)>) -> Vec<TopEntry> {
    let mut entries: Vec<TopEntry> = map
        .into_iter()
        .map(|(name, (plays, listened_ms))| TopEntry { name, plays, listened_ms })
        .collect();
    entries.sort_by(|a, b| {
        b.plays
            .cmp(&a.plays)
            .then_with(|| b.listened_ms.cmp(&a.listened_ms))
            .then_with(|| a.name.cmp(&b.name))
    });
    entries.truncate(TOP_LIMIT);
    entries
}

/// Days-since-epoch to `YYYY-MM-DD` (Howard Hinnant's civil-from-days).
fn date_from_day(days: i64) -> String {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
```

- [ ] **Step 4: Run the stats tests to verify they pass**

Run: `cargo test -p oxitide-app history::tests`
Expected: PASS (12 tracker + 10 store + 6 stats tests).

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/history.rs
git commit -m "feat(history): aggregate stats on read"
```

---

### Task 4: Config, state, commands, wiring

**Files:**
- Modify: `crates/oxitide-app/src/config.rs` (section ~194-227, tests ~585)
- Modify: `crates/oxitide-app/src/state.rs` (fields ~42, accessors ~137)
- Modify: `crates/oxitide-app/src/commands.rs` (DTO ~76, `auth_logout` ~227, `build_player` ~463, commands after `set_volume` ~1216)
- Modify: `crates/oxitide-app/src/main.rs` (startup ~187, exit ~406, `generate_handler!` ~281-395)

**Interfaces:**
- Consumes: `history::{HistoryStore, history_path, HistoryRecord, Stats}` (Tasks 1–3).
- Produces (IPC): `get_history_page(offset, limit)`, `get_stats(offset_minutes)`, `clear_history()`, `set_history_enabled(enabled)`, `set_history_max_entries(max_entries)` — consumed by Tasks 5–7.

- [ ] **Step 1: Write the failing config and state tests**

Add to `crates/oxitide-app/src/config.rs` tests:

```rust
    #[test]
    fn history_defaults_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let config = Config {
            history: HistoryConfig { enabled: false, max_entries: 500 },
            ..Config::default()
        };
        config.save_to(&path).unwrap();
        let loaded = Config::load_from(&path);
        assert!(!loaded.history.enabled);
        assert_eq!(loaded.history.max_entries, 500);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pre_history_file_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[audio]\nvolume = 0.5\n").unwrap();
        let loaded = Config::load_from(&path);
        assert!(loaded.history.enabled);
        assert_eq!(loaded.history.max_entries, 10_000);
        let _ = fs::remove_dir_all(dir);
    }
```

Add to `crates/oxitide-app/src/state.rs` tests:

```rust
    #[test]
    fn history_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.history().is_none());
        let dir = std::env::temp_dir().join(format!("oxitide-state-history-{}", std::process::id()));
        let store = Arc::new(crate::history::HistoryStore::start(
            dir.join("history.jsonl"),
            100,
            true,
        ));
        state.set_history(store);
        assert!(state.history().is_some());
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p oxitide-app history_defaults_and_round_trip; if ($?) { cargo test -p oxitide-app history_starts_unset }`
Expected: compile failure — no `history` config section or `AppState.history`.

- [ ] **Step 3: Add the config section**

In `crates/oxitide-app/src/config.rs`:

1. Add `pub history: HistoryConfig,` to `Config` after `scrobbling` (line 17).
2. Add after `ScrobblingConfig` (line 227):

```rust
/// Local playback history (spec §5.1, §6.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub enabled: bool,
    pub max_entries: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { enabled: true, max_entries: crate::history::DEFAULT_MAX_ENTRIES }
    }
}
```

3. Fix the exhaustive `Config` literal in `save_and_load_round_trip` (line ~329): add `history: HistoryConfig::default(),`.

- [ ] **Step 4: Add the state accessor**

In `crates/oxitide-app/src/state.rs`:

1. Add the field to `AppState` after `session_saver`:

```rust
    /// Local playback history; `None` when the app has no writable data
    /// directory.
    pub history: Mutex<Option<Arc<crate::history::HistoryStore>>>,
```

2. Add accessors after `session_saver()`:

```rust
    pub fn set_history(&self, store: Arc<crate::history::HistoryStore>) {
        *self.history.lock().unwrap() = Some(store);
    }

    pub fn history(&self) -> Option<Arc<crate::history::HistoryStore>> {
        self.history.lock().unwrap().clone()
    }
```

- [ ] **Step 5: Add the commands**

In `crates/oxitide-app/src/commands.rs`, after `set_volume` (line 1216):

```rust
/// One page of the local history log, newest first.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryPageDto {
    pub items: Vec<crate::history::HistoryRecord>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[tauri::command]
pub fn get_history_page(
    state: State<'_, AppState>,
    offset: usize,
    limit: usize,
) -> Result<HistoryPageDto, String> {
    let limit = limit.clamp(1, 500);
    let (items, total) = match state.history() {
        Some(store) => store.page(offset, limit),
        None => (Vec::new(), 0),
    };
    Ok(HistoryPageDto { items, total, offset, limit })
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>, offset_minutes: i32) -> Result<crate::history::Stats, String> {
    Ok(match state.history() {
        Some(store) => store.stats(offset_minutes),
        None => crate::history::Stats::default(),
    })
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(store) = state.history() {
        store.clear();
    }
    Ok(())
}

#[tauri::command]
pub fn set_history_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    if let Some(store) = state.history() {
        store.set_enabled(enabled);
    }
    let mut config = state.config();
    config.history.enabled = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_history_max_entries(state: State<'_, AppState>, max_entries: usize) -> Result<(), String> {
    let max_entries = max_entries.clamp(1, 1_000_000);
    if let Some(store) = state.history() {
        store.set_max_entries(max_entries);
    }
    let mut config = state.config();
    config.history.max_entries = max_entries;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

Add a DTO test next to `track_page_dto_serializes_fields`:

```rust
    #[test]
    fn history_page_dto_serializes_fields() {
        let dto = HistoryPageDto { items: Vec::new(), total: 3, offset: 1, limit: 100 };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["total"], 3);
        assert_eq!(value["offset"], 1);
        assert_eq!(value["limit"], 100);
        assert!(value["items"].as_array().unwrap().is_empty());
    }
```

- [ ] **Step 6: Wire the listener, logout, startup, and exit**

1. In `build_player`, after the scrobbler subscribe (commands.rs:463-468), add the third listener:

```rust
    let history_handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        if let Some(history) = history_handle.state::<AppState>().history() {
            history.record(&event);
        }
    }));
```

2. In `auth_logout` (commands.rs:227-239), before `state.logout()`:

```rust
    if let Some(history) = state.history() {
        history.finalize_now();
    }
```

3. In `main.rs`, after the session-saver block (line ~192):

```rust
    if let Some(path) = history::history_path() {
        let config = state.config();
        state.set_history(std::sync::Arc::new(history::HistoryStore::start(
            path,
            config.history.max_entries,
            config.history.enabled,
        )));
    }
```

4. In `main.rs` `RunEvent::Exit` (line 406-418), after the session-saver flush:

```rust
                if let Some(history) = state.history() {
                    history.finalize_now();
                }
```

5. Register the five commands in `generate_handler!` after `commands::set_volume` (line 350):

```rust
            commands::get_history_page,
            commands::get_stats,
            commands::clear_history,
            commands::set_history_enabled,
            commands::set_history_max_entries,
```

- [ ] **Step 7: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green, including the two config tests, the state test, and the DTO test.

- [ ] **Step 8: Commit**

```bash
git add crates/oxitide-app/src/config.rs crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(history): expose history and stats to the UI"
```

---

### Task 5: UI plumbing and the History view

**Files:**
- Modify: `ui/src/lib/api.ts` (types near line 57, `Config` line 162-198, `trackToQueueItem` line 210, wrappers ~331)
- Modify: `ui/src/lib/nav.svelte.ts` (tabs line 11-18, `Route` line 17-35, `sameRoute` line 55-65, `parseRoute` line 89-160)
- Modify: `ui/src/lib/nav.test.ts` (new describe)
- Modify: `ui/src/lib/components/Icon.svelte` (after `"moon"`, line 51)
- Modify: `ui/src/lib/components/Sidebar.svelte` (after the Feed button, line 125-127)
- Modify: `ui/src/App.svelte` (imports ~8-26, route switch ~534)
- Create: `ui/src/lib/historyTime.ts` + `ui/src/lib/historyTime.test.ts`
- Create: `ui/src/views/History.svelte`
- Create: `ui/src/lib/components/HistoryStats.svelte`

**Interfaces:**
- Consumes: IPC from Task 4 (`get_history_page`, `get_stats`).
- Produces: `history` route with `recent`/`stats` tabs; `historyTime.{relativeTime, dayLabel, groupByDay}`; `api.getHistoryPage/getStats` — consumed by Tasks 6–7.

- [ ] **Step 1: Write the failing tests**

Add to `ui/src/lib/nav.test.ts`:

```ts
describe("history routes", () => {
  it("round-trips a history tab through storage", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "history", tab: "stats" });

    const restored = createNav(storage);

    expect(restored.route).toEqual({ name: "history", tab: "stats" });
  });

  it("treats different tabs of history as distinct entries", () => {
    const nav = createNav();
    nav.goto({ name: "history", tab: "recent" });
    nav.goto({ name: "history", tab: "stats" });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "history", tab: "recent" });
  });

  it("ignores a goto to the same history tab", () => {
    const nav = createNav();
    nav.goto({ name: "history", tab: "recent" });
    nav.goto({ name: "history", tab: "recent" });

    nav.back();
    expect(nav.route).toEqual({ name: "home" });
  });

  it("rejects stored history routes with an unknown tab", () => {
    const raw = '{"history":[{"name":"history","tab":"charts"}],"index":0}';
    expect(createNav(fakeStorage(raw)).route).toEqual({ name: "home" });
  });
});
```

Create `ui/src/lib/historyTime.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { groupByDay, relativeTime } from "./historyTime";

// Local time on purpose: the helpers are local-day based, so constructing the
// clock with the local Date constructor keeps the tests timezone-independent.
const NOW = new Date(2026, 9, 4, 12, 0, 0).getTime();

function hoursAgo(hours: number): number {
  return Math.floor((NOW - hours * 3_600_000) / 1000);
}

describe("relativeTime", () => {
  it("formats just now, minutes, hours, and days", () => {
    expect(relativeTime(hoursAgo(0.2), NOW)).toBe("just now");
    expect(relativeTime(hoursAgo(0.4), NOW)).toBe("24 min ago");
    expect(relativeTime(hoursAgo(3), NOW)).toBe("3 h ago");
    expect(relativeTime(hoursAgo(50), NOW)).toBe("2 d ago");
  });
});

describe("groupByDay", () => {
  it("groups consecutive local days with Today/Yesterday labels", () => {
    const entries = [
      { started_at: hoursAgo(1) },
      { started_at: hoursAgo(5) },
      { started_at: hoursAgo(30) },
      { started_at: hoursAgo(74) },
    ];
    const groups = groupByDay(entries, NOW);
    expect(groups.map((group) => group.label)).toEqual([
      "Today",
      "Yesterday",
      "Thu, Oct 1",
    ]);
    expect(groups[0].entries).toHaveLength(2);
    expect(groups[1].entries).toHaveLength(1);
    expect(groups[2].entries).toHaveLength(1);
  });

  it("returns one group for a single day", () => {
    const entries = [{ started_at: hoursAgo(1) }];
    expect(groupByDay(entries, NOW)).toEqual([{ label: "Today", entries }]);
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npm --prefix ui test -- src/lib/nav.test.ts src/lib/historyTime.test.ts`
Expected: nav tests fail on the unknown route; historyTime fails to resolve the module.

- [ ] **Step 3: Add the IPC types and wrappers**

In `ui/src/lib/api.ts`:

1. After `TrackPage` (line 57):

```ts
export interface HistoryEntry {
  track_id: number; title: string; artist: string; album: string;
  duration_secs: number; cover?: string | null;
  started_at: number; listened_ms: number; completed: boolean;
}
export interface HistoryPage { items: HistoryEntry[]; total: number; offset: number; limit: number }
export interface StatsTopEntry { name: string; plays: number; listened_ms: number }
export interface StatsTopTrack { track_id: number; title: string; artist: string; plays: number }
export interface StatsDaily { date: string; plays: number; listened_ms: number }
export interface Stats {
  total_listened_ms: number; listened_ms_7d: number; listened_ms_30d: number;
  total_plays: number; plays_7d: number; plays_30d: number;
  top_artists: StatsTopEntry[]; top_tracks: StatsTopTrack[]; top_albums: StatsTopEntry[];
  daily: StatsDaily[];
}
```

2. Add to `Config` after `scrobbling` (line 197):

```ts
  history: { enabled: boolean; max_entries: number };
```

3. After `trackToQueueItem` (line 219):

```ts
export function historyToQueueItem(entry: HistoryEntry): QueueItem {
  return {
    track_id: entry.track_id,
    title: entry.title,
    artist: entry.artist,
    album: entry.album,
    duration_secs: entry.duration_secs,
    cover: entry.cover ?? null,
  };
}
```

4. Add the wrappers after `setPlaySource` (line 331):

```ts
  getHistoryPage: (offset: number, limit: number) =>
    invoke<HistoryPage>("get_history_page", { offset, limit }),
  getStats: (offsetMinutes: number) => invoke<Stats>("get_stats", { offsetMinutes }),
  clearHistory: () => invoke<void>("clear_history"),
  setHistoryEnabled: (enabled: boolean) => invoke<void>("set_history_enabled", { enabled }),
  setHistoryMaxEntries: (maxEntries: number) =>
    invoke<void>("set_history_max_entries", { maxEntries }),
```

- [ ] **Step 4: Add the route, icon, and sidebar entry**

In `ui/src/lib/nav.svelte.ts`:

1. After the `ARTIST_TABS` block (line 18):

```ts
/// Tabs of the History page; route values, so they live with the route.
export type HistoryTab = "recent" | "stats";
const HISTORY_TABS: readonly HistoryTab[] = ["recent", "stats"];
```

2. Add to the `Route` union after `{ name: "feed" }` (line 19):

```ts
  | { name: "history"; tab: HistoryTab }
```

3. Add to `sameRoute` after the `artist-all` branch (line 61):

```ts
  if (a.name === "history" && b.name === "history") {
    return a.tab === b.tab;
  }
```

4. Add a `case "history":` to `parseRoute`'s switch after the `feed` case:

```ts
    case "history": {
      const tab = record.tab;
      if (typeof tab === "string" && HISTORY_TABS.includes(tab as HistoryTab)) {
        return { name: "history", tab: tab as HistoryTab };
      }
      return null;
    }
```

In `ui/src/lib/components/Icon.svelte`, after the `"moon"` entry (line 51):

```ts
    "history": '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /> <path d="M3 3v5h5" /> <path d="M12 7v5l4 2" />',
```

In `ui/src/lib/components/Sidebar.svelte`, after the Feed button (line 125-127):

```svelte
    <button class="nav-item" class:active={nav.route.name === "history"} aria-current={nav.route.name === "history" ? "page" : undefined} onclick={() => nav.goto({ name: "history", tab: "recent" })} title="History" aria-label="History">
      <Icon name="history" />{#if !rail}<span>History</span>{/if}
    </button>
```

In `ui/src/App.svelte`:

1. Add the view import with the others:

```ts
  import History from "./views/History.svelte";
```

2. Add the route branch after the feed line (line 534):

```svelte
          {:else if nav.route.name === "history"}<History {nav} {player} {toasts} tab={nav.route.tab} />
```

- [ ] **Step 5: Create the time helpers**

Create `ui/src/lib/historyTime.ts`:

```ts
/// Local-day grouping and relative timestamps for the History view. `nowMs`
/// is injected so the logic is testable in any timezone.

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/// "just now", "24 min ago", "3 h ago", "2 d ago".
export function relativeTime(startedAtSecs: number, nowMs: number): string {
  const age = Math.max(0, nowMs - startedAtSecs * 1000);
  if (age < MINUTE) return "just now";
  if (age < HOUR) return `${Math.floor(age / MINUTE)} min ago`;
  if (age < DAY) return `${Math.floor(age / HOUR)} h ago`;
  return `${Math.floor(age / DAY)} d ago`;
}

function startOfLocalDay(ms: number): number {
  const date = new Date(ms);
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/// "Today" / "Yesterday" / "Thu, Oct 1" for a local calendar day.
export function dayLabel(dayMs: number, nowMs: number): string {
  const today = startOfLocalDay(nowMs);
  if (dayMs === today) return "Today";
  if (dayMs === today - DAY) return "Yesterday";
  return new Intl.DateTimeFormat("en-US", {
    weekday: "short",
    month: "short",
    day: "numeric",
  }).format(new Date(dayMs));
}

export interface HistoryGroup<T> {
  label: string;
  entries: T[];
}

/// Group newest-first entries into consecutive local-day buckets.
export function groupByDay<T extends { started_at: number }>(
  entries: T[],
  nowMs: number,
): HistoryGroup<T>[] {
  const groups: HistoryGroup<T>[] = [];
  let currentDay: number | null = null;
  let current: HistoryGroup<T> | null = null;
  for (const entry of entries) {
    const day = startOfLocalDay(entry.started_at * 1000);
    if (day !== currentDay) {
      currentDay = day;
      current = { label: dayLabel(day, nowMs), entries: [] };
      groups.push(current);
    }
    current!.entries.push(entry);
  }
  return groups;
}
```

- [ ] **Step 6: Create the History view and stats panel**

Create `ui/src/views/History.svelte`:

```svelte
<script lang="ts">
  import { api, historyToQueueItem, type HistoryEntry, type Stats } from "../lib/api";
  import { groupByDay, relativeTime } from "../lib/historyTime";
  import type { HistoryTab, Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import HistoryStats from "../lib/components/HistoryStats.svelte";
  import Loading from "../lib/components/Loading.svelte";

  let {
    nav,
    player,
    toasts,
    tab,
  }: { nav: Nav; player: Player; toasts: Toasts; tab: HistoryTab } = $props();

  const TABS: { id: HistoryTab; label: string }[] = [
    { id: "recent", label: "Recently played" },
    { id: "stats", label: "Stats" },
  ];
  const PAGE = 100;

  let entries = $state<HistoryEntry[]>([]);
  let total = $state(0);
  let recentLoaded = $state(false);
  let recentLoading = $state(false);
  let recentError = $state("");

  let stats = $state<Stats | null>(null);
  let statsLoaded = $state(false);
  let statsError = $state("");

  const groups = $derived(groupByDay(entries, Date.now()));

  async function loadRecent(reset: boolean) {
    if (recentLoading) return;
    recentLoading = true;
    recentError = "";
    try {
      const page = await api.getHistoryPage(reset ? 0 : entries.length, PAGE);
      entries = reset ? page.items : [...entries, ...page.items];
      total = page.total;
    } catch (e) {
      recentError = String(e);
    } finally {
      recentLoading = false;
    }
  }

  async function loadStats() {
    statsError = "";
    try {
      stats = await api.getStats(new Date().getTimezoneOffset());
    } catch (e) {
      statsError = String(e);
    }
  }

  $effect(() => {
    if (tab === "recent" && !recentLoaded) {
      recentLoaded = true;
      void loadRecent(true);
    }
  });

  $effect(() => {
    if (tab === "stats" && !statsLoaded) {
      statsLoaded = true;
      void loadStats();
    }
  });

  function playEntry(entry: HistoryEntry) {
    const index = entries.indexOf(entry);
    if (index < 0) return;
    playSource.set(null);
    void api.playTracks(entries.map(historyToQueueItem), index).catch((e) => {
      toasts.show(String(e), { severity: "error" });
    });
  }

  function selectTab(next: HistoryTab) {
    nav.replace({ name: "history", tab: next });
  }
</script>

<div class="view-fill">
  <h2>History</h2>
  <nav class="tabs" aria-label="History sections">
    {#each TABS as entry (entry.id)}
      <button
        class="tab"
        class:active={tab === entry.id}
        aria-current={tab === entry.id ? "page" : undefined}
        onclick={() => selectTab(entry.id)}
      >{entry.label}</button>
    {/each}
  </nav>

  {#if tab === "recent"}
    <div class="tab-body">
      {#if recentError}
        <ErrorState message={recentError} onRetry={() => void loadRecent(true)} />
      {:else if !recentLoaded && entries.length === 0}
        <Loading />
      {:else if entries.length === 0}
        <EmptyState message="Nothing played yet." />
      {:else}
        {#each groups as group (group.label)}
          <section class="day">
            <h3>{group.label}</h3>
            {#each group.entries as entry, i (entry.started_at + "-" + entry.track_id + "-" + i)}
              <button
                class="row"
                class:playing={player.state.current?.track_id === entry.track_id && !entry.completed}
                onclick={() => playEntry(entry)}
                title={`Play ${entry.title}`}
              >
                <Cover uuid={entry.cover} size={40} radius={4} alt="" />
                <span class="meta">
                  <strong>{entry.title}</strong>
                  <span class="muted">{entry.artist}{entry.album ? ` — ${entry.album}` : ""}</span>
                </span>
                <span class="muted tnum when">{relativeTime(entry.started_at, Date.now())}</span>
              </button>
            {/each}
          </section>
        {/each}
        {#if entries.length < total}
          <p class="more">
            <button onclick={() => void loadRecent(false)} disabled={recentLoading}>Load more</button>
          </p>
        {/if}
      {/if}
    </div>
  {:else}
    <div class="tab-body">
      {#if statsError}
        <ErrorState message={statsError} onRetry={() => void loadStats()} />
      {:else if !stats}
        <Loading />
      {:else}
        <HistoryStats {stats} />
      {/if}
    </div>
  {/if}
</div>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--line);
    margin: 8px 0 16px;
  }
  .tab {
    margin-bottom: -1px;
    padding: 8px 12px;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
  }
  .tab:hover,
  .tab.active {
    color: var(--text);
  }
  .tab.active {
    border-bottom-color: var(--accent);
  }
  .tab-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 24px;
  }
  .day h3 {
    margin: 14px 0 6px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 4px;
    text-align: left;
    border-radius: var(--radius-control);
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row.playing strong {
    color: var(--accent);
  }
  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .meta strong,
  .meta .muted {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta .muted {
    font-size: 12px;
  }
  .when {
    font-size: 12px;
    white-space: nowrap;
  }
  .more {
    text-align: center;
    margin: 16px 0;
  }
</style>
```

Create `ui/src/lib/components/HistoryStats.svelte` (summary + top lists; the chart lands in Task 6):

```svelte
<script lang="ts">
  import { formatCount, formatTotalDuration } from "../format";
  import type { Stats } from "../api";

  let { stats }: { stats: Stats } = $props();
</script>

{#if stats.total_plays === 0}
  <p class="muted">No plays yet.</p>
{:else}
  <div class="cards">
    <div class="card">
      <span class="muted">All time</span>
      <strong class="tnum">{formatTotalDuration(stats.total_listened_ms / 1000)}</strong>
      <span class="muted">{formatCount(stats.total_plays)} plays</span>
    </div>
    <div class="card">
      <span class="muted">Last 7 days</span>
      <strong class="tnum">{formatTotalDuration(stats.listened_ms_7d / 1000)}</strong>
      <span class="muted">{formatCount(stats.plays_7d)} plays</span>
    </div>
    <div class="card">
      <span class="muted">Last 30 days</span>
      <strong class="tnum">{formatTotalDuration(stats.listened_ms_30d / 1000)}</strong>
      <span class="muted">{formatCount(stats.plays_30d)} plays</span>
    </div>
  </div>

  <div class="tops">
    <section>
      <h3>Top artists</h3>
      {#each stats.top_artists as entry (entry.name)}
        <p class="top">
          <span class="name">{entry.name}</span>
          <span class="muted tnum">{entry.plays}</span>
        </p>
      {/each}
    </section>
    <section>
      <h3>Top tracks</h3>
      {#each stats.top_tracks as entry (entry.track_id)}
        <p class="top">
          <span class="name">{entry.title} <span class="muted">{entry.artist}</span></span>
          <span class="muted tnum">{entry.plays}</span>
        </p>
      {/each}
    </section>
    <section>
      <h3>Top albums</h3>
      {#each stats.top_albums as entry (entry.name)}
        <p class="top">
          <span class="name">{entry.name}</span>
          <span class="muted tnum">{entry.plays}</span>
        </p>
      {/each}
    </section>
  </div>
{/if}

<style>
  .cards {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 20px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 150px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    background: var(--surface-1);
  }
  .card strong {
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 640;
  }
  .tops {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 20px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    margin: 0;
    padding: 4px 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
```

- [ ] **Step 7: Run the UI gates**

Run: `npm --prefix ui test -- src/lib/nav.test.ts src/lib/historyTime.test.ts; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green (nav tests, historyTime tests, then the full suite and svelte-check).

- [ ] **Step 8: Run the full Windows gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/nav.svelte.ts ui/src/lib/nav.test.ts ui/src/lib/components/Icon.svelte ui/src/lib/components/Sidebar.svelte ui/src/App.svelte ui/src/lib/historyTime.ts ui/src/lib/historyTime.test.ts ui/src/views/History.svelte ui/src/lib/components/HistoryStats.svelte
git commit -m "feat(ui): add the history view and recent list"
```

---

### Task 6: Stats chart

**Files:**
- Create: `ui/src/lib/historyChart.ts` + `ui/src/lib/historyChart.test.ts`
- Modify: `ui/src/lib/components/HistoryStats.svelte`

**Interfaces:**
- Consumes: `Stats.daily` (Task 5).
- Produces: the 30-day plays chart.

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/historyChart.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { barPercents } from "./historyChart";

describe("barPercents", () => {
  it("scales against the tallest day", () => {
    expect(barPercents([0, 2, 4])).toEqual([0, 50, 100]);
  });

  it("uses a floor of one so all-zero days stay flat", () => {
    expect(barPercents([0, 0, 0])).toEqual([0, 0, 0]);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npm --prefix ui test -- src/lib/historyChart.test.ts`
Expected: FAIL — cannot resolve `./historyChart`.

- [ ] **Step 3: Implement the chart helper and render it**

Create `ui/src/lib/historyChart.ts`:

```ts
/// Bar heights (percent of the tallest day) for the 30-day plays chart.
export function barPercents(plays: number[]): number[] {
  const max = Math.max(1, ...plays);
  return plays.map((value) => (value / max) * 100);
}
```

In `ui/src/lib/components/HistoryStats.svelte`:

1. Add the import:

```ts
  import { barPercents } from "../historyChart";
```

2. Add the chart between the cards `</div>` and `<div class="tops">`:

```svelte
  {@const heights = barPercents(stats.daily.map((day) => day.plays))}
  <section class="chart-wrap">
    <h3>Last 30 days</h3>
    <div class="chart" role="img" aria-label="Plays per day over the last 30 days">
      {#each stats.daily as day, i (day.date)}
        <div class="bar" title={`${day.date}: ${day.plays} ${day.plays === 1 ? "play" : "plays"}`}>
          <span class="fill" class:empty={day.plays === 0} style:height={`${heights[i]}%`}></span>
        </div>
      {/each}
    </div>
  </section>
```

3. Add the styles:

```css
  .chart-wrap {
    margin: 4px 0 20px;
  }
  .chart {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 110px;
  }
  .bar {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .fill {
    width: 100%;
    min-height: 2px;
    border-radius: 2px 2px 0 0;
    background: var(--accent);
  }
  .fill.empty {
    background: var(--line);
  }
```

- [ ] **Step 4: Run the UI gates**

Run: `npm --prefix ui test -- src/lib/historyChart.test.ts; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 5: Run the full Windows gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/historyChart.ts ui/src/lib/historyChart.test.ts ui/src/lib/components/HistoryStats.svelte
git commit -m "feat(ui): chart the last 30 days of plays"
```

---

### Task 7: Settings controls, smoke checklist, verification

**Files:**
- Modify: `ui/src/views/Settings.svelte` (imports, state ~41, functions ~321, storage tab ~658-703)
- Create: `docs/superpowers/plans/notes/2026-10-04-track10-3-history-stats-smoke.md`

**Interfaces:**
- Consumes: `api.{setHistoryEnabled, setHistoryMaxEntries, clearHistory}` (Task 5); `Config.history` (Task 5).
- Produces: the human verification record for this plan.

- [ ] **Step 1: Add the Settings controls**

In `ui/src/views/Settings.svelte`:

1. Add the import with the other component imports:

```ts
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
```

2. Add the state near the other local state (line ~41):

```ts
  let confirmingHistoryClear = $state(false);
```

3. Add the functions next to `setCacheMaxMb` (line ~321):

```ts
  function setHistoryEnabled(enabled: boolean) {
    if (!config) return;
    config = { ...config, history: { ...config.history, enabled } };
    api.setHistoryEnabled(enabled).catch(apply);
  }

  function setHistoryMaxEntries(value: number) {
    if (!config) return;
    const max_entries = Math.min(100_000, Math.max(100, Math.round(value)));
    config = { ...config, history: { ...config.history, max_entries } };
    api.setHistoryMaxEntries(max_entries).catch(apply);
  }

  function clearHistory() {
    confirmingHistoryClear = false;
    api.clearHistory().catch(apply);
  }
```

4. Add the section in the Storage tab after the cache block (line 702, before the panel's closing `</div>`):

```svelte
      <h3>Listening history</h3>
      <p class="muted">Recorded locally in history.jsonl and used by the History view and stats.</p>
      <p>
        <label>
          <input
            type="checkbox"
            checked={config.history.enabled}
            onchange={(e) => setHistoryEnabled((e.target as HTMLInputElement).checked)}
          />
          Record listening history
        </label>
      </p>
      <p>
        <label>
          Max entries:
          <input
            type="number"
            min="100"
            max="100000"
            value={config.history.max_entries}
            onchange={(e) => setHistoryMaxEntries(Number((e.target as HTMLInputElement).value))}
          />
        </label>
      </p>
      <p><button onclick={() => (confirmingHistoryClear = true)}>Clear history</button></p>
      {#if confirmingHistoryClear}
        <ConfirmDialog
          title="Clear listening history"
          message="This permanently deletes your local play history and stats."
          onconfirm={clearHistory}
          oncancel={() => (confirmingHistoryClear = false)}
        />
      {/if}
```

- [ ] **Step 2: Run the UI gates**

Run: `npm --prefix ui run check; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 3: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-04-track10-3-history-stats-smoke.md`:

```markdown
# Track 10.3 history-and-stats smoke checklist (2026-10-04)

Human check with a real account and audio. Run `npm --prefix ui run app:dev`
(Vite + Tauri). Mark each item pass/fail with a note; failures become plan
fixes, not silent exceptions.

1. Recently played
   - Play a track past 30 s, skip to the next, then open History from the
     sidebar.
   - PASS = the track appears under "Today" with its cover, title, artist,
     album, and a relative time; clicking it starts playback from that row.
2. Skips are not recorded
   - Skip a track after ~10 s. PASS = it does not appear in Recently played.
3. In-flight play
   - Start a track and open History while it plays.
     PASS = the current track is the first row, before it has been finalized.
4. Stats
   - Open the Stats tab. PASS = all-time/7-day/30-day listening time and play
     counts are plausible; top artists/tracks/albums list real plays; the
     30-day chart shows bars for the days you played, and the in-flight track
     is not counted until it finalizes.
5. Restart persistence
   - Quit and relaunch. PASS = the same history and stats return.
6. Day grouping
   - Play something, then (optionally) change the system clock forward a day
     and play again. PASS = the two plays sit under "Yesterday" and "Today".
7. Clear
   - Settings → Storage → Clear history (confirm).
     PASS = Recently played and Stats are empty and history.jsonl is gone.
8. Disable
   - Settings → Storage, turn off "Record listening history", play a track
     past 30 s. PASS = the play does not appear; re-enabling records new plays
     again.
9. Max entries (optional)
   - Set Max entries to 100, finalize more than 100 plays (or inspect the
     file). PASS = history.jsonl holds at most 100 lines, newest kept.

Record results below.

- [ ] 1 recently played
- [ ] 2 skips not recorded
- [ ] 3 in-flight play
- [ ] 4 stats + chart
- [ ] 5 restart persistence
- [ ] 6 day grouping
- [ ] 7 clear
- [ ] 8 disable
- [ ] 9 max entries
```

- [ ] **Step 4: Run the final gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add ui/src/views/Settings.svelte docs/superpowers/plans/notes/2026-10-04-track10-3-history-stats-smoke.md
git commit -m "feat(ui): add history controls and the smoke checklist"
```

The human runs the checklist separately (AGENTS.md: GUI smoke checks require a
human, a real account, and audio). Failures become plan fixes, not silent
exceptions.

---

## Self-Review

- **Spec coverage (spec §5.1, §5.3, §6.3, §7, §8; QoL-D9):**
  - `history.jsonl` record fields, append on finalize, prune at `history.max_entries` → Tasks 1–2 (with the `cover` refinement).
  - Recorder rides the scrobbler's engine-event feed, finalizes on transition / queue end / exit, thresholds ≥30 s or ≥50% under 60 s, `completed` at ≥90% or natural end → Task 1; exit/logout flush → Tasks 2/4.
  - `get_history_page` prepends the in-flight play; `clear_history` empties the log → Tasks 2–4.
  - History route from the library sidebar with Recently-played and Stats tabs; day grouping with relative timestamps; clicking a row plays it → Task 5.
  - Stats totals (all-time / 7 / 30 days), play counts, top artists/tracks/albums, 30-day inline CSS chart; local-timezone day grouping with UTC `started_at` → Tasks 3, 5, 6.
  - `config.toml` `[history] enabled` + `max_entries`, commands, `ui/src/lib/api.ts` sync, `generate_handler!` → Task 4; retention/privacy controls → Task 7.
  - `get_stats` ≤50 ms at the 10k cap → the Task 3 timing test; writes off the position path → Task 2's writer thread; no new dependencies → Global Constraints.
- **Placeholder scan:** no TBD/TODO; every code step carries complete code. The Stats tab is fully functional at the end of Task 5; Task 6 adds the chart on top.
- **Type consistency:** `HistoryRecord` (Rust) ↔ `HistoryEntry` (TS) field names match the serde output (`track_id`, `started_at`, `listened_ms`, `completed`, `cover`); `Stats` field names match `Stats`/`TopEntry`/`TopTrack`/`DailyEntry`; `get_stats`'s `offset_minutes` ↔ `offsetMinutes` follows the IPC contract; `HistoryTab` is identical in `nav.svelte.ts`, `History.svelte`, and `App.svelte`; `barPercents` is defined in Task 6 and consumed there.
- **Review Focus:** each line names its pinning tests or smoke step; the five lines cover skipped plays, exactly-once finalization, in-flight exclusion from stats, prune/malformed resilience, and local-day bucketing.

