use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// The decoded `playbackinfo` response. `manifest` is raw bytes: XML for DASH,
/// JSON for BTS.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackInfo {
    pub quality: Quality,
    pub manifest_mime: String,
    pub manifest: Vec<u8>,
}

#[derive(Deserialize)]
struct RawPlaybackInfo {
    #[serde(default, rename = "manifestMimeType")]
    manifest_mime_type: String,
    #[serde(default)]
    manifest: String,
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
            manifest_mime: raw.manifest_mime_type,
            manifest,
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
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[test]
    fn quality_maps_to_tidal_params() {
        assert_eq!(Quality::Low.tidal_param(), "LOW");
        assert_eq!(Quality::Max.tidal_param(), "HI_RES_LOSSLESS");
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
        let info = client(&server).playback_info(77646170, Quality::Max).await.unwrap();
        assert_eq!(info.manifest_mime, "application/dash+xml");
        assert_eq!(info.manifest, mpd.as_bytes());
    }

    #[tokio::test]
    async fn playback_info_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1/playbackinfo"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        assert!(client(&server).playback_info(1, Quality::Lossless).await.is_err());
    }
}
