use std::fs;
use std::path::{Path, PathBuf};

use oxitide_audio::engine::StreamMeta;
use oxitide_audio::queue::{QueueItem, RepeatMode};
use serde::{Deserialize, Serialize};

pub const SESSION_VERSION: u32 = 1;

/// One queue item as persisted. `explicit`/`ai` are reserved for the Track 10.5
/// content filters; they are `None` until that plan populates them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// The last stream actually served for the current track, so a paused
    /// restore can still render the detailed quality readout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<StreamMeta>,
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
            stream: None,
            play_source: None,
            saved_at: 0,
        }
    }
}

/// `%APPDATA%\OxiTide\config\session.json`, beside `config.toml`.
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
            tracing::warn!(
                "malformed session at {} ({:?} at line {}, column {})",
                path.display(),
                e.classify(),
                e.line(),
                e.column()
            );
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

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxitide_audio::engine::{EngineEvent, PlaybackState};

use crate::config::ContentConfig;
use crate::track_flags::{is_blocked, TrackFlagsStore};

/// Caps how often position-only updates are written. State changes (track,
/// queue, pause, shuffle, repeat) are urgent and flush on the next saver wake
/// (the loop polls at most every 100 ms); this window only paces position ticks.
pub const SESSION_DEBOUNCE: Duration = Duration::from_secs(3);

#[allow(clippy::large_enum_variant)]
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
    flags: Arc<Mutex<Option<Arc<TrackFlagsStore>>>>,
}

