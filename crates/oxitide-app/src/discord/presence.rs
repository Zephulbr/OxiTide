use crate::media_controls::{cover_url, PlayState, TrackMeta};

pub const COVER_SIZE: u32 = 640;
const FIELD_LIMIT: usize = 128;
const SEEK_DRIFT_MS: u64 = 2_000;
pub const REBASE_MIN_MS: i64 = 5_000;
pub const REFRESH_MS: i64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivitySpec {
    pub details: String,
    pub state: String,
    pub large_image: String,
    pub large_text: String,
    pub small_image: &'static str,
    pub small_text: &'static str,
    pub timestamps: Option<(i64, i64)>,
    pub button_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desired {
    pub track: Option<TrackMeta>,
    pub play: PlayState,
    pub position_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sent {
    pub track_id: Option<u64>,
    pub play: PlayState,
    pub start_ms: i64,
    pub at_ms: i64,
    pub shown: bool,
}

impl Sent {
    pub const NEVER: Sent =
        Sent { track_id: None, play: PlayState::Stopped, start_ms: 0, at_ms: 0, shown: false };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Clear,
    Send(ActivitySpec),
}

pub fn clip(text: &str) -> String {
    if text.chars().count() <= FIELD_LIMIT {
        return text.to_string();
    }
    let mut out: String = text.chars().take(FIELD_LIMIT - 1).collect();
    out.push('…');
    out
}

pub fn track_url(track_id: u64) -> String {
    format!("https://tidal.com/track/{track_id}")
}

pub fn assemble(
    track: Option<&TrackMeta>,
    play: PlayState,
    position_ms: u64,
    now_ms: i64,
) -> Option<ActivitySpec> {
    let track = track?;
    if play == PlayState::Stopped {
        return None;
    }
    let artist_empty = track.artist.trim().is_empty();
    let details =
        if track.title.trim().is_empty() { "Unknown track".to_string() } else { clip(&track.title) };
    let artist =
        if artist_empty { "Unknown artist".to_string() } else { clip(&track.artist) };
    let state = if play == PlayState::Paused {
        if artist_empty { "Paused".to_string() } else { clip(&format!("Paused — {artist}")) }
    } else {
        artist
    };
    let (large_image, large_text) =
        match track.cover.as_deref().and_then(|cover| cover_url(cover, COVER_SIZE)) {
            Some(url) => (
                url,
                if track.album.trim().is_empty() {
                    "OxiTide".to_string()
                } else {
                    clip(&track.album)
                },
            ),
            None => ("logo".to_string(), "OxiTide".to_string()),
        };
    let timestamps = if play == PlayState::Playing && track.duration_ms > 0 {
        let position = position_ms.min(track.duration_ms);
        let start = now_ms - position as i64;
        Some((start, start + track.duration_ms as i64))
    } else {
        None
    };
    Some(ActivitySpec {
        details,
        state,
        large_image,
        large_text,
        small_image: "logo",
        small_text: "OxiTide",
        timestamps,
        button_url: track_url(track.track_id),
    })
}

pub fn should_rebase(last_start_ms: i64, last_sent_at_ms: i64, now_ms: i64, position_ms: u64) -> bool {
    let position_at_send = last_sent_at_ms - last_start_ms;
    let expected = position_at_send + (now_ms - last_sent_at_ms);
    let drift = (position_ms as i64 - expected).unsigned_abs();
    drift > SEEK_DRIFT_MS
}

pub fn plan_action(desired: &Desired, sent: &Sent, now_ms: i64) -> Action {
    let Some(spec) = assemble(desired.track.as_ref(), desired.play, desired.position_ms, now_ms)
    else {
        return if sent.shown { Action::Clear } else { Action::None };
    };
    let track_changed = sent.track_id != desired.track.as_ref().map(|track| track.track_id);
    let play_changed = sent.play != desired.play;
    let refresh_due = now_ms - sent.at_ms >= REFRESH_MS;
    let rebase_due = desired.play == PlayState::Playing
        && sent.shown
        && now_ms - sent.at_ms >= REBASE_MIN_MS
        && should_rebase(sent.start_ms, sent.at_ms, now_ms, desired.position_ms);
    if !sent.shown || track_changed || play_changed || refresh_due || rebase_due {
        Action::Send(spec)
    } else {
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000;

    fn meta(track_id: u64, cover: Option<&str>) -> TrackMeta {
        TrackMeta {
            track_id,
            title: format!("Title {track_id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: 200_000,
            cover: cover.map(str::to_string),
        }
    }

    fn desired(track: Option<TrackMeta>, play: PlayState, position_ms: u64) -> Desired {
        Desired { track, play, position_ms }
    }

    fn shown(track_id: u64, play: PlayState, start_ms: i64, at_ms: i64) -> Sent {
        Sent { track_id: Some(track_id), play, start_ms, at_ms, shown: true }
    }

    #[test]
    fn playing_maps_all_fields() {
        let spec = assemble(
            Some(&meta(7, Some("2fa8b7a6-9d5b-4a1e-9f0a-4a1e9f0a4a1e"))),
            PlayState::Playing,
            30_000,
            NOW,
        )
        .unwrap();
        assert_eq!(spec.details, "Title 7");
        assert_eq!(spec.state, "Artist");
        assert_eq!(
            spec.large_image,
            "https://resources.tidal.com/images/2fa8b7a6/9d5b/4a1e/9f0a/4a1e9f0a4a1e/640x640.jpg"
        );
        assert_eq!(spec.large_text, "Album");
        assert_eq!(spec.small_image, "logo");
        assert_eq!(spec.small_text, "OxiTide");
        assert_eq!(spec.timestamps, Some((NOW - 30_000, NOW - 30_000 + 200_000)));
        assert_eq!(spec.button_url, "https://tidal.com/track/7");
    }

    #[test]
    fn paused_prefixes_state_and_drops_timestamps() {
        let spec = assemble(Some(&meta(7, None)), PlayState::Paused, 30_000, NOW).unwrap();
        assert_eq!(spec.state, "Paused — Artist");
        assert_eq!(spec.timestamps, None);
        assert_eq!(spec.large_image, "logo");
        assert_eq!(spec.large_text, "OxiTide");
    }

    #[test]
    fn stopped_or_missing_track_is_none() {
        assert_eq!(assemble(None, PlayState::Stopped, 0, NOW), None);
        assert_eq!(assemble(None, PlayState::Playing, 0, NOW), None);
        assert_eq!(assemble(Some(&meta(7, None)), PlayState::Stopped, 0, NOW), None);
    }

    #[test]
    fn zero_duration_omits_timestamps() {
        let mut track = meta(7, None);
        track.duration_ms = 0;
        let spec = assemble(Some(&track), PlayState::Playing, 0, NOW).unwrap();
        assert_eq!(spec.timestamps, None);
    }

    #[test]
    fn position_past_duration_is_clamped() {
        let spec = assemble(Some(&meta(7, None)), PlayState::Playing, 250_000, NOW).unwrap();
        assert_eq!(spec.timestamps, Some((NOW - 200_000, NOW)));
    }

    #[test]
    fn empty_metadata_falls_back() {
        let mut track = meta(7, None);
        track.title = String::new();
        track.artist = String::new();
        track.album = String::new();
        let spec = assemble(Some(&track), PlayState::Playing, 0, NOW).unwrap();
        assert_eq!(spec.details, "Unknown track");
        assert_eq!(spec.state, "Unknown artist");
        assert_eq!(spec.large_text, "OxiTide");
        let paused = assemble(Some(&track), PlayState::Paused, 0, NOW).unwrap();
        assert_eq!(paused.state, "Paused");
    }

    #[test]
    fn clip_keeps_128_and_truncates_with_ellipsis() {
        let at_limit = "a".repeat(128);
        assert_eq!(clip(&at_limit), at_limit);
        let clipped = clip(&"a".repeat(129));
        assert_eq!(clipped.chars().count(), 128);
        assert!(clipped.ends_with('…'));
        assert_eq!(clip(&"é".repeat(200)).chars().count(), 128);
    }

    #[test]
    fn track_url_shape() {
        assert_eq!(track_url(42), "https://tidal.com/track/42");
    }

    #[test]
    fn should_rebase_threshold() {
        let start = NOW - 30_000;
        assert!(!should_rebase(start, NOW, NOW, 30_000));
        assert!(!should_rebase(start, NOW, NOW, 32_000));
        assert!(should_rebase(start, NOW, NOW, 32_001));
        assert!(should_rebase(start, NOW, NOW, 27_000));
    }

    #[test]
    fn plan_action_shows_first_and_suppresses_repeats() {
        let d = desired(Some(meta(7, None)), PlayState::Playing, 0);
        assert!(matches!(plan_action(&d, &Sent::NEVER, NOW), Action::Send(_)));
        let s = shown(7, PlayState::Playing, NOW, NOW);
        assert_eq!(plan_action(&d, &s, NOW + 1_000), Action::None);
    }

    #[test]
    fn plan_action_clears_when_idle() {
        let none = desired(None, PlayState::Stopped, 0);
        assert_eq!(plan_action(&none, &Sent::NEVER, NOW), Action::None);
        let s = shown(7, PlayState::Playing, NOW, NOW);
        assert_eq!(plan_action(&none, &s, NOW), Action::Clear);
    }

    #[test]
    fn plan_action_sends_on_track_and_play_changes() {
        let s = shown(7, PlayState::Playing, NOW, NOW);
        let next_track = desired(Some(meta(8, None)), PlayState::Playing, 0);
        assert!(matches!(plan_action(&next_track, &s, NOW + 1_000), Action::Send(_)));
        let paused = desired(Some(meta(7, None)), PlayState::Paused, 0);
        assert!(matches!(plan_action(&paused, &s, NOW + 1_000), Action::Send(_)));
    }

    #[test]
    fn plan_action_refreshes_after_thirty_seconds() {
        let d = desired(Some(meta(7, None)), PlayState::Playing, 60_999);
        let s = shown(7, PlayState::Playing, NOW - 31_000, NOW);
        assert_eq!(plan_action(&d, &s, NOW + 29_999), Action::None);
        assert!(matches!(plan_action(&d, &s, NOW + 30_000), Action::Send(_)));
    }

    #[test]
    fn plan_action_rebases_a_seek_but_not_faster_than_five_seconds() {
        let s = shown(7, PlayState::Playing, NOW - 30_000, NOW);
        let seeked = desired(Some(meta(7, None)), PlayState::Playing, 90_000);
        assert_eq!(plan_action(&seeked, &s, NOW + 1_000), Action::None);
        assert!(matches!(plan_action(&seeked, &s, NOW + 5_000), Action::Send(_)));
    }

    #[test]
    fn plan_action_never_rebases_while_paused() {
        let s = shown(7, PlayState::Paused, NOW, NOW);
        let d = desired(Some(meta(7, None)), PlayState::Paused, 90_000);
        assert_eq!(plan_action(&d, &s, NOW + 6_000), Action::None);
    }
}
