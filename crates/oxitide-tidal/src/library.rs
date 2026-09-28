use serde::{Deserialize, Serialize};

use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{Album, Artist, ItemPage, Mix, Track, Video};

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

    /// Walk every page of the user's favorite videos.
    pub async fn all_favorite_videos(&self) -> Result<Vec<Video>> {
        let favs: Vec<Favorited<Video>> = self
            .get_all_pages(&format!("{}/videos", self.user_base()), &[], DEFAULT_PAGE_SIZE)
            .await?;
        Ok(favs.into_iter().map(|f| f.item).collect())
    }

    /// Favorite mixes & radio (v2). The envelope varies by account/endpoint, so
    /// both `{data:[…]}` and `{items:[…]}` are accepted, with entries bare or
    /// wrapped in `{mix}`/`{data}`.
    pub async fn favorite_mixes(&self, limit: u32, offset: u32) -> Result<(Vec<Mix>, u32)> {
        let raw: serde_json::Value = self.get_v2("favorites/mixes", &page(limit, offset)).await?;
        Ok(mixes_from_json(&raw))
    }

    /// Walk every page of the user's favorite mixes. The v2 endpoint rejects
    /// limits above 50, so pages are capped there and the walk stops after 20
    /// pages as a broken-total guard.
    pub async fn all_favorite_mixes(&self) -> Result<Vec<Mix>> {
        const PAGE_SIZE: u32 = 50;
        const MAX_PAGES: usize = 20;
        let mut all = Vec::new();
        let mut offset: u32 = 0;
        for _ in 0..MAX_PAGES {
            let (mixes, total) = self.favorite_mixes(PAGE_SIZE, offset).await?;
            let received = mixes.len() as u32;
            all.extend(mixes);
            offset = offset.saturating_add(received);
            if received < PAGE_SIZE || (total > 0 && offset >= total) {
                break;
            }
        }
        Ok(all)
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

fn mixes_from_json(raw: &serde_json::Value) -> (Vec<Mix>, u32) {
    let entries = raw
        .get("data")
        .or_else(|| raw.get("items"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = raw
        .get("totalNumberOfItems")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(entries.len() as u64) as u32;
    let mixes = entries.iter().filter_map(mix_from_value).collect();
    (mixes, total)
}

fn mix_from_value(value: &serde_json::Value) -> Option<Mix> {
    let mix = value
        .get("mix")
        .or_else(|| value.get("data"))
        .unwrap_or(value);
    let id = value_to_string(mix.get("id")?)?;
    let title = mix
        .get("title")
        .and_then(serde_json::Value::as_str)
        .or_else(|| mix.pointer("/titleTextInfo/text").and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .to_string();
    let sub_title = mix
        .get("subTitle")
        .and_then(serde_json::Value::as_str)
        .or_else(|| mix.pointer("/subtitleTextInfo/text").and_then(serde_json::Value::as_str))
        .map(str::to_string);
    let mix_type = mix.get("mixType").and_then(serde_json::Value::as_str).map(str::to_string);
    let image = mix
        .pointer("/images/SMALL/url")
        .and_then(serde_json::Value::as_str)
        .or_else(|| mix.pointer("/mixImages/0/url").and_then(serde_json::Value::as_str))
        .map(str::to_string);
    Some(Mix { id, title, sub_title, mix_type, image })
}

fn value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
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
                "items": (1..=100)
                    .map(|id| serde_json::json!({"item": {"id": id, "title": "T"}}))
                    .collect::<Vec<_>>(),
                "totalNumberOfItems": 101, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 101, "title": "Last"}}],
                "totalNumberOfItems": 101, "limit": 100, "offset": 100
            })))
            .mount(&server).await;
        let tracks = client(&server).all_favorite_tracks().await.unwrap();
        assert_eq!(tracks.len(), 101);
        assert_eq!(tracks[100].id, 101);
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

    #[tokio::test]
    async fn all_favorite_videos_unwraps_the_item_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/videos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 3, "title": "Clip", "duration": 120,
                    "artists": [{"id": 1, "name": "A"}], "image": "img-3"}}],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        let videos = client(&server).all_favorite_videos().await.unwrap();
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].id, 3);
        assert_eq!(videos[0].image.as_deref(), Some("img-3"));
    }

    #[tokio::test]
    async fn favorite_mixes_accepts_the_v2_data_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/v2/favorites/mixes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "m1", "title": "Daily Mix", "subTitle": "For you",
                     "images": {"SMALL": {"url": "https://img/small.jpg"}}},
                    {"mix": {"id": "m2", "title": "Artist Radio",
                     "mixImages": [{"url": "https://img/a.jpg"}]}}
                ],
                "totalNumberOfItems": 2
            })))
            .mount(&server).await;
        let (mixes, total) = client(&server).favorite_mixes(50, 0).await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(mixes[0].id, "m1");
        assert_eq!(mixes[0].image.as_deref(), Some("https://img/small.jpg"));
        assert_eq!(mixes[1].id, "m2");
        assert_eq!(mixes[1].image.as_deref(), Some("https://img/a.jpg"));
    }

    #[tokio::test]
    async fn favorite_mixes_accepts_the_v1_items_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/v2/favorites/mixes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": "m3", "title": "Mood", "type": "MIX"}]
            })))
            .mount(&server).await;
        let (mixes, total) = client(&server).favorite_mixes(50, 0).await.unwrap();
        assert_eq!(mixes.len(), 1);
        assert_eq!(mixes[0].id, "m3");
        assert_eq!(total, 1);
    }

    #[tokio::test]
    async fn all_favorite_mixes_walks_fifty_item_pages() {
        let server = MockServer::start().await;
        let responder = |req: &wiremock::Request| {
            let pairs: Vec<(String, String)> = req
                .url
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            let limit = pairs
                .iter()
                .find(|(key, _)| key == "limit")
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            assert_eq!(limit, "50", "the v2 endpoint rejects limits above 50");
            let offset: u32 = pairs
                .iter()
                .find(|(key, _)| key == "offset")
                .map(|(_, value)| value.parse().unwrap_or(0))
                .unwrap_or(0);
            let count = match offset {
                0 => 50,
                50 => 2,
                _ => 0,
            };
            let items: Vec<serde_json::Value> = (0..count)
                .map(|i| {
                    serde_json::json!({
                        "id": format!("m-{}", offset + i),
                        "title": "Mix"
                    })
                })
                .collect();
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": items,
                "totalNumberOfItems": 52
            }))
        };
        Mock::given(method("GET"))
            .and(path("/v2/favorites/mixes"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;

        let mixes = client(&server).all_favorite_mixes().await.unwrap();
        assert_eq!(mixes.len(), 52);
        assert_eq!(mixes[0].id, "m-0");
        assert_eq!(mixes[51].id, "m-51");
    }
}
