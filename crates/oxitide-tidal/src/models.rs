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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub picture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistCreator {
    pub id: u64,
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
    #[serde(default, skip_serializing)]
    pub creator: Option<PlaylistCreator>,
    #[serde(skip_deserializing, default)]
    pub owned: bool,
}

impl Playlist {
    /// Mark editability from the creator id. `assume_owned` decides the
    /// missing-creator case: list endpoints return only the user's playlists,
    /// single fetches can be anybody's.
    pub fn mark_owned(&mut self, user_id: u64, assume_owned: bool) {
        self.owned = match &self.creator {
            Some(creator) => creator.id == user_id,
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
    pub fn track_items(&self) -> &[Track] { &self.tracks.items }
    pub fn album_items(&self) -> &[Album] { &self.albums.items }
    pub fn artist_items(&self) -> &[Artist] { &self.artists.items }
    pub fn playlist_items(&self) -> &[Playlist] { &self.playlists.items }
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
    fn partial_track_json_parses_with_defaults() {
        let raw = r#"{"id": 5, "title": "No Extras"}"#;
        let t: Track = serde_json::from_str(raw).unwrap();
        assert!(t.artists.is_empty());
        assert!(t.album.is_none());
        assert!(t.audio_quality.is_none());
        assert!(!t.is_hi_res());
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
    fn playlist_without_creator_uses_the_assumed_ownership() {
        let raw = r#"{"uuid": "u", "title": "Mine"}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();

        playlist.mark_owned(42, true);
        assert!(playlist.owned);

        playlist.mark_owned(42, false);
        assert!(!playlist.owned);
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
}
