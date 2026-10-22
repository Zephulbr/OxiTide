use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::auth::{self, SessionInfo};
use crate::error::{display_chain, is_transport, Error, Result};
use crate::models::ItemPage;
use crate::token_store::{TokenSet, TokenStore};

/// Default page size used when transparently walking paginated endpoints.
pub const DEFAULT_PAGE_SIZE: u32 = 100;

/// Windows of a paginated walk in flight at once once the total is known
/// (the UI pager uses the same bound against the same API).
pub const PAGE_CONCURRENCY: usize = 4;

/// Attempts for one GET when the network fails before a response arrives or
/// a gateway answers 502/503/504. GETs are idempotent, so this is safe.
const GET_ATTEMPTS: u32 = 3;

/// The HTTP client for TIDAL API calls.
///
/// Playback leaves the API idle for a whole track, and routers/NATs and
/// TIDAL's edge silently drop idle keep-alive connections; reusing one fails
/// with "error sending request". Idle sockets are therefore retired after
/// 30 s, TCP keepalive holds NAT entries open for the ones in use, and every
/// request has a deadline so a dead socket cannot stall the stream resolver.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(30))
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Wait before GET attempt `attempt + 1`: 250 ms, then 750 ms.
fn get_backoff(attempt: u32) -> Duration {
    Duration::from_millis(250 * 3u64.pow(attempt.saturating_sub(1)))
}

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

/// The JSON:API media type the OpenAPI host requires on request bodies.
const JSON_API: &str = "application/vnd.api+json";

/// A write's request body.
#[derive(Clone, Copy)]
enum Body<'a> {
    Empty,
    Form(&'a [(&'a str, String)]),
    /// A JSON:API document and the `Idempotency-Key` every resend repeats.
    JsonApi(&'a serde_json::Value, &'a str),
}

/// A random version-4 UUID for the OpenAPI `Idempotency-Key` header.
fn idempotency_key() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS random source unavailable");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[derive(Clone)]
pub struct TidalClient {
    http: reqwest::Client,
    base_url: String,
    v2_base_url: String,
    openapi_base_url: String,
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
        // The public OpenAPI host (JSON:API). The internal v2 endpoint omits
        // playlist follower counts, and the official host accepts the same
        // session token. Tests get a sibling path so wiremock can serve it.
        let openapi_base_url = if trimmed == "https://api.tidal.com/v1" {
            "https://openapi.tidal.com/v2".to_string()
        } else {
            format!("{trimmed}/openapi/v2")
        };
        Self {
            http,
            base_url,
            v2_base_url,
            openapi_base_url,
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

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
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
        self.mutate(
            reqwest::Method::POST,
            &self.base_url,
            path,
            &[],
            Body::Form(form),
            etag,
        )
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
        self.mutate(
            reqwest::Method::DELETE,
            &self.base_url,
            path,
            &[],
            Body::Empty,
            etag,
        )
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
            .mutate(
                reqwest::Method::PUT,
                &self.v2_base_url,
                path,
                params,
                Body::Empty,
                None,
            )
            .await?;
        decode_json(resp).await
    }

    /// PUT with query params on the v2 API, discarding the response body (several
    /// v2 collection writes answer 204).
    pub async fn put_v2(&self, path: &str, params: &[(&str, String)]) -> Result<()> {
        self.mutate(
            reqwest::Method::PUT,
            &self.v2_base_url,
            path,
            params,
            Body::Empty,
            None,
        )
        .await
        .map(|_| ())
    }

