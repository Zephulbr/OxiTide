use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumRef {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub cover: Option<String>,
    /// Embedded only in playlist track payloads (multi-sort probe); other
    /// lists resolve it through the album release-date store.
    #[serde(default, rename = "releaseDate", skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaMetadata {
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub album: Option<AlbumRef>,
    #[serde(default, rename = "audioQuality")]
    pub audio_quality: Option<String>,
    #[serde(default, rename = "trackNumber")]
    pub track_number: u32,
    #[serde(default, rename = "volumeNumber")]
    pub volume_number: u32,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default, rename = "mediaMetadata")]
    pub media_metadata: Option<MediaMetadata>,
    /// When the track joined this playlist (playlist payloads) or the
    /// favorites (copied from the favorites wrapper's `created`).
    #[serde(default, rename = "dateAdded", skip_serializing_if = "Option::is_none")]
    pub date_added: Option<String>,
    /// TIDAL's global popularity, 0–100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popularity: Option<u32>,
}

impl Track {
    pub fn is_hi_res(&self) -> bool {
        self.media_metadata
            .as_ref()
            .map(|m| m.tags.iter().any(|t| t == "HIRES_LOSSLESS"))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default, rename = "numberOfTracks")]
    pub number_of_tracks: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default, rename = "audioQuality")]
    pub audio_quality: Option<String>,
    /// Release type: `ALBUM`, `EP` or `SINGLE`.
    #[serde(default, rename = "type")]
    pub album_type: Option<String>,
}

/// Public v2 album facts (spec §5.3). Absent attributes stay `None` and the
/// UI omits them; the public API exposes no label field (only `copyright`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfo {
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub barcode_id: Option<String>,
    #[serde(default)]
    pub number_of_volumes: Option<u32>,
    #[serde(default)]
    pub number_of_items: Option<u32>,
}

/// Track credits from the public v2 `credits` relationship (spec §5.3).
/// An empty `roles` list is a settled "no credits exposed" result.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCredits {
    #[serde(default)]
    pub roles: Vec<CreditRole>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditRole {
    pub role: String,
    #[serde(default)]
    pub contributors: Vec<CreditContributor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditContributor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub picture: Option<String>,
    /// Album cover TIDAL shows for an artist without a picture. It is an album
    /// image, so it has album sizes (no 480; see `ui/src/lib/artwork.ts`).
    #[serde(
        default,
        rename(deserialize = "selectedAlbumCoverFallback"),
        alias = "cover_fallback",
        skip_serializing_if = "Option::is_none"
    )]
    pub cover_fallback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixes: Option<ArtistMixes>,
}

/// TIDAL's artist mix ids (uppercase keys verbatim in the v1 payload).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct ArtistMixes {
    #[serde(default)]
    pub artist_mix: Option<String>,
    #[serde(default)]
    pub master_artist_mix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistLink {
    /// Uppercased site key from the API, e.g. "TWITTER".
    pub site: String,
    pub url: String,
}

/// One bio text run; `artist_id` marks a `[wimpLink artistId="…"]` link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BioSegment {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistProfile {
    #[serde(default)]
    pub number_of_followers: Option<u64>,
    #[serde(default)]
    pub external_links: Vec<ArtistLink>,
    /// Cleaned biography text (no wimpLink/HTML), `None` when absent/empty.
    #[serde(default)]
    pub bio: Option<String>,
    /// In-text bio runs with `[wimpLink artistId]` navigation (spec §6.3).
    #[serde(default)]
    pub bio_segments: Vec<BioSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtistDiscography {
    Albums,
    EpsAndSingles,
    Compilations,
}

