use std::sync::mpsc::SyncSender;

use oxitide_audio::engine::{EngineCmd, EngineEvent};
use oxitide_audio::queue::QueueItem;

#[cfg(windows)]
#[path = "media_controls_win.rs"]
mod win;

#[cfg(target_os = "linux")]
#[path = "media_controls_linux.rs"]
mod linux;

#[cfg(windows)]
pub use win::MediaSession;

#[cfg(target_os = "linux")]
pub use linux::MediaSession;

#[cfg(not(any(windows, target_os = "linux")))]
pub use stub::MediaSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    Playing,
    Paused,
    #[default]
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
        EngineEvent::Position { position_ms, duration_ms, .. } => {
            let position =
                if *duration_ms > 0 { (*position_ms).min(*duration_ms) } else { *position_ms };
            (
                vec![SessionUpdate::Position { position_ms: position, duration_ms: *duration_ms }],
                last_track,
            )
        }
        EngineEvent::Beat(_) | EngineEvent::Error { .. } => (Vec::new(), last_track),
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

/// The MPRIS-facing playback snapshot. Platform-neutral so the fold below is
/// unit-tested on Windows; only the Linux adapter turns it into D-Bus calls.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MprisState {
    pub track: Option<TrackMeta>,
    pub status: PlayState,
    pub position_ms: u64,
    pub duration_ms: u64,
    /// MPRIS `Stop` was the last transport request. The engine has no stop
    /// state (it pauses at 0), so its paused reports keep showing `Stopped`
    /// until playback resumes or the track changes.
    pub stopped: bool,
}

/// What the adapter must push after folding one session update.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MprisEffect {
    /// Metadata (or its clearing) changed.
    Track,
    /// Playback status changed.
    Playback,
    /// Position advanced; `seeked` is true when it jumped unexpectedly.
    Position { seeked: bool },
}

/// Position updates arrive about every 500 ms while playing; a jump larger
/// than this from the expected progression is an external seek.
#[cfg(any(target_os = "linux", test))]
const SEEK_TOLERANCE_MS: u64 = 1_500;

/// Whether a position update jumped (an external seek) rather than progressed.
#[cfg(any(target_os = "linux", test))]
fn is_seek(previous: PlayState, previous_ms: u64, elapsed_ms: u64, incoming_ms: u64) -> bool {
    previous == PlayState::Playing
        && incoming_ms.abs_diff(previous_ms.saturating_add(elapsed_ms)) > SEEK_TOLERANCE_MS
}

/// Fold one neutral session update into the snapshot and report what changed.
#[cfg(any(target_os = "linux", test))]
pub fn apply_session_update(
    state: &mut MprisState,
    update: &SessionUpdate,
    elapsed_ms: u64,
) -> MprisEffect {
    match update {
        SessionUpdate::Track(track) => {
            state.track = track.clone();
            state.position_ms = 0;
            state.duration_ms = track.as_ref().map_or(0, |track| track.duration_ms);
            state.stopped = false;
            MprisEffect::Track
        }
        SessionUpdate::Playback(play) => {
            if *play != PlayState::Paused {
                state.stopped = false;
            }
            state.status = if state.stopped { PlayState::Stopped } else { *play };
            MprisEffect::Playback
        }
        SessionUpdate::Position { position_ms, duration_ms } => {
            let seeked = is_seek(state.status, state.position_ms, elapsed_ms, *position_ms);
            state.position_ms = *position_ms;
            state.duration_ms = *duration_ms;
            MprisEffect::Position { seeked }
        }
    }
}

/// MPRIS `Play`: no-op unless paused or stopped.
#[cfg(any(target_os = "linux", test))]
pub fn control_for_play(state: &MprisState) -> Option<ControlCommand> {
    (state.status != PlayState::Playing).then_some(ControlCommand::PlayPause)
}

/// MPRIS `Pause`: no-op unless playing.
#[cfg(any(target_os = "linux", test))]
pub fn control_for_pause(state: &MprisState) -> Option<ControlCommand> {
    (state.status == PlayState::Playing).then_some(ControlCommand::PlayPause)
}

/// MPRIS `Stop`: the engine has no separate stop state, so stop pauses and
/// rewinds to the start, and the snapshot reports `Stopped` from now on (see
/// [`MprisState::stopped`]). The queue and metadata stay, so `Play` resumes
/// the same track from 0. Returns the engine commands to run, in order.
#[cfg(any(target_os = "linux", test))]
pub fn request_stop(state: &mut MprisState) -> Vec<ControlCommand> {
    if state.track.is_none() {
        return Vec::new();
    }
    let mut commands = Vec::new();
    if state.status == PlayState::Playing {
        commands.push(ControlCommand::PlayPause);
    }
    commands.push(ControlCommand::Seek(0));
    state.stopped = true;
    state.status = PlayState::Stopped;
    state.position_ms = 0;
    commands
}

