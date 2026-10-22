//! TIDAL's Save for Later list (public OpenAPI `userCollectionSaveForLaters`).
//! The list holds `(type, id, addedAt)` references only; pages are hydrated
//! into the v1 models the UI already renders. Mixes are stored as `playlists`
//! references carrying the mix id (the OpenAPI playlist reads back
//! `playlistType: "MIX"`), so the id's shape tells them apart.

use futures_util::stream::{self, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::client::{TidalClient, PAGE_CONCURRENCY};
use crate::error::{Error, Result};
use crate::models::{Album, Artist, Mix, Playlist, Track};

const ITEMS_PATH: &str = "userCollectionSaveForLaters/me/relationships/items";
/// The add/remove endpoints take at most 20 references per request.
const WRITE_CHUNK: usize = 20;
/// Bounds `later_ids` so a broken cursor can never loop forever.
const MAX_PAGES: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaterKind {
    Track,
    Album,
    Artist,
    Playlist,
    Mix,
    Video,
}

impl LaterKind {
    /// The JSON:API resource type. Mixes travel as `playlists`.
    pub fn api_type(self) -> &'static str {
        match self {
            LaterKind::Track => "tracks",
            LaterKind::Album => "albums",
            LaterKind::Artist => "artists",
            LaterKind::Playlist | LaterKind::Mix => "playlists",
            LaterKind::Video => "videos",
        }
    }

    /// The kind of a stored reference. A `playlists` reference whose id is
    /// not a playlist UUID is a mix (mix ids are bare hex).
    fn from_ref(api_type: &str, id: &str) -> Option<Self> {
        Some(match api_type {
            "tracks" => LaterKind::Track,
            "albums" => LaterKind::Album,
            "artists" => LaterKind::Artist,
            "playlists" if is_uuid(id) => LaterKind::Playlist,
            "playlists" => LaterKind::Mix,
            "videos" => LaterKind::Video,
            _ => return None,
        })
    }
}

/// A dashed UUID (`8-4-4-4-12`), the shape of every user and editorial
/// playlist id.
fn is_uuid(id: &str) -> bool {
    let groups: Vec<&str> = id.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(group, len)| group.len() == len && group.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// One saved reference, newest first in list order.
#[derive(Debug, Clone, PartialEq)]
pub struct LaterRef {
    pub kind: LaterKind,
    pub id: String,
    pub added_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LaterPage {
    pub items: Vec<LaterRef>,
    pub cursor: Option<String>,
}

/// Every saved id, by kind. Videos are only counted (OxiTide plays none).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct LaterIds {
    pub tracks: Vec<u64>,
    pub albums: Vec<u64>,
    pub artists: Vec<u64>,
    pub playlists: Vec<String>,
    pub mixes: Vec<String>,
    pub videos: u32,
}

/// A hydrated saved item: `{"kind": "track", "item": {â€¦}}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "item", rename_all = "lowercase")]
pub enum LaterItem {
    Track(Track),
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
    Mix(Mix),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LaterEntry {
    #[serde(flatten)]
    pub item: LaterItem,
    pub added_at: Option<String>,
}

/// What an add skipped: `(kind, id, reason)`, reason `ALREADY_PRESENT` or
/// `NOT_FOUND`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LaterWrite {
    pub skipped: Vec<(LaterKind, String, String)>,
}

#[derive(Deserialize)]
struct RawDoc {
    #[serde(default)]
    data: Vec<RawRef>,
    #[serde(default)]
    links: Option<RawLinks>,
    #[serde(default)]
    meta: Option<RawMeta>,
}

#[derive(Deserialize)]
struct RawRef {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    meta: Option<RawRefMeta>,
}

#[derive(Deserialize)]
struct RawRefMeta {
    #[serde(rename = "addedAt", default)]
    added_at: Option<String>,
}

#[derive(Deserialize)]
struct RawLinks {
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    meta: Option<RawLinksMeta>,
}

#[derive(Deserialize)]
struct RawLinksMeta {
    #[serde(rename = "nextCursor", default)]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct RawMeta {
    #[serde(default)]
    skipped: Vec<RawSkipped>,
}

#[derive(Deserialize)]
struct RawSkipped {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    reason: String,
}

/// The cursor for the next page: `links.meta.nextCursor`, else the
/// `page[cursor]` query value of `links.next`.
fn next_cursor(links: Option<RawLinks>) -> Option<String> {
    let links = links?;
    if let Some(cursor) = links.meta.and_then(|meta| meta.next_cursor) {
        if !cursor.is_empty() {
            return Some(cursor);
        }
    }
    let next = links.next?;
    let query = next.split_once('?')?.1;
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == "page[cursor]")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
}

