//! Last.fm client: signed API calls (spec §4, §7).

use md5::{Digest, Md5};
use oxitide_audio::queue::QueueItem;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LastFmError {
    #[error("last.fm is temporarily unavailable")]
    Retryable,
    #[error("last.fm rate limited the request")]
    RateLimited { retry_after_secs: Option<u64> },
    #[error("last.fm authentication failed")]
    Auth,
    #[error("last.fm authorization is not complete")]
    NotAuthorized,
    #[error("last.fm rejected the request")]
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

#[derive(Debug, Clone, PartialEq)]
pub struct LastFmSession {
    pub username: String,
    pub key: String,
}

/// `api_sig`: MD5 hex of name+value pairs (excluding `format`/`callback`)
/// sorted by name, with the shared secret appended (spec §4).
pub fn api_sig(params: &[(String, String)], secret: &str) -> String {
    let mut pairs: Vec<(&str, &str)> = params
        .iter()
        .filter(|(key, _)| key != "format" && key != "callback")
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let mut material = String::new();
    for (key, value) in pairs {
        material.push_str(key);
        material.push_str(value);
    }
    material.push_str(secret);
    let mut hasher = Md5::new();
    hasher.update(material.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub struct LastFmClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    secret: String,
}

impl LastFmClient {
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        api_key: String,
        secret: String,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            secret,
        }
    }

    pub fn auth_url(&self, token: &str) -> String {
        format!("https://www.last.fm/api/auth/?api_key={}&token={}", self.api_key, token)
    }

    fn signed(&self, method: &str, mut params: Vec<(String, String)>) -> Vec<(String, String)> {
        params.push(("method".into(), method.to_string()));
        params.push(("api_key".into(), self.api_key.clone()));
        params.push(("format".into(), "json".into()));
        let signature = api_sig(&params, &self.secret);
        params.push(("api_sig".into(), signature));
        params
    }

    async fn get(
        &self,
        method: &str,
        params: Vec<(String, String)>,
    ) -> Result<serde_json::Value, LastFmError> {
        let query = self.signed(method, params);
        let response = self
            .http
            .get(&self.base_url)
            .query(&query)
            .send()
            .await
            .map_err(|_| LastFmError::Retryable)?;
        self.parse(response).await
    }

    async fn post(
        &self,
        method: &str,
        params: Vec<(String, String)>,
    ) -> Result<serde_json::Value, LastFmError> {
        let form = self.signed(method, params);
        let response = self
            .http
            .post(&self.base_url)
            .form(&form)
            .send()
            .await
            .map_err(|_| LastFmError::Retryable)?;
        self.parse(response).await
    }

    async fn parse(&self, response: reqwest::Response) -> Result<serde_json::Value, LastFmError> {
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(LastFmError::RateLimited { retry_after_secs: retry_after_secs(&response) });
        }
        if status.is_server_error() {
            return Err(LastFmError::Retryable);
        }
        let value: serde_json::Value = match response.json().await {
            Ok(value) => value,
            // A non-JSON body is an infrastructure response (gateway/CDN), not an
            // authoritative rejection; Last.fm returns JSON error bodies for real
            // rejections, so retry rather than dropping the batch.
            Err(_) => return Err(LastFmError::Retryable),
        };
        if let Some(code) = value.get("error").and_then(|field| field.as_u64()) {
            return Err(match code {
                14 => LastFmError::NotAuthorized,
                9 | 4 | 10 => LastFmError::Auth,
                16 | 29 => LastFmError::Retryable,
                _ => LastFmError::Permanent,
            });
        }
        if !status.is_success() {
            return Err(if status.is_client_error() {
                LastFmError::Permanent
            } else {
                LastFmError::Retryable
            });
        }
        Ok(value)
    }

    pub async fn get_token(&self) -> Result<String, LastFmError> {
        let value = self.get("auth.getToken", Vec::new()).await?;
        value
            .get("token")
            .and_then(|field| field.as_str())
            .map(str::to_string)
            .ok_or(LastFmError::Permanent)
    }

    pub async fn get_session(&self, token: &str) -> Result<LastFmSession, LastFmError> {
        let value = self
            .get("auth.getSession", vec![("token".into(), token.to_string())])
            .await?;
        let session = value.get("session").ok_or(LastFmError::Permanent)?;
        let username = session
            .get("name")
            .and_then(|field| field.as_str())
            .ok_or(LastFmError::Permanent)?
            .to_string();
        let key = session
            .get("key")
            .and_then(|field| field.as_str())
            .ok_or(LastFmError::Permanent)?
            .to_string();
        Ok(LastFmSession { username, key })
    }

    pub async fn now_playing(
        &self,
        session_key: &str,
        item: &QueueItem,
    ) -> Result<(), LastFmError> {
        let mut params = vec![("sk".to_string(), session_key.to_string())];
        params.extend(track_params(item, None));
        self.post("track.updateNowPlaying", params).await?;
        Ok(())
    }

    pub async fn scrobble(
        &self,
        session_key: &str,
        plays: &[(QueueItem, i64)],
    ) -> Result<(), LastFmError> {
        if plays.is_empty() {
            return Ok(());
        }
        let mut params = vec![("sk".to_string(), session_key.to_string())];
        for (index, (item, timestamp)) in plays.iter().enumerate() {
            params.push((format!("artist[{index}]"), item.artist.clone()));
            params.push((format!("track[{index}]"), item.title.clone()));
            params.push((format!("timestamp[{index}]"), timestamp.to_string()));
            if !item.album.is_empty() {
                params.push((format!("album[{index}]"), item.album.clone()));
            }
            if item.duration_secs > 0 {
                params.push((format!("duration[{index}]"), item.duration_secs.to_string()));
            }
        }
        self.post("track.scrobble", params).await?;
        Ok(())
    }
}

