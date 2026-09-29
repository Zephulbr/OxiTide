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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixDetail {
    pub id: String,
    pub title: String,
    /// Best-effort header artwork (absolute CDN url); `None` when absent.
    #[serde(default)]
    pub artwork: Option<String>,
    pub tracks: Vec<crate::models::Track>,
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
    /// TIDAL spells mix subtitles `subTitle` (capital T).
    #[serde(default, alias = "subTitle")]
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
    /// Page links (editorial rows) carry their artwork here.
    #[serde(default, rename = "imageId")]
    pub image_id: Option<String>,
    /// Mix artwork: absolute CDN urls under `images.{SMALL,MEDIUM,LARGE}.url`.
    #[serde(default)]
    pub images: Option<RawImages>,
    #[serde(default)]
    pub album: Option<RawAlbum>,
    /// `MIXED_TYPES_LIST` entries wrap the payload as `{type, item}`.
    #[serde(default)]
    pub item: Option<Box<RawItem>>,
}

#[derive(Debug, Deserialize)]
struct RawImages {
    #[serde(default, rename = "SMALL")]
    pub small: Option<RawImageUrl>,
    #[serde(default, rename = "MEDIUM")]
    pub medium: Option<RawImageUrl>,
    #[serde(default, rename = "LARGE")]
    pub large: Option<RawImageUrl>,
}

#[derive(Debug, Deserialize)]
struct RawImageUrl {
    #[serde(default)]
    pub url: Option<String>,
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

    /// The discovery mix feed; same rows/modules shape as home and explore.
    pub async fn for_you_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/for_you", &[]).await
    }

    /// The signed-in user's mixes & radio, as a page.
    pub async fn mixes_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/my_collection_my_mixes", &[])
            .await
    }

    /// Resolve a mix id into its playable track list. Videos are dropped because
    /// the engine is audio-only; `pages/mix` returns tracks and videos together.
    pub async fn mix_page(&self, mix_id: &str) -> Result<MixDetail> {
        let params = vec![
            ("mixId", mix_id.to_string()),
            ("deviceType", "BROWSER".to_string()),
            ("platform", "WEB".to_string()),
        ];
        let raw: serde_json::Value = self.get("pages/mix", &params).await?;
        Ok(mix_detail_from_json(mix_id, &raw))
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
            let items: Vec<PageItem> = raw_items
                .into_iter()
                .map(|i| page_item_from_raw(&module.module_type, i))
                .collect();
            sections.push(PageSection {
                title: module.title,
                items,
            });
        }
    }
    sections
}

/// Map a raw page entry into a `PageItem`. `module_type` is the fallback kind
/// when the entry carries no `type` of its own. `MIXED_TYPES_LIST` entries wrap
/// the payload as `{type, item}`; the wrapper type is the discriminator and is
/// preserved when the inner object has none.
fn page_item_from_raw(module_type: &str, mut raw: RawItem) -> PageItem {
    if let Some(inner) = raw.item.take() {
        let wrapper_type = std::mem::take(&mut raw.item_type);
        raw = *inner;
        if raw.item_type.is_empty() {
            raw.item_type = wrapper_type;
        }
    }
    let id = item_id(&raw);
    let item_type = if raw.item_type.is_empty() {
        module_type.to_string()
    } else {
        raw.item_type
    };
    PageItem {
        id,
        title: raw.title,
        subtitle: raw.subtitle.or(raw.description),
        item_type,
        artwork: raw
            .cover
            .or(raw.square_image)
            .or(raw.image)
            .or(raw.picture)
            .or_else(|| raw.album.and_then(|album| album.cover))
            .or(raw.image_id)
            .or_else(|| {
                raw.images
                    .and_then(|images| images.medium.or(images.large).or(images.small))
                    .and_then(|image| image.url)
            }),
    }
}

