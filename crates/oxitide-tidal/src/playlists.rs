use serde::Deserialize;

use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::{Error, Result};
use crate::models::{ItemPage, Playlist, Track};

fn page_params(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

/// JSON:API responses from the v2 API wrap the payload in `{ "data": ... }`.
#[derive(Debug, Deserialize)]
struct V2Envelope<T> {
    data: T,
}

/// Rows per add request; the items endpoint honors 100-row additions.
const ADD_CHUNK: usize = 100;

/// `POST .../items` response; ids may arrive as numbers or numeric strings.
#[derive(Debug, Deserialize)]
struct AddedItems {
    #[serde(rename = "addedItemIds")]
    added_item_ids: Option<Vec<IdOrString>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IdOrString {
    Number(u64),
    Text(String),
}

impl IdOrString {
    fn as_id(&self) -> Option<u64> {
        match self {
            IdOrString::Number(number) => Some(*number),
            IdOrString::Text(text) => text.parse().ok(),
        }
    }
}

/// TIDAL's stale-precondition response for playlist writes.
fn is_stale_etag(error: &Error) -> bool {
    matches!(error, Error::Api { status: 412, .. })
}

impl TidalClient {
    pub async fn playlist(&self, id: &str) -> Result<Playlist> {
        let mut playlist: Playlist = self.get(&format!("playlists/{id}"), &[]).await?;
        playlist.mark_owned(self.session().user_id, false);
        Ok(playlist)
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
        let mut items = page.items;
        for playlist in &mut items {
            playlist.mark_owned(self.session().user_id, true);
        }
        Ok(items)
    }

    /// Walk every page of the user's playlists.
    pub async fn all_user_playlists(&self) -> Result<Vec<Playlist>> {
        let mut items: Vec<Playlist> = self
            .get_all_pages(
                &format!("users/{}/playlists", self.session().user_id),
                &[],
                DEFAULT_PAGE_SIZE,
            )
            .await?;
        for playlist in &mut items {
            playlist.mark_owned(self.session().user_id, true);
        }
        Ok(items)
    }

    /// The playlist plus the `etag` TIDAL requires in `If-None-Match` for writes.
    async fn playlist_and_etag(&self, id: &str) -> Result<(Playlist, Option<String>)> {
        self.get_with_etag(&format!("playlists/{id}"), &[]).await
    }

    /// Just the current playlist `etag`; used between chunks and on a 412 retry.
    async fn playlist_etag(&self, id: &str) -> Result<Option<String>> {
        Ok(self.playlist_and_etag(id).await?.1)
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
        let mut playlist = envelope.data;
        playlist.mark_owned(self.session().user_id, true);
        Ok(playlist)
    }

    /// Rename a playlist and/or change its description.
    pub async fn rename_playlist(&self, id: &str, title: &str, description: &str) -> Result<()> {
        let form = [
            ("title", title.to_string()),
            ("description", description.to_string()),
        ];
        let path = format!("playlists/{id}");
        let mut etag = self.playlist_etag(id).await?;
        match self.post_form_with_etag(&path, &form, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.post_form_with_etag(&path, &form, etag.as_deref()).await
            }
            result => result,
        }
    }

    /// Delete a playlist.
    pub async fn delete_playlist(&self, id: &str) -> Result<()> {
        let path = format!("playlists/{id}");
        let mut etag = self.playlist_etag(id).await?;
        match self.delete_with_etag(&path, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.delete_with_etag(&path, etag.as_deref()).await
            }
            result => result,
        }
    }

    /// Add tracks to a playlist in request order, returning the ids the server
    /// reports as added. Tracks already present are skipped unless
    /// `allow_duplicates` is set; when the response omits `addedItemIds`,
    /// every requested id is assumed added.
    pub async fn add_playlist_tracks(
        &self,
        id: &str,
        track_ids: &[u64],
        allow_duplicates: bool,
    ) -> Result<Vec<u64>> {
        let mut requested: Vec<u64> = Vec::new();
        for track_id in track_ids {
            if !requested.contains(track_id) {
                requested.push(*track_id);
            }
        }
        if requested.is_empty() {
            return Ok(Vec::new());
        }
        let (playlist, mut etag) = self.playlist_and_etag(id).await?;
        let chunks: Vec<&[u64]> = requested.chunks(ADD_CHUNK).collect();
        let mut added: Vec<u64> = Vec::new();
        for (index, chunk) in chunks.iter().enumerate() {
            let form = [
                ("onArtifactNotFound", "SKIP".to_string()),
                (
                    "trackIds",
                    chunk
                        .iter()
                        .map(|track_id| track_id.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                ),
                (
                    "toIndex",
                    (playlist.number_of_tracks as usize + added.len()).to_string(),
                ),
                (
                    "onDupes",
                    if allow_duplicates { "ADD" } else { "SKIP" }.to_string(),
                ),
            ];
            let params = [("limit", chunk.len().to_string())];
            let path = format!("playlists/{id}/items");
            let response: AddedItems = match self
                .post_form_json_with_etag(&path, &params, &form, etag.as_deref())
                .await
            {
                Err(e) if is_stale_etag(&e) => {
                    etag = self.playlist_etag(id).await?;
                    self.post_form_json_with_etag(&path, &params, &form, etag.as_deref())
                        .await?
                }
                result => result?,
            };
            match response.added_item_ids {
                Some(values) => {
                    added.extend(values.iter().filter_map(IdOrString::as_id));
                }
                None => added.extend(chunk.iter().copied()),
            }
            // Each write rotates the playlist etag; refresh before the next chunk.
            if index + 1 < chunks.len() {
                etag = self.playlist_etag(id).await?;
            }
        }
        Ok(added)
    }

    /// Remove playlist rows by zero-based index.
    pub async fn remove_playlist_items(&self, id: &str, indices: &[u32]) -> Result<()> {
        if indices.is_empty() {
            return Ok(());
        }
        let list = indices
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let path = format!("playlists/{id}/items/{list}");
        let mut etag = self.playlist_etag(id).await?;
        match self.delete_with_etag(&path, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.delete_with_etag(&path, etag.as_deref()).await
            }
            result => result,
        }
    }

    /// Move playlist rows (zero-based indices) to `to_index`.
    pub async fn move_playlist_items(&self, id: &str, indices: &[u32], to_index: u32) -> Result<()> {
        if indices.is_empty() {
            return Ok(());
        }
        let list = indices
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let path = format!("playlists/{id}/items/{list}");
        let form = [("toIndex", to_index.to_string())];
        let mut etag = self.playlist_etag(id).await?;
        match self.post_form_with_etag(&path, &form, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.post_form_with_etag(&path, &form, etag.as_deref()).await
            }
            result => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
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
        assert!(playlist.owned);
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
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid"))
            .and(header("if-none-match", "\"e1\""))
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
        playlist_page(&server, 0).await;
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
    async fn delete_playlist_sends_delete_with_etag() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("DELETE"))
            .and(path("/playlists/p-uuid"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        client(&server).delete_playlist("p-uuid").await.unwrap();
    }

    async fn playlist_page(server: &MockServer, total: u32) {
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"e1\"")
                    .set_body_json(serde_json::json!({
                        "uuid": "p-uuid", "title": "P", "numberOfTracks": total
                    })),
            )
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn add_playlist_tracks_skips_duplicates_and_reports_added() {
        let server = MockServer::start().await;
        playlist_page(&server, 5).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e1\""))
            .and(query_param("limit", "1"))
            .and(body_string_contains("trackIds=7"))
            .and(body_string_contains("toIndex=5"))
            .and(body_string_contains("onDupes=SKIP"))
            .and(body_string_contains("onArtifactNotFound=SKIP"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [7]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7], false)
            .await
            .unwrap();
        assert_eq!(added, vec![7]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_allow_duplicates_uses_add() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(body_string_contains("toIndex=0"))
            .and(body_string_contains("onDupes=ADD"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [7]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7], true)
            .await
            .unwrap();
        assert_eq!(added, vec![7]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_accepts_string_ids() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": ["7", "8"]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7, 8], false)
            .await
            .unwrap();
        assert_eq!(added, vec![7, 8]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_missing_added_ids_assumes_all_added() {
        let server = MockServer::start().await;
        playlist_page(&server, 1).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7, 8], false)
            .await
            .unwrap();
        assert_eq!(added, vec![7, 8]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_dedupes_and_chunks() {
        let server = MockServer::start().await;
        let gets = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let responder = {
            let gets = gets.clone();
            move |_req: &wiremock::Request| {
                let n = gets.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ResponseTemplate::new(200)
                    .insert_header("etag", if n == 0 { "\"e1\"" } else { "\"e2\"" })
                    .set_body_json(serde_json::json!({
                        "uuid": "p-uuid", "title": "P", "numberOfTracks": 5
                    }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
        let ids: Vec<u64> = (1..=101).collect();
        let first_chunk = ids[..100]
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join("%2C");
        // A repeated id must not turn into a duplicate inside one request.
        let requested: Vec<u64> = std::iter::once(1u64).chain(ids.iter().copied()).collect();
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e1\""))
            .and(query_param("limit", "100"))
            .and(body_string_contains(&format!("trackIds={first_chunk}")))
            .and(body_string_contains("toIndex=5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": ids[..100].to_vec()
            })))
            .mount(&server).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e2\""))
            .and(query_param("limit", "1"))
            .and(body_string_contains("trackIds=101"))
            .and(body_string_contains("toIndex=105"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [101]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &requested, false)
            .await
            .unwrap();
        assert_eq!(added.len(), 101);
        assert_eq!(added[0], 1);
        assert_eq!(added[100], 101);
    }

    #[tokio::test]
    async fn add_playlist_tracks_retries_once_on_stale_etag() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let server = MockServer::start().await;
        let gets = Arc::new(AtomicUsize::new(0));
        let responder = {
            let gets = gets.clone();
            move |_req: &wiremock::Request| {
                let n = gets.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200)
                    .insert_header("etag", if n == 0 { "\"e1\"" } else { "\"e2\"" })
                    .set_body_json(serde_json::json!({
                        "uuid": "p-uuid", "title": "P", "numberOfTracks": 0
                    }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(ResponseTemplate::new(412).set_body_json(serde_json::json!({
                "status": 412, "subStatus": 7002,
                "userMessage": "You must send the correct Etag value in the If-None-Match header to modify a playlist"
            })))
            .mount(&server).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e2\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [7]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7], false)
            .await
            .unwrap();
        assert_eq!(added, vec![7]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_persistent_412_surfaces() {
        let server = MockServer::start().await;
        let gets = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let responder = {
            let gets = gets.clone();
            move |_req: &wiremock::Request| {
                let n = gets.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ResponseTemplate::new(200)
                    .insert_header("etag", if n == 0 { "\"e1\"" } else { "\"e2\"" })
                    .set_body_json(serde_json::json!({
                        "uuid": "p-uuid", "title": "P", "numberOfTracks": 0
                    }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
        let conflict = || {
            ResponseTemplate::new(412).set_body_json(serde_json::json!({
                "status": 412, "subStatus": 7002,
                "userMessage": "You must send the correct Etag value in the If-None-Match header to modify a playlist"
            }))
        };
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(conflict())
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e2\""))
            .respond_with(conflict())
            .expect(1)
            .mount(&server)
            .await;
        let err = client(&server)
            .add_playlist_tracks("p-uuid", &[7], false)
            .await
            .unwrap_err();
        assert!(matches!(err, crate::error::Error::Api { status: 412, .. }), "got {err:?}");
    }

    #[tokio::test]
    async fn rename_playlist_401_after_a_refresh_is_forbidden() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("POST")).and(path("/playlists/p-uuid"))
            .and(header("authorization", "Bearer AT"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/playlists/p-uuid"))
            .and(header("authorization", "Bearer NEW"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "NEW", "refresh_token": "RT2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let err = client(&server)
            .rename_playlist("p-uuid", "New name", "")
            .await
            .unwrap_err();
        assert!(matches!(err, crate::error::Error::Forbidden), "expected Forbidden, got {err:?}");
    }

    #[tokio::test]
    async fn add_playlist_tracks_empty_list_makes_no_request() {
        let server = MockServer::start().await;
        let added = client(&server).add_playlist_tracks("p-uuid", &[], false).await.unwrap();
        assert!(added.is_empty());
    }

    #[tokio::test]
    async fn remove_playlist_items_sends_indices_path() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("DELETE"))
            .and(path("/playlists/p-uuid/items/2,5"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        client(&server)
            .remove_playlist_items("p-uuid", &[2, 5])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn move_playlist_items_sends_to_index() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items/4"))
            .and(header("if-none-match", "\"e1\""))
            .and(body_string_contains("toIndex=1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server).await;
        client(&server)
            .move_playlist_items("p-uuid", &[4], 1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn remove_playlist_items_skips_empty_indices() {
        let server = MockServer::start().await;
        // No mock is mounted: any request would 404 and fail the unwrap.
        client(&server)
            .remove_playlist_items("p-uuid", &[])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn move_playlist_items_skips_empty_indices() {
        let server = MockServer::start().await;
        // No mock is mounted: any request would 404 and fail the unwrap.
        client(&server)
            .move_playlist_items("p-uuid", &[], 3)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn playlist_marks_owned_when_the_creator_is_the_session_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "p-uuid", "title": "P", "creator": {"id": 42}
            })))
            .mount(&server).await;
        assert!(client(&server).playlist("p-uuid").await.unwrap().owned);
    }

    #[tokio::test]
    async fn playlist_marks_not_owned_for_another_or_missing_creator() {
        let other = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "p-uuid", "title": "P", "creator": {"id": 7}
            })))
            .mount(&other).await;
        assert!(!client(&other).playlist("p-uuid").await.unwrap().owned);

        let missing = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "p-uuid", "title": "P"
            })))
            .mount(&missing).await;
        assert!(!client(&missing).playlist("p-uuid").await.unwrap().owned);
    }

    #[tokio::test]
    async fn user_playlists_marks_items_owned_even_without_a_creator() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/playlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"uuid": "a", "title": "A"}, {"uuid": "b", "title": "B"}],
                "totalNumberOfItems": 2, "limit": 50, "offset": 0
            })))
            .mount(&server).await;
        let items = client(&server).user_playlists(50, 0).await.unwrap();
        assert!(items.iter().all(|playlist| playlist.owned));
    }
}