fn track_params(item: &QueueItem, timestamp: Option<i64>) -> Vec<(String, String)> {
    let mut params = vec![
        ("artist".to_string(), item.artist.clone()),
        ("track".to_string(), item.title.clone()),
    ];
    if !item.album.is_empty() {
        params.push(("album".to_string(), item.album.clone()));
    }
    if item.duration_secs > 0 {
        params.push(("duration".to_string(), item.duration_secs.to_string()));
    }
    if let Some(timestamp) = timestamp {
        params.push(("timestamp".to_string(), timestamp.to_string()));
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method as http_method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn encode(input: &str) -> String {
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("x", input)
            .finish()
            .trim_start_matches("x=")
            .to_string()
    }

    fn client(server: &MockServer) -> LastFmClient {
        LastFmClient::new(reqwest::Client::new(), server.uri(), "abc123".into(), "s3cr3t".into())
    }

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
        }
    }

    #[test]
    fn signature_matches_a_known_vector() {
        // md5("api_key"+"abc123"+"method"+"auth.getToken"+"token"+"tok"+"s3cr3t")
        let params = vec![
            ("api_key".to_string(), "abc123".to_string()),
            ("method".to_string(), "auth.getToken".to_string()),
            ("token".to_string(), "tok".to_string()),
        ];
        assert_eq!(api_sig(&params, "s3cr3t"), "50d433db8fa2c45553410dd8d1230608");
    }

    #[test]
    fn signature_excludes_format_and_callback() {
        let base = vec![
            ("api_key".to_string(), "abc123".to_string()),
            ("method".to_string(), "auth.getToken".to_string()),
        ];
        let mut with_noise = base.clone();
        with_noise.push(("format".to_string(), "json".to_string()));
        with_noise.push(("callback".to_string(), "cb".to_string()));
        assert_eq!(api_sig(&base, "s"), api_sig(&with_noise, "s"));
    }

    #[tokio::test]
    async fn get_token_returns_the_token() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "token": "tok" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).get_token().await.unwrap(), "tok");
    }

    #[tokio::test]
    async fn get_session_maps_error_14_to_not_authorized() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "error": 14, "message": "unauthorized token" })),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).get_session("tok").await, Err(LastFmError::NotAuthorized));
    }

    #[tokio::test]
    async fn get_session_parses_the_session() {
        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "session": { "name": "v", "key": "sk" } }),
            ))
            .mount(&server)
            .await;
        let session = client(&server).get_session("tok").await.unwrap();
        assert_eq!(session.username, "v");
        assert_eq!(session.key, "sk");
    }

    #[tokio::test]
    async fn scrobble_sends_signed_indexed_form_fields() {
        let server = MockServer::start().await;
        let expected_sig = api_sig(
            &[
                ("album[0]".into(), "Record".into()),
                ("api_key".into(), "abc123".into()),
                ("artist[0]".into(), "Band".into()),
                ("duration[0]".into(), "210".into()),
                ("method".into(), "track.scrobble".into()),
                ("sk".into(), "sk".into()),
                ("timestamp[0]".into(), "1700000000".into()),
                ("track[0]".into(), "Song".into()),
            ],
            "s3cr3t",
        );
        Mock::given(http_method("POST"))
            .and(path("/"))
            .and(body_string_contains(format!("api_sig={}", encode(&expected_sig))))
            .and(body_string_contains("method=track.scrobble"))
            .and(body_string_contains("artist%5B0%5D=Band"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "scrobbles": {} })),
            )
            .mount(&server)
            .await;
        client(&server)
            .scrobble("sk", &[(item(), 1_700_000_000)])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn scrobble_sends_a_fifty_item_batch() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .and(body_string_contains("method=track.scrobble"))
            .and(body_string_contains("artist%5B49%5D=Band"))
            .and(body_string_contains("timestamp%5B49%5D=1700000049"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "scrobbles": {} })),
            )
            .mount(&server)
            .await;
        let plays: Vec<(QueueItem, i64)> = (0..50)
            .map(|index| {
                let mut song = item();
                song.track_id = index;
                (song, 1_700_000_000 + index as i64)
            })
            .collect();
        client(&server).scrobble("sk", &plays).await.unwrap();
    }

    #[tokio::test]
    async fn a_429_honors_the_retry_after_header() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "42"))
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).now_playing("sk", &item()).await,
            Err(LastFmError::RateLimited { retry_after_secs: Some(42) })
        );

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).now_playing("sk", &item()).await,
            Err(LastFmError::RateLimited { retry_after_secs: None })
        );
    }

    #[tokio::test]
    async fn non_json_client_error_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(
                ResponseTemplate::new(403).set_body_string("<html>forbidden</html>"),
            )
            .mount(&server)
            .await;
        assert_eq!(client(&server).now_playing("sk", &item()).await, Err(LastFmError::Retryable));
    }

    #[tokio::test]
    async fn error_9_is_auth_and_16_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 9 })))
            .mount(&server)
            .await;
        assert_eq!(client(&server).now_playing("sk", &item()).await, Err(LastFmError::Auth));

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 16 })))
            .mount(&server)
            .await;
        assert_eq!(client(&server).now_playing("sk", &item()).await, Err(LastFmError::Retryable));
    }
}
