use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{Album, Artist, ItemPage, Track};

fn page_params(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl TidalClient {
    pub async fn track(&self, id: u64) -> Result<Track> {
        self.get(&format!("tracks/{id}"), &[]).await
    }

    pub async fn album(&self, id: u64) -> Result<Album> {
        self.get(&format!("albums/{id}"), &[]).await
    }

    pub async fn album_tracks(&self, id: u64, limit: u32, offset: u32) -> Result<Vec<Track>> {
        let page: ItemPage<Track> =
            self.get(&format!("albums/{id}/tracks"), &page_params(limit, offset)).await?;
        Ok(page.items)
    }

    /// Fetch every track of an album, walking all pages.
    pub async fn album_tracks_all(&self, id: u64) -> Result<Vec<Track>> {
        self.get_all_pages(&format!("albums/{id}/tracks"), &[], DEFAULT_PAGE_SIZE)
            .await
    }

    pub async fn artist(&self, id: u64) -> Result<Artist> {
        self.get(&format!("artists/{id}"), &[]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::error::Error;
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

    #[tokio::test]
    async fn album_tracks_sends_limit_and_offset() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/albums/5/tracks"))
            .and(query_param("limit", "50")).and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 1, "title": "T1", "artists": []}],
                "totalNumberOfItems": 1, "limit": 50, "offset": 100
            })))
            .mount(&server).await;
        let tracks = client(&server).album_tracks(5, 50, 100).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "T1");
    }

    #[tokio::test]
    async fn album_tracks_all_fetches_every_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/albums/9/tracks"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": (1..=100)
                    .map(|id| serde_json::json!({"id": id, "title": "T"}))
                    .collect::<Vec<_>>(),
                "totalNumberOfItems": 101, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/albums/9/tracks"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 101, "title": "Last"}],
                "totalNumberOfItems": 101, "limit": 100, "offset": 100
            })))
            .mount(&server).await;
        let tracks = client(&server).album_tracks_all(9).await.unwrap();
        assert_eq!(tracks.len(), 101);
        assert_eq!(tracks[100].title, "Last");
    }

    #[tokio::test]
    async fn album_tracks_handles_empty_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/albums/5/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "totalNumberOfItems": 0, "limit": 50, "offset": 0
            })))
            .mount(&server).await;
        assert!(client(&server).album_tracks(5, 50, 0).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn track_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/999"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        assert!(matches!(client(&server).track(999).await.unwrap_err(), Error::NotFound(_)));
    }
}
