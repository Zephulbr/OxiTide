use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

/// One entry in the signed-in user's feed: a release activity from an artist
/// they follow (`GET /v2/feed/activities`). History-mix activities are not
/// represented yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedActivity {
    /// Album id (numeric).
    pub id: u64,
    pub title: String,
    /// Composed like the official client: `Album · Artist`, `Single by Artist`.
    pub subtitle: String,
    /// Album cover uuid, resolved against the image CDN by the UI.
    pub artwork: Option<String>,
    pub explicit: bool,
    /// True when TIDAL serves the release in Hi-Res Lossless.
    pub hires: bool,
    /// RFC 3339 timestamp the activity occurred at.
    pub occurred_at: String,
    pub seen: bool,
    /// Release type: `ALBUM`, `EP` or `SINGLE` (empty when absent).
    #[serde(default)]
    pub release_type: String,
}

#[derive(Debug, Deserialize)]
struct RawFeed {
    #[serde(default)]
    activities: Vec<RawActivity>,
}

#[derive(Debug, Deserialize)]
struct RawActivity {
    #[serde(default, rename = "followableActivity")]
    followable_activity: Option<RawFollowableActivity>,
    #[serde(default)]
    seen: bool,
}

#[derive(Debug, Deserialize)]
struct RawFollowableActivity {
    #[serde(default)]
    album: Option<RawFeedAlbum>,
    #[serde(default, rename = "activityType")]
    activity_type: String,
    #[serde(default, rename = "occurredAt")]
    occurred_at: String,
}

#[derive(Debug, Deserialize)]
struct RawFeedAlbum {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    artists: Vec<RawFeedArtist>,
    #[serde(default, rename = "mainArtists")]
    main_artists: Vec<RawFeedArtist>,
    #[serde(default, rename = "type")]
    album_type: String,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    explicit: bool,
    #[serde(default, rename = "audioQuality")]
    audio_quality: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawFeedArtist {
    #[serde(default)]
    name: String,
}

fn activity_from_raw(raw: RawActivity) -> Option<FeedActivity> {
    let follow = raw.followable_activity?;
    if follow.activity_type != "NEW_ALBUM_RELEASE" {
        return None;
    }
    let album = follow.album?;
    let artists: Vec<String> = if album.main_artists.is_empty() {
        album
            .artists
            .into_iter()
            .map(|artist| artist.name)
            .collect()
    } else {
        album
            .main_artists
            .into_iter()
            .map(|artist| artist.name)
            .collect()
    };
    let artists = artists.join(", ");
    let (label, separator) = match album.album_type.as_str() {
        "SINGLE" => ("Single", " by "),
        "EP" => ("EP", " by "),
        _ => ("Album", " · "),
    };
    let subtitle = if artists.is_empty() {
        label.to_string()
    } else {
        format!("{label}{separator}{artists}")
    };
    Some(FeedActivity {
        id: album.id,
        title: album.title,
        subtitle,
        artwork: album.cover,
        explicit: album.explicit,
        hires: album.audio_quality.as_deref() == Some("HI_RES_LOSSLESS"),
        occurred_at: follow.occurred_at,
        seen: raw.seen,
        release_type: album.album_type,
    })
}

impl TidalClient {
    /// The signed-in user's feed: release activities from followed artists.
    pub async fn feed_activities(&self) -> Result<Vec<FeedActivity>> {
        let raw: RawFeed = self
            .get_v2(
                "feed/activities",
                &[("userId", self.session().user_id.to_string())],
            )
            .await?;
        Ok(raw
            .activities
            .into_iter()
            .filter_map(activity_from_raw)
            .collect())
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
    async fn feed_activities_requests_v2_feed_for_the_session_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/feed/activities"))
            .and(query_param("userId", "42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "activities": [{
                    "followableActivity": {
                        "album": {
                            "id": 161451880,
                            "title": "A very chilly christmas",
                            "artists": [{"id": 3840013, "name": "Chilly Gonzales"}],
                            "mainArtists": [{"id": 3840013, "name": "Chilly Gonzales"}],
                            "type": "ALBUM",
                            "audioQuality": "LOSSLESS",
                            "releaseDate": "2020-11-13",
                            "cover": "c9536722-36cb-4464-b77b-c318370f9d74",
                            "explicit": false
                        },
                        "activityType": "NEW_ALBUM_RELEASE",
                        "occurredAt": "2020-11-12T23:11:54.927211Z"
                    },
                    "seen": false
                }],
                "stats": {"totalNotSeenActivities": 1}
            })))
            .mount(&server)
            .await;
        let activities = client(&server).feed_activities().await.unwrap();
        assert_eq!(activities.len(), 1);
        assert_eq!(activities[0].id, 161451880);
        assert_eq!(activities[0].title, "A very chilly christmas");
        assert_eq!(activities[0].subtitle, "Album · Chilly Gonzales");
        assert_eq!(
            activities[0].artwork.as_deref(),
            Some("c9536722-36cb-4464-b77b-c318370f9d74")
        );
        assert!(!activities[0].explicit);
        assert!(!activities[0].hires);
        assert_eq!(activities[0].occurred_at, "2020-11-12T23:11:54.927211Z");
        assert!(!activities[0].seen);
    }

    #[tokio::test]
    async fn feed_activities_composes_single_subtitle_and_flags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/feed/activities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "activities": [{
                    "followableActivity": {
                        "album": {
                            "id": 7,
                            "title": "Don't Stop Me Now / In Only Seven Days",
                            "artists": [{"id": 1, "name": "Queen"}, {"id": 2, "name": "Guest"}],
                            "mainArtists": [{"id": 1, "name": "Queen"}],
                            "type": "SINGLE",
                            "audioQuality": "HI_RES_LOSSLESS",
                            "cover": "c-7",
                            "explicit": true
                        },
                        "activityType": "NEW_ALBUM_RELEASE",
                        "occurredAt": "2026-09-20T08:00:00Z"
                    },
                    "seen": true
                }]
            })))
            .mount(&server)
            .await;
        let activities = client(&server).feed_activities().await.unwrap();
        assert_eq!(activities[0].subtitle, "Single by Queen");
        assert!(activities[0].explicit);
        assert!(activities[0].hires);
        assert!(activities[0].seen);
    }

    #[tokio::test]
    async fn feed_activities_skips_history_mixes_and_payloadless_entries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/feed/activities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "activities": [
                    {
                        "followableActivity": {
                            "historyMix": {"id": "mix-1", "mixType": "HISTORY_MONTHLY_MIX"},
                            "activityType": "NEW_HISTORY_MIX",
                            "occurredAt": "2026-09-01T00:00:00Z"
                        },
                        "seen": true
                    },
                    {
                        "followableActivity": {
                            "activityType": "NEW_ALBUM_RELEASE",
                            "occurredAt": "2026-09-02T00:00:00Z"
                        },
                        "seen": false
                    }
                ]
            })))
            .mount(&server)
            .await;
        assert!(client(&server).feed_activities().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn feed_activities_with_no_entries_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/feed/activities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "activities": [],
                "stats": {"totalNotSeenActivities": 0}
            })))
            .mount(&server)
            .await;
        assert!(client(&server).feed_activities().await.unwrap().is_empty());
    }
}
