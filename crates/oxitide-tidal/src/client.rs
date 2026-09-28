use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::auth::{self, SessionInfo};
use crate::error::{Error, Result};
use crate::models::ItemPage;
use crate::token_store::{TokenSet, TokenStore};

/// Default page size used when transparently walking paginated endpoints.
pub const DEFAULT_PAGE_SIZE: u32 = 100;

/// Decode a JSON response body, keeping serde's field/line detail on failure.
/// reqwest's own decode error only says "error decoding response body", which
/// makes a response/DTO mismatch impossible to diagnose. Never logs the body:
/// token endpoints pass through here.
pub(crate) async fn decode_json<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    let status = resp.status();
    let url = resp.url().clone();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp.bytes().await?;
    serde_json::from_slice(&body).map_err(|e| {
        tracing::warn!(
            %status,
            %url,
            %content_type,
            bytes = body.len(),
            "TIDAL response did not match its expected shape: {e}"
        );
        Error::Json(e)
    })
}

#[derive(Clone)]
pub struct TidalClient {
    http: reqwest::Client,
    base_url: String,
    v2_base_url: String,
    token_url: String,
    session: SessionInfo,
    tokens: Arc<RwLock<TokenSet>>,
    store: Arc<dyn TokenStore>,
    refresh_lock: Arc<Mutex<()>>,
}

