use serde::Deserialize;

use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{
    Album, AlbumInfo, Artist, ArtistDiscography, ArtistLink, ArtistProfile, ItemPage, Track,
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

        let bio = raw
            .included
            .iter()
            .find(|entry| entry.resource_type == "artistBiographies")
            .and_then(|entry| entry.attributes.as_ref())
            .and_then(|attributes| attributes.text.as_deref())
            .and_then(clean_bio);

        Ok(ArtistProfile {
            number_of_followers: attributes.and_then(|attributes| attributes.number_of_followers),
            external_links,
            bio,
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
struct OpenApiCopyright {
    #[serde(default)]
    text: Option<String>,
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
}
