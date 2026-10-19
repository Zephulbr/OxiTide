use serde::Deserialize;

use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{
    Album, AlbumInfo, Artist, ArtistDiscography, ArtistLink, ArtistProfile, BioSegment,
    CreditContributor, CreditRole, ItemPage, Track, TrackCredits,
};

fn page_params(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl TidalClient {
    pub async fn track(&self, id: u64) -> Result<Track> {
        self.get(&format!("tracks/{id}"), &[]).await
    }

    pub async fn album(&self, id: u64) -> Result<Album> {
        self.get(&format!("albums/{id}"), &[]).await
    }

    pub async fn album_tracks(&self, id: u64, limit: u32, offset: u32) -> Result<Vec<Track>> {
        let page: ItemPage<Track> = self
            .get(&format!("albums/{id}/tracks"), &page_params(limit, offset))
            .await?;
        Ok(page.items)
    }

    /// Fetch every track of an album, walking all pages.
    pub async fn album_tracks_all(&self, id: u64) -> Result<Vec<Track>> {
        self.get_all_pages(&format!("albums/{id}/tracks"), &[], DEFAULT_PAGE_SIZE)
            .await
    }

    /// Album facts from the public OpenAPI; every field is optional and a
    /// missing document body yields `AlbumInfo::default()`.
    pub async fn album_info(&self, id: u64) -> Result<AlbumInfo> {
        let params = [("countryCode", self.session().country_code.clone())];
        let raw: OpenApiAlbumInfo = self.get_openapi(&format!("albums/{id}"), &params).await?;
        Ok(raw
            .data
            .and_then(|data| data.attributes)
            .map(|attributes| AlbumInfo {
                release_date: attributes.release_date,
                copyright: attributes.copyright.and_then(|copyright| copyright.text),
                barcode_id: attributes.barcode_id,
                number_of_volumes: attributes.number_of_volumes,
                number_of_items: attributes.number_of_items,
            })
            .unwrap_or_default())
    }

    /// Release dates for a caller-chosen batch of album ids through one public
    /// OpenAPI `albums?filter[id]=` call. Ids the API omits or that fail to
    /// parse are left out; an album without a date maps to `None`.
    pub async fn album_release_dates(&self, ids: &[u64]) -> Result<Vec<(u64, Option<String>)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut params: Vec<(&str, String)> =
            vec![("countryCode", self.session().country_code.clone())];
        params.extend(ids.iter().map(|id| ("filter[id]", id.to_string())));
        let raw: OpenApiAlbumList = self.get_openapi("albums", &params).await?;
        Ok(raw
            .data
            .into_iter()
            .filter_map(|item| {
                let id = item.id.parse::<u64>().ok()?;
                Some((id, item.attributes.and_then(|a| a.release_date)))
            })
            .collect())
    }

    /// Role-grouped credits from the public v2 track document. An empty
    /// `roles` list means the API exposed no credits for this track.
    pub async fn track_credits(&self, id: u64) -> Result<TrackCredits> {
        let params = [
            ("countryCode", self.session().country_code.clone()),
            ("include", "credits.artist".to_string()),
        ];
        let raw: OpenApiTrackCredits = self.get_openapi(&format!("tracks/{id}"), &params).await?;
        Ok(credits_from_document(raw))
    }

    pub async fn artist(&self, id: u64) -> Result<Artist> {
        self.get(&format!("artists/{id}"), &[]).await
    }

    /// One bounded window of the artist's most popular tracks.
    pub async fn artist_top_tracks(&self, id: u64, limit: u32) -> Result<Vec<Track>> {
        Ok(self.artist_top_tracks_page(id, limit, 0).await?.0)
    }

    /// One page of the artist's most popular tracks plus the server total.
    pub async fn artist_top_tracks_page(
        &self,
        id: u64,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Track>, u32)> {
        let page: ItemPage<Track> = self
            .get(
                &format!("artists/{id}/toptracks"),
                &[
                    ("limit", limit.to_string()),
                    ("offset", offset.to_string()),
                ],
            )
            .await?;
        Ok((page.items, page.total_number_of_items))
    }

    /// One page of an artist's discography under a TIDAL filter
    /// (`ALBUMS` / `EPSANDSINGLES` / `COMPILATIONS`).
    pub async fn artist_albums(
        &self,
        id: u64,
        filter: ArtistDiscography,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Album>> {
        let page: ItemPage<Album> = self
            .get(
                &format!("artists/{id}/albums"),
                &[
                    ("filter", filter.tidal_param().to_string()),
                    ("limit", limit.to_string()),
                    ("offset", offset.to_string()),
                ],
            )
            .await?;
        Ok(page.items)
    }

    /// Every page of an artist's discography under one TIDAL filter.
    pub async fn artist_albums_all(
        &self,
        id: u64,
        filter: ArtistDiscography,
    ) -> Result<Vec<Album>> {
        let params = [("filter", filter.tidal_param().to_string())];
        self.get_all_pages(&format!("artists/{id}/albums"), &params, DEFAULT_PAGE_SIZE)
            .await
    }

    /// Fans, external links, and biography from one public-OpenAPI call.
    /// `TIDAL_SHARING` self-links and empty/duplicate hrefs are dropped;
    /// every field is optional.
    pub async fn artist_profile(&self, id: u64) -> Result<ArtistProfile> {
        let params = [
            ("countryCode", self.session().country_code.clone()),
            ("include", "biography".to_string()),
        ];
        let raw: OpenApiArtistProfile = self
            .get_openapi(&format!("artists/{id}"), &params)
            .await?;

        let attributes = raw.data.and_then(|data| data.attributes);
        let mut seen = std::collections::HashSet::new();
        let external_links = attributes
            .as_ref()
            .map(|attributes| attributes.external_links.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|link| {
                let kind = link
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.kind.as_deref())
                    .unwrap_or("");
                !kind.eq_ignore_ascii_case("TIDAL_SHARING")
            })
            .filter(|link| !link.href.trim().is_empty())
            .filter(|link| seen.insert(link.href.clone()))
            .map(|link| ArtistLink {
                site: link
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.kind.clone())
                    .unwrap_or_default()
                    .to_uppercase(),
                url: link.href.clone(),
            })
            .collect();

        let biography = raw
            .included
            .iter()
            .find(|entry| entry.resource_type == "artistBiographies")
            .and_then(|entry| entry.attributes.as_ref())
            .and_then(|attributes| attributes.text.as_deref());
        let bio = biography.and_then(clean_bio);
        let bio_segments = biography.map(bio_segments).unwrap_or_default();

        Ok(ArtistProfile {
            number_of_followers: attributes.and_then(|attributes| attributes.number_of_followers),
            external_links,
            bio,
            bio_segments,
        })
    }
}