/// Best-effort mapping of a foreign item shape into a `PageItem`; accepts
/// `{item: …}` wrappers (favorites/purchases) and bare objects, ids or uuids.
pub fn page_item_from_value(value: &serde_json::Value) -> Option<PageItem> {
    let item = value.get("item").unwrap_or(value);
    let id = item
        .get("id")
        .map(value_to_id)
        .or_else(|| {
            item.get("uuid")
                .and_then(|u| u.as_str())
                .map(str::to_string)
        })
        .filter(|id| !id.is_empty())?;
    let title = item
        .get("title")
        .and_then(serde_json::Value::as_str)
        .or_else(|| item.get("name").and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .to_string();
    let item_type = item
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let artwork = ["cover", "squareImage", "image", "picture"]
        .iter()
        .find_map(|key| item.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string);
    Some(PageItem {
        id,
        title,
        subtitle: None,
        item_type,
        artwork,
    })
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

fn mix_detail_from_json(mix_id: &str, raw: &serde_json::Value) -> MixDetail {
    let title = raw
        .pointer("/rows/0/modules/0/mix/title")
        .or_else(|| raw.pointer("/rows/0/modules/0/title"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut tracks = Vec::new();
    if let Some(rows) = raw.get("rows").and_then(serde_json::Value::as_array) {
        for row in rows {
            let Some(modules) = row.get("modules").and_then(serde_json::Value::as_array) else {
                continue;
            };
            for module in modules {
                let module_type = module
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let module_is_track = module_type.to_uppercase().contains("TRACK");
                let mut items: Vec<&serde_json::Value> = module
                    .get("items")
                    .and_then(serde_json::Value::as_array)
                    .map(|values| values.iter().collect())
                    .unwrap_or_default();
                if let Some(paged) = module
                    .pointer("/pagedList/items")
                    .and_then(serde_json::Value::as_array)
                {
                    items.extend(paged.iter());
                }
                for item in items {
                    let item_type = item
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    let is_track = if item_type.is_empty() {
                        module_is_track
                    } else {
                        item_type.to_uppercase().contains("TRACK")
                    };
                    if !is_track {
                        continue;
                    }
                    if let Ok(track) = serde_json::from_value::<crate::models::Track>(item.clone())
                    {
                        tracks.push(track);
                    }
                }
            }
        }
    }
    MixDetail {
        id: mix_id.to_string(),
        title,
        artwork: header_artwork(raw),
        tracks,
    }
}

/// Mix headers carry artwork as absolute urls under `images.*.url`; the same
/// shape library mixes use.
fn header_artwork(raw: &serde_json::Value) -> Option<String> {
    let mix = raw.pointer("/rows/0/modules/0/mix")?;
    mix.pointer("/images/MEDIUM/url")
        .or_else(|| mix.pointer("/images/LARGE/url"))
        .or_else(|| mix.pointer("/images/SMALL/url"))
        .or_else(|| mix.get("squareImage"))
        .or_else(|| mix.get("image"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
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
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"modules": [
                        {"type": "TRACK_LIST", "title": "New Tracks", "pagedList": {"items": [
                            {"id": 561714507, "title": "Solaire"}
                        ]}}
                    ]}
                ]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections[0].items[0].id, "561714507");
        assert_eq!(sections[0].items[0].item_type, "TRACK_LIST");
    }

    #[tokio::test]
    async fn home_page_with_no_rows_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "rows": [] })),
            )
            .mount(&server)
            .await;
        assert!(client(&server).home_page().await.unwrap().is_empty());
    }

    /// Review Focus: home items carry artwork under different fields per
    /// type; the mapping must pick the first available in precedence order.
    #[tokio::test]
    async fn home_item_artwork_uses_first_available_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIXED", "title": "Mix", "pagedList": {"items": [
                    {"id": 1, "title": "Album", "cover": "c-1"},
                    {"uuid": "p-2", "title": "Playlist", "squareImage": "sq-2", "image": "im-2"},
                    {"id": 3, "title": "Artist", "picture": "pic-3"},
                    {"id": 4, "title": "Nothing"}
                ]}}]}]
            })))
            .mount(&server)
            .await;
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
        assert_eq!(
            items[0].artwork.as_deref(),
            Some("39e3dad3-33fb-47f2-87f8-8b6f8c55734c")
        );
        assert_eq!(items[1].artwork, None);
        assert_eq!(items[2].artwork.as_deref(), Some("c-5"));
    }

    #[tokio::test]
    async fn explore_page_reads_the_explore_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/explore"))
            .and(query_param("deviceType", "BROWSER"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "PLAYLIST_LIST", "title": "For You", "items": [
                    {"uuid": "p-1", "title": "Mix"}
                ]}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).explore_page().await.unwrap();
        assert_eq!(sections[0].title, "For You");
        assert_eq!(sections[0].items[0].id, "p-1");
    }

    #[tokio::test]
    async fn for_you_page_reads_the_for_you_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/for_you"))
            .and(query_param("deviceType", "BROWSER"))
            .and(query_param("platform", "WEB"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIX_LIST", "title": "Made for you",
                    "pagedList": {"items": [
                        {"id": "mix-1", "title": "Daily Mix 1", "subTitle": "Daily Mix"}
                    ]}}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).for_you_page().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Made for you");
        assert_eq!(sections[0].items[0].id, "mix-1");
        assert_eq!(sections[0].items[0].subtitle.as_deref(), Some("Daily Mix"));
    }

    #[tokio::test]
    async fn for_you_page_with_no_rows_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/for_you"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "rows": [] })),
            )
            .mount(&server)
            .await;
        assert!(client(&server).for_you_page().await.unwrap().is_empty());
    }

    /// Mix rows carry their cover as `images.MEDIUM.url` (a full CDN url, not a
    /// dashed uuid) and their subtitle under `subTitle` (capital T).
    #[tokio::test]
    async fn home_mix_items_map_images_and_sub_title() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIX_LIST", "title": "Custom mixes", "pagedList": {"items": [
                    {"id": "mix-1", "title": "My Daily Discovery", "subTitle": "Daily Mix",
                     "images": {
                        "SMALL": {"url": "https://img/small.jpg"},
                        "MEDIUM": {"url": "https://img/medium.jpg"},
                        "LARGE": {"url": "https://img/large.jpg"}
                     }}
                ]}}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).home_page().await.unwrap();
        let item = &sections[0].items[0];
        assert_eq!(item.subtitle.as_deref(), Some("Daily Mix"));
        assert_eq!(item.artwork.as_deref(), Some("https://img/medium.jpg"));
    }

    /// `MIXED_TYPES_LIST` entries wrap the payload as `{type, item}`; the
    /// wrapper type is the item's discriminator and the inner object holds the
    /// id/title/artwork.
    #[tokio::test]
    async fn home_wrapped_items_are_unwrapped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIXED_TYPES_LIST", "title": "New updates", "pagedList": {"items": [
                    {"type": "ALBUM", "item": {"id": 7, "title": "Release", "cover": "c-7"}}
                ]}}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).home_page().await.unwrap();
        let item = &sections[0].items[0];
        assert_eq!(item.id, "7");
        assert_eq!(item.title, "Release");
        assert_eq!(item.artwork.as_deref(), Some("c-7"));
        assert_eq!(item.item_type, "ALBUM");
    }

    /// `PAGE_LINKS`-style rows carry their artwork in `imageId`.
    #[tokio::test]
    async fn home_page_link_items_use_image_id_artwork() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "PAGE_LINKS_CLOUD", "title": "Explore", "pagedList": {"items": [
                    {"title": "TIDAL Rising", "imageId": "img-1", "apiPath": "pages/rising"}
                ]}}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections[0].items[0].artwork.as_deref(), Some("img-1"));
    }

    #[tokio::test]
    async fn mix_page_keeps_tracks_and_drops_videos() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/mix"))
            .and(query_param("mixId", "mix-7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"modules": [{"type": "MIX_HEADER", "mix": {"id": "mix-7", "title": "Daily Mix"}}]},
                    {"modules": [
                        {"type": "TRACK_LIST", "title": "Tracks", "pagedList": {"items": [
                            {"id": 1, "title": "Song", "duration": 200, "artists": [{"id": 5, "name": "A"}]}
                        ]}},
                        {"type": "VIDEO_LIST", "title": "Videos", "items": [
                            {"id": 99, "title": "Clip", "type": "VIDEO"}
                        ]}
                    ]}
                ]
            })))
            .mount(&server).await;
        let detail = client(&server).mix_page("mix-7").await.unwrap();
        assert_eq!(detail.title, "Daily Mix");
        assert_eq!(detail.tracks.len(), 1);
        assert_eq!(detail.tracks[0].id, 1);
        assert_eq!(detail.tracks[0].duration, 200);
    }

    #[tokio::test]
    async fn mix_page_with_only_videos_has_no_tracks() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/mix"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"modules": [{"type": "MIX_HEADER", "mix": {"id": "m", "title": "Video Mix"}}]},
                    {"modules": [{"type": "VIDEO_LIST", "items": [{"id": 4, "title": "Clip"}]}]}
                ]
            })))
            .mount(&server)
            .await;
        let detail = client(&server).mix_page("m").await.unwrap();
        assert!(detail.tracks.is_empty());
        assert_eq!(detail.title, "Video Mix");
    }

    #[tokio::test]
    async fn mix_page_reads_header_artwork() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/mix"))
            .and(query_param("mixId", "mix-7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIX_HEADER", "mix": {
                    "id": "mix-7", "title": "Daily Mix",
                    "images": {
                        "SMALL": {"url": "https://img/s.jpg"},
                        "MEDIUM": {"url": "https://img/m.jpg"}
                    }
                }}]}]
            })))
            .mount(&server)
            .await;
        let detail = client(&server).mix_page("mix-7").await.unwrap();
        assert_eq!(detail.artwork.as_deref(), Some("https://img/m.jpg"));
    }

    #[tokio::test]
    async fn mix_page_without_images_has_no_artwork() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/mix"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIX_HEADER", "mix": {"id": "m", "title": "T"}}]}]
            })))
            .mount(&server)
            .await;
        assert!(client(&server).mix_page("m").await.unwrap().artwork.is_none());
    }
}