fn write_doc(items: &[(LaterKind, String)]) -> serde_json::Value {
    let data: Vec<_> = items
        .iter()
        .map(|(kind, id)| serde_json::json!({"type": kind.api_type(), "id": id}))
        .collect();
    serde_json::json!({ "data": data })
}

impl TidalClient {
    fn later_params(&self) -> Vec<(&'static str, String)> {
        vec![("countryCode", self.session().country_code.clone())]
    }

    /// One page of saved references, newest first. Unknown types are dropped.
    pub async fn later_page(&self, cursor: Option<&str>) -> Result<LaterPage> {
        let mut params = self.later_params();
        if let Some(value) = cursor {
            params.push(("page[cursor]", value.to_string()));
        }
        let raw: RawDoc = self.get_openapi(ITEMS_PATH, &params).await?;
        let items = raw
            .data
            .into_iter()
            .filter_map(|item| {
                Some(LaterRef {
                    kind: LaterKind::from_ref(&item.kind, &item.id)?,
                    id: item.id,
                    added_at: item.meta.and_then(|meta| meta.added_at),
                })
            })
            .collect();
        Ok(LaterPage {
            items,
            cursor: next_cursor(raw.links),
        })
    }

    /// Every saved reference, following the cursor (at most `MAX_PAGES`).
    pub async fn later_refs(&self) -> Result<Vec<LaterRef>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let page = self.later_page(cursor.as_deref()).await?;
            let empty = page.items.is_empty();
            all.extend(page.items);
            match page.cursor {
                Some(next) if !empty && Some(&next) != cursor.as_ref() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(all)
    }

    /// Every saved id, by kind.
    pub async fn later_ids(&self) -> Result<LaterIds> {
        let mut ids = LaterIds::default();
        for item in self.later_refs().await? {
            match item.kind {
                LaterKind::Track => ids.tracks.extend(item.id.parse::<u64>().ok()),
                LaterKind::Album => ids.albums.extend(item.id.parse::<u64>().ok()),
                LaterKind::Artist => ids.artists.extend(item.id.parse::<u64>().ok()),
                LaterKind::Playlist => ids.playlists.push(item.id),
                LaterKind::Mix => ids.mixes.push(item.id),
                LaterKind::Video => ids.videos += 1,
            }
        }
        Ok(ids)
    }

    /// Save references, 20 per request. Items TIDAL skips are reported, not
    /// failed.
    pub async fn later_add(&self, items: &[(LaterKind, String)]) -> Result<LaterWrite> {
        let mut write = LaterWrite::default();
        for chunk in items.chunks(WRITE_CHUNK) {
            let raw: RawDoc = self
                .post_openapi(ITEMS_PATH, &self.later_params(), &write_doc(chunk))
                .await?;
            for skipped in raw.meta.map(|meta| meta.skipped).unwrap_or_default() {
                if let Some(kind) = LaterKind::from_ref(&skipped.kind, &skipped.id) {
                    write.skipped.push((kind, skipped.id, skipped.reason));
                }
            }
        }
        Ok(write)
    }

    /// Remove references, 20 per request.
    pub async fn later_remove(&self, items: &[(LaterKind, String)]) -> Result<()> {
        for chunk in items.chunks(WRITE_CHUNK) {
            self.delete_openapi(ITEMS_PATH, &self.later_params(), Some(&write_doc(chunk)))
                .await?;
        }
        Ok(())
    }

    /// Fill references in with their v1 models, keeping order. Videos and
    /// items that no longer load (removed, unavailable here) are dropped; a
    /// transient failure fails the page so it can be retried.
    pub async fn later_entries(&self, refs: Vec<LaterRef>) -> Result<Vec<LaterEntry>> {
        let entries: Vec<Option<LaterEntry>> = stream::iter(refs)
            .map(|item| async move {
                match self.later_item(item.kind, &item.id).await {
                    Ok(Some(found)) => Ok(Some(LaterEntry {
                        item: found,
                        added_at: item.added_at,
                    })),
                    Ok(None) => Ok(None),
                    Err(e) if e.is_transient() => Err(e),
                    Err(e) => {
                        tracing::debug!(
                            "save for later: skipping {:?} {}: {e}",
                            item.kind,
                            item.id
                        );
                        Ok(None)
                    }
                }
            })
            .buffered(PAGE_CONCURRENCY)
            .try_collect()
            .await?;
        Ok(entries.into_iter().flatten().collect())
    }

