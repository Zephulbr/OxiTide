use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Low,
    High,
    Lossless,
    Max,
}

impl Quality {
    pub fn tidal_param(self) -> &'static str {
        match self {
            Quality::Low => "LOW",
            Quality::High => "HIGH",
            Quality::Lossless => "LOSSLESS",
            Quality::Max => "HI_RES_LOSSLESS",
        }
    }

    /// Lowercase name used in cache file names and user-facing errors.
    pub fn label(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::High => "high",
            Quality::Lossless => "lossless",
            Quality::Max => "max",
        }
    }

    /// Parse the `audioQuality` value returned by `playbackinfo`.
    pub fn from_tidal_param(value: &str) -> Option<Quality> {
        match value.to_ascii_uppercase().as_str() {
            "LOW" => Some(Quality::Low),
            "HIGH" => Some(Quality::High),
            "LOSSLESS" => Some(Quality::Lossless),
            "HI_RES_LOSSLESS" | "HI_RES" | "MASTER" => Some(Quality::Max),
            _ => None,
        }
    }
}

/// Loudness metadata from `playbackinfo`: gains in dB, peaks as linear sample
/// amplitude (1.0 = full scale). Each field is absent when TIDAL omits it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplayGain {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// The decoded `playbackinfo` response. `manifest` is raw bytes: XML for DASH,
/// JSON for BTS.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackInfo {
    /// The quality that was requested.
    pub quality: Quality,
    /// The quality the API says it served, when reported.
    pub actual_quality: Option<Quality>,
    pub manifest_mime: String,
    pub manifest: Vec<u8>,
    pub replay_gain: ReplayGain,
}

#[derive(Deserialize)]
struct RawPlaybackInfo {
    #[serde(default, rename = "manifestMimeType")]
    manifest_mime_type: String,
    #[serde(default)]
    manifest: String,
    #[serde(default, rename = "audioQuality")]
    audio_quality: Option<String>,
    #[serde(default, rename = "trackReplayGain")]
    track_replay_gain: Option<f32>,
    #[serde(default, rename = "trackPeakAmplitude")]
    track_peak_amplitude: Option<f32>,
    #[serde(default, rename = "albumReplayGain")]
    album_replay_gain: Option<f32>,
    #[serde(default, rename = "albumPeakAmplitude")]
    album_peak_amplitude: Option<f32>,
}

impl TidalClient {
    pub async fn playback_info(&self, track_id: u64, quality: Quality) -> Result<PlaybackInfo> {
        let raw: RawPlaybackInfo = self
            .get(
                &format!("tracks/{track_id}/playbackinfo"),
                &[
                    ("audioquality", quality.tidal_param().to_string()),
                    ("playbackmode", "STREAM".to_string()),
                    ("assetpresentation", "FULL".to_string()),
                ],
            )
            .await?;
        let manifest = base64::engine::general_purpose::STANDARD
            .decode(raw.manifest.as_bytes())
            .map_err(|e| crate::error::Error::Auth(format!("bad manifest base64: {e}")))?;
        Ok(PlaybackInfo {
            quality,
            actual_quality: raw
                .audio_quality
                .as_deref()
                .and_then(Quality::from_tidal_param),
            manifest_mime: raw.manifest_mime_type,
            manifest,
            replay_gain: ReplayGain {
                track_gain_db: raw.track_replay_gain,
                track_peak: raw.track_peak_amplitude,
                album_gain_db: raw.album_replay_gain,
                album_peak: raw.album_peak_amplitude,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo {
                session_id: "s".into(),
                country_code: "GR".into(),
                user_id: 1,
            },
            TokenSet {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                token_type: "Bearer".into(),
                expires_at: None,
            },
            Arc::new(MemoryStore::default()),
        )
    }

    #[test]
    fn quality_maps_to_tidal_params() {
        assert_eq!(Quality::Low.tidal_param(), "LOW");
        assert_eq!(Quality::Max.tidal_param(), "HI_RES_LOSSLESS");
    }

    #[test]
    fn quality_label_is_lowercase() {
        assert_eq!(Quality::Low.label(), "low");
        assert_eq!(Quality::High.label(), "high");
        assert_eq!(Quality::Lossless.label(), "lossless");
        assert_eq!(Quality::Max.label(), "max");
    }

    #[test]
    fn quality_orders_from_low_to_max() {
        assert!(Quality::Low < Quality::High);
        assert!(Quality::High < Quality::Lossless);
        assert!(Quality::Lossless < Quality::Max);
    }

    #[test]
    fn from_tidal_param_parses_known_values() {
        assert_eq!(Quality::from_tidal_param("LOW"), Some(Quality::Low));
        assert_eq!(Quality::from_tidal_param("HIGH"), Some(Quality::High));
        assert_eq!(
            Quality::from_tidal_param("LOSSLESS"),
            Some(Quality::Lossless)
        );
        assert_eq!(
            Quality::from_tidal_param("HI_RES_LOSSLESS"),
            Some(Quality::Max)
        );
        assert_eq!(Quality::from_tidal_param("hi_res"), Some(Quality::Max));
        assert_eq!(Quality::from_tidal_param("MASTER"), Some(Quality::Max));
        assert_eq!(Quality::from_tidal_param("something_else"), None);
    }

    #[tokio::test]
    async fn playback_info_parses_audio_quality() {
        let server = MockServer::start().await;
        let mpd = "<MPD></MPD>";
        let encoded = base64::engine::general_purpose::STANDARD.encode(mpd);
        Mock::given(method("GET"))
            .and(path("/tracks/7/playbackinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifestMimeType": "application/dash+xml",
                "manifest": encoded,
                "audioQuality": "LOSSLESS"
            })))
            .mount(&server)
            .await;
        let info = client(&server)
            .playback_info(7, Quality::Max)
            .await
            .unwrap();
        assert_eq!(info.actual_quality, Some(Quality::Lossless));
    }