    /// POST a JSON:API document on the public OpenAPI base and decode the
    /// response. One `Idempotency-Key` covers the request and its resends (401
    /// refresh, 429), so a write TIDAL already applied is replayed, not repeated.
    /// Callers pass `countryCode` explicitly, as with `get_openapi`.
    pub async fn post_openapi<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        doc: &serde_json::Value,
    ) -> Result<T> {
        let key = idempotency_key();
        let resp = self
            .mutate(
                reqwest::Method::POST,
                &self.openapi_base_url,
                path,
                params,
                Body::JsonApi(doc, &key),
                None,
            )
            .await?;
        decode_json(resp).await
    }

    /// DELETE on the public OpenAPI base, with a JSON:API document when the
    /// endpoint takes one (relationship removals), discarding the response.
    pub async fn delete_openapi(
        &self,
        path: &str,
        params: &[(&str, String)],
        doc: Option<&serde_json::Value>,
    ) -> Result<()> {
        let key = idempotency_key();
        let body = match doc {
            Some(doc) => Body::JsonApi(doc, &key),
            None => Body::Empty,
        };
        self.mutate(
            reqwest::Method::DELETE,
            &self.openapi_base_url,
            path,
            params,
            body,
            None,
        )
        .await
        .map(|_| ())
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
            .mutate(
                reqwest::Method::POST,
                &self.base_url,
                path,
                params,
                Body::Form(form),
                None,
            )
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
            .mutate(
                reqwest::Method::POST,
                &self.base_url,
                path,
                params,
                Body::Form(form),
                etag,
            )
            .await?;
        decode_json(resp).await
    }

    /// Walk every page of a TIDAL list endpoint, following `totalNumberOfItems`
    /// and `offset`, returning the concatenated `items`.
    ///
    /// Offsets index the full scanned list: TIDAL omits unavailable rows from
    /// `items` while still counting them in `totalNumberOfItems`, so each
    /// request advances by its requested window size. Advancing by the received
    /// count would re-scan the tail of the previous window and return
    /// duplicates.
    ///
    /// Once the first page reports a total, the remaining windows are known
    /// and are fetched [`PAGE_CONCURRENCY`] at a time, appended in offset
    /// order (Track 13: a 2 000-track playlist is 20 round trips, no longer
    /// 20 sequential ones). Without a total the walk stays sequential.
    pub async fn get_all_pages<T: DeserializeOwned>(
        &self,
        path: &str,
        base_params: &[(&str, String)],
        page_size: u32,
    ) -> Result<Vec<T>> {
        let fetch = |offset: u32| {
            let mut params: Vec<(&str, String)> = base_params.to_vec();
            params.push(("limit", page_size.to_string()));
            params.push(("offset", offset.to_string()));
            async move { self.get::<ItemPage<T>>(path, &params).await }
        };
        let first = fetch(0).await?;
        let received = first.items.len() as u32;
        let total = first.total_number_of_items;
        let mut all: Vec<T> = first.items;
        if received == 0 {
            return Ok(all);
        }
        if total > 0 {
            let offsets: Vec<u32> = (1..)
                .map(|i: u32| i.saturating_mul(page_size))
                .take_while(|offset| *offset < total)
                .collect();
            for group in offsets.chunks(PAGE_CONCURRENCY) {
                let pages =
                    futures_util::future::try_join_all(group.iter().map(|&offset| fetch(offset))).await?;
                let mut empty = false;
                for page in pages {
                    empty |= page.items.is_empty();
                    all.extend(page.items);
                }
                // Same stop rule as the sequential walk: an empty window
                // means the server has nothing further.
                if empty {
                    break;
                }
            }
            return Ok(all);
        }
        // No total reported: walk until a short page.
        let mut offset = page_size;
        let mut last = received;
        while last == page_size {
            let page = fetch(offset).await?;
            last = page.items.len() as u32;
            all.extend(page.items);
            offset = offset.saturating_add(page_size);
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
        let url = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let mut attempt = 1;
        loop {
            let result = self
                .http
                .get(&url)
                .bearer_auth(token)
                .query(&query)
                .send()
                .await;
            let reason = match &result {
                Err(e) if is_transport(e) => display_chain(e),
                Ok(resp) if matches!(resp.status().as_u16(), 502..=504) => {
                    format!("status {}", resp.status())
                }
                _ => return Ok(result?),
            };
            if attempt >= GET_ATTEMPTS {
                return Ok(result?);
            }
            let wait = get_backoff(attempt);
            tracing::warn!(
                "GET {path} failed ({reason}); retrying in {wait:?} ({attempt}/{GET_ATTEMPTS})"
            );
            tokio::time::sleep(wait).await;
            attempt += 1;
        }
    }

    /// GET on the v2 API base (no `countryCode`), sharing `get`'s refresh-on-401.
    pub async fn get_v2<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        self.get_at_base(&self.v2_base_url, path, params).await
    }

    /// GET on the public OpenAPI base (JSON:API). Callers pass `countryCode`
    /// explicitly; the base does not get it injected like the v1 base.
    pub async fn get_openapi<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        self.get_at_base(&self.openapi_base_url, path, params).await
    }

    /// Shared non-v1 GET: refresh-on-401, then decode.
    async fn get_at_base<T: DeserializeOwned>(
        &self,
        base: &str,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        let token = self.access_token();
        let mut resp = self.send_get_at(base, path, params, &token).await?;
        let mut refreshed = false;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            refreshed = true;
            resp = self
                .send_get_at(base, path, params, &self.access_token())
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
        body: Body<'_>,
        etag: Option<&str>,
    ) -> Result<reqwest::Response> {
        let country_code = base == self.base_url.as_str();
        let token = self.access_token();
        let mut refreshed = false;
        let mut resp = self
            .send_mutation(
                &method,
                base,
                path,
                params,
                body,
                &token,
                country_code,
                etag,
            )
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
                    body,
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
                    body,
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
        body: Body<'_>,
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
        let send = || {
            let mut request = self
                .http
                .request(method.clone(), &url)
                .bearer_auth(token)
                .query(&query);
            match body {
                Body::Empty | Body::Form([]) => {}
                Body::Form(form) => request = request.form(form),
                Body::JsonApi(doc, key) => {
                    request = request
                        .header(reqwest::header::CONTENT_TYPE, JSON_API)
                        .header(reqwest::header::ACCEPT, JSON_API)
                        .header("Idempotency-Key", key)
                        .body(doc.to_string());
                }
            }
            if let Some(value) = etag {
                request = request.header(reqwest::header::IF_NONE_MATCH, value);
            }
            request.send()
        };
        match send().await {
            // Writes are not idempotent: retry only when the connection never
            // opened, so the server cannot have seen the request.
            Err(e) if e.is_connect() => {
                let wait = get_backoff(1);
                tracing::warn!(
                    "{method} {path} could not connect ({}); retrying in {wait:?}",
                    display_chain(&e)
                );
                tokio::time::sleep(wait).await;
                Ok(send().await?)
            }
            result => Ok(result?),
        }
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
            s => Err(Error::Api {
                status: s.as_u16(),
                message: resp.text().await.unwrap_or_default(),
            }),
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
        SessionInfo {
            session_id: "s".into(),
            country_code: "GR".into(),
            user_id: 1,
        }
    }

    fn tokens() -> TokenSet {
        TokenSet {
            access_token: "old".into(),
            refresh_token: Some("rt".into()),
            token_type: "Bearer".into(),
            expires_at: None,
        }
    }

    #[tokio::test]
    async fn get_parses_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 7})))
            .mount(&server)
            .await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            store,
        );
        let t: Thing = client.get("tracks/1", &[]).await.unwrap();
        assert_eq!(t.value, 7);
    }

    #[tokio::test]
    async fn get_maps_a_json_shape_mismatch_to_json_error_with_detail() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"value": "seven"}"#))
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
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        match &err {
            Error::Json(e) => {
                assert!(
                    e.to_string().contains("expected u32"),
                    "lost the serde detail: {e}"
                );
            }
            other => panic!("expected Error::Json, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_maps_a_non_json_body_to_json_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>not json</html>"))
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
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(
            matches!(err, Error::Json(_)),
            "expected Error::Json, got {err:?}"
        );
    }

    #[tokio::test]
    async fn get_refreshes_once_on_401() {
        let server = MockServer::start().await;
        // First call with old token -> 401
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        // Retry with refreshed token -> 200
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 9})))
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;

        let store = Arc::new(MemoryStore::default());
        store.save(&tokens()).unwrap();
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            store.clone(),
        );
        let t: Thing = client.get("tracks/1", &[]).await.unwrap();
        assert_eq!(t.value, 9);
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn concurrent_401s_refresh_exactly_once() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 5})))
            .mount(&server)
            .await;
        // Exactly one token POST is allowed for two concurrent 401s.
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .expect(1)
            .mount(&server).await;

        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            store.clone(),
        );
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
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let store = Arc::new(MemoryStore::default());
        let no_refresh = TokenSet {
            access_token: "old".into(),
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_at: None,
        };
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            no_refresh,
            store,
        );
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Unauthorized));
    }

    /// A revoked refresh token makes TIDAL answer `400 invalid_grant`; the
    /// client must surface that as `Unauthorized` so the UI can offer a
    /// re-login instead of a retryable API error.
    #[tokio::test]
    async fn refresh_rejected_with_invalid_grant_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "refresh token is invalid"
            })))
            .mount(&server)
            .await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            store,
        );
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(
            matches!(err, Error::Unauthorized),
            "expected Unauthorized, got {err:?}"
        );
    }

    #[tokio::test]
    async fn get_maps_429_and_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/a"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/b"))
            .respond_with(ResponseTemplate::new(404))
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
        assert!(matches!(
            client.get::<Thing>("a", &[]).await.unwrap_err(),
            Error::RateLimited
        ));
        assert!(matches!(
            client.get::<Thing>("b", &[]).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn post_form_sends_form_and_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer old"))
            .and(body_string_contains("trackId=7"))
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
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn delete_maps_no_content_to_ok() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/users/1/favorites/tracks/7"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(204))
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
        client.delete("users/1/favorites/tracks/7").await.unwrap();
    }

    #[tokio::test]
    async fn post_form_refreshes_once_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            store.clone(),
        );
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
        Mock::given(method("POST"))
            .and(path("/users/1/favorites/tracks"))
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
        client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn post_form_gives_up_after_a_second_429() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/1/favorites/tracks"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
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
        let err = client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::RateLimited),
            "expected RateLimited, got {err:?}"
        );
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
    async fn post_openapi_sends_json_api_and_keeps_its_idempotency_key_across_a_refresh() {
        use std::sync::Mutex;
        let server = MockServer::start().await;
        let keys = Arc::new(Mutex::new(Vec::<String>::new()));
        let responder = {
            let keys = keys.clone();
            move |req: &wiremock::Request| {
                let key = req
                    .headers
                    .get("idempotency-key")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                keys.lock().unwrap().push(key);
                let auth = req.headers.get("authorization").and_then(|v| v.to_str().ok());
                if auth == Some("Bearer old") {
                    ResponseTemplate::new(401)
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 5}))
                }
            }
        };
        Mock::given(method("POST"))
            .and(path("/openapi/v2/things"))
            .and(header("content-type", "application/vnd.api+json"))
            .and(query_param("countryCode", "GR"))
            .and(body_string_contains("\"type\":\"tracks\""))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
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
        let doc = serde_json::json!({"data": [{"type": "tracks", "id": "7"}]});
        let thing: Thing = client
            .post_openapi("things", &[("countryCode", "GR".to_string())], &doc)
            .await
            .unwrap();
        assert_eq!(thing.value, 5);
        let keys = keys.lock().unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].len(), 36, "a UUID key is sent");
        assert_eq!(keys[0], keys[1], "the resend repeats the key");
    }

    #[tokio::test]
    async fn delete_openapi_sends_the_document_when_given() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/openapi/v2/things/relationships/items"))
            .and(header("content-type", "application/vnd.api+json"))
            .and(body_string_contains("\"id\":\"7\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/openapi/v2/shares/s-1"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
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
        let doc = serde_json::json!({"data": [{"type": "tracks", "id": "7"}]});
        client
            .delete_openapi("things/relationships/items", &[], Some(&doc))
            .await
            .unwrap();
        client.delete_openapi("shares/s-1", &[], None).await.unwrap();
    }

    #[test]
    fn idempotency_keys_are_distinct_v4_uuids() {
        let a = idempotency_key();
        let b = idempotency_key();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4", "version nibble");
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
        Mock::given(method("GET"))
            .and(path("/playlists/p"))
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
        let (thing, etag) = client
            .get_with_etag::<Thing>("playlists/p", &[])
            .await
            .unwrap();
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
    async fn get_all_pages_advances_by_the_requested_window_not_the_received_count() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/things"))
            .and(query_param("offset", "0"))
            .and(query_param("limit", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"value": 1}],
                "totalNumberOfItems": 3, "limit": 2, "offset": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/things"))
            .and(query_param("offset", "2"))
            .and(query_param("limit", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"value": 2}, {"value": 3}],
                "totalNumberOfItems": 3, "limit": 2, "offset": 2
            })))
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
        let things: Vec<Thing> = client.get_all_pages("things", &[], 2).await.unwrap();
        assert_eq!(
            things.len(),
            3,
            "a page omitted an unavailable row; the walk must not re-read it"
        );
        assert_eq!(things[0].value, 1);
        assert_eq!(things[1].value, 2);
        assert_eq!(things[2].value, 3);
    }

    /// Track 13: windows after the first run concurrently (later ones answer
    /// first here) yet land in offset order, and each is requested once.
    #[tokio::test]
    async fn get_all_pages_fetches_known_windows_concurrently_in_order() {
        let server = MockServer::start().await;
        for page in 0..5u64 {
            let offset = page * 2;
            let items: Vec<_> = (offset..(offset + 2).min(9))
                .map(|v| serde_json::json!({ "value": v }))
                .collect();
            Mock::given(method("GET"))
                .and(path("/things"))
                .and(query_param("offset", offset.to_string()))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_delay(std::time::Duration::from_millis(80 - page * 15))
                        .set_body_json(serde_json::json!({
                            "items": items, "totalNumberOfItems": 9, "limit": 2, "offset": offset
                        })),
                )
                .expect(1)
                .mount(&server)
                .await;
        }
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let things: Vec<Thing> = client.get_all_pages("things", &[], 2).await.unwrap();
        let values: Vec<u64> = things.iter().map(|t| t.value as u64).collect();
        assert_eq!(values, (0..9).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn get_all_pages_without_a_total_walks_to_a_short_page() {
        let server = MockServer::start().await;
        for (offset, items) in [(0, vec![1, 2]), (2, vec![3, 4]), (4, vec![5])] {
            let items: Vec<_> = items.into_iter().map(|v| serde_json::json!({ "value": v })).collect();
            Mock::given(method("GET"))
                .and(path("/things"))
                .and(query_param("offset", offset.to_string()))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": items, "totalNumberOfItems": 0, "limit": 2, "offset": offset
                })))
                .expect(1)
                .mount(&server)
                .await;
        }
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let things: Vec<Thing> = client.get_all_pages("things", &[], 2).await.unwrap();
        assert_eq!(things.iter().map(|t| t.value).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn get_401_after_a_successful_refresh_is_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
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
        assert!(
            matches!(err, Error::Forbidden),
            "expected Forbidden, got {err:?}"
        );
    }

    #[tokio::test]
    async fn post_form_401_after_a_successful_refresh_is_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/playlists/p"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
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
        assert!(
            matches!(err, Error::Forbidden),
            "expected Forbidden, got {err:?}"
        );
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
        Mock::given(method("GET"))
            .and(path("/v2/things"))
            .and(query_param("limit", "50"))
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
            .get_v2("things", &[("limit", "50".to_string())])
            .await
            .unwrap();
        assert_eq!(thing.value, 8);
    }

    #[tokio::test]
    async fn put_v2_targets_v2_and_tolerates_an_empty_body() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/folders/rename"))
            .and(query_param("name", "New"))
            .respond_with(ResponseTemplate::new(204))
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
            .put_v2("folders/rename", &[("name", "New".to_string())])
            .await
            .unwrap();
    }

    fn client_at(base: String) -> TidalClient {
        TidalClient::new(
            http_client(),
            base.clone(),
            format!("{base}/token"),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        )
    }

    /// A raw HTTP server that hangs up on the first `drops` connections after
    /// reading their request (as a stale keep-alive socket or a reset does),
    /// then answers `{"value": 5}`. Returns its base URL and a count of
    /// accepted connections.
    fn flaky_server(drops: usize) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let n = count.fetch_add(1, Ordering::SeqCst);
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => request.extend_from_slice(&buf[..read]),
                    }
                }
                if n < drops {
                    continue; // dropping `stream` closes it without a response
                }
                let body = r#"{"value": 5}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (base, accepted)
    }

    #[tokio::test]
    async fn get_retries_a_connection_dropped_before_the_response() {
        let (base, accepted) = flaky_server(2);
        let t: Thing = client_at(base)
            .get("tracks/1/playbackinfo", &[])
            .await
            .unwrap();
        assert_eq!(t.value, 5);
        assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn get_gives_up_after_three_dropped_connections() {
        let (base, accepted) = flaky_server(usize::MAX);
        let err = client_at(base)
            .get::<Thing>("tracks/1", &[])
            .await
            .unwrap_err();
        assert!(err.is_transient(), "got {err:?}");
        assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn get_retries_a_gateway_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 4})))
            .mount(&server)
            .await;
        let t: Thing = client_at(server.uri()).get("tracks/1", &[]).await.unwrap();
        assert_eq!(t.value, 4);
    }

    #[tokio::test]
    async fn get_does_not_retry_a_plain_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;
        let err = client_at(server.uri())
            .get::<Thing>("tracks/1", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Api { status: 500, .. }), "got {err:?}");
    }

    #[tokio::test]
    async fn a_write_is_not_resent_after_the_request_went_out() {
        let (base, accepted) = flaky_server(usize::MAX);
        let err = client_at(base)
            .post_form("users/1/favorites/tracks", &[("trackIds", "7".to_string())])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Http(_)), "got {err:?}");
        assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