impl SessionSaver {
    pub fn start(path: PathBuf, debounce: Duration) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let flags = Arc::new(Mutex::new(None));
        let worker_flags = flags.clone();
        thread::Builder::new()
            .name("oxitide-session".into())
            .spawn(move || saver_main(path, debounce, worker_flags, rx))
            .expect("spawn session saver");
        Self { tx, flags }
    }

    /// Wire the flag cache after both services exist (main.rs startup).
    pub fn set_flags(&self, source: Arc<TrackFlagsStore>) {
        *self.flags.lock().unwrap() = Some(source);
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

fn session_from_state(
    state: &PlaybackState,
    play_source: Option<String>,
    saved_at: u64,
    flags: Option<&TrackFlagsStore>,
) -> Session {
    Session {
        version: SESSION_VERSION,
        queue: state
            .queue
            .iter()
            .map(|item| {
                let mut saved = SessionItem::from(item);
                if let Some(entry) = flags.and_then(|store| store.get(item.track_id)) {
                    saved.explicit = entry.explicit;
                    saved.ai = entry.ai;
                }
                saved
            })
            .collect(),
        current_index: state.current_index,
        position_ms: state.position_ms,
        shuffle: state.shuffle,
        repeat: state.repeat,
        play_source,
        stream: state.stream.clone(),
        saved_at,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilteredQueue {
    pub items: Vec<SessionItem>,
    pub current_index: Option<usize>,
    pub position_ms: u64,
}

/// Drop persisted items the active filters hide (unknown flags pass), then
/// remap the current index: the same item when kept, else the next kept one,
/// else the previous kept one. Dropping the current item resets position.
pub fn filter_session_queue(
    queue: Vec<SessionItem>,
    current_index: Option<usize>,
    position_ms: u64,
    content: &ContentConfig,
) -> FilteredQueue {
    let blocked: Vec<bool> = queue
        .iter()
        .map(|item| is_blocked(item.explicit, item.ai, content))
        .collect();
    if !blocked.iter().any(|blocked| *blocked) {
        return FilteredQueue { items: queue, current_index, position_ms };
    }
    let kept: Vec<usize> = (0..queue.len()).filter(|index| !blocked[*index]).collect();
    let current_kept = current_index
        .and_then(|index| blocked.get(index).copied())
        .is_some_and(|blocked| !blocked);
    let position_ms = if current_kept { position_ms } else { 0 };
    let Some(current) = current_index else {
        let items = queue
            .into_iter()
            .zip(blocked.iter())
            .filter(|(_, blocked)| !**blocked)
            .map(|(item, _)| item)
            .collect();
        return FilteredQueue { items, current_index: None, position_ms };
    };
    let new_current = if current_kept {
        kept.iter().position(|index| *index == current)
    } else {
        kept.iter().position(|index| *index >= current).or(Some(kept.len().saturating_sub(1)))
    };
    let new_current = new_current.filter(|_| !kept.is_empty());
    let items = queue
        .into_iter()
        .zip(blocked.iter())
        .filter(|(_, blocked)| !**blocked)
        .map(|(item, _)| item)
        .collect();
    FilteredQueue { items, current_index: new_current, position_ms }
}

fn saver_main(
    path: PathBuf,
    debounce: Duration,
    flags: Arc<Mutex<Option<Arc<TrackFlagsStore>>>>,
    rx: Receiver<SaverMsg>,
) {
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
                let session_changed = match snapshot.as_ref() {
                    None => true,
                    Some(prev) => {
                        prev.queue != state.queue
                            || prev.current_index != state.current_index
                            || prev.shuffle != state.shuffle
                            || prev.repeat != state.repeat
                            || prev.playing != state.playing
                    }
                };
                if let Some(prev) = snapshot.as_ref() {
                    if prev.position_ms != state.position_ms {
                        dirty = true;
                    }
                }
                if session_changed {
                    dirty = true;
                    urgent = true;
                }
                snapshot = Some(state);
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
                    write_snapshot(&path, snapshot.as_ref(), &play_source, &flags);
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
            write_snapshot(&path, snapshot.as_ref(), &play_source, &flags);
            dirty = false;
            urgent = false;
            last_flush = Instant::now();
        }
    }
}

fn write_snapshot(
    path: &Path,
    snapshot: Option<&PlaybackState>,
    play_source: &Option<String>,
    flags: &Arc<Mutex<Option<Arc<TrackFlagsStore>>>>,
) {
    let Some(state) = snapshot else { return };
    let saved_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let source = flags.lock().unwrap().clone();
    let session = session_from_state(state, play_source.clone(), saved_at, source.as_deref());
    if let Err(e) = save(path, &session) {
        tracing::warn!("failed to save session: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_audio::engine::{EngineEvent, PlaybackState};
    use oxitide_tidal::stream::Quality;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

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
            stream: Some(StreamMeta {
                quality: Some(Quality::Max),
                codec: Some("flac".into()),
                sample_rate: 44_100,
                bit_depth: 24,
                channels: 2,
                bitrate_kbps: Some(1058),
                replay_gain_db: None,
            }),
            saved_at: 1_700_000_000,
        }
    }

    #[test]
    fn session_from_state_captures_the_stream_meta() {
        let mut state = playback_state(&[1], Some(0), 0);
        state.stream = Some(StreamMeta {
            quality: Some(Quality::Max),
            codec: Some("flac".into()),
            sample_rate: 44_100,
            bit_depth: 24,
            channels: 2,
            bitrate_kbps: Some(1058),
            replay_gain_db: None,
        });
        let saved = session_from_state(&state, None, 100, None);
        assert_eq!(saved.stream.expect("stream").bit_depth, 24);
    }

    #[test]
    fn legacy_session_without_stream_loads_with_none() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let mut value = serde_json::to_value(session()).unwrap();
        value.as_object_mut().unwrap().remove("stream");
        fs::write(&path, value.to_string()).unwrap();
        assert_eq!(load(&path).unwrap().stream, None);
        let _ = fs::remove_dir_all(dir);
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
            dsp_active: false,
            dsp_stages: Vec::new(),
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

    /// clear() removes the persisted file.
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
    fn saver_volume_only_states_stay_off_the_write_path() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let saver = SessionSaver::start(path.clone(), Duration::from_secs(60));
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 100)));
        saver.flush_now();
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        let mut quiet = playback_state(&[1], Some(0), 100);
        quiet.volume = 0.5;
        saver.record(&EngineEvent::State(quiet));
        std::thread::sleep(Duration::from_millis(300));
        let after = fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before, after, "a volume-only state must not rewrite the session");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saver_queue_change_is_urgent() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let saver = SessionSaver::start(path.clone(), Duration::from_secs(60));
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 0)));
        saver.flush_now();
        assert_eq!(load(&path).unwrap().queue.len(), 1);
        saver.record(&EngineEvent::State(playback_state(&[1, 2], Some(0), 0)));
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(load(&path).unwrap().queue.len(), 2, "queue edits must flush promptly");
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

    fn content(block_explicit: bool, block_ai: bool) -> crate::config::ContentConfig {
        crate::config::ContentConfig { block_explicit, block_ai, ..Default::default() }
    }

    fn flagged(id: u64, explicit: Option<bool>, ai: Option<bool>) -> SessionItem {
        SessionItem { explicit, ai, ..item(id) }
    }

    #[test]
    fn filter_session_queue_drops_blocked_and_remaps_the_dropped_current() {
        let queue = vec![
            flagged(1, Some(true), None),
            flagged(2, None, Some(true)),
            flagged(3, None, None),
        ];
        // The current item (index 1, track 2) is AI-blocked and must be replaced
        // by the next kept item (track 3), with the position reset.
        let filtered = filter_session_queue(queue, Some(1), 5_000, &content(false, true));
        assert_eq!(
            filtered.items.iter().map(|item| item.track_id).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(filtered.current_index, Some(1));
        assert_eq!(filtered.position_ms, 0);
    }

    #[test]
    fn filter_session_queue_falls_back_to_the_previous_kept_current() {
        let queue = vec![flagged(9, None, None), flagged(2, None, Some(true))];
        let filtered = filter_session_queue(queue, Some(1), 5_000, &content(false, true));
        assert_eq!(filtered.items.iter().map(|item| item.track_id).collect::<Vec<_>>(), vec![9]);
        assert_eq!(filtered.current_index, Some(0));
        assert_eq!(filtered.position_ms, 0);
    }

    #[test]
    fn filter_session_queue_keeps_position_when_the_current_item_survives() {
        let queue = vec![flagged(1, Some(true), None), flagged(2, Some(false), None)];
        let filtered = filter_session_queue(queue, Some(1), 42_000, &content(true, false));
        assert_eq!(filtered.items.iter().map(|item| item.track_id).collect::<Vec<_>>(), vec![2]);
        assert_eq!(filtered.current_index, Some(0));
        assert_eq!(filtered.position_ms, 42_000);
    }

    #[test]
    fn filter_session_queue_passes_through_when_filters_are_off() {
        let queue = vec![flagged(1, Some(true), Some(true))];
        let filtered = filter_session_queue(queue, Some(0), 1_000, &ContentConfig::default());
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.current_index, Some(0));
        assert_eq!(filtered.position_ms, 1_000);
    }

    #[test]
    fn session_from_state_captures_cached_flags() {
        let store = crate::track_flags::TrackFlagsStore::start(None);
        store.record_explicit(3, true);
        let state = playback_state(&[3], Some(0), 0);
        let session = session_from_state(&state, None, 100, Some(&store));
        assert_eq!(session.queue[0].explicit, Some(true));
        assert_eq!(session.queue[0].ai, None);
    }

    #[test]
    fn saver_persists_flags_set_after_start() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let store = std::sync::Arc::new(crate::track_flags::TrackFlagsStore::start(None));
        store.record_explicit(1, true);
        let saver = SessionSaver::start(path.clone(), Duration::from_secs(60));
        saver.set_flags(store);
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 0)));
        saver.flush_now();
        let session = load(&path).expect("session");
        assert_eq!(session.queue[0].explicit, Some(true));
        let _ = fs::remove_dir_all(dir);
    }
}