    #[tokio::test]
    async fn playback_info_parses_replay_gain() {
        let server = MockServer::start().await;
        let encoded = base64::engine::general_purpose::STANDARD.encode("<MPD></MPD>");
        Mock::given(method("GET"))
            .and(path("/tracks/9/playbackinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifestMimeType": "application/dash+xml",
                "manifest": encoded,
                "trackReplayGain": -9.5,
                "trackPeakAmplitude": 0.98,
                "albumReplayGain": -8.25,
                "albumPeakAmplitude": 1.0
            })))
            .mount(&server)
            .await;
        let info = client(&server)
            .playback_info(9, Quality::Lossless)
            .await
            .unwrap();
        assert_eq!(
            info.replay_gain,
            ReplayGain {
                track_gain_db: Some(-9.5),
                track_peak: Some(0.98),
                album_gain_db: Some(-8.25),
                album_peak: Some(1.0),
            }
        );
    }

    #[tokio::test]
    async fn playback_info_without_audio_quality_is_none() {
        let server = MockServer::start().await;
        let encoded = base64::engine::general_purpose::STANDARD.encode("<MPD></MPD>");
        Mock::given(method("GET"))
            .and(path("/tracks/8/playbackinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifestMimeType": "application/dash+xml",
                "manifest": encoded
            })))
            .mount(&server)
            .await;
        let info = client(&server)
            .playback_info(8, Quality::Lossless)
            .await
            .unwrap();
        assert_eq!(info.actual_quality, None);
        assert_eq!(info.replay_gain, ReplayGain::default());
    }

    #[tokio::test]
    async fn playback_info_requests_quality_and_decodes_base64() {
        let server = MockServer::start().await;
        let mpd = "<MPD></MPD>";
        let encoded = base64::engine::general_purpose::STANDARD.encode(mpd);
        Mock::given(method("GET"))
            .and(path("/tracks/77646170/playbackinfo"))
            .and(query_param("audioquality", "HI_RES_LOSSLESS"))
            .and(query_param("playbackmode", "STREAM"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifestMimeType": "application/dash+xml",
                "manifest": encoded
            })))
            .mount(&server)
            .await;
        let info = client(&server)
            .playback_info(77646170, Quality::Max)
            .await
            .unwrap();
        assert_eq!(info.manifest_mime, "application/dash+xml");
        assert_eq!(info.manifest, mpd.as_bytes());
    }

    #[tokio::test]
    async fn playback_info_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1/playbackinfo"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(client(&server)
            .playback_info(1, Quality::Lossless)
            .await
            .is_err());
    }
}
