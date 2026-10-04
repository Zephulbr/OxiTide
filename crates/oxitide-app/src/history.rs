//! Local playback history: a pure listen tracker, a JSONL store with a
//! background writer, and stats derived on read (spec §5.1, §6.3; QoL-D9).
//! No network, no engine changes: it rides the same event feed as scrobbling.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        self.flush();
    }

    /// Wait for the writer to process every queued append. Reads go through
    /// this so a page never races a just-finalized play.
    fn flush(&self) {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        if self.tx.send(WriterMsg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv_timeout(Duration::from_secs(2));
        }
    }

    /// Newest-first page; the in-flight play is prepended (spec §6.3) and is
    /// included in `total`.
    pub fn page(&self, offset: usize, limit: usize) -> (Vec<HistoryRecord>, usize) {
        self.flush();
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
        if self.tx.send(WriterMsg::Clear).is_ok() {
            self.flush();
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
        assert_eq!(store.page(0, 10).1, 1, "the page read also drains the writer");
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
}
