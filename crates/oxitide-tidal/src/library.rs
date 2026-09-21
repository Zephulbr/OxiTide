use crate::client::TidalClient;
use crate::error::Result;
use crate::models::{Album, Artist, ItemPage, Playlist, Track};

fn page(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl TidalClient {
    fn user_base(&self) -> String {
        format!("users/{}/favorites", self.session().user_id)
    }

    pub async fn favorite_tracks(&self, limit: u32, offset: u32) -> Result<Vec<Track>> {
        let p: ItemPage<Track> = self
            .get(&format!("{}/tracks", self.user_base()), &page(limit, offset))
            .await?;
        Ok(p.items)
    }

    pub async fn favorite_albums(&self, limit: u32, offset: u32) -> Result<Vec<Album>> {
        let p: ItemPage<Album> = self
            .get(&format!("{}/albums", self.user_base()), &page(limit, offset))
            .await?;
        Ok(p.items)
    }

    pub async fn favorite_artists(&self, limit: u32, offset: u32) -> Result<Vec<Artist>> {
        let p: ItemPage<Artist> = self
            .get(&format!("{}/artists", self.user_base()), &page(limit, offset))
            .await?;
        Ok(p.items)
    }

    pub async fn user_playlists(&self, limit: u32, offset: u32) -> Result<Vec<Playlist>> {
        let p: ItemPage<Playlist> = self
            .get(
                &format!("users/{}/playlists", self.session().user_id),
                &page(limit, offset),
            )
            .await?;
        Ok(p.items)
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
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 42 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn favorite_tracks_pages_by_offset() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("offset", "200")).and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 7, "title": "Fav"}], "totalNumberOfItems": 201, "limit": 100, "offset": 200
            })))
            .mount(&server).await;
        let tracks = client(&server).favorite_tracks(100, 200).await.unwrap();
        assert_eq!(tracks[0].id, 7);
    }

    #[tokio::test]
    async fn user_playlists_handles_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/playlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "totalNumberOfItems": 0, "limit": 50, "offset": 0
            })))
            .mount(&server).await;
        assert!(client(&server).user_playlists(50, 0).await.unwrap().is_empty());
    }
}
