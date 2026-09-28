use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default = "default_kind")]
    pub item_type: String,
    /// Artwork uuid, resolved best-effort from the type-varying raw fields.
    #[serde(default)]
    pub artwork: Option<String>,
}

fn default_kind() -> String {
    "unknown".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSection {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<PageItem>,
}

#[derive(Debug, Deserialize)]
struct RawRow {
    #[serde(default)]
    pub modules: Vec<RawModule>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    #[serde(default)]
    pub title: String,
    #[serde(rename = "type", default)]
    pub module_type: String,
    #[serde(default)]
    pub items: Vec<RawItem>,
    #[serde(default, rename = "pagedList")]
    pub paged_list: Option<RawPagedList>,
}

#[derive(Debug, Deserialize)]
struct RawPagedList {
    #[serde(default)]
    pub items: Vec<RawItem>,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub item_type: String,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default, rename = "squareImage")]
    pub square_image: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub album: Option<RawAlbum>,
}

#[derive(Debug, Deserialize)]
struct RawAlbum {
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPage {
    #[serde(default)]
    pub rows: Vec<RawRow>,
}

fn page_params() -> Vec<(&'static str, String)> {
    vec![("deviceType", "BROWSER".into()), ("platform", "WEB".into())]
}

impl TidalClient {
    /// Parse any v1 page endpoint that uses the rows/modules format.
    async fn page_sections_at(
        &self,
        path: &str,
        extra: &[(&str, String)],
    ) -> Result<Vec<PageSection>> {
        let mut params = page_params();
        params.extend(extra.iter().cloned());
        let raw: RawPage = self.get(path, &params).await?;
        Ok(sections_from_page(raw))
    }

    /// Fetch the home feed. TIDAL requires `deviceType`/`platform` on this endpoint.
    pub async fn home_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/home", &[]).await
    }