/// Strip TIDAL bio markup: `[wimpLink ...]Name[/wimpLink]` keeps `Name`,
/// `<br>`/`<br/>`/`<br />` become paragraph breaks, and any other tag is
/// dropped. Returns `None` when nothing but markup remains.
pub fn clean_bio(raw: &str) -> Option<String> {
    let text = strip_tags(&strip_line_breaks(&strip_wimp_links(raw)));
    let paragraphs: Vec<&str> = text
        .split('\n')
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .collect();
    if paragraphs.is_empty() {
        None
    } else {
        Some(paragraphs.join("\n\n"))
    }
}

/// Split a raw bio into in-text runs: plain text plus
/// `[wimpLink artistId="N"]` links. Fragments are cleaned like `clean_bio`
/// but the runs are preserved verbatim (inline spacing and `\n` line breaks
/// included); `clean_bio` instead normalizes those runs into paragraphs.
pub fn bio_segments(raw: &str) -> Vec<BioSegment> {
    let mut segments: Vec<BioSegment> = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find("[wimpLink") {
        push_bio_text(&mut segments, clean_fragment(&rest[..start]));
        let Some(open_end) = rest[start..].find(']') else {
            return segments;
        };
        let open_tag = &rest[start..start + open_end + 1];
        let after_open = &rest[start + open_end + 1..];
        let Some(close) = after_open.find("[/wimpLink]") else {
            push_bio_text(&mut segments, clean_fragment(after_open));
            return segments;
        };
        push_bio_segment(
            &mut segments,
            clean_fragment(&after_open[..close]),
            parse_wimp_artist_id(open_tag),
        );
        rest = &after_open[close + "[/wimpLink]".len()..];
    }
    push_bio_text(&mut segments, clean_fragment(rest));
    segments
}

