use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;
use crate::models::Playlist;

/// 50 is the v2 folders endpoint's maximum; the cap below bounds a broken
/// cursor so a server bug can never loop forever.
const FOLDER_PAGE_LIMIT: u32 = 50;
const MAX_FOLDER_PAGES: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CollectionEntry {
    Folder(CollectionFolder),
    Playlist(Playlist),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionFolder {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "totalNumberOfItems")]
    pub total_number_of_items: u32,
}

#[derive(Debug, Deserialize)]
struct RawCollectionPage {
    #[serde(default)]
    items: Vec<RawCollectionItem>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCollectionItem {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    data: serde_json::Value,
}

impl TidalClient {
    /// Every folder and playlist inside `folder_id` (`"root"` for the top
    /// level), following the v2 cursor to the end. `order` is omitted when
    /// `None`, leaving the server's default ordering.
    pub async fn collection_items(
        &self,
        folder_id: &str,
        order: Option<&str>,
    ) -> Result<Vec<CollectionEntry>> {
        let mut entries = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_FOLDER_PAGES {
            let mut params = vec![
                ("folderId", folder_id.to_string()),
                ("limit", FOLDER_PAGE_LIMIT.to_string()),
            ];
            if let Some(value) = order {
                params.push(("order", value.to_string()));
            }
            if let Some(value) = &cursor {
                params.push(("cursor", value.clone()));
            }
            let page: RawCollectionPage = self
                .get_v2("my-collection/playlists/folders", &params)
                .await?;
            let next = page.cursor.clone();
            let empty = page.items.is_empty();
            for item in page.items {
                if let Some(entry) = self.collection_entry(item) {
                    entries.push(entry);
                }
            }
            match next {
                Some(value) if !empty && Some(&value) != cursor.as_ref() => cursor = Some(value),
                _ => break,
            }
        }
        Ok(entries)
    }

    /// Create a folder inside `parent_id` (`"root"` for the top level).
    pub async fn create_folder(&self, name: &str, parent_id: &str) -> Result<CollectionFolder> {
        #[derive(Deserialize)]
        struct RawFolder {
            #[serde(default)]
            id: String,
            #[serde(default)]
            name: Option<String>,
            #[serde(default, rename = "totalNumberOfItems")]
            total_number_of_items: u32,
        }
        #[derive(Deserialize)]
        struct Envelope {
            data: RawFolder,
        }
        let envelope: Envelope = self
            .put_json(
                "my-collection/playlists/folders/create-folder",
                &[("name", name.to_string()), ("folderId", parent_id.to_string())],
            )
            .await?;
        Ok(CollectionFolder {
            id: envelope.data.id,
            name: envelope.data.name.unwrap_or_else(|| name.to_string()),
            total_number_of_items: envelope.data.total_number_of_items,
        })
    }

    pub async fn rename_folder(&self, id: &str, name: &str) -> Result<()> {
        self.put_v2(
            "my-collection/playlists/folders/rename",
            &[
                ("trn", format!("trn:folder:{id}")),
                ("name", name.to_string()),
            ],
        )
        .await
    }

    pub async fn remove_collection_entry(&self, kind: &str, id: &str) -> Result<()> {
        let trn = if kind == "playlist" {
            format!("trn:playlist:{id}")
        } else {
            format!("trn:folder:{id}")
        };
        self.put_v2(
            "my-collection/playlists/folders/remove",
            &[("trns", trn)],
        )
        .await
    }

    pub async fn move_playlists(&self, ids: &[String], folder_id: &str) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let trns = ids
            .iter()
            .map(|id| format!("trn:playlist:{id}"))
            .collect::<Vec<_>>()
            .join(",");
        self.put_v2(
            "my-collection/playlists/folders/move",
            &[("folderId", folder_id.to_string()), ("trns", trns)],
        )
        .await
    }