    /// Fetch the explore page (same rows/modules shape as home).
    pub async fn explore_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/explore", &[]).await
    }

    /// The signed-in user's mixes & radio, as a page.
    pub async fn mixes_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/my_collection_my_mixes", &[]).await
    }

    /// The user's feed. TIDAL's endpoint path is unconfirmed; candidates are
    /// probed in order and a missing feed degrades to an empty list.
    pub async fn feed_page(&self) -> Result<Vec<PageSection>> {
        for path in ["pages/feed", "pages/my_collection_feed"] {
            match self.page_sections_at(path, &[]).await {
                Ok(sections) => return Ok(sections),
                Err(crate::error::Error::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(Vec::new())
    }
}

fn sections_from_page(raw: RawPage) -> Vec<PageSection> {
    let mut sections = Vec::new();
    for row in raw.rows {
        for module in row.modules {
            let mut raw_items = module.items;
            if let Some(paged) = module.paged_list {
                raw_items.extend(paged.items);
            }
            if raw_items.is_empty() {
                continue;
            }
            let module_type = module.module_type.clone();
            let items: Vec<PageItem> = raw_items
                .into_iter()
                .map(|i| PageItem {
                    id: item_id(&i),
                    title: i.title,
                    subtitle: i.subtitle.or(i.description),
                    item_type: if i.item_type.is_empty() {
                        module_type.clone()
                    } else {
                        i.item_type
                    },
                    artwork: i
                        .cover
                        .or(i.square_image)
                        .or(i.image)
                        .or(i.picture)
                        .or_else(|| i.album.and_then(|album| album.cover)),
                })
                .collect();
            sections.push(PageSection { title: module.title, items });
        }
    }
    sections
}

/// TIDAL uses a numeric `id` for tracks/albums and a `uuid` for playlists.
fn item_id(item: &RawItem) -> String {
    let from_id = value_to_id(&item.id);
    if !from_id.is_empty() {
        return from_id;
    }
    item.uuid.clone().unwrap_or_default()
}

fn value_to_id(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
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
    async fn home_page_uses_module_title_and_paged_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .and(query_param("deviceType", "BROWSER"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"modules": [
                        {"type": "PLAYLIST_LIST", "title": "The Hits", "pagedList": {"items": [
                            {"uuid": "abc-123", "title": "Top Hits", "description": "desc", "type": "EDITORIAL"}
                        ]}}
                    ]},
                    {"modules": [{"type": "TRACK_LIST", "title": "New Tracks", "items": []}]}
                ]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "The Hits");
        assert_eq!(sections[0].items[0].id, "abc-123");
        assert_eq!(sections[0].items[0].subtitle.as_deref(), Some("desc"));
        assert_eq!(sections[0].items[0].item_type, "EDITORIAL");
    }

    #[tokio::test]
    async fn home_page_numeric_id_and_module_type_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"modules": [
                        {"type": "TRACK_LIST", "title": "New Tracks", "pagedList": {"items": [
                            {"id": 561714507, "title": "Solaire"}
                        ]}}
                    ]}
                ]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections[0].items[0].id, "561714507");
        assert_eq!(sections[0].items[0].item_type, "TRACK_LIST");
    }

    #[tokio::test]
    async fn home_page_with_no_rows_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "rows": [] })))
            .mount(&server).await;
        assert!(client(&server).home_page().await.unwrap().is_empty());
    }

    /// Review Focus: home items carry artwork under different fields per
    /// type; the mapping must pick the first available in precedence order.
    #[tokio::test]
    async fn home_item_artwork_uses_first_available_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIXED", "title": "Mix", "pagedList": {"items": [
                    {"id": 1, "title": "Album", "cover": "c-1"},
                    {"uuid": "p-2", "title": "Playlist", "squareImage": "sq-2", "image": "im-2"},
                    {"id": 3, "title": "Artist", "picture": "pic-3"},
                    {"id": 4, "title": "Nothing"}
                ]}}]}]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        let items = &sections[0].items;
        assert_eq!(items[0].artwork.as_deref(), Some("c-1"));
        assert_eq!(items[1].artwork.as_deref(), Some("sq-2"));
        assert_eq!(items[2].artwork.as_deref(), Some("pic-3"));
        assert_eq!(items[3].artwork, None);
    }

    /// Real `TRACK_LIST` items carry no top-level art fields; a track's cover
    /// lives under its nested `album.cover`. A top-level field still wins.
    #[tokio::test]
    async fn home_item_artwork_reads_nested_album_cover_for_tracks() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "TRACK_LIST", "title": "New Tracks", "pagedList": {"items": [
                    {"id": 564082204, "title": "Patient Zero", "album": {"id": 564082191, "cover": "39e3dad3-33fb-47f2-87f8-8b6f8c55734c"}},
                    {"id": 562261670, "title": "Fireflies", "album": {"id": 562261665, "cover": null}},
                    {"id": 5, "title": "Top Level Wins", "cover": "c-5", "album": {"id": 6, "cover": "a-5"}}
                ]}}]}]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        let items = &sections[0].items;
        assert_eq!(items[0].artwork.as_deref(), Some("39e3dad3-33fb-47f2-87f8-8b6f8c55734c"));
        assert_eq!(items[1].artwork, None);
        assert_eq!(items[2].artwork.as_deref(), Some("c-5"));
    }

    #[tokio::test]
    async fn explore_page_reads_the_explore_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/explore"))
            .and(query_param("deviceType", "BROWSER"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "PLAYLIST_LIST", "title": "For You", "items": [
                    {"uuid": "p-1", "title": "Mix"}
                ]}]}]
            })))
            .mount(&server).await;
        let sections = client(&server).explore_page().await.unwrap();
        assert_eq!(sections[0].title, "For You");
        assert_eq!(sections[0].items[0].id, "p-1");
    }

    #[tokio::test]
    async fn feed_page_falls_back_when_the_first_candidate_is_missing() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/feed"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/pages/my_collection_feed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "ALBUM_LIST", "title": "New Releases", "items": [
                    {"id": 9, "title": "Album", "cover": "c-9"}
                ]}]}]
            })))
            .mount(&server).await;
        let sections = client(&server).feed_page().await.unwrap();
        assert_eq!(sections[0].title, "New Releases");
        assert_eq!(sections[0].items[0].item_type, "ALBUM_LIST");
    }

    #[tokio::test]
    async fn feed_page_is_empty_when_every_candidate_is_missing() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/feed"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        Mock::given(method("GET")).and(path("/pages/my_collection_feed"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        assert!(client(&server).feed_page().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn feed_page_propagates_non_404_failures() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/feed"))
            .respond_with(ResponseTemplate::new(500)).mount(&server).await;
        assert!(client(&server).feed_page().await.is_err());
    }
}
