//! Local playback history: a pure listen tracker, a JSONL store with a
//! background writer, and stats derived on read (spec §5.1, §6.3; QoL-D9).
//! No network, no engine changes: it rides the same event feed as scrobbling.

use std::path::PathBuf;

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

/// `%APPDATA%\OxiTide\history.jsonl`, beside `config.toml`.
pub fn history_path() -> Option<PathBuf> {
    crate::config::config_path().map(|path| path.with_file_name("history.jsonl"))
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
}
