use crate::client::TidalClient;
use crate::error::Result;
use crate::models::{SearchResults, TopHit};

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
        let mut results: SearchResults = self.get("search", &params).await?;
        let user_id = self.session().user_id;
        for playlist in &mut results.playlists.items {
            playlist.mark_owned(user_id, false);
        }
        if let Some(TopHit::Playlist(playlist)) = &mut results.top_hit {
            playlist.mark_owned(user_id, false);
        }
        Ok(results)
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
    async fn search_parses_grouped_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("query", "beck"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": [{"id": 1, "title": "Loser", "artists": []}]},
                "albums": {"items": [{"id": 2, "title": "Sea Change"}]},
                "artists": {"items": [{"id": 3, "name": "Beck"}]},
                "playlists": {"items": [{"uuid": "u", "title": "Mix"}]}
            })))
            .mount(&server)
            .await;
        let r = client(&server)
            .search("beck", &["tracks", "albums", "artists", "playlists"], 20, 0)
            .await
            .unwrap();
        assert_eq!(r.track_items()[0].title, "Loser");
        assert_eq!(r.album_items()[0].title, "Sea Change");
        assert_eq!(r.artist_items()[0].name, "Beck");
        assert_eq!(r.playlist_items()[0].uuid, "u");
    }

    #[tokio::test]
    async fn search_reads_the_top_hit_and_totals() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "artists": {"items": [{"id": 3, "name": "Beck"}], "totalNumberOfItems": 41},
                "topHit": {"type": "ARTISTS", "value": {"id": 3, "name": "Beck"}}
            })))
            .mount(&server)
            .await;
        let r = client(&server).search("beck", &["artists"], 20, 0).await.unwrap();
        assert_eq!(r.artists.total_number_of_items, 41);
        match r.top_hit {
            Some(TopHit::Artist(artist)) => assert_eq!(artist.name, "Beck"),
            other => panic!("expected an artist top hit, got {other:?}"),
        }
        let json = serde_json::to_value(&client(&server).search("beck", &["artists"], 20, 0).await.unwrap()).unwrap();
        assert_eq!(json["topHit"]["kind"], "artist");
        assert_eq!(json["topHit"]["item"]["name"], "Beck");
    }

    #[tokio::test]
    async fn an_unusable_top_hit_is_ignored_not_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": [{"id": 1, "title": "Loser", "artists": []}]},
                "topHit": {"type": "VIDEOS", "value": {"id": 9, "title": "Clip"}}
            })))
            .mount(&server)
            .await;
        let r = client(&server).search("beck", &["tracks"], 20, 0).await.unwrap();
        assert!(r.top_hit.is_none());
        assert_eq!(r.track_items()[0].title, "Loser");
    }

    #[tokio::test]
    async fn search_with_no_results_is_empty_not_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": []}, "albums": {"items": []},
                "artists": {"items": []}, "playlists": {"items": []}
            })))
            .mount(&server)
            .await;
        let r = client(&server)
            .search("zzzz", &["tracks"], 20, 0)
            .await
            .unwrap();
        assert!(r.track_items().is_empty() && r.album_items().is_empty());
    }

    #[tokio::test]
    async fn search_decodes_an_editorial_playlist_without_a_creator_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": []}, "albums": {"items": []}, "artists": {"items": []},
                "playlists": {"items": [
                    {"uuid": "editorial", "title": "Blur Mix", "creator": {"name": "TIDAL"}}
                ]}
            })))
            .mount(&server)
            .await;
        let results = client(&server)
            .search("blur", &["playlists"], 20, 0)
            .await
            .unwrap();
        assert_eq!(results.playlist_items()[0].uuid, "editorial");
        assert!(!results.playlist_items()[0].owned);
    }

    #[tokio::test]
    async fn search_marks_playlist_ownership() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": []}, "albums": {"items": []},
                "artists": {"items": []},
                "playlists": {"items": [
                    {"uuid": "mine", "title": "Mine", "creator": {"id": 1}},
                    {"uuid": "theirs", "title": "Theirs", "creator": {"id": 2}},
                    {"uuid": "unknown", "title": "Unknown"}
                ]}
            })))
            .mount(&server)
            .await;
        let results = client(&server)
            .search("mix", &["playlists"], 20, 0)
            .await
            .unwrap();
        assert!(results.playlist_items()[0].owned);
        assert!(!results.playlist_items()[1].owned);
        assert!(!results.playlist_items()[2].owned);
    }
}
