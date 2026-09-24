use serde::Deserialize;

use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{ItemPage, Playlist, Track};

fn page_params(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

/// JSON:API responses from the v2 API wrap the payload in `{ "data": ... }`.
#[derive(Debug, Deserialize)]
struct V2Envelope<T> {
    data: T,
}

impl TidalClient {
    pub async fn playlist(&self, id: &str) -> Result<Playlist> {
        self.get(&format!("playlists/{id}"), &[]).await
    }

    /// One page of playlist tracks plus the server-reported total.
    pub async fn playlist_tracks_page(
        &self,
        id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Track>, u32)> {
        let page: ItemPage<Track> =
            self.get(&format!("playlists/{id}/tracks"), &page_params(limit, offset)).await?;
        Ok((page.items, page.total_number_of_items))
    }

    /// Fetch every track of a playlist, walking all pages.
    pub async fn playlist_tracks_all(&self, id: &str) -> Result<Vec<Track>> {
        self.get_all_pages(&format!("playlists/{id}/tracks"), &[], DEFAULT_PAGE_SIZE)
            .await
    }

    /// One page of the signed-in user's playlists.
    pub async fn user_playlists(&self, limit: u32, offset: u32) -> Result<Vec<Playlist>> {
        let page: ItemPage<Playlist> = self
            .get(
                &format!("users/{}/playlists", self.session().user_id),
                &page_params(limit, offset),
            )
            .await?;
        Ok(page.items)
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

    /// Create a playlist in the user's root folder (v2 endpoint).
    pub async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist> {
        let envelope: V2Envelope<Playlist> = self
            .put_json(
                "my-collection/playlists/folders/create-playlist",
                &[
                    ("name", name.to_string()),
                    ("description", description.to_string()),
                    ("folderId", "root".to_string()),
                ],
            )
            .await?;
        Ok(envelope.data)
    }

    /// Rename a playlist and/or change its description.
    pub async fn rename_playlist(&self, id: &str, title: &str, description: &str) -> Result<()> {
        self.post_form(
            &format!("playlists/{id}"),
            &[
                ("title", title.to_string()),
                ("description", description.to_string()),
            ],
        )
        .await
    }

    /// Delete a playlist.
    pub async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.delete(&format!("playlists/{id}")).await
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
    async fn playlist_tracks_page_returns_items_and_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid/tracks"))
            .and(query_param("limit", "100")).and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 2, "title": "P1", "artists": [{"id": 9, "name": "X"}]}],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        let (tracks, total) = client(&server).playlist_tracks_page("p-uuid", 100, 0).await.unwrap();
        assert_eq!(tracks[0].artists[0].name, "X");
        assert_eq!(total, 1);
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

    #[tokio::test]
    async fn create_playlist_puts_v2_and_parses_the_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/create-playlist"))
            .and(query_param("name", "Mix"))
            .and(query_param("description", "for the road"))
            .and(query_param("folderId", "root"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "uuid": "new-uuid",
                    "title": "Mix",
                    "description": "for the road",
                    "numberOfTracks": 0,
                    "duration": 0
                }
            })))
            .mount(&server).await;
        let playlist = client(&server)
            .create_playlist("Mix", "for the road")
            .await
            .unwrap();
        assert_eq!(playlist.uuid, "new-uuid");
        assert_eq!(playlist.title, "Mix");
        assert_eq!(playlist.number_of_tracks, 0);
    }

    #[tokio::test]
    async fn create_playlist_without_uuid_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/create-playlist"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"title": "No id"}
            })))
            .mount(&server).await;
        assert!(client(&server).create_playlist("No id", "").await.is_err());
    }

    #[tokio::test]
    async fn rename_playlist_posts_title_and_description() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid"))
            .and(body_string_contains("title=New+name"))
            .and(body_string_contains("description=Notes"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server).await;
        client(&server)
            .rename_playlist("p-uuid", "New name", "Notes")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn rename_playlist_maps_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server).await;
        let err = client(&server)
            .rename_playlist("p-uuid", "Nope", "")
            .await
            .unwrap_err();
        assert!(matches!(err, crate::error::Error::Forbidden), "expected Forbidden, got {err:?}");
    }

    #[tokio::test]
    async fn delete_playlist_sends_delete() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        client(&server).delete_playlist("p-uuid").await.unwrap();
    }
}