/// MPRIS relative `Seek` offset -> absolute engine target, clamped to the
/// track bounds when the duration is known.
#[cfg(any(target_os = "linux", test))]
pub fn seek_target(state: &MprisState, offset_ms: i64) -> u64 {
    let target = (state.position_ms as i64).saturating_add(offset_ms).max(0) as u64;
    if state.duration_ms > 0 {
        target.min(state.duration_ms)
    } else {
        target
    }
}

/// `.desktop` ids MPRIS `DesktopEntry` may name, most specific first: the
/// deb/AppImage bundle's `OxiTide.desktop`, the AUR package's
/// `oxitide.desktop`, and the handler entry the deep-link plugin writes for
/// dev and AppImage runs. Desktops resolve the player's name and icon from it.
#[cfg(any(target_os = "linux", test))]
pub const DESKTOP_ENTRIES: [&str; 3] = ["OxiTide", "oxitide", "oxitide-app-handler"];

/// The first desktop entry `installed` reports, else the bundle's name.
#[cfg(any(target_os = "linux", test))]
pub fn desktop_entry(installed: impl Fn(&str) -> bool) -> &'static str {
    DESKTOP_ENTRIES.into_iter().find(|name| installed(name)).unwrap_or(DESKTOP_ENTRIES[0])
}

/// D-Bus object path for `mpris:trackid`. Object-path elements may not start
/// with a digit, hence the `t` prefix.
#[cfg(any(target_os = "linux", test))]
pub fn track_path(track_id: u64) -> String {
    format!("/com/oxitide/track/t{track_id}")
}

