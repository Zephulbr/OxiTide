use std::sync::mpsc::SyncSender;

use oxitide_audio::engine::{EngineCmd, EngineEvent};
use oxitide_audio::queue::QueueItem;

#[cfg(windows)]
#[path = "media_controls_win.rs"]
mod win;

#[cfg(windows)]
pub use win::MediaSession;

#[cfg(not(windows))]
pub use stub::MediaSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMeta {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionUpdate {
    Track(Option<TrackMeta>),
    Playback(PlayState),
    Position { position_ms: u64, duration_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    PlayPause,
    Next,
    Prev,
    Seek(u64),
}

#[derive(Clone)]
pub struct SessionSender {
    tx: SyncSender<SessionUpdate>,
}

impl SessionSender {
    pub(crate) fn new(tx: SyncSender<SessionUpdate>) -> Self {
        Self { tx }
    }

    pub fn send(&self, update: SessionUpdate) {
        let _ = self.tx.try_send(update);
    }
}

pub fn cover_url(cover: &str, size: u32) -> Option<String> {
    if cover.is_empty() {
        return None;
    }
    if cover.starts_with("http") {
        return Some(cover.to_string());
    }
    Some(format!(
        "https://resources.tidal.com/images/{}/{size}x{size}.jpg",
        cover.replace('-', "/")
    ))
}

fn track_meta(item: &QueueItem) -> TrackMeta {
    TrackMeta {
        track_id: item.track_id,
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_secs as u64 * 1000,
        cover: item.cover.clone(),
    }
}

pub fn plan(event: &EngineEvent, last_track: Option<u64>) -> (Vec<SessionUpdate>, Option<u64>) {
    match event {
        EngineEvent::NowPlaying(Some(item)) => {
            if last_track == Some(item.track_id) {
                (Vec::new(), last_track)
            } else {
                (vec![SessionUpdate::Track(Some(track_meta(item)))], Some(item.track_id))
            }
        }
        EngineEvent::NowPlaying(None) => (
            vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)],
            None,
        ),
        EngineEvent::State(snapshot) => match &snapshot.current {
            Some(item) => {
                let play = if snapshot.playing { PlayState::Playing } else { PlayState::Paused };
                let mut updates = vec![SessionUpdate::Playback(play)];
                if last_track == Some(item.track_id) {
                    (updates, last_track)
                } else {
                    updates.insert(0, SessionUpdate::Track(Some(track_meta(item))));
                    (updates, Some(item.track_id))
                }
            }
            None => (
                vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)],
                None,
            ),
        },
        EngineEvent::Position { position_ms, duration_ms } => {
            let position =
                if *duration_ms > 0 { (*position_ms).min(*duration_ms) } else { *position_ms };
            (
                vec![SessionUpdate::Position { position_ms: position, duration_ms: *duration_ms }],
                last_track,
            )
        }
        EngineEvent::Error { .. } => (Vec::new(), last_track),
    }
}

pub fn to_engine_cmd(cmd: ControlCommand) -> EngineCmd {
    match cmd {
        ControlCommand::PlayPause => EngineCmd::PlayPause,
        ControlCommand::Next => EngineCmd::Next,
        ControlCommand::Prev => EngineCmd::Prev,
        ControlCommand::Seek(ms) => EngineCmd::Seek(ms),
    }
}

#[cfg(not(windows))]
mod stub {
    use std::sync::mpsc;

    use oxitide_audio::engine::Engine;

    use super::{SessionSender, SessionUpdate};

    pub struct MediaSession {
        tx: mpsc::SyncSender<SessionUpdate>,
    }

    impl MediaSession {
        pub fn spawn(_engine: Engine, _hwnd: isize, _enabled: bool) -> Result<Self, String> {
            let (tx, _rx) = mpsc::sync_channel(1);
            Ok(MediaSession { tx })
        }

        pub fn sender(&self) -> SessionSender {
            SessionSender::new(self.tx.clone())
        }

        pub fn set_enabled(&self, _enabled: bool) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_audio::engine::PlaybackState;
    use oxitide_audio::output::OutputMode;
    use oxitide_audio::queue::{QueueItem, RepeatMode};
    use oxitide_tidal::stream::Quality;

