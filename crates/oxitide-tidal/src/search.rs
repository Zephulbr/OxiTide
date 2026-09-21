use crate::client::TidalClient;
use crate::error::Result;
use crate::models::SearchResults;

impl TidalClient {
    pub async fn search(
        &self,
        query: &str,
        types: &[&str],
        limit: u32,
        offset: u32,
    ) -> Result<SearchResults> {
        let params = vec![
            ("query", query.to_string()),
            ("types", types.join(",")),
            ("limit", limit.to_string()),
            ("offset", offset.to_string()),
        ];
        self.get("search", &params).await
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
            reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn search_parses_grouped_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/search"))
            .and(query_param("query", "beck"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": [{"id": 1, "title": "Loser", "artists": []}]},
                "albums": {"items": [{"id": 2, "title": "Sea Change"}]},
                "artists": {"items": [{"id": 3, "name": "Beck"}]},
                "playlists": {"items": [{"uuid": "u", "title": "Mix"}]}
            })))
            .mount(&server).await;
        let r = client(&server).search("beck", &["tracks", "albums", "artists", "playlists"], 20, 0)
            .await.unwrap();
        assert_eq!(r.track_items()[0].title, "Loser");
        assert_eq!(r.album_items()[0].title, "Sea Change");
        assert_eq!(r.artist_items()[0].name, "Beck");
        assert_eq!(r.playlist_items()[0].uuid, "u");
    }

    #[tokio::test]
    async fn search_with_no_results_is_empty_not_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": []}, "albums": {"items": []},
                "artists": {"items": []}, "playlists": {"items": []}
            })))
            .mount(&server).await;
        let r = client(&server).search("zzzz", &["tracks"], 20, 0).await.unwrap();
        assert!(r.track_items().is_empty() && r.album_items().is_empty());
    }
}
