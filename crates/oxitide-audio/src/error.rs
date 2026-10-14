use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("tidal error: {0}")]
    Tidal(#[from] oxitide_tidal::Error),
    #[error("manifest error: {0}")]
    Manifest(String),
    #[error("mpd parse error: {0}")]
    Mpd(String),
    #[error("fetch error: {0}")]
    Fetch(String),
    /// The CDN refused a media URL (a 4xx other than 429, typically an
    /// expired signed URL). Retrying the same URL is pointless; a fresh
    /// resolve is not.
    #[error("fetch error: {0}")]
    FetchRejected(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("{0}")]
    Output(String),
    #[error("track unavailable at {0}; enable fallback or lower the streaming quality")]
    Unavailable(String),
    #[error("unsupported audio: {0}")]
    Unsupported(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Whether this failure means the track itself cannot be played and
    /// playback should skip to the next queue item. Transient fetch/decode
    /// failures, auth expiry, and quality-policy errors are not skippable.
    pub fn is_track_unavailable(&self) -> bool {
        matches!(
            self,
            Error::Tidal(oxitide_tidal::Error::Forbidden)
                | Error::Tidal(oxitide_tidal::Error::NotFound(_))
                | Error::Unsupported(_)
        )
    }

    /// Whether starting the track again could succeed: a network or TIDAL
    /// hiccup while resolving, or a failed segment fetch (a fresh resolve
    /// also replaces signed CDN URLs that may have expired while the track
    /// waited as the prefetch).
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Tidal(e) => e.is_transient(),
            Error::Fetch(_) | Error::FetchRejected(_) => true,
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_error_has_stable_message() {
        assert_eq!(Error::Manifest("bad".into()).to_string(), "manifest error: bad");
    }

    #[test]
    fn tap_error_includes_message() {
        assert_eq!(Error::Decode("boom".into()).to_string(), "decode error: boom");
    }

    #[test]
    fn output_error_has_stable_message() {
        assert_eq!(
            Error::Output("Audio device has been disconnected or stalled".into()).to_string(),
            "Audio device has been disconnected or stalled"
        );
    }

    #[test]
    fn unsupported_error_has_stable_message() {
        assert_eq!(
            Error::Unsupported("he-aac".into()).to_string(),
            "unsupported audio: he-aac"
        );
    }

    #[test]
    fn unavailable_error_has_stable_message() {
        assert_eq!(
            Error::Unavailable("lossless".into()).to_string(),
            "track unavailable at lossless; enable fallback or lower the streaming quality"
        );
    }

    #[test]
    fn forbidden_and_not_found_are_track_unavailable() {
        assert!(Error::Tidal(oxitide_tidal::Error::Forbidden).is_track_unavailable());
        assert!(Error::Tidal(oxitide_tidal::Error::NotFound("gone".into())).is_track_unavailable());
    }

    #[test]
    fn unsupported_codec_is_track_unavailable() {
        assert!(Error::Unsupported("he-aac".into()).is_track_unavailable());
    }

    #[test]
    fn transient_and_policy_errors_are_not_track_unavailable() {
        assert!(
            !Error::Tidal(oxitide_tidal::Error::Unauthorized).is_track_unavailable(),
            "auth must stop, not skip"
        );
        assert!(!Error::Fetch("network down".into()).is_track_unavailable());
        assert!(!Error::Decode("boom".into()).is_track_unavailable());
        assert!(!Error::Manifest("bad".into()).is_track_unavailable());
        assert!(!Error::Unavailable("low".into()).is_track_unavailable());
    }

    #[test]
    fn network_failures_are_transient() {
        assert!(Error::Fetch("reset".into()).is_transient());
        assert!(Error::FetchRejected("status 403".into()).is_transient());
        assert!(Error::Tidal(oxitide_tidal::Error::RateLimited).is_transient());
    }

    #[test]
    fn auth_policy_and_decode_failures_are_not_transient() {
        assert!(!Error::Tidal(oxitide_tidal::Error::Unauthorized).is_transient());
        assert!(!Error::Tidal(oxitide_tidal::Error::Forbidden).is_transient());
        assert!(!Error::Decode("boom".into()).is_transient());
        assert!(!Error::Manifest("bad".into()).is_transient());
        assert!(!Error::Unavailable("low".into()).is_transient());
    }
}