impl ArtistDiscography {
    /// The `filter` query value TIDAL expects on `artists/{id}/albums`.
    pub fn tidal_param(self) -> &'static str {
        match self {
            ArtistDiscography::Albums => "ALBUMS",
            ArtistDiscography::EpsAndSingles => "EPSANDSINGLES",
            ArtistDiscography::Compilations => "COMPILATIONS",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistCreator {
    #[serde(default, deserialize_with = "optional_creator_id")]
    pub id: Option<u64>,
}

/// Creator ids arrive as numbers or numeric strings, and curated/editorial
/// playlists can omit the id entirely. An unknown or unparseable id never
/// matches a user id, so it marks the playlist unowned instead of failing the
/// whole response decode.
fn optional_creator_id<'de, D>(deserializer: D) -> std::result::Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Number(u64),
        Text(String),
    }

    match Option::<Raw>::deserialize(deserializer)? {
        Some(Raw::Number(id)) => Ok(Some(id)),
        Some(Raw::Text(text)) => Ok(text.parse().ok()),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub uuid: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "numberOfTracks")]
    pub number_of_tracks: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub image: Option<String>,
    /// Square playlist artwork. `image` is the wide variant for playlists (its
    /// square sizes 403 on the CDN), so square surfaces want this field.
    #[serde(default, rename = "squareImage")]
    pub square_image: Option<String>,
    /// When the playlist was created; `null` when TIDAL omits it. Drives the
    /// sidebar's "Created date" ordering.
    #[serde(default)]
    pub created: Option<String>,
    /// When TIDAL last recorded an item added to this playlist; `null` for
    /// playlists that never had a track added. Drives the picker's
    /// recent-activity ordering.
    #[serde(default, rename = "lastItemAddedAt")]
    pub last_item_added_at: Option<String>,
    /// When TIDAL last recorded any playlist edit (rename, track change).
    #[serde(default, rename = "lastUpdated")]
    pub last_updated: Option<String>,
    /// Public/private visibility; `null` when TIDAL omits it (then the client
    /// falls back to the v2 metadata fetch).
    #[serde(default, rename = "publicPlaylist")]
    pub public_playlist: Option<bool>,
    /// Follower count. The v1 payload omits it, so the client fills it from
    /// the v2 metadata endpoint; `null` when neither source reports it.
    #[serde(default, rename = "numberOfFollowers")]
    pub number_of_followers: Option<u32>,
    #[serde(default, skip_serializing)]
    pub creator: Option<PlaylistCreator>,
    #[serde(skip_deserializing, default)]
    pub owned: bool,
}