    fn item(track_id: u64, cover: Option<&str>) -> QueueItem {
        QueueItem {
            track_id,
            title: format!("Title {track_id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_secs: 200,
            cover: cover.map(str::to_string),
        }
    }

    fn state(playing: bool, current: Option<QueueItem>) -> EngineEvent {
        EngineEvent::State(PlaybackState {
            playing,
            buffering: false,
            current,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            preferred_quality: Quality::Lossless,
            output_mode: OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            queue: Vec::new(),
            stream: None,
        })
    }

    #[test]
    fn cover_url_handles_uuid_and_absolute() {
        assert_eq!(cover_url("", 320), None);
        assert_eq!(
            cover_url("2fa8b7a6-9d5b-4a1e-9f0a-4a1e9f0a4a1e", 320).unwrap(),
            "https://resources.tidal.com/images/2fa8b7a6/9d5b/4a1e/9f0a/4a1e9f0a4a1e/320x320.jpg"
        );
        assert_eq!(
            cover_url("https://example.com/a.jpg", 320).unwrap(),
            "https://example.com/a.jpg"
        );
    }

    #[test]
    fn now_playing_emits_track_once() {
        let (updates, last) = plan(&EngineEvent::NowPlaying(Some(item(7, Some("x")))), None);
        assert_eq!(last, Some(7));
        assert_eq!(
            updates,
            vec![SessionUpdate::Track(Some(TrackMeta {
                track_id: 7,
                title: "Title 7".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                duration_ms: 200_000,
                cover: Some("x".into()),
            }))]
        );
        let (again, last) = plan(&EngineEvent::NowPlaying(Some(item(7, Some("x")))), last);
        assert!(again.is_empty());
        assert_eq!(last, Some(7));
    }

    #[test]
    fn now_playing_none_clears_and_stops() {
        let (updates, last) = plan(&EngineEvent::NowPlaying(None), Some(7));
        assert_eq!(
            updates,
            vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)]
        );
        assert_eq!(last, None);
    }

    #[test]
    fn state_sets_playback_and_new_track_only() {
        let (updates, last) = plan(&state(true, Some(item(3, None))), None);
        assert_eq!(
            updates,
            vec![
                SessionUpdate::Track(Some(TrackMeta {
                    track_id: 3,
                    title: "Title 3".into(),
                    artist: "Artist".into(),
                    album: "Album".into(),
                    duration_ms: 200_000,
                    cover: None,
                })),
                SessionUpdate::Playback(PlayState::Playing),
            ]
        );
        let (again, _) = plan(&state(false, Some(item(3, None))), last);
        assert_eq!(again, vec![SessionUpdate::Playback(PlayState::Paused)]);
    }

    #[test]
    fn state_without_current_is_stopped() {
        let (updates, last) = plan(&state(false, None), Some(3));
        assert_eq!(
            updates,
            vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)]
        );
        assert_eq!(last, None);
    }

    #[test]
    fn position_is_clamped_to_duration() {
        let (updates, _) =
            plan(&EngineEvent::Position { position_ms: 250_000, duration_ms: 200_000 }, None);
        assert_eq!(
            updates,
            vec![SessionUpdate::Position { position_ms: 200_000, duration_ms: 200_000 }]
        );
        let (updates, _) = plan(&EngineEvent::Position { position_ms: 5, duration_ms: 0 }, None);
        assert_eq!(updates, vec![SessionUpdate::Position { position_ms: 5, duration_ms: 0 }]);
    }

    #[test]
    fn control_commands_map_to_engine_commands() {
        assert!(matches!(to_engine_cmd(ControlCommand::PlayPause), EngineCmd::PlayPause));
        assert!(matches!(to_engine_cmd(ControlCommand::Next), EngineCmd::Next));
        assert!(matches!(to_engine_cmd(ControlCommand::Prev), EngineCmd::Prev));
        assert!(matches!(to_engine_cmd(ControlCommand::Seek(42)), EngineCmd::Seek(42)));
    }
}