fn clean_fragment(raw: &str) -> String {
    strip_tags(&strip_line_breaks(raw))
}

/// Append a verbatim text run; consecutive runs for the same target merge.
fn push_bio_text(segments: &mut Vec<BioSegment>, text: String) {
    push_bio_segment(segments, text, None);
}

fn push_bio_segment(segments: &mut Vec<BioSegment>, text: String, artist_id: Option<u64>) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = segments.last_mut() {
        if last.artist_id == artist_id {
            last.text.push_str(&text);
            return;
        }
    }
    segments.push(BioSegment { text, artist_id });
}

/// `[wimpLink artistId="13" …]` -> `Some(13)`; absent or malformed ids fall
/// back to plain text.
fn parse_wimp_artist_id(open_tag: &str) -> Option<u64> {
    let start = open_tag.find("artistId=\"")? + "artistId=\"".len();
    let rest = &open_tag[start..];
    let end = rest.find('"')?;
    rest[..end].parse::<u64>().ok()
}

/// `[wimpLink artistId="1"]Name[/wimpLink]` -> `Name`; a malformed open tag
/// drops the remainder rather than leaking raw markup.
fn strip_wimp_links(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("[wimpLink") {
        out.push_str(&rest[..start]);
        let Some(open_end) = rest[start..].find(']') else {
            rest = "";
            break;
        };
        let after_open = &rest[start + open_end + 1..];
        let Some(close) = after_open.find("[/wimpLink]") else {
            out.push_str(after_open);
            rest = "";
            break;
        };
        out.push_str(&after_open[..close]);
        rest = &after_open[close + "[/wimpLink]".len()..];
    }
    out.push_str(rest);
    out
}

