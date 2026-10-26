//! ListenBrainz client (spec §4, §7).

use oxitide_audio::queue::QueueItem;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ListenBrainzError {
    #[error("listenbrainz is temporarily unavailable")]
    Retryable,
    #[error("listenbrainz rate limited the request")]
    RateLimited { retry_after_secs: Option<u64> },
    #[error("listenbrainz authentication failed")]
    Auth,
    #[error("listenbrainz rejected the request")]
    Permanent,
}

/// Parse a `Retry-After` header as integer seconds (spec §8); absent or
/// unparseable headers yield `None`.
fn retry_after_secs(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

pub struct ListenBrainzClient {
    http: reqwest::Client,
    base_url: String,
}

impl ListenBrainzClient {
    pub fn new(http: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub async fn validate(&self, token: &str) -> Result<String, ListenBrainzError> {
        let response = self
            .http
            .get(self.url("/validate-token"))
            .header("Authorization", format!("Token {token}"))
            .send()
            .await
            .map_err(|_| ListenBrainzError::Retryable)?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ListenBrainzError::Auth);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ListenBrainzError::RateLimited {
                retry_after_secs: retry_after_secs(&response),
            });
        }
        if status.is_server_error() {
            return Err(ListenBrainzError::Retryable);
        }
        if !status.is_success() {
            return Err(ListenBrainzError::Permanent);
        }
        let value: serde_json::Value =
            response.json().await.map_err(|_| ListenBrainzError::Permanent)?;
        let valid = value.get("valid").and_then(|field| field.as_bool()).unwrap_or(false);
        if !valid {
            return Err(ListenBrainzError::Auth);
        }
        value
            .get("user_name")
            .and_then(|field| field.as_str())
            .map(str::to_string)
            .ok_or(ListenBrainzError::Permanent)
    }

    pub async fn playing_now(
        &self,
        token: &str,
        item: &QueueItem,
    ) -> Result<(), ListenBrainzError> {
        let body = serde_json::json!({
            "listen_type": "playing_now",
            "payload": [{ "track_metadata": track_metadata(item) }]
        });
        self.submit(token, &body).await
    }

    pub async fn submit_listen(
        &self,
        token: &str,
        item: &QueueItem,
        listened_at: i64,
    ) -> Result<(), ListenBrainzError> {
        let body = serde_json::json!({
            "listen_type": "single",
            "payload": [{
                "track_metadata": track_metadata(item),
                "listened_at": listened_at
            }]
        });
        self.submit(token, &body).await
    }

    async fn submit(
        &self,
        token: &str,
        body: &serde_json::Value,
    ) -> Result<(), ListenBrainzError> {
        let response = self
            .http
            .post(self.url("/submit-listens"))
            .header("Authorization", format!("Token {token}"))
            .json(body)
            .send()
            .await
            .map_err(|_| ListenBrainzError::Retryable)?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ListenBrainzError::Auth);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ListenBrainzError::RateLimited {
                retry_after_secs: retry_after_secs(&response),
            });
        }
        if status.is_server_error() {
            return Err(ListenBrainzError::Retryable);
        }
        if !status.is_success() {
            return Err(ListenBrainzError::Permanent);
        }
        Ok(())
    }
}

pub fn track_metadata(item: &QueueItem) -> serde_json::Value {
    let mut info = serde_json::Map::new();
    info.insert("listening_from".into(), serde_json::json!("OxiTide"));
    if item.duration_secs > 0 {
        info.insert("duration_ms".into(), serde_json::json!(u64::from(item.duration_secs) * 1000));
    }
    let mut metadata = serde_json::Map::new();
    metadata.insert("artist_name".into(), serde_json::json!(item.artist));
    metadata.insert("track_name".into(), serde_json::json!(item.title));
    if !item.album.is_empty() {
        metadata.insert("release_name".into(), serde_json::json!(item.album));
    }
    metadata.insert("additional_info".into(), serde_json::Value::Object(info));
    serde_json::Value::Object(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method as http_method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn item() -> QueueItem {
        QueueItem {
            track_id: 7,
            title: "Song".into(),
            artist: "Band".into(),
            album: "Record".into(),
            duration_secs: 210,
            cover: None,
            artist_refs: Vec::new(),
            album_id: None,
            autoplay: false,
        }
    }

    fn client(server: &MockServer) -> ListenBrainzClient {
        ListenBrainzClient::new(reqwest::Client::new(), server.uri())
    }

    #[tokio::test]
    async fn validate_returns_the_username() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(path("/validate-token"))
            .and(header("authorization", "Token tok"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "valid": true, "user_name": "v" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).validate("tok").await.unwrap(), "v");
    }

    #[tokio::test]
    async fn validate_rejects_an_invalid_token() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "valid": false, "message": "bad token" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).validate("tok").await, Err(ListenBrainzError::Auth));
    }

    #[tokio::test]
    async fn validate_maps_401_to_auth() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        assert_eq!(client(&server).validate("tok").await, Err(ListenBrainzError::Auth));
    }

    #[tokio::test]
    async fn playing_now_sends_the_expected_payload() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/submit-listens"))
            .and(header("authorization", "Token tok"))
            .and(body_partial_json(serde_json::json!({
                "listen_type": "playing_now",
                "payload": [{
                    "track_metadata": {
                        "artist_name": "Band",
                        "track_name": "Song",
                        "release_name": "Record",
                        "additional_info": { "listening_from": "OxiTide", "duration_ms": 210000 }
                    }
                }]
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        client(&server).playing_now("tok", &item()).await.unwrap();
    }

    #[tokio::test]
    async fn submit_listen_adds_listened_at() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/submit-listens"))
            .and(body_partial_json(serde_json::json!({
                "listen_type": "single",
                "payload": [{ "listened_at": 1700000000 }]
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        client(&server)
            .submit_listen("tok", &item(), 1_700_000_000)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn submit_listen_maps_5xx_to_retryable() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).submit_listen("tok", &item(), 1).await,
            Err(ListenBrainzError::Retryable)
        );
    }

    #[tokio::test]
    async fn a_429_honors_the_retry_after_header() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "42"))
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).submit_listen("tok", &item(), 1).await,
            Err(ListenBrainzError::RateLimited { retry_after_secs: Some(42) })
        );

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).submit_listen("tok", &item(), 1).await,
            Err(ListenBrainzError::RateLimited { retry_after_secs: None })
        );
    }
}