    fn collection_entry(&self, item: RawCollectionItem) -> Option<CollectionEntry> {
        match item.kind.to_uppercase().as_str() {
            "FOLDER" => {
                let id = value_to_string(item.data.get("id")?)?;
                let total = item
                    .data
                    .get("totalNumberOfItems")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                let name = item
                    .name
                    .or_else(|| {
                        item.data
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or_default();
                Some(CollectionEntry::Folder(CollectionFolder {
                    id,
                    name,
                    total_number_of_items: total,
                }))
            }
            "PLAYLIST" => {
                let mut playlist: Playlist = serde_json::from_value(item.data).ok()?;
                playlist.mark_owned(self.session().user_id, true);
                Some(CollectionEntry::Playlist(playlist))
            }
            _ => None,
        }
    }
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
    use crate::auth::SessionInfo;
    use crate::client::TidalClient;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
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
    async fn collection_items_walks_cursor_pages_and_parses_both_kinds() {
        let server = MockServer::start().await;
        let responder = |req: &wiremock::Request| {
            let has_cursor = req.url.query_pairs().any(|(key, _)| key == "cursor");
            if has_cursor {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [
                        {"type": "PLAYLIST", "data": {"uuid": "p-2", "title": "Second",
                            "numberOfTracks": 3, "creator": {"id": 42}}}
                    ],
                    "totalNumberOfItems": 2
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [
                        {"type": "FOLDER", "name": "MISC",
                            "data": {"id": "f-1", "totalNumberOfItems": 10}},
                        {"type": "PLAYLIST", "data": {"uuid": "p-1", "title": "Road trip",
                            "numberOfTracks": 12, "squareImage": "sq-1", "creator": {"id": 42}}}
                    ],
                    "cursor": "c2",
                    "totalNumberOfItems": 2
                }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/v2/my-collection/playlists/folders"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;

        let entries = client(&server).collection_items("root", None).await.unwrap();
        assert_eq!(entries.len(), 3);
        match &entries[0] {
            crate::folders::CollectionEntry::Folder(folder) => {
                assert_eq!(folder.id, "f-1");
                assert_eq!(folder.name, "MISC");
                assert_eq!(folder.total_number_of_items, 10);
            }
            other => panic!("expected folder first, got {other:?}"),
        }
        match &entries[1] {
            crate::folders::CollectionEntry::Playlist(playlist) => {
                assert_eq!(playlist.uuid, "p-1");
                assert!(playlist.owned, "assume_owned applies to collection playlists");
            }
            other => panic!("expected playlist, got {other:?}"),
        }
        match &entries[2] {
            crate::folders::CollectionEntry::Playlist(playlist) => {
                assert_eq!(playlist.uuid, "p-2");
            }
            other => panic!("expected playlist, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn collection_items_stops_at_the_page_cap() {
        let server = MockServer::start().await;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let responder = {
            let calls = calls.clone();
            move |_req: &wiremock::Request| {
                let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [
                        {"type": "PLAYLIST", "data": {"uuid": "p-1", "title": "One"}}
                    ],
                    "cursor": format!("cursor-{n}")
                }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/v2/my-collection/playlists/folders"))
            .respond_with(responder)
            .expect(20)
            .mount(&server)
            .await;

        let entries = client(&server).collection_items("root", None).await.unwrap();
        assert_eq!(entries.len(), 20);
    }

    #[tokio::test]
    async fn collection_items_sends_order_only_when_provided() {
        let server = MockServer::start().await;
        let saw_name_order = Arc::new(AtomicBool::new(false));
        let saw_omitted_order = Arc::new(AtomicBool::new(false));
        let responder = {
            let saw_name_order = saw_name_order.clone();
            let saw_omitted_order = saw_omitted_order.clone();
            move |req: &wiremock::Request| {
                match req
                    .url
                    .query_pairs()
                    .find(|(key, _)| key == "order")
                    .map(|(_, value)| value.to_string())
                    .as_deref()
                {
                    Some("NAME") => saw_name_order.store(true, Ordering::SeqCst),
                    None => saw_omitted_order.store(true, Ordering::SeqCst),
                    Some(_) => {}
                }
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [], "totalNumberOfItems": 0
                }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/v2/my-collection/playlists/folders"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;

        let client = client(&server);
        client.collection_items("root", Some("NAME")).await.unwrap();
        client.collection_items("root", None).await.unwrap();

        assert!(saw_name_order.load(Ordering::SeqCst), "order=NAME must be sent when requested");
        assert!(
            saw_omitted_order.load(Ordering::SeqCst),
            "the order param must be omitted when not requested"
        );
    }

    #[tokio::test]
    async fn collection_items_empty_page_returns_nothing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/my-collection/playlists/folders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "totalNumberOfItems": 0
            })))
            .mount(&server)
            .await;
        assert!(client(&server).collection_items("root", None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_folder_puts_to_v2_and_parses_the_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/create-folder"))
            .and(wiremock::matchers::query_param("name", "Road"))
            .and(wiremock::matchers::query_param("folderId", "root"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"id": "f-9", "totalNumberOfItems": 0}
            })))
            .mount(&server)
            .await;
        let folder = client(&server).create_folder("Road", "root").await.unwrap();
        assert_eq!(folder.id, "f-9");
        assert_eq!(folder.name, "Road");
    }

    #[tokio::test]
    async fn rename_folder_sends_the_folder_trn() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/rename"))
            .and(wiremock::matchers::query_param("trn", "trn:folder:f-1"))
            .and(wiremock::matchers::query_param("name", "Renamed"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        client(&server).rename_folder("f-1", "Renamed").await.unwrap();
    }

    #[tokio::test]
    async fn remove_collection_entry_prefixes_the_trn_by_kind() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/remove"))
            .and(wiremock::matchers::query_param("trns", "trn:playlist:p-1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/remove"))
            .and(wiremock::matchers::query_param("trns", "trn:folder:f-1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        client(&server).remove_collection_entry("playlist", "p-1").await.unwrap();
        client(&server).remove_collection_entry("folder", "f-1").await.unwrap();
    }

    #[tokio::test]
    async fn move_playlists_joins_trns_and_targets_the_folder() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/move"))
            .and(wiremock::matchers::query_param("folderId", "f-2"))
            .and(wiremock::matchers::query_param(
                "trns",
                "trn:playlist:p-1,trn:playlist:p-2",
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        client(&server)
            .move_playlists(&["p-1".to_string(), "p-2".to_string()], "f-2")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn move_playlists_with_no_ids_makes_no_request() {
        let server = MockServer::start().await;
        client(&server).move_playlists(&[], "f-2").await.unwrap();
    }
}
