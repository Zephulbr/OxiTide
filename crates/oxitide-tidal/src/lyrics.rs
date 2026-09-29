//! Legacy (v1) track lyrics — synced `subtitles` and plain `lyrics` text.
//! The official JSON:API lyrics resource is INTERNAL-tier, so the legacy
//! endpoint is the TIDAL source (Track 4a spec §5.1, §7).

use serde::Deserialize;

use crate::client::TidalClient;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TidalLyrics {
    #[serde(default, rename = "trackId")]
    pub track_id: u64,
    /// Plain lyrics, newline separated.
    #[serde(default)]
    pub lyrics: String,
    /// Synced LRC lyrics; empty when the track has only plain text.
    #[serde(default)]
    pub subtitles: String,
    #[serde(default, rename = "lyricsProvider")]
    pub provider: String,
    #[serde(default, rename = "isRightToLeft")]
    pub right_to_left: bool,
}

impl TidalClient {
    /// Fetch a track's lyrics. A 404 means "no lyrics for this track", not an
    /// error; everything else propagates.
    pub async fn track_lyrics(&self, track_id: u64) -> Result<Option<TidalLyrics>> {
        let path = format!("tracks/{track_id}/lyrics");
        match self.get::<TidalLyrics>(&path, &[]).await {
            Ok(lyrics) => Ok(Some(lyrics)),
            Err(Error::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
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

    #[tokio::test]
    async fn track_lyrics_parses_the_legacy_payload() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .and(query_param("countryCode", "GR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7,
                "lyrics": "line one\nline two",
                "subtitles": "[00:01.00] line one\n[00:02.50] line two",
                "lyricsProvider": "TIDAL",
                "isRightToLeft": false
            })))
            .mount(&server)
            .await;
        let lyrics = client(&server).track_lyrics(7).await.unwrap().unwrap();
        assert_eq!(lyrics.track_id, 7);
        assert_eq!(lyrics.lyrics, "line one\nline two");
        assert!(lyrics.subtitles.contains("[00:01.00]"));
        assert_eq!(lyrics.provider, "TIDAL");
        assert!(!lyrics.right_to_left);
    }

    #[tokio::test]
    async fn track_lyrics_maps_404_to_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/8/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert_eq!(client(&server).track_lyrics(8).await.unwrap(), None);
    }

    #[tokio::test]
    async fn track_lyrics_propagates_other_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/9/lyrics"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
        let err = client(&server).track_lyrics(9).await.unwrap_err();
        assert!(matches!(err, Error::Api { status: 500, .. }), "got {err:?}");
    }
}