impl Playlist {
    /// Mark editability from the creator id. `assume_owned` decides the
    /// missing-or-unknown-creator case: list endpoints return only the user's
    /// playlists, single fetches (and search) can be anybody's.
    pub fn mark_owned(&mut self, user_id: u64, assume_owned: bool) {
        self.owned = match &self.creator {
            Some(creator) => creator.id == Some(user_id),
            None => assume_owned,
        };
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemPage<T> {
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
    #[serde(default, rename = "totalNumberOfItems")]
    pub total_number_of_items: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchGroup<T> {
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}

fn empty_group<T>() -> SearchGroup<T> {
    SearchGroup { items: Vec::new() }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    #[serde(default = "empty_group")]
    pub tracks: SearchGroup<Track>,
    #[serde(default = "empty_group")]
    pub albums: SearchGroup<Album>,
    #[serde(default = "empty_group")]
    pub artists: SearchGroup<Artist>,
    #[serde(default = "empty_group")]
    pub playlists: SearchGroup<Playlist>,
}

impl SearchResults {
    pub fn track_items(&self) -> &[Track] {
        &self.tracks.items
    }
    pub fn album_items(&self) -> &[Album] {
        &self.albums.items
    }
    pub fn artist_items(&self) -> &[Artist] {
        &self.artists.items
    }
    pub fn playlist_items(&self) -> &[Playlist] {
        &self.playlists.items
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "subTitle")]
    pub sub_title: Option<String>,
    #[serde(default, rename = "mixType")]
    pub mix_type: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_track_json_parses() {
        let raw = r#"{
            "id": 77646170, "title": "The Golden Age", "duration": 276,
            "artists": [{"id": 1, "name": "Beck"}],
            "album": {"id": 77646169, "title": "Sea Change", "cover": "abc-def"},
            "audioQuality": "LOSSLESS", "trackNumber": 1, "volumeNumber": 1,
            "explicit": false, "mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"]}
        }"#;
        let t: Track = serde_json::from_str(raw).unwrap();
        assert_eq!(t.id, 77646170);
        assert_eq!(t.artists[0].name, "Beck");
        assert_eq!(t.album.as_ref().unwrap().title, "Sea Change");
        assert!(t.is_hi_res());
    }

    #[test]
    fn album_info_serializes_camel_case_and_omits_nothing() {
        let info = AlbumInfo {
            release_date: Some("2017-06-30".into()),
            copyright: Some("(C) 2017 S. Carter Enterprises".into()),
            barcode_id: Some("00881377068540".into()),
            number_of_volumes: Some(2),
            number_of_items: Some(24),
        };
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["releaseDate"], "2017-06-30");
        assert_eq!(value["barcodeId"], "00881377068540");
        assert_eq!(value["numberOfVolumes"], 2);
        assert_eq!(value["numberOfItems"], 24);
        assert_eq!(AlbumInfo::default().number_of_volumes, None);
    }

    #[test]
    fn partial_track_json_parses_with_defaults() {
        let raw = r#"{"id": 5, "title": "No Extras"}"#;
        let t: Track = serde_json::from_str(raw).unwrap();
        assert!(t.artists.is_empty());
        assert!(t.album.is_none());
        assert!(t.audio_quality.is_none());
        assert!(!t.is_hi_res());
        let value = serde_json::to_value(&t).unwrap();
        assert!(value.get("dateAdded").is_none());
        assert!(value.get("popularity").is_none());
    }

    #[test]
    fn playlist_track_sort_fields_round_trip() {
        // Shape from the 2026-10-19 multi-sort probe (playlists/{uuid}/tracks).
        let raw = r#"{
            "id": 9, "title": "T", "popularity": 71,
            "dateAdded": "2025-08-06T17:50:35.372+0000",
            "album": {"id": 3, "title": "A", "cover": null, "releaseDate": "1992-09-21"}
        }"#;
        let t: Track = serde_json::from_str(raw).unwrap();
        assert_eq!(t.popularity, Some(71));
        assert_eq!(t.date_added.as_deref(), Some("2025-08-06T17:50:35.372+0000"));
        assert_eq!(t.album.as_ref().unwrap().release_date.as_deref(), Some("1992-09-21"));
        let value = serde_json::to_value(&t).unwrap();
        assert_eq!(value["dateAdded"], "2025-08-06T17:50:35.372+0000");
        assert_eq!(value["popularity"], 71);
        assert_eq!(value["album"]["releaseDate"], "1992-09-21");
    }

    #[test]
    fn empty_item_page_parses() {
        let raw = r#"{"items": [], "totalNumberOfItems": 0, "limit": 100, "offset": 0}"#;
        let p: ItemPage<Track> = serde_json::from_str(raw).unwrap();
        assert!(p.items.is_empty());
        assert_eq!(p.total_number_of_items, 0);
    }

    #[test]
    fn playlist_creator_parses_and_marks_ownership() {
        let raw = r#"{"uuid": "u", "title": "Mine", "creator": {"id": 42}}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();

        playlist.mark_owned(42, false);
        assert!(playlist.owned);

        playlist.mark_owned(7, false);
        assert!(!playlist.owned);
    }

    #[test]
    fn playlist_creator_without_an_id_is_unknown_and_not_owned() {
        let raw = r#"{"uuid": "u", "title": "Editorial", "creator": {"name": "TIDAL"}}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();

        assert_eq!(playlist.creator.as_ref().unwrap().id, None);

        playlist.mark_owned(42, false);
        assert!(!playlist.owned);
    }

    #[test]
    fn playlist_creator_accepts_a_numeric_string_id() {
        let raw = r#"{"uuid": "u", "title": "Mine", "creator": {"id": "42"}}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();

        playlist.mark_owned(42, false);
        assert!(playlist.owned);
    }

    #[test]
    fn playlist_without_creator_uses_the_assumed_ownership() {
        let raw = r#"{"uuid": "u", "title": "Mine"}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();

        playlist.mark_owned(42, true);
        assert!(playlist.owned);

        playlist.mark_owned(42, false);
        assert!(!playlist.owned);
    }

    #[test]
    fn playlist_parses_and_serializes_square_image() {
        let raw =
            r#"{"uuid": "u", "title": "T", "image": "wide-uuid", "squareImage": "square-uuid"}"#;
        let playlist: Playlist = serde_json::from_str(raw).unwrap();
        assert_eq!(playlist.square_image.as_deref(), Some("square-uuid"));

        let value = serde_json::to_value(&playlist).unwrap();
        assert_eq!(value["squareImage"], serde_json::json!("square-uuid"));

        let raw_missing = r#"{"uuid": "u", "title": "T"}"#;
        let playlist: Playlist = serde_json::from_str(raw_missing).unwrap();
        assert!(playlist.square_image.is_none());
    }

    #[test]
    fn playlist_serializes_owned_but_not_creator() {
        let raw = r#"{"uuid": "u", "title": "T", "creator": {"id": 1}}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();
        playlist.mark_owned(1, false);

        let value = serde_json::to_value(&playlist).unwrap();

        assert_eq!(value["owned"], serde_json::json!(true));
        assert!(value.get("creator").is_none());
    }

    #[test]
    fn mix_parses_v2_fields() {
        let raw =
            r#"{"id": "m1", "title": "Daily", "subTitle": "For you", "mixType": "DAILY_MIX"}"#;
        let mix: Mix = serde_json::from_str(raw).unwrap();
        assert_eq!(mix.id, "m1");
        assert_eq!(mix.sub_title.as_deref(), Some("For you"));
        assert_eq!(mix.mix_type.as_deref(), Some("DAILY_MIX"));
    }

    #[test]
    fn artist_profile_serializes_camel_case() {
        let profile = ArtistProfile {
            number_of_followers: Some(1106540),
            external_links: vec![ArtistLink {
                site: "TWITTER".into(),
                url: "https://twitter.com/acdc".into(),
            }],
            bio: None,
            bio_segments: Vec::new(),
        };
        let json = serde_json::to_value(&profile).unwrap();
        assert_eq!(json["numberOfFollowers"], 1106540);
        assert_eq!(json["externalLinks"][0]["site"], "TWITTER");
    }

    #[test]
    fn artist_parses_and_serializes_mixes() {
        let raw =
            r#"{"id": 1, "name": "A", "mixes": {"ARTIST_MIX": "abc", "MASTER_ARTIST_MIX": "def"}}"#;
        let artist: Artist = serde_json::from_str(raw).unwrap();
        let mixes = artist.mixes.as_ref().unwrap();
        assert_eq!(mixes.artist_mix.as_deref(), Some("abc"));
        assert_eq!(mixes.master_artist_mix.as_deref(), Some("def"));

        let value = serde_json::to_value(&artist).unwrap();
        assert_eq!(value["mixes"]["ARTIST_MIX"], "abc");
    }

    #[test]
    fn artist_without_mixes_omits_the_field() {
        let artist: Artist = serde_json::from_str(r#"{"id": 1, "name": "A"}"#).unwrap();
        assert!(artist.mixes.is_none());
        let value = serde_json::to_value(&artist).unwrap();
        assert!(value.get("mixes").is_none());
    }

    #[test]
    fn artist_reads_album_cover_fallback() {
        let raw = r#"{"id": 1, "name": "A", "picture": null, "selectedAlbumCoverFallback": "c-1"}"#;
        let artist: Artist = serde_json::from_str(raw).unwrap();
        assert_eq!(artist.cover_fallback.as_deref(), Some("c-1"));

        let value = serde_json::to_value(&artist).unwrap();
        assert_eq!(value["cover_fallback"], "c-1");
        let back: Artist = serde_json::from_value(value).unwrap();
        assert_eq!(back.cover_fallback.as_deref(), Some("c-1"));
    }
}