impl TidalClient {
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        token_url: impl Into<String>,
        session: SessionInfo,
        tokens: TokenSet,
        store: Arc<dyn TokenStore>,
    ) -> Self {
        let base_url = base_url.into();
        let trimmed = base_url.trim_end_matches('/');
        let v2_base_url = match trimmed.strip_suffix("/v1") {
            Some(prefix) => format!("{prefix}/v2"),
            None => format!("{trimmed}/v2"),
        };
        Self {
            http,
            base_url,
            v2_base_url,
            token_url: token_url.into(),
            session,
            tokens: Arc::new(RwLock::new(tokens)),
            store,
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn session(&self) -> &SessionInfo {
        &self.session
    }

    fn access_token(&self) -> String {
        self.tokens.read().unwrap().access_token.clone()
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)]) -> Result<T> {
        let (value, _) = self.get_with_etag(path, params).await?;
        Ok(value)
    }

    /// GET that also returns the response `etag` header (playlist writes must
    /// send it verbatim in `If-None-Match`), sharing `get`'s single-flight
    /// refresh-on-401. A 401 that survives a *successful* refresh is an
    /// authorization failure, not an expired session.
    pub async fn get_with_etag<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<(T, Option<String>)> {
        let token = self.access_token();
        let mut resp = self.send_get(path, params, &token).await?;
        let mut refreshed = false;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            refreshed = true;
            resp = self.send_get(path, params, &self.access_token()).await?;
        }
        if refreshed && resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Forbidden);
        }
        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        Ok((Self::decode(resp).await?, etag))
    }

    /// Send a form-encoded POST, refreshing once on 401 and honoring a
    /// `Retry-After` hint once on 429.
    pub async fn post_form(&self, path: &str, form: &[(&str, String)]) -> Result<()> {
        self.post_form_with_etag(path, form, None).await
    }

    /// Form POST with an optional `If-None-Match` value (playlist writes).
    pub async fn post_form_with_etag(
        &self,
        path: &str,
        form: &[(&str, String)],
        etag: Option<&str>,
    ) -> Result<()> {
        self.mutate(reqwest::Method::POST, &self.base_url, path, &[], form, etag)
            .await
            .map(|_| ())
    }

    /// Send a DELETE, refreshing once on 401 and honoring a `Retry-After` hint
    /// once on 429.
    pub async fn delete(&self, path: &str) -> Result<()> {
        self.delete_with_etag(path, None).await
    }

    /// DELETE with an optional `If-None-Match` value (playlist writes).
    pub async fn delete_with_etag(&self, path: &str, etag: Option<&str>) -> Result<()> {
        self.mutate(reqwest::Method::DELETE, &self.base_url, path, &[], &[], etag)
            .await
            .map(|_| ())
    }

    /// Send a PUT with query params on the v2 API and decode the JSON response.
    pub async fn put_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        let resp = self
            .mutate(reqwest::Method::PUT, &self.v2_base_url, path, params, &[], None)
            .await?;
        decode_json(resp).await
    }

    /// Send a form-encoded POST and decode the JSON response, sharing the write
    /// verbs' refresh-on-401 and 429 retry behavior.
    pub async fn post_form_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
    ) -> Result<T> {
        let resp = self
            .mutate(reqwest::Method::POST, &self.base_url, path, params, form, None)
            .await?;
        decode_json(resp).await
    }

    /// Form POST returning JSON with an optional `If-None-Match` value.
    pub async fn post_form_json_with_etag<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
        etag: Option<&str>,
    ) -> Result<T> {
        let resp = self
            .mutate(reqwest::Method::POST, &self.base_url, path, params, form, etag)
            .await?;
        decode_json(resp).await
    }

    /// Walk every page of a TIDAL list endpoint, following `totalNumberOfItems`
    /// and `offset`, returning the concatenated `items`.
    pub async fn get_all_pages<T: DeserializeOwned>(
        &self,
        path: &str,
        base_params: &[(&str, String)],
        page_size: u32,
    ) -> Result<Vec<T>> {
        let mut all: Vec<T> = Vec::new();
        let mut offset: u32 = 0;
        loop {
            let mut params: Vec<(&str, String)> = base_params.to_vec();
            params.push(("limit", page_size.to_string()));
            params.push(("offset", offset.to_string()));
            let page: ItemPage<T> = self.get(path, &params).await?;
            let received = page.items.len() as u32;
            let total = page.total_number_of_items;
            all.extend(page.items);
            offset = offset.saturating_add(received);
            if received == 0 {
                break;
            }
            if total > 0 {
                if offset >= total {
                    break;
                }
            } else if received < page_size {
                // No total reported: stop once the server returns a short page.
                break;
            }
        }
        Ok(all)
    }

    async fn send_get(
        &self,
        path: &str,
        params: &[(&str, String)],
        token: &str,
    ) -> Result<reqwest::Response> {
        self.send_get_at(&self.base_url, path, params, token).await
    }

    async fn send_get_at(
        &self,
        base: &str,
        path: &str,
        params: &[(&str, String)],
        token: &str,
    ) -> Result<reqwest::Response> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if base == self.base_url.as_str() {
            query.push(("countryCode", self.session.country_code.clone()));
        }
        query.extend(params.iter().cloned());
        let url = format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'));
        Ok(self.http.get(url).bearer_auth(token).query(&query).send().await?)
    }

    /// GET on the v2 API base (no `countryCode`), sharing `get`'s refresh-on-401.
    pub async fn get_v2<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        let token = self.access_token();
        let mut resp = self.send_get_at(&self.v2_base_url, path, params, &token).await?;
        let mut refreshed = false;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            refreshed = true;
            resp = self
                .send_get_at(&self.v2_base_url, path, params, &self.access_token())
                .await?;
        }
        if refreshed && resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Forbidden);
        }
        Self::decode(resp).await
    }

    /// Shared write path: v1 bases get the `countryCode` query, 401 refreshes
    /// once, 429 retries once after `Retry-After`, and non-success statuses map
    /// to errors.
    async fn mutate(
        &self,
        method: reqwest::Method,
        base: &str,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
        etag: Option<&str>,
    ) -> Result<reqwest::Response> {
        let country_code = base == self.base_url.as_str();
        let token = self.access_token();
        let mut refreshed = false;
        let mut resp = self
            .send_mutation(&method, base, path, params, form, &token, country_code, etag)
            .await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            refreshed = true;
            resp = self
                .send_mutation(
                    &method,
                    base,
                    path,
                    params,
                    form,
                    &self.access_token(),
                    country_code,
                    etag,
                )
                .await?;
        }
        // A 401 that survives a successful refresh is a permission failure;
        // only a failed refresh means the session is gone.
        if refreshed && resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Forbidden);
        }
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            tokio::time::sleep(Self::retry_after(&resp)).await;
            resp = self
                .send_mutation(
                    &method,
                    base,
                    path,
                    params,
                    form,
                    &self.access_token(),
                    country_code,
                    etag,
                )
                .await?;
        }
        Self::decode_status(resp).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_mutation(
        &self,
        method: &reqwest::Method,
        base: &str,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
        token: &str,
        country_code: bool,
        etag: Option<&str>,
    ) -> Result<reqwest::Response> {
        let mut query: Vec<(&str, String)> = if country_code {
            vec![("countryCode", self.session.country_code.clone())]
        } else {
            Vec::new()
        };
        query.extend(params.iter().cloned());
        let url = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let mut request = self
            .http
            .request(method.clone(), url)
            .bearer_auth(token)
            .query(&query);
        if !form.is_empty() {
            request = request.form(form);
        }
        if let Some(value) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, value);
        }
        Ok(request.send().await?)
    }

    /// `Retry-After` in whole seconds, capped at 5s; a missing or HTTP-date
    /// value falls back to 1s.
    fn retry_after(resp: &reqwest::Response) -> std::time::Duration {
        resp.headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(|secs| std::time::Duration::from_secs(secs.min(5)))
            .unwrap_or(std::time::Duration::from_secs(1))
    }

    /// Refresh the access token once per expiry, even when many requests hit a
    /// 401 concurrently. `stale_access_token` is the token that just failed; if
    /// another task already refreshed while we waited for the lock, the current
    /// in-memory token will differ and we skip the redundant refresh.
    async fn try_refresh(&self, stale_access_token: &str) -> Result<()> {
        let _guard = self.refresh_lock.lock().await;
        if self.access_token() != stale_access_token {
            return Ok(());
        }
        let (refresh_token, previous_refresh) = {
            let tokens = self.tokens.read().unwrap();
            let rt = tokens.refresh_token.clone().ok_or(Error::Unauthorized)?;
            (rt.clone(), rt)
        };
        let new_tokens = match auth::refresh(
            &self.http,
            &self.token_url,
            &refresh_token,
            Some(previous_refresh.as_str()),
        )
        .await
        {
            Ok(tokens) => tokens,
            // The access token was rejected and the refresh failed for a
            // non-transport reason: the session is gone (TIDAL answers
            // `400 invalid_grant` for a revoked refresh token). Surface it as
            // `Unauthorized` so the UI offers a re-login instead of a raw API
            // error with a Retry that can never succeed.
            Err(e @ (Error::Http(_) | Error::Io(_))) => return Err(e),
            Err(_) => return Err(Error::Unauthorized),
        };
        *self.tokens.write().unwrap() = new_tokens.clone();
        self.store.save(&new_tokens)?;
        Ok(())
    }

    async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        decode_json(Self::decode_status(resp).await?).await
    }

    /// Map a non-success status to an error, returning the response otherwise.
    async fn decode_status(resp: reqwest::Response) -> Result<reqwest::Response> {
        match resp.status() {
            s if s.is_success() => Ok(resp),
            reqwest::StatusCode::UNAUTHORIZED => Err(Error::Unauthorized),
            reqwest::StatusCode::FORBIDDEN => Err(Error::Forbidden),
            reqwest::StatusCode::NOT_FOUND => Err(Error::NotFound(resp.url().path().to_string())),
            reqwest::StatusCode::TOO_MANY_REQUESTS => Err(Error::RateLimited),
            s => Err(Error::Api { status: s.as_u16(), message: resp.text().await.unwrap_or_default() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_store::MemoryStore;
    use serde::Deserialize;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Deserialize, PartialEq, Debug)]
    struct Thing {
        value: u32,
    }

    fn session() -> SessionInfo {
        SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 }
    }

    fn tokens() -> TokenSet {
        TokenSet { access_token: "old".into(), refresh_token: Some("rt".into()),
            token_type: "Bearer".into(), expires_at: None }
    }

    #[tokio::test]
    async fn get_parses_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 7})))
            .mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store);
        let t: Thing = client.get("tracks/1", &[]).await.unwrap();
        assert_eq!(t.value, 7);
    }

    #[tokio::test]
    async fn get_maps_a_json_shape_mismatch_to_json_error_with_detail() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"value": "seven"}"#))
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        match &err {
            Error::Json(e) => {
                assert!(e.to_string().contains("expected u32"), "lost the serde detail: {e}");
            }
            other => panic!("expected Error::Json, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_maps_a_non_json_body_to_json_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>not json</html>"))
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Json(_)), "expected Error::Json, got {err:?}");
    }

    #[tokio::test]
    async fn get_refreshes_once_on_401() {
        let server = MockServer::start().await;
        // First call with old token -> 401
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        // Retry with refreshed token -> 200
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 9})))
            .mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;

        let store = Arc::new(MemoryStore::default());
        store.save(&tokens()).unwrap();
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store.clone());
        let t: Thing = client.get("tracks/1", &[]).await.unwrap();
        assert_eq!(t.value, 9);
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn concurrent_401s_refresh_exactly_once() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 5})))
            .mount(&server).await;
        // Exactly one token POST is allowed for two concurrent 401s.
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .expect(1)
            .mount(&server).await;

        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store.clone());
        let c1 = client.clone();
        let c2 = client.clone();
        let (a, b) = tokio::join!(
            async move { c1.get::<Thing>("tracks/1", &[]).await },
            async move { c2.get::<Thing>("tracks/1", &[]).await }
        );
        assert_eq!(a.unwrap().value, 5);
        assert_eq!(b.unwrap().value, 5);
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn get_without_refresh_token_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let no_refresh = TokenSet { access_token: "old".into(), refresh_token: None,
            token_type: "Bearer".into(), expires_at: None };
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), no_refresh, store);
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Unauthorized));
    }

    /// A revoked refresh token makes TIDAL answer `400 invalid_grant`; the
    /// client must surface that as `Unauthorized` so the UI can offer a
    /// re-login instead of a retryable API error.
    #[tokio::test]
    async fn refresh_rejected_with_invalid_grant_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "refresh token is invalid"
            })))
            .mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store);
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Unauthorized), "expected Unauthorized, got {err:?}");
    }

    #[tokio::test]
    async fn get_maps_429_and_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a"))
            .respond_with(ResponseTemplate::new(429)).mount(&server).await;
        Mock::given(method("GET")).and(path("/b"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        assert!(matches!(client.get::<Thing>("a", &[]).await.unwrap_err(), Error::RateLimited));
        assert!(matches!(client.get::<Thing>("b", &[]).await.unwrap_err(), Error::NotFound(_)));
    }

    #[tokio::test]
    async fn post_form_sends_form_and_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer old"))
            .and(body_string_contains("trackId=7"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn delete_maps_no_content_to_ok() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE")).and(path("/users/1/favorites/tracks/7"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        client.delete("users/1/favorites/tracks/7").await.unwrap();
    }

    #[tokio::test]
    async fn post_form_refreshes_once_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store.clone());
        client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn post_form_retries_once_on_429_with_retry_after() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let server = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));
        let responder = {
            let attempts = attempts.clone();
            move |_req: &wiremock::Request| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(429).insert_header("retry-after", "0")
                } else {
                    ResponseTemplate::new(200)
                }
            }
        };
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .respond_with(responder)
            .expect(2)
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn post_form_gives_up_after_a_second_429() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
            .expect(2)
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        let err = client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::RateLimited), "expected RateLimited, got {err:?}");
    }

    #[tokio::test]
    async fn put_json_targets_v2_without_country_code() {
        let server = MockServer::start().await;
        let responder = |req: &wiremock::Request| {
            let has_country = req.url.query_pairs().any(|(key, _)| key == "countryCode");
            if has_country {
                ResponseTemplate::new(500)
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 3}))
            }
        };
        Mock::given(method("PUT"))
            .and(path("/v2/things"))
            .and(query_param("name", "mix"))
            .respond_with(responder)
            .mount(&server)
            .await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let thing: Thing = client
            .put_json("things", &[("name", "mix".to_string())])
            .await
            .unwrap();
        assert_eq!(thing.value, 3);
    }

    #[tokio::test]
    async fn put_json_refreshes_once_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/things"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/v2/things"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 9})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .expect(1)
            .mount(&server)
            .await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            store.clone(),
        );
        let thing: Thing = client.put_json("things", &[]).await.unwrap();
        assert_eq!(thing.value, 9);
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn post_form_json_retries_once_on_429() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let server = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));
        let responder = {
            let attempts = attempts.clone();
            move |_req: &wiremock::Request| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(429).insert_header("retry-after", "0")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 4}))
                }
            }
        };
        Mock::given(method("POST"))
            .and(path("/playlists/p/items"))
            .and(body_string_contains("a=b"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let thing: Thing = client
            .post_form_json("playlists/p/items", &[], &[("a", "b".to_string())])
            .await
            .unwrap();
        assert_eq!(thing.value, 4);
    }

    #[tokio::test]
    async fn put_json_retries_once_on_429() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let server = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));
        let responder = {
            let attempts = attempts.clone();
            move |_req: &wiremock::Request| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(429).insert_header("retry-after", "0")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 6}))
                }
            }
        };
        Mock::given(method("PUT"))
            .and(path("/v2/things"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let thing: Thing = client.put_json("things", &[]).await.unwrap();
        assert_eq!(thing.value, 6);
    }

    #[tokio::test]
    async fn get_with_etag_returns_the_header_verbatim() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"e1\"")
                    .set_body_json(serde_json::json!({"value": 1})),
            )
            .mount(&server)
            .await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let (thing, etag) = client.get_with_etag::<Thing>("playlists/p", &[]).await.unwrap();
        assert_eq!(thing.value, 1);
        assert_eq!(etag.as_deref(), Some("\"e1\""));
    }

    #[tokio::test]
    async fn post_form_with_etag_sends_if_none_match() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p/items"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        client
            .post_form_with_etag("playlists/p/items", &[], Some("\"e1\""))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_401_after_a_successful_refresh_is_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Forbidden), "expected Forbidden, got {err:?}");
    }

    #[tokio::test]
    async fn post_form_401_after_a_successful_refresh_is_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/playlists/p"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/playlists/p"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let err = client.post_form("playlists/p", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Forbidden), "expected Forbidden, got {err:?}");
    }

    #[tokio::test]
    async fn get_v2_targets_v2_without_country_code() {
        let server = MockServer::start().await;
        let responder = |req: &wiremock::Request| {
            let has_country = req.url.query_pairs().any(|(key, _)| key == "countryCode");
            if has_country {
                ResponseTemplate::new(500)
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 8}))
            }
        };
        Mock::given(method("GET")).and(path("/v2/things"))
            .and(query_param("limit", "50"))
            .respond_with(responder)
            .mount(&server).await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let thing: Thing = client
            .get_v2("things", &[("limit", "50".to_string())])
            .await
            .unwrap();
        assert_eq!(thing.value, 8);
    }
}
