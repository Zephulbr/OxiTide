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
    pending_start: Option<QueueItem>,
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
        self.pending_start = None;
        if item.artist.is_empty() || item.title.is_empty() {
            tracing::debug!(
                track_id = item.track_id,
                "skipping now-playing/scrobbling for missing artist or title"
            );
            self.current = None;
            return Vec::new();
        }
        self.current = Some(item.clone());
        self.pending_start = Some(item);
        Vec::new()
    }

    fn position(&mut self, position_ms: i64, duration_ms: i64) -> Vec<ScrobbleIntent> {
        // Borrow `current` for the common no-op path; only clone it when we
        // actually emit a scrobble (spec §10: allocate on track change only).
        let Some(duration_secs) = self.current.as_ref().map(|item| item.duration_secs) else {
            return Vec::new();
        };
        let delta = position_ms - self.last_pos_ms;
        if delta > 0 && delta <= SEEK_MAX_MS {
            self.listened_ms += delta;
        }
        self.last_pos_ms = position_ms;
        if self.scrobbled {
            return Vec::new();
        }
        let duration = if duration_ms > 0 { duration_ms } else { duration_secs as i64 * 1000 };
        let threshold = (duration / 2).min(SCROBBLE_MIN_MS);
        if duration_secs > MIN_TRACK_SECS && self.listened_ms >= threshold {
            self.scrobbled = true;
            let item = self.current.clone().expect("current is present");
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
        if let Some(item) = self.pending_start.take() {
            self.paused_since = None;
            self.started_at = now;
            return vec![ScrobbleIntent::NowPlaying(item)];
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
    fn backward_seek_is_not_counted() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 200), 100);
        for step in 1..=90 {
            pos(&mut tracker, step * 1000, 200_000);
        }
        // Seek back 60 s; the negative delta must not add to listened time.
        assert!(!scrobbled(&pos(&mut tracker, 30_000, 200_000)));
        // Nine forward seconds still fall short of the 100 s threshold.
        for step in 31..=39 {
            assert!(!scrobbled(&pos(&mut tracker, step * 1000, 200_000)), "step {step}");
        }
        assert!(scrobbled(&pos(&mut tracker, 40_000, 200_000)));
    }

    #[test]
    fn no_accumulation_while_paused() {
        let mut tracker = ScrobbleTracker::default();
        start(&mut tracker, item(1, 200), 100);
        for step in 1..=5 {
            pos(&mut tracker, step * 1000, 200_000);
        }
        let out = tracker.on_event(&EngineEvent::State(playback_state(false)), 1_000);
        assert!(out.is_empty());
        let mut out = Vec::new();
        for _ in 0..200 {
            out.extend(pos(&mut tracker, 5_000, 200_000));
        }
        assert!(!scrobbled(&out), "paused stretch must not accumulate: {out:?}");
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
        tracker.on_event(&EngineEvent::State(playback_state(true)), 100);
        for step in 1..=50 {
            pos(&mut tracker, step * 1000, 100_000);
        }
        // Replaying the same id must be a fresh play.
        let out = start(&mut tracker, item(1, 100), 200);
        assert!(out.is_empty(), "start only pends the ping");
        let out = tracker.on_event(&EngineEvent::State(playback_state(true)), 200);
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
        let out = tracker.on_event(&state(true), 110);
        assert!(matches!(out.as_slice(), [ScrobbleIntent::NowPlaying(_)]), "playback start pings");
        assert!(tracker.on_event(&state(false), 1_000).is_empty());
        assert!(tracker.on_event(&state(true), 1_010).is_empty(), "short pause should not re-ping");
        tracker.on_event(&state(false), 2_000);
        let out = tracker.on_event(&state(true), 2_100);
        assert!(matches!(out.as_slice(), [ScrobbleIntent::NowPlaying(_)]));
    }

    #[test]
    fn paused_restore_does_not_ping_until_playback_starts() {
        let mut tracker = ScrobbleTracker::default();
        assert!(tracker
            .on_event(&EngineEvent::NowPlaying(Some(item(1, 200))), 100)
            .is_empty(), "restore must not ping");
        assert!(tracker
            .on_event(&EngineEvent::State(playback_state(false)), 110)
            .is_empty(), "restored-paused must not ping");
        let out = tracker.on_event(&EngineEvent::State(playback_state(true)), 5_000);
        assert!(matches!(out.as_slice(), [ScrobbleIntent::NowPlaying(_)]), "first play emits one ping");
        assert!(
            tracker.on_event(&EngineEvent::State(playback_state(true)), 5_100).is_empty(),
            "no duplicate ping"
        );
    }

    fn playback_state(playing: bool) -> oxitide_audio::engine::PlaybackState {
        oxitide_audio::engine::PlaybackState {
            playing,
            buffering: false,
            current: Some(item(1, 200)),
            current_index: None,
            position_ms: 0,
            duration_ms: 200_000,
            volume: 1.0,
            shuffle: false,
            repeat: oxitide_audio::queue::RepeatMode::Off,
            preferred_quality: oxitide_tidal::stream::Quality::Lossless,
            output_mode: oxitide_audio::output::OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            dsp_active: false,
            dsp_stages: Vec::new(),
            crossfade_ms: 0,
            queue: Vec::new(),
            stream: None,
        }
    }
}