    async fn later_item(&self, kind: LaterKind, id: &str) -> Result<Option<LaterItem>> {
        let numeric = || {
            id.parse::<u64>()
                .map_err(|_| Error::NotFound(id.to_string()))
        };
        Ok(Some(match kind {
            LaterKind::Track => LaterItem::Track(self.track(numeric()?).await?),
            LaterKind::Album => LaterItem::Album(self.album(numeric()?).await?),
            LaterKind::Artist => LaterItem::Artist(self.artist(numeric()?).await?),
            LaterKind::Playlist => LaterItem::Playlist(self.playlist(id).await?),
            LaterKind::Mix => LaterItem::Mix(self.mix_info(id).await?),
            LaterKind::Video => return Ok(None),
        }))
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

    const ITEMS: &str = "/openapi/v2/userCollectionSaveForLaters/me/relationships/items";

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo {
                session_id: "s".into(),
                country_code: "GR".into(),
                user_id: 42,
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
    async fn later_page_parses_refs_and_the_meta_cursor() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(ITEMS))
            .and(query_param("countryCode", "GR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "7", "type": "tracks", "meta": {"addedAt": "2026-10-15T07:45:45Z"}},
                    {"id": "0b6c3f4e-1d2a-4b5c-8d9e-0f1a2b3c4d5e", "type": "playlists", "meta": {"addedAt": "2026-10-14T00:00:00Z"}},
                    {"id": "x", "type": "somethingNew"}
                ],
                "links": {"self": "/x", "next": "/x?page[cursor]=ignored", "meta": {"nextCursor": "c2"}}
            })))
            .mount(&server)
            .await;
        let page = client(&server).later_page(None).await.unwrap();
        assert_eq!(page.items.len(), 2, "unknown types are dropped");
        assert_eq!(page.items[0].kind, LaterKind::Track);
        assert_eq!(
            page.items[0].added_at.as_deref(),
            Some("2026-10-15T07:45:45Z")
        );
        assert_eq!(page.items[1].id, "0b6c3f4e-1d2a-4b5c-8d9e-0f1a2b3c4d5e");
        assert_eq!(page.cursor.as_deref(), Some("c2"));
    }

    #[test]
    fn next_cursor_falls_back_to_the_next_link() {
        let links: RawLinks = serde_json::from_value(serde_json::json!({
            "self": "/x", "next": "/userCollectionSaveForLaters/me/relationships/items?countryCode=GR&page%5Bcursor%5D=abc"
        }))
        .unwrap();
        assert_eq!(next_cursor(Some(links)).as_deref(), Some("abc"));
        let none: RawLinks = serde_json::from_value(serde_json::json!({"self": "/x"})).unwrap();
        assert_eq!(next_cursor(Some(none)), None);
    }

    #[tokio::test]
    async fn later_ids_walks_pages_and_groups_by_kind() {
        let server = MockServer::start().await;
        let responder = |req: &wiremock::Request| {
            let second = req
                .url
                .query_pairs()
                .any(|(k, v)| k == "page[cursor]" && v == "c2");
            if second {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": [
                        {"id": "9", "type": "artists"},
                        {"id": "0019899a0394b695de9b4d526786f3", "type": "playlists"},
                        {"id": "v-1", "type": "videos"}
                    ],
                    "links": {"self": "/x"}
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": [
                        {"id": "7", "type": "tracks"},
                        {"id": "8", "type": "albums"},
                        {"id": "0b6c3f4e-1d2a-4b5c-8d9e-0f1a2b3c4d5e", "type": "playlists"}
                    ],
                    "links": {"self": "/x", "meta": {"nextCursor": "c2"}}
                }))
            }
        };
        Mock::given(method("GET"))
            .and(path(ITEMS))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
        let ids = client(&server).later_ids().await.unwrap();
        assert_eq!(ids.tracks, vec![7]);
        assert_eq!(ids.albums, vec![8]);
        assert_eq!(ids.artists, vec![9]);
        assert_eq!(
            ids.playlists,
            vec!["0b6c3f4e-1d2a-4b5c-8d9e-0f1a2b3c4d5e".to_string()]
        );
        assert_eq!(
            ids.mixes,
            vec!["0019899a0394b695de9b4d526786f3".to_string()]
        );
        assert_eq!(ids.videos, 1);
    }

    #[test]
    fn playlist_refs_without_a_uuid_are_mixes() {
        let uuid = "0b6c3f4e-1d2a-4b5c-8d9e-0f1a2b3c4d5e";
        assert_eq!(
            LaterKind::from_ref("playlists", uuid),
            Some(LaterKind::Playlist)
        );
        assert_eq!(
            LaterKind::from_ref("playlists", "0019899a0394b695de9b4d526786f3"),
            Some(LaterKind::Mix)
        );
        assert_eq!(
            LaterKind::from_ref("playlists", "0b6c3f4e-1d2a-4b5c-8d9e"),
            Some(LaterKind::Mix)
        );
        assert_eq!(LaterKind::Mix.api_type(), "playlists");
        assert_eq!(LaterKind::from_ref("mixes", "x"), None);
    }

    #[tokio::test]
    async fn later_refs_stops_on_a_repeated_cursor() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(ITEMS))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "7", "type": "tracks"}],
                "links": {"self": "/x", "meta": {"nextCursor": "same"}}
            })))
            .expect(2)
            .mount(&server)
            .await;
        let refs = client(&server).later_refs().await.unwrap();
        assert_eq!(refs.len(), 2);
    }

    #[tokio::test]
    async fn later_add_chunks_by_20_and_reports_skips() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(ITEMS))
            .and(query_param("countryCode", "GR"))
            .and(body_string_contains("\"type\":\"tracks\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [],
                "links": {"self": "/x"},
                "meta": {"skipped": [{"id": "1", "type": "tracks", "reason": "ALREADY_PRESENT"}]}
            })))
            .expect(2)
            .mount(&server)
            .await;
        let items: Vec<_> = (1..=21)
            .map(|i| (LaterKind::Track, i.to_string()))
            .collect();
        let write = client(&server).later_add(&items).await.unwrap();
        assert_eq!(write.skipped.len(), 2, "one skip echoed per chunk");
        assert_eq!(
            write.skipped[0],
            (
                LaterKind::Track,
                "1".to_string(),
                "ALREADY_PRESENT".to_string()
            )
        );
    }

    #[tokio::test]
    async fn later_add_maps_a_full_collection_to_an_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(ITEMS))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "errors": [{"code": "TOO_MANY_ITEMS_IN_COLLECTION", "status": "409"}]
            })))
            .mount(&server)
            .await;
        let err = client(&server)
            .later_add(&[(LaterKind::Album, "8".to_string())])
            .await
            .unwrap_err();
        match err {
            Error::Api {
                status: 409,
                message,
            } => {
                assert!(message.contains("TOO_MANY_ITEMS_IN_COLLECTION"))
            }
            other => panic!("expected a 409 api error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn later_remove_deletes_with_a_body() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path(ITEMS))
            .and(body_string_contains("\"type\":\"artists\""))
            .and(body_string_contains("\"id\":\"9\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;
        client(&server)
            .later_remove(&[(LaterKind::Artist, "9".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn later_entries_hydrates_in_order_and_drops_what_no_longer_loads() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 7, "title": "Seven", "duration": 100, "artists": []
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/albums/8"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/artists/9"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 9, "name": "Nine"
            })))
            .mount(&server)
            .await;
        let refs = vec![
            LaterRef {
                kind: LaterKind::Artist,
                id: "9".into(),
                added_at: Some("b".into()),
            },
            LaterRef {
                kind: LaterKind::Album,
                id: "8".into(),
                added_at: None,
            },
            LaterRef {
                kind: LaterKind::Video,
                id: "v".into(),
                added_at: None,
            },
            LaterRef {
                kind: LaterKind::Track,
                id: "7".into(),
                added_at: Some("a".into()),
            },
        ];
        let entries = client(&server).later_entries(refs).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert!(matches!(&entries[0].item, LaterItem::Artist(a) if a.id == 9));
        assert!(matches!(&entries[1].item, LaterItem::Track(t) if t.id == 7));
        let json = serde_json::to_value(&entries[1]).unwrap();
        assert_eq!(json["kind"], "track");
        assert_eq!(json["item"]["id"], 7);
        assert_eq!(json["added_at"], "a");
    }

    #[tokio::test]
    async fn later_entries_hydrates_a_mix_from_its_page_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/mix"))
            .and(query_param("mixId", "0019ab"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIX_HEADER", "mix": {
                    "id": "0019ab", "title": "a tombstone", "subTitle": "Kensuke Ushio"
                }}]}]
            })))
            .mount(&server)
            .await;
        let refs = vec![LaterRef {
            kind: LaterKind::Mix,
            id: "0019ab".into(),
            added_at: None,
        }];
        let entries = client(&server).later_entries(refs).await.unwrap();
        let json = serde_json::to_value(&entries[0]).unwrap();
        assert_eq!(json["kind"], "mix");
        assert_eq!(json["item"]["id"], "0019ab");
        assert_eq!(json["item"]["title"], "a tombstone");
        assert_eq!(json["item"]["subTitle"], "Kensuke Ushio");
    }

    #[tokio::test]
    async fn later_entries_fails_the_page_on_a_transient_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let refs = vec![LaterRef {
            kind: LaterKind::Track,
            id: "7".into(),
            added_at: None,
        }];
        assert!(client(&server).later_entries(refs).await.is_err());
    }
}
