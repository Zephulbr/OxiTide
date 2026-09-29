use serde_json::Value;

use crate::client::TidalClient;
use crate::error::Result;
use crate::models::{Artist, Track};

/// Radio responses are fixed-size; 50 matches the official clients' first page.
pub const RADIO_LIMIT: u32 = 50;
/// The artist page renders a single row of similar artists.
pub const SIMILAR_ARTISTS_LIMIT: u32 = 20;

impl TidalClient {
    /// Audio-only radio seeded on a track (`tracks/{id}/radio`).
    pub async fn track_radio(&self, id: u64) -> Result<Vec<Track>> {
        let params = [("limit", RADIO_LIMIT.to_string())];
        let raw: Value = self.get(&format!("tracks/{id}/radio"), &params).await?;
        Ok(radio_tracks_from_value(&raw))
    }

    /// Audio-only radio seeded on an artist (`artists/{id}/radio`).
    pub async fn artist_radio(&self, id: u64) -> Result<Vec<Track>> {
        let params = [("limit", RADIO_LIMIT.to_string())];
        let raw: Value = self.get(&format!("artists/{id}/radio"), &params).await?;
        Ok(radio_tracks_from_value(&raw))
    }

    /// Artists similar to `id` (`artists/{id}/similar`).
    pub async fn similar_artists(&self, id: u64) -> Result<Vec<Artist>> {
        let params = [("limit", SIMILAR_ARTISTS_LIMIT.to_string())];
        let raw: Value = self.get(&format!("artists/{id}/similar"), &params).await?;
        Ok(similar_artists_from_value(&raw))
    }
}

/// v1 collection endpoints return `{"items": […]}`; a bare array is accepted
/// too because the live envelope was never captured.
fn items_from_value(raw: &Value) -> &[Value] {
    raw.as_array()
        .or_else(|| raw.get("items").and_then(Value::as_array))
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// Drops videos (the engine is audio-only) and entries that do not parse as
/// tracks, mirroring `mix_detail_from_json`'s best-effort item mapping.
fn radio_tracks_from_value(raw: &Value) -> Vec<Track> {
    items_from_value(raw)
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) != Some("VIDEO"))
        .filter_map(|item| serde_json::from_value::<Track>(item.clone()).ok())
        .collect()
}

fn similar_artists_from_value(raw: &Value) -> Vec<Artist> {
    items_from_value(raw)
        .iter()
        .filter_map(|item| serde_json::from_value::<Artist>(item.clone()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::auth::SessionInfo;
    use crate::error::Error;
    use crate::token_store::{MemoryStore, TokenSet};
    use crate::TidalClient;
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
    async fn track_radio_sends_the_limit_and_drops_videos() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/5/radio"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {"id": 1, "title": "Song", "duration": 200, "artists": [], "type": "TRACK"},
                    {"id": 2, "title": "Clip", "duration": 60, "artists": [], "type": "VIDEO"}
                ]
            })))
            .mount(&server)
            .await;
        let tracks = client(&server).track_radio(5).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 1);
    }

    #[tokio::test]
    async fn track_radio_accepts_a_bare_array_and_skips_junk() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/5/radio"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"type": "TRACK"},
                {"id": 3, "title": "Good", "artists": []}
            ])))
            .mount(&server)
            .await;
        let tracks = client(&server).track_radio(5).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 3);
    }

    #[tokio::test]
    async fn artist_radio_reads_the_artist_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/radio"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 4, "title": "A", "artists": []}]
            })))
            .mount(&server)
            .await;
        assert_eq!(client(&server).artist_radio(9).await.unwrap()[0].id, 4);
    }

    #[tokio::test]
    async fn similar_artists_maps_id_name_and_picture() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/similar"))
            .and(query_param("limit", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 11, "name": "B", "picture": "p-11"}]
            })))
            .mount(&server)
            .await;
        let artists = client(&server).similar_artists(9).await.unwrap();
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].id, 11);
        assert_eq!(artists[0].name, "B");
        assert_eq!(artists[0].picture.as_deref(), Some("p-11"));
    }

    #[tokio::test]
    async fn radio_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/999/radio"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).track_radio(999).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }
}