#[cfg(not(any(windows, target_os = "linux")))]
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
            artist_refs: Vec::new(),
            album_id: None,
            autoplay: false,
        }
    }

    fn state(playing: bool, current: Option<QueueItem>) -> EngineEvent {
        EngineEvent::State(PlaybackState {
            playing,
            buffering: false,
            current,
            current_index: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            preferred_quality: Quality::Lossless,
            output_mode: OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            dsp_active: false,
            dsp_stages: Vec::new(),
            crossfade_ms: 0,
            last_transition: None,
            queue: Default::default(),
            queue_rev: 0,
            ended: false,
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
            plan(&EngineEvent::Position { position_ms: 250_000, duration_ms: 200_000, lead_ms: 0 }, None);
        assert_eq!(
            updates,
            vec![SessionUpdate::Position { position_ms: 200_000, duration_ms: 200_000 }]
        );
        let (updates, _) = plan(&EngineEvent::Position { position_ms: 5, duration_ms: 0, lead_ms: 0 }, None);
        assert_eq!(updates, vec![SessionUpdate::Position { position_ms: 5, duration_ms: 0 }]);
    }

    #[test]
    fn control_commands_map_to_engine_commands() {
        assert!(matches!(to_engine_cmd(ControlCommand::PlayPause), EngineCmd::PlayPause));
        assert!(matches!(to_engine_cmd(ControlCommand::Next), EngineCmd::Next));
        assert!(matches!(to_engine_cmd(ControlCommand::Prev), EngineCmd::Prev));
        assert!(matches!(to_engine_cmd(ControlCommand::Seek(42)), EngineCmd::Seek(42)));
    }

    fn meta(track_id: u64) -> TrackMeta {
        TrackMeta {
            track_id,
            title: format!("Title {track_id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: 200_000,
            cover: Some("x".into()),
        }
    }

    #[test]
    fn mpris_state_folds_track_playback_and_position() {
        let mut snapshot = MprisState::default();
        assert_eq!(
            apply_session_update(&mut snapshot, &SessionUpdate::Track(Some(meta(7))), 0),
            MprisEffect::Track
        );
        assert_eq!(snapshot.track.as_ref().unwrap().track_id, 7);
        assert_eq!(snapshot.duration_ms, 200_000);
        assert_eq!(snapshot.position_ms, 0);

        assert_eq!(
            apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Playing), 0),
            MprisEffect::Playback
        );
        assert_eq!(snapshot.status, PlayState::Playing);

        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 500, duration_ms: 200_000 },
                500,
            ),
            MprisEffect::Position { seeked: false }
        );
        assert_eq!(snapshot.position_ms, 500);
    }

    #[test]
    fn mpris_state_track_change_replaces_metadata_and_resets_position() {
        let mut snapshot = MprisState::default();
        apply_session_update(&mut snapshot, &SessionUpdate::Track(Some(meta(1))), 0);
        apply_session_update(
            &mut snapshot,
            &SessionUpdate::Position { position_ms: 60_000, duration_ms: 200_000 },
            0,
        );
        assert_eq!(
            apply_session_update(&mut snapshot, &SessionUpdate::Track(Some(meta(2))), 0),
            MprisEffect::Track
        );
        let track = snapshot.track.expect("track");
        assert_eq!(track.track_id, 2);
        assert_eq!(snapshot.position_ms, 0);
        assert_eq!(snapshot.duration_ms, 200_000);
    }

    #[test]
    fn mpris_state_flags_external_seeks_but_not_progress_or_pauses() {
        let mut snapshot = MprisState::default();
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Playing), 0);
        apply_session_update(
            &mut snapshot,
            &SessionUpdate::Position { position_ms: 10_000, duration_ms: 200_000 },
            0,
        );
        // 600 ms later, 10.6 s of progress is not a seek.
        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 10_600, duration_ms: 200_000 },
                600,
            ),
            MprisEffect::Position { seeked: false }
        );
        // A 50 s jump forward while playing is an external seek.
        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 60_000, duration_ms: 200_000 },
                600,
            ),
            MprisEffect::Position { seeked: true }
        );
        // A paused gap resumes where it left off: not a seek.
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Paused), 0);
        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 60_000, duration_ms: 200_000 },
                5_000,
            ),
            MprisEffect::Position { seeked: false }
        );
    }

    #[test]
    fn mpris_seek_target_clamps_to_the_track() {
        let snapshot = MprisState {
            track: None,
            status: PlayState::Playing,
            position_ms: 30_000,
            duration_ms: 200_000,
            stopped: false,
        };
        assert_eq!(seek_target(&snapshot, -45_000), 0);
        assert_eq!(seek_target(&snapshot, 10_000), 40_000);
        assert_eq!(seek_target(&snapshot, 500_000), 200_000);
        let unknown = MprisState { duration_ms: 0, ..snapshot };
        assert_eq!(seek_target(&unknown, 500_000), 530_000);
    }

    #[test]
    fn mpris_controls_gate_on_playback_state() {
        let playing = MprisState { status: PlayState::Playing, ..Default::default() };
        let paused = MprisState { status: PlayState::Paused, ..Default::default() };
        assert_eq!(control_for_play(&playing), None);
        assert_eq!(control_for_pause(&playing), Some(ControlCommand::PlayPause));
        assert_eq!(control_for_play(&paused), Some(ControlCommand::PlayPause));
        assert_eq!(control_for_pause(&paused), None);
    }

    #[test]
    fn mpris_stop_pauses_rewinds_and_reports_stopped() {
        let mut playing = MprisState {
            track: Some(meta(1)),
            status: PlayState::Playing,
            position_ms: 60_000,
            duration_ms: 200_000,
            stopped: false,
        };
        assert_eq!(
            request_stop(&mut playing),
            vec![ControlCommand::PlayPause, ControlCommand::Seek(0)]
        );
        assert_eq!(playing.status, PlayState::Stopped);
        assert_eq!(playing.position_ms, 0);
        assert!(playing.track.is_some(), "stop keeps the track so Play resumes it");

        let mut paused = MprisState { status: PlayState::Paused, stopped: false, ..playing.clone() };
        assert_eq!(request_stop(&mut paused), vec![ControlCommand::Seek(0)]);

        let mut empty = MprisState::default();
        assert!(request_stop(&mut empty).is_empty());
        assert!(!empty.stopped);
    }

    #[test]
    fn mpris_stopped_survives_the_engines_pause_until_play() {
        let mut snapshot = MprisState {
            track: Some(meta(1)),
            status: PlayState::Playing,
            ..Default::default()
        };
        request_stop(&mut snapshot);
        // The engine reports the pause the stop issued: still Stopped.
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Paused), 0);
        assert_eq!(snapshot.status, PlayState::Stopped);
        assert_eq!(control_for_play(&snapshot), Some(ControlCommand::PlayPause));
        // Play resumes: the stop is over.
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Playing), 0);
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Paused), 0);
        assert_eq!(snapshot.status, PlayState::Paused);

        // A track change also ends it.
        request_stop(&mut snapshot);
        apply_session_update(&mut snapshot, &SessionUpdate::Track(Some(meta(2))), 0);
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Paused), 0);
        assert_eq!(snapshot.status, PlayState::Paused);
    }

    #[test]
    fn mpris_desktop_entry_follows_the_installed_package() {
        assert_eq!(desktop_entry(|name| name == "OxiTide" || name == "oxitide"), "OxiTide");
        assert_eq!(desktop_entry(|name| name == "oxitide"), "oxitide");
        assert_eq!(desktop_entry(|name| name == "oxitide-app-handler"), "oxitide-app-handler");
        assert_eq!(desktop_entry(|_| false), "OxiTide");
    }

    #[test]
    fn mpris_track_path_is_a_valid_dbus_object_path() {
        assert_eq!(track_path(7), "/com/oxitide/track/t7");
        assert_eq!(track_path(u64::MAX), "/com/oxitide/track/t18446744073709551615");
    }
}
