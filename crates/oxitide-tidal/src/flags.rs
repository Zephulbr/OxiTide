//! Public-OpenAPI track flags: the `ai` label and a second source for
//! `explicit`. One JSON:API call can request many ids.

use serde::Deserialize;

use crate::client::TidalClient;
use crate::Result;

/// Flags for one track as the public v2 `tracks` attributes report them.
/// `None` means the attribute was absent; callers treat unknown as not blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackFlags {
    pub explicit: Option<bool>,
    pub ai: Option<bool>,
}

impl TidalClient {
    /// Flags for a caller-chosen batch of ids. Response order is preserved;
    /// ids that fail to parse are omitted, and unknown attributes stay `None`.
    pub async fn track_flags(&self, ids: &[u64]) -> Result<Vec<(u64, TrackFlags)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut params: Vec<(&str, String)> =
            vec![("countryCode", self.session().country_code.clone())];
        params.extend(ids.iter().map(|id| ("filter[id]", id.to_string())));
        let raw: OpenApiTracks = self.get_openapi("tracks", &params).await?;
        Ok(raw
            .data
            .into_iter()
            .filter_map(|resource| {
                let id = resource.id.parse::<u64>().ok()?;
                let attributes = resource.attributes.unwrap_or_default();
                Some((id, TrackFlags { explicit: attributes.explicit, ai: attributes.ai }))
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct OpenApiTracks {
    #[serde(default)]
    data: Vec<OpenApiTrack>,
}

#[derive(Debug, Deserialize)]
struct OpenApiTrack {
    #[serde(default)]
    id: String,
    #[serde(default)]
    attributes: Option<OpenApiTrackAttributes>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenApiTrackAttributes {
    #[serde(default)]
    explicit: Option<bool>,
    #[serde(default)]
    ai: Option<bool>,
}

#[cfg(test)]
mod tests {
    use crate::auth::SessionInfo;
    use crate::client::TidalClient;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

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
    async fn parses_flags_and_keeps_response_order() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .and(query_param("countryCode", "GR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "1", "type": "tracks", "attributes": {"explicit": true, "ai": true}},
                    {"id": "2", "type": "tracks", "attributes": {"explicit": false}}
                ]
            })))
            .mount(&server)
            .await;
        let flags = client(&server).track_flags(&[2, 1]).await.unwrap();
        assert_eq!(
            flags,
            vec![
                (1, super::TrackFlags { explicit: Some(true), ai: Some(true) }),
                (2, super::TrackFlags { explicit: Some(false), ai: None }),
            ]
        );
    }

    #[tokio::test]
    async fn sends_one_filter_param_per_id() {
        let server = MockServer::start().await;
        let responder = |req: &Request| {
            let ids: Vec<String> = req
                .url
                .query_pairs()
                .filter(|(key, _)| key == "filter[id]")
                .map(|(_, value)| value.into_owned())
                .collect();
            if ids == vec!["1".to_string(), "2".to_string()] {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []}))
            } else {
                ResponseTemplate::new(500)
            }
        };
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(responder)
            .mount(&server)
            .await;
        let out = client(&server).track_flags(&[1, 2]).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn missing_attributes_are_unknown_and_junk_ids_are_skipped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "7", "type": "tracks"},
                    {"id": "not-a-number", "type": "tracks", "attributes": {"ai": true}}
                ]
            })))
            .mount(&server)
            .await;
        let flags = client(&server).track_flags(&[7]).await.unwrap();
        assert_eq!(flags, vec![(7, super::TrackFlags { explicit: None, ai: None })]);
    }

    #[tokio::test]
    async fn empty_ids_skip_the_request() {
        let server = MockServer::start().await;
        let flags = client(&server).track_flags(&[]).await.unwrap();
        assert!(flags.is_empty());
    }
}
