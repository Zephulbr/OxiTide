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
    /// level), following the v2 cursor to the end.
    pub async fn collection_items(&self, folder_id: &str) -> Result<Vec<CollectionEntry>> {
        let mut entries = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_FOLDER_PAGES {
            let mut params = vec![
                ("folderId", folder_id.to_string()),
                ("limit", FOLDER_PAGE_LIMIT.to_string()),
                ("order", "NAME".to_string()),
            ];
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

        let entries = client(&server).collection_items("root").await.unwrap();
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

        let entries = client(&server).collection_items("root").await.unwrap();
        assert_eq!(entries.len(), 20);
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
        assert!(client(&server).collection_items("root").await.unwrap().is_empty());
    }
}
