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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchResults {
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub albums: Vec<Album>,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub playlists: Vec<Playlist>,
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
}