/// Case-insensitive `<br>`, `<br/>`, `<br />` -> newline. Operates byte-wise
/// on tag matches only, so non-ASCII text passes through unchanged.
fn strip_line_breaks(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let mut out = String::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        let tail = &lower[index..];
        if tail.starts_with("<br />") {
            out.push('\n');
            index += 6;
        } else if tail.starts_with("<br/>") {
            out.push('\n');
            index += 5;
        } else if tail.starts_with("<br>") {
            out.push('\n');
            index += 4;
        } else {
            let ch = raw[index..].chars().next().expect("char boundary");
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

/// Drop well-formed `<...>` tags, keeping unmatched or malformed `<` as text
/// (a bio like "AC/DC <3 rock" must not lose everything after the `<`).
fn strip_tags(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let tag_like = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '/');
        if !tag_like {
            out.push('<');
            rest = after;
            continue;
        }
        match after.find('>') {
            Some(close) if !after[..close].contains('<') => {
                rest = &after[close + 1..];
            }
            _ => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumInfo {
    #[serde(default)]
    data: Option<OpenApiAlbumInfoData>,
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumInfoData {
    #[serde(default)]
    attributes: Option<OpenApiAlbumInfoAttributes>,
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumInfoAttributes {
    #[serde(default, rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(default)]
    copyright: Option<OpenApiCopyright>,
    #[serde(default, rename = "barcodeId")]
    barcode_id: Option<String>,
    #[serde(default, rename = "numberOfVolumes")]
    number_of_volumes: Option<u32>,
    #[serde(default, rename = "numberOfItems")]
    number_of_items: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumList {
    #[serde(default)]
    data: Vec<OpenApiAlbumListItem>,
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumListItem {
    #[serde(default)]
    id: String,
    #[serde(default)]
    attributes: Option<OpenApiAlbumDate>,
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumDate {
    #[serde(default, rename = "releaseDate")]
    release_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCopyright {
    #[serde(default)]
    text: Option<String>,
}

/// Group included `credits` resources by role, preserving first-seen role and
/// contributor order; the credit's `artist` relationship id becomes
/// `artist_id` when present and parseable. Non-credit or unreferenced
/// `included` entries, blank rows, and duplicate (role, name, artist) rows are
/// dropped.
fn credits_from_document(raw: OpenApiTrackCredits) -> TrackCredits {
    let referenced: std::collections::HashSet<String> = raw
        .data
        .and_then(|data| data.relationships)
        .and_then(|relationships| relationships.credits)
        .map(|credits| credits.data.into_iter().map(|entry| entry.id).collect())
        .unwrap_or_default();
    let mut order: Vec<String> = Vec::new();
    let mut roles: std::collections::HashMap<String, Vec<CreditContributor>> =
        std::collections::HashMap::new();
    let mut seen: std::collections::HashSet<(String, String, Option<u64>)> =
        std::collections::HashSet::new();
    for resource in raw.included {
        if resource.resource_type != "credits" || !referenced.contains(&resource.id) {
            continue;
        }
        let Some(attributes) = resource.attributes else { continue };
        if attributes.name.trim().is_empty() || attributes.role.trim().is_empty() {
            continue;
        }
        let artist_id = resource
            .relationships
            .and_then(|relationships| relationships.artist)
            .and_then(|artist| artist.data)
            .and_then(|data| data.id.parse::<u64>().ok());
        if !seen.insert((attributes.role.clone(), attributes.name.clone(), artist_id)) {
            continue;
        }
        if !roles.contains_key(&attributes.role) {
            order.push(attributes.role.clone());
        }
        roles.entry(attributes.role).or_default().push(CreditContributor {
            name: attributes.name,
            artist_id,
        });
    }
    TrackCredits {
        roles: order
            .into_iter()
            .map(|role| CreditRole {
                contributors: roles.remove(&role).unwrap_or_default(),
                role,
            })
            .collect(),
    }
}

#[derive(Debug, Deserialize)]
struct OpenApiTrackCredits {
    #[serde(default)]
    data: Option<OpenApiTrackCreditsData>,
    #[serde(default)]
    included: Vec<OpenApiCreditResource>,
}

#[derive(Debug, Deserialize)]
struct OpenApiTrackCreditsData {
    #[serde(default)]
    relationships: Option<OpenApiTrackCreditsRelationships>,
}

#[derive(Debug, Deserialize)]
struct OpenApiTrackCreditsRelationships {
    #[serde(default)]
    credits: Option<OpenApiCreditRefs>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditRefs {
    #[serde(default)]
    data: Vec<OpenApiCreditRef>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditRef {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditResource {
    #[serde(default, rename = "type")]
    resource_type: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    attributes: Option<OpenApiCreditAttributes>,
    #[serde(default)]
    relationships: Option<OpenApiCreditRelationships>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditAttributes {
    #[serde(default)]
    name: String,
    #[serde(default)]
    role: String,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditRelationships {
    #[serde(default)]
    artist: Option<OpenApiCreditArtist>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditArtist {
    #[serde(default)]
    data: Option<OpenApiCreditArtistData>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditArtistData {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenApiArtistProfile {
    #[serde(default)]
    data: Option<OpenApiArtistData>,
    #[serde(default)]
    included: Vec<OpenApiIncluded>,
}

#[derive(Debug, Deserialize)]
struct OpenApiArtistData {
    #[serde(default)]
    attributes: Option<OpenApiArtistAttributes>,
}

#[derive(Debug, Deserialize)]
struct OpenApiArtistAttributes {
    #[serde(default, rename = "numberOfFollowers")]
    number_of_followers: Option<u64>,
    #[serde(default, rename = "externalLinks")]
    external_links: Vec<OpenApiExternalLink>,
}

#[derive(Debug, Deserialize)]
struct OpenApiExternalLink {
    #[serde(default)]
    href: String,
    #[serde(default)]
    meta: Option<OpenApiExternalLinkMeta>,
}

#[derive(Debug, Deserialize)]
struct OpenApiExternalLinkMeta {
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenApiIncluded {
    #[serde(default, rename = "type")]
    resource_type: String,
    #[serde(default)]
    attributes: Option<OpenApiBioAttributes>,
}

#[derive(Debug, Deserialize)]
struct OpenApiBioAttributes {
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::error::Error;
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
    async fn album_tracks_sends_limit_and_offset() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/albums/5/tracks"))
            .and(query_param("limit", "50"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 1, "title": "T1", "artists": []}],
                "totalNumberOfItems": 1, "limit": 50, "offset": 100
            })))
            .mount(&server)
            .await;
        let tracks = client(&server).album_tracks(5, 50, 100).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "T1");
    }

    #[tokio::test]
    async fn album_tracks_all_fetches_every_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/albums/9/tracks"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": (1..=100)
                    .map(|id| serde_json::json!({"id": id, "title": "T"}))
                    .collect::<Vec<_>>(),
                "totalNumberOfItems": 101, "limit": 100, "offset": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/albums/9/tracks"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 101, "title": "Last"}],
                "totalNumberOfItems": 101, "limit": 100, "offset": 100
            })))
            .mount(&server)
            .await;
        let tracks = client(&server).album_tracks_all(9).await.unwrap();
        assert_eq!(tracks.len(), 101);
        assert_eq!(tracks[100].title, "Last");
    }

    #[tokio::test]
    async fn album_tracks_handles_empty_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/albums/5/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "totalNumberOfItems": 0, "limit": 50, "offset": 0
            })))
            .mount(&server)
            .await;
        assert!(client(&server)
            .album_tracks(5, 50, 0)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn album_info_reads_facts_and_sends_country_code() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums/9"))
            .and(query_param("countryCode", "GR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "9",
                    "type": "albums",
                    "attributes": {
                        "releaseDate": "2017-06-30",
                        "copyright": { "text": "(C) 2017 S. Carter Enterprises" },
                        "barcodeId": "00881377068540",
                        "numberOfVolumes": 2,
                        "numberOfItems": 24,
                        "title": "4:44"
                    }
                }
            })))
            .mount(&server)
            .await;
        let info = client(&server).album_info(9).await.unwrap();
        assert_eq!(info.release_date.as_deref(), Some("2017-06-30"));
        assert_eq!(info.copyright.as_deref(), Some("(C) 2017 S. Carter Enterprises"));
        assert_eq!(info.barcode_id.as_deref(), Some("00881377068540"));
        assert_eq!(info.number_of_volumes, Some(2));
        assert_eq!(info.number_of_items, Some(24));
    }

    #[tokio::test]
    async fn album_release_dates_batches_ids_and_skips_bad_rows() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums"))
            .and(query_param("countryCode", "GR"))
            .respond_with(|req: &wiremock::Request| {
                let ids: Vec<String> = req
                    .url
                    .query_pairs()
                    .filter(|(key, _)| key == "filter[id]")
                    .map(|(_, value)| value.into_owned())
                    .collect();
                assert_eq!(ids, ["1", "2", "3"]);
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": [
                        {"id": "1", "type": "albums", "attributes": {"releaseDate": "1992-09-21"}},
                        {"id": "2", "type": "albums", "attributes": {}},
                        {"id": "nope", "type": "albums", "attributes": {"releaseDate": "2000-01-01"}}
                    ]
                }))
            })
            .mount(&server)
            .await;
        let dates = client(&server).album_release_dates(&[1, 2, 3]).await.unwrap();
        assert_eq!(dates, vec![(1, Some("1992-09-21".to_string())), (2, None)]);
        assert!(client(&server).album_release_dates(&[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn album_info_missing_attributes_default_to_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums/9"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "id": "9", "type": "albums", "attributes": { "title": "X" } }
            })))
            .mount(&server)
            .await;
        let info = client(&server).album_info(9).await.unwrap();
        assert_eq!(info, AlbumInfo::default());
    }

    #[tokio::test]
    async fn album_info_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums/999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).album_info(999).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn track_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).track(999).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }

    #[test]
    fn clean_bio_keeps_wimp_link_text_and_breaks_paragraphs() {
        let raw = "See [wimpLink artistId=\"1\"]the band[/wimpLink].<br/>Next line.";
        assert_eq!(
            clean_bio(raw).as_deref(),
            Some("See the band.\n\nNext line.")
        );
    }

    #[test]
    fn clean_bio_strips_other_tags_and_empty_input() {
        assert_eq!(clean_bio("<p>Hi</p>"), Some("Hi".into()));
        assert_eq!(clean_bio("   <br/>  "), None);
        assert_eq!(clean_bio(""), None);
    }

    #[test]
    fn clean_bio_handles_album_links_and_variant_breaks() {
        let raw = "A [wimpLink albumId=\"2\"]record[/wimpLink]<br />B<br>B2";
        assert_eq!(clean_bio(raw).as_deref(), Some("A record\n\nB\n\nB2"));
    }

    #[test]
    fn clean_bio_keeps_unmatched_angle_brackets_as_text() {
        assert_eq!(clean_bio("AC/DC <3 rock"), Some("AC/DC <3 rock".into()));
        assert_eq!(clean_bio("a < b <p>x</p>"), Some("a < b x".into()));
    }

    #[test]
    fn bio_segments_plain_text_is_one_run() {
        let segments = bio_segments("Just a bio.");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Just a bio.");
        assert_eq!(segments[0].artist_id, None);
    }

    #[test]
    fn bio_segments_capture_artist_links() {
        let segments =
            bio_segments("See [wimpLink artistId=\"13\"]Band[/wimpLink]. <br/>Then more.");
        let texts: Vec<&str> = segments.iter().map(|segment| segment.text.as_str()).collect();
        assert_eq!(texts, vec!["See ", "Band", ". \nThen more."]);
        let ids: Vec<Option<u64>> = segments.iter().map(|segment| segment.artist_id).collect();
        assert_eq!(ids, vec![None, Some(13), None]);
    }

    #[test]
    fn bio_segments_without_artist_id_stay_text() {
        let segments = bio_segments("[wimpLink foo=\"bar\"]Label[/wimpLink]");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Label");
        assert_eq!(segments[0].artist_id, None);
    }

    #[test]
    fn bio_segments_concat_matches_clean_bio() {
        let raw = "One [wimpLink artistId=\"1\"]two[/wimpLink]<br/>three";
        let concatenated: String = bio_segments(raw).iter().map(|s| s.text.clone()).collect();
        assert_eq!(concatenated, "One two\nthree");
        assert_eq!(clean_bio(raw).as_deref(), Some("One two\n\nthree"));
    }

    #[test]
    fn discography_filter_params_match_tidal() {
        assert_eq!(ArtistDiscography::Albums.tidal_param(), "ALBUMS");
        assert_eq!(ArtistDiscography::EpsAndSingles.tidal_param(), "EPSANDSINGLES");
        assert_eq!(ArtistDiscography::Compilations.tidal_param(), "COMPILATIONS");
    }

    #[tokio::test]
    async fn artist_top_tracks_sends_limit_and_parses_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/toptracks"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 1, "title": "Hit", "artists": []}],
                "totalNumberOfItems": 200, "limit": 10, "offset": 0
            })))
            .mount(&server)
            .await;
        let tracks = client(&server).artist_top_tracks(9, 10).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Hit");
    }

    #[tokio::test]
    async fn artist_top_tracks_page_sends_offset_and_returns_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/toptracks"))
            .and(query_param("limit", "100"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 201, "title": "Later", "artists": []}],
                "totalNumberOfItems": 284, "limit": 100, "offset": 100
            })))
            .mount(&server)
            .await;
        let (tracks, total) = client(&server)
            .artist_top_tracks_page(9, 100, 100)
            .await
            .unwrap();
        assert_eq!(total, 284);
        assert_eq!(tracks[0].title, "Later");
    }

    #[tokio::test]
    async fn artist_albums_sends_the_filter_and_parses_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/albums"))
            .and(query_param("filter", "EPSANDSINGLES"))
            .and(query_param("limit", "50"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 7, "title": "Single", "artists": [], "numberOfTracks": 1}],
                "totalNumberOfItems": 1, "limit": 50, "offset": 0
            })))
            .mount(&server)
            .await;
        let albums = client(&server)
            .artist_albums(9, ArtistDiscography::EpsAndSingles, 50, 0)
            .await
            .unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].title, "Single");
    }

    #[tokio::test]
    async fn artist_albums_sends_the_albums_filter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/albums"))
            .and(query_param("filter", "ALBUMS"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 8, "title": "Album", "artists": []}],
                "totalNumberOfItems": 1, "limit": 50, "offset": 0
            })))
            .mount(&server)
            .await;
        let albums = client(&server)
            .artist_albums(9, ArtistDiscography::Albums, 50, 0)
            .await
            .unwrap();
        assert_eq!(albums[0].title, "Album");
    }

    #[tokio::test]
    async fn artist_albums_sends_the_compilations_filter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/albums"))
            .and(query_param("filter", "COMPILATIONS"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 9, "title": "Appears On", "artists": []}],
                "totalNumberOfItems": 1, "limit": 50, "offset": 0
            })))
            .mount(&server)
            .await;
        let albums = client(&server)
            .artist_albums(9, ArtistDiscography::Compilations, 50, 0)
            .await
            .unwrap();
        assert_eq!(albums[0].title, "Appears On");
    }

    #[tokio::test]
    async fn artist_albums_all_fetches_every_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/albums"))
            .and(query_param("filter", "ALBUMS"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": (1..=100)
                    .map(|id| serde_json::json!({"id": id, "title": "A"}))
                    .collect::<Vec<_>>(),
                "totalNumberOfItems": 101, "limit": 100, "offset": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/artists/9/albums"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 101, "title": "Last"}],
                "totalNumberOfItems": 101, "limit": 100, "offset": 100
            })))
            .mount(&server)
            .await;
        let albums = client(&server)
            .artist_albums_all(9, ArtistDiscography::Albums)
            .await
            .unwrap();
        assert_eq!(albums.len(), 101);
        assert_eq!(albums[100].title, "Last");
    }

    #[tokio::test]
    async fn artist_profile_reads_followers_links_and_bio() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/9"))
            .and(query_param("countryCode", "GR"))
            .and(query_param("include", "biography"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "attributes": {
                        "numberOfFollowers": 1106540,
                        "externalLinks": [
                            {"href": "https://twitter.com/acdc", "meta": {"type": "TWITTER"}},
                            {"href": "https://tidal.com/browse/artist/9", "meta": {"type": "TIDAL_SHARING"}},
                            {"href": "https://twitter.com/acdc", "meta": {"type": "TWITTER"}}
                        ]
                    },
                    "id": "9",
                    "type": "artists"
                },
                "included": [{
                    "attributes": {
                        "source": "TiVo",
                        "text": "See [wimpLink artistId=\"1\"]Band[/wimpLink].<br/>More."
                    },
                    "id": "9",
                    "type": "artistBiographies"
                }]
            })))
            .mount(&server)
            .await;
        let profile = client(&server).artist_profile(9).await.unwrap();
        assert_eq!(profile.number_of_followers, Some(1106540));
        assert_eq!(profile.external_links.len(), 1);
        assert_eq!(profile.external_links[0].site, "TWITTER");
        assert_eq!(profile.external_links[0].url, "https://twitter.com/acdc");
        assert_eq!(profile.bio.as_deref(), Some("See Band.\n\nMore."));
        assert_eq!(profile.bio_segments.len(), 3);
        assert_eq!(profile.bio_segments[0].text, "See ");
        assert_eq!(profile.bio_segments[1].text, "Band");
        assert_eq!(profile.bio_segments[1].artist_id, Some(1));
        assert_eq!(profile.bio_segments[2].text, ".\nMore.");
    }

    #[tokio::test]
    async fn artist_profile_tolerates_missing_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/9"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"id": "9", "type": "artists"}
            })))
            .mount(&server)
            .await;
        let profile = client(&server).artist_profile(9).await.unwrap();
        assert_eq!(profile.number_of_followers, None);
        assert!(profile.external_links.is_empty());
        assert_eq!(profile.bio, None);
    }

    #[tokio::test]
    async fn artist_profile_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/9"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).artist_profile(9).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }

    #[test]
    fn credits_group_by_role_in_first_seen_order() {
        let raw: OpenApiTrackCredits = serde_json::from_value(serde_json::json!({
            "data": {
                "relationships": { "credits": { "data": [
                    { "id": "c1", "type": "credits" },
                    { "id": "c2", "type": "credits" },
                    { "id": "c3", "type": "credits" }
                ] } }
            },
            "included": [
                { "id": "c1", "type": "credits",
                  "attributes": { "name": "John Lennon", "role": "Vocals" },
                  "relationships": { "artist": { "data": { "id": "13", "type": "artists" } } } },
                { "id": "c2", "type": "credits",
                  "attributes": { "name": "Paul McCartney", "role": "Vocals" } },
                { "id": "c3", "type": "credits",
                  "attributes": { "name": "George Martin", "role": "Producer" } }
            ]
        }))
        .unwrap();
        let credits = credits_from_document(raw);
        assert_eq!(credits.roles.len(), 2);
        assert_eq!(credits.roles[0].role, "Vocals");
        assert_eq!(credits.roles[0].contributors.len(), 2);
        assert_eq!(credits.roles[0].contributors[0].artist_id, Some(13));
        assert_eq!(credits.roles[0].contributors[1].artist_id, None);
        assert_eq!(credits.roles[1].role, "Producer");
    }

    #[test]
    fn credits_ignore_unreferenced_or_non_credit_included() {
        let raw: OpenApiTrackCredits = serde_json::from_value(serde_json::json!({
            "data": { "relationships": { "credits": { "data": [{ "id": "c1", "type": "credits" }] } } },
            "included": [
                { "id": "c9", "type": "credits", "attributes": { "name": "Ghost", "role": "Vocals" } },
                { "id": "a1", "type": "artists", "attributes": { "name": "Artist" } }
            ]
        }))
        .unwrap();
        assert!(credits_from_document(raw).roles.is_empty());
    }

    #[test]
    fn credits_drop_blank_and_duplicate_rows() {
        let raw: OpenApiTrackCredits = serde_json::from_value(serde_json::json!({
            "data": { "relationships": { "credits": { "data": [
                { "id": "c1" }, { "id": "c2" }, { "id": "c3" }, { "id": "c4" }
            ] } } },
            "included": [
                { "id": "c1", "type": "credits", "attributes": { "name": "A", "role": "Vocals" } },
                { "id": "c2", "type": "credits", "attributes": { "name": "A", "role": "Vocals" } },
                { "id": "c3", "type": "credits", "attributes": { "name": "", "role": "Vocals" } },
                { "id": "c4", "type": "credits", "attributes": { "name": "B" } }
            ]
        }))
        .unwrap();
        let credits = credits_from_document(raw);
        assert_eq!(credits.roles.len(), 1);
        assert_eq!(credits.roles[0].contributors.len(), 1);
    }

    #[test]
    fn credits_without_a_relationship_are_empty() {
        let raw: OpenApiTrackCredits =
            serde_json::from_value(serde_json::json!({ "data": { "id": "1" } })).unwrap();
        assert!(credits_from_document(raw).roles.is_empty());
    }

    #[tokio::test]
    async fn track_credits_reads_the_include_and_maps_roles() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks/1"))
            .and(query_param("countryCode", "GR"))
            .and(query_param("include", "credits.artist"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "1",
                    "type": "tracks",
                    "relationships": { "credits": { "data": [
                        { "id": "c1", "type": "credits" },
                        { "id": "c2", "type": "credits" }
                    ] } }
                },
                "included": [
                    { "id": "c1", "type": "credits",
                      "attributes": { "name": "John Lennon", "role": "Vocals" },
                      "relationships": { "artist": { "data": { "id": "13", "type": "artists" } } } },
                    { "id": "c2", "type": "credits",
                      "attributes": { "name": "George Martin", "role": "Producer" } }
                ]
            })))
            .mount(&server)
            .await;
        let credits = client(&server).track_credits(1).await.unwrap();
        assert_eq!(credits.roles.len(), 2);
        assert_eq!(credits.roles[0].contributors[0].name, "John Lennon");
        assert_eq!(credits.roles[0].contributors[0].artist_id, Some(13));
        assert_eq!(credits.roles[1].contributors[0].artist_id, None);
    }

    #[tokio::test]
    async fn track_credits_without_a_credits_relationship_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "id": "1", "type": "tracks", "attributes": { "title": "X" } }
            })))
            .mount(&server)
            .await;
        assert!(client(&server).track_credits(1).await.unwrap().roles.is_empty());
    }
}
