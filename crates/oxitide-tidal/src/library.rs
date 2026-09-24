use serde::{Deserialize, Serialize};

use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{Album, Artist, ItemPage, Track};

/// Favorites endpoints wrap each entry as `{ "created": ..., "item": {...} }`.
#[derive(Debug, Deserialize)]
struct Favorited<T> {
    item: T,
}

/// Window used when hydrating favorite ids; the favorites endpoint is known to
/// honor 1000-row requests.
const FAVORITE_ID_PAGE_SIZE: u32 = 1000;

#[derive(Debug, Clone, Serialize)]
pub struct FavoriteIds {
    pub tracks: Vec<u64>,
    pub albums: Vec<u64>,
    pub artists: Vec<u64>,
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

    async fn set_favorite(&self, kind: &str, id_key: &str, id: u64, favorite: bool) -> Result<()> {
        if favorite {
            self.post_form(&format!("{}/{kind}", self.user_base()), &[(id_key, id.to_string())])
                .await
        } else {
            self.delete(&format!("{}/{kind}/{id}", self.user_base())).await
        }
    }

    pub async fn set_favorite_track(&self, id: u64, favorite: bool) -> Result<()> {
        self.set_favorite("tracks", "trackId", id, favorite).await
    }

    pub async fn set_favorite_album(&self, id: u64, favorite: bool) -> Result<()> {
        self.set_favorite("albums", "albumId", id, favorite).await
    }

    pub async fn set_favorite_artist(&self, id: u64, favorite: bool) -> Result<()> {
        self.set_favorite("artists", "artistId", id, favorite).await
    }

    /// Walk the three favorites lists in parallel and return only the ids.
    pub async fn favorite_ids(&self) -> Result<FavoriteIds> {
        let base = self.user_base();
        let tracks_path = format!("{base}/tracks");
        let albums_path = format!("{base}/albums");
        let artists_path = format!("{base}/artists");
        let (tracks, albums, artists) = tokio::join!(
            self.get_all_pages::<Favorited<Track>>(&tracks_path, &[], FAVORITE_ID_PAGE_SIZE),
            self.get_all_pages::<Favorited<Album>>(&albums_path, &[], FAVORITE_ID_PAGE_SIZE),
            self.get_all_pages::<Favorited<Artist>>(&artists_path, &[], FAVORITE_ID_PAGE_SIZE),
        );
        Ok(FavoriteIds {
            tracks: tracks?.into_iter().map(|f| f.item.id).collect(),
            albums: albums?.into_iter().map(|f| f.item.id).collect(),
            artists: artists?.into_iter().map(|f| f.item.id).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{body_string_contains, method, path, query_param};
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
    async fn set_favorite_track_adds_and_removes() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/42/favorites/tracks"))
            .and(body_string_contains("trackId=7"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("DELETE")).and(path("/users/42/favorites/tracks/7"))
            .respond_with(ResponseTemplate::new(204)).mount(&server).await;
        let client = client(&server);
        client.set_favorite_track(7, true).await.unwrap();
        client.set_favorite_track(7, false).await.unwrap();
    }

    #[tokio::test]
    async fn set_favorite_album_adds_and_removes() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/42/favorites/albums"))
            .and(body_string_contains("albumId=9"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("DELETE")).and(path("/users/42/favorites/albums/9"))
            .respond_with(ResponseTemplate::new(204)).mount(&server).await;
        let client = client(&server);
        client.set_favorite_album(9, true).await.unwrap();
        client.set_favorite_album(9, false).await.unwrap();
    }

    #[tokio::test]
    async fn set_favorite_artist_adds_and_removes() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/42/favorites/artists"))
            .and(body_string_contains("artistId=11"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("DELETE")).and(path("/users/42/favorites/artists/11"))
            .respond_with(ResponseTemplate::new(204)).mount(&server).await;
        let client = client(&server);
        client.set_favorite_artist(11, true).await.unwrap();
        client.set_favorite_artist(11, false).await.unwrap();
    }

    #[tokio::test]
    async fn favorite_ids_walks_all_three_lists() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("limit", "1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 1, "title": "A"}}, {"item": {"id": 2, "title": "B"}}],
                "totalNumberOfItems": 2, "limit": 1000, "offset": 0
            })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/users/42/favorites/albums"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 3, "title": "C"}}],
                "totalNumberOfItems": 1, "limit": 1000, "offset": 0
            })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/users/42/favorites/artists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 4, "name": "D"}}],
                "totalNumberOfItems": 1, "limit": 1000, "offset": 0
            })))
            .mount(&server).await;
        let ids = client(&server).favorite_ids().await.unwrap();
        assert_eq!(ids.tracks, vec![1, 2]);
        assert_eq!(ids.albums, vec![3]);
        assert_eq!(ids.artists, vec![4]);
    }
}
