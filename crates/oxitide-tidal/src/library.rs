use serde::Deserialize;

use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{Album, Artist, ItemPage, Playlist, Track};

/// Favorites endpoints wrap each entry as `{ "created": ..., "item": {...} }`.
#[derive(Debug, Deserialize)]
struct Favorited<T> {
    item: T,
}

fn page(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl TidalClient {
    fn user_base(&self) -> String {
        format!("users/{}/favorites", self.session().user_id)
    }

    /// One page of favorite tracks plus the server-reported total.
    pub async fn favorite_tracks_page(&self, limit: u32, offset: u32) -> Result<(Vec<Track>, u32)> {
        let p: ItemPage<Favorited<Track>> = self
            .get(&format!("{}/tracks", self.user_base()), &page(limit, offset))
            .await?;
        Ok((p.items.into_iter().map(|f| f.item).collect(), p.total_number_of_items))
    }

    pub async fn favorite_albums(&self, limit: u32, offset: u32) -> Result<Vec<Album>> {
        let p: ItemPage<Favorited<Album>> = self
            .get(&format!("{}/albums", self.user_base()), &page(limit, offset))
            .await?;
        Ok(p.items.into_iter().map(|f| f.item).collect())
    }

    pub async fn favorite_artists(&self, limit: u32, offset: u32) -> Result<Vec<Artist>> {
        let p: ItemPage<Favorited<Artist>> = self
            .get(&format!("{}/artists", self.user_base()), &page(limit, offset))
            .await?;
        Ok(p.items.into_iter().map(|f| f.item).collect())
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

    /// Walk every page of the user's favorite tracks.
    pub async fn all_favorite_tracks(&self) -> Result<Vec<Track>> {
        let favs: Vec<Favorited<Track>> = self
            .get_all_pages(&format!("{}/tracks", self.user_base()), &[], DEFAULT_PAGE_SIZE)
            .await?;
        Ok(favs.into_iter().map(|f| f.item).collect())
    }

    /// Walk every page of the user's favorite albums.
    pub async fn all_favorite_albums(&self) -> Result<Vec<Album>> {
        let favs: Vec<Favorited<Album>> = self
            .get_all_pages(&format!("{}/albums", self.user_base()), &[], DEFAULT_PAGE_SIZE)
            .await?;
        Ok(favs.into_iter().map(|f| f.item).collect())
    }

    /// Walk every page of the user's favorite artists.
    pub async fn all_favorite_artists(&self) -> Result<Vec<Artist>> {
        let favs: Vec<Favorited<Artist>> = self
            .get_all_pages(&format!("{}/artists", self.user_base()), &[], DEFAULT_PAGE_SIZE)
            .await?;
        Ok(favs.into_iter().map(|f| f.item).collect())
    }

    /// Walk every page of the user's playlists.
    pub async fn all_user_playlists(&self) -> Result<Vec<Playlist>> {
        self.get_all_pages(
            &format!("users/{}/playlists", self.session().user_id),
            &[],
            DEFAULT_PAGE_SIZE,
        )
        .await
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
    async fn favorite_tracks_page_returns_items_and_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("offset", "200")).and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"created": "2025-01-01T00:00:00.000+0000", "item": {"id": 7, "title": "Fav"}}],
                "totalNumberOfItems": 201, "limit": 100, "offset": 200
            })))
            .mount(&server).await;
        let (tracks, total) = client(&server).favorite_tracks_page(100, 200).await.unwrap();
        assert_eq!(tracks[0].id, 7);
        assert_eq!(total, 201);
    }

    #[tokio::test]
    async fn all_favorite_tracks_fetches_every_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 1, "title": "A"}}],
                "totalNumberOfItems": 2, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("offset", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 2, "title": "B"}}],
                "totalNumberOfItems": 2, "limit": 100, "offset": 1
            })))
            .mount(&server).await;
        let tracks = client(&server).all_favorite_tracks().await.unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[1].id, 2);
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
