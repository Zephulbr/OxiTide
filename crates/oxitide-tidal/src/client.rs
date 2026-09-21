use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;

use crate::auth::{self, SessionInfo};
use crate::error::{Error, Result};
use crate::token_store::{TokenSet, TokenStore};

#[derive(Clone)]
pub struct TidalClient {
    http: reqwest::Client,
    base_url: String,
    token_url: String,
    session: SessionInfo,
    tokens: Arc<RwLock<TokenSet>>,
    store: Arc<dyn TokenStore>,
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
        Self {
            http,
            base_url: base_url.into(),
            token_url: token_url.into(),
            session,
            tokens: Arc::new(RwLock::new(tokens)),
            store,
        }
    }

    pub fn session(&self) -> &SessionInfo {
        &self.session
    }

    fn access_token(&self) -> String {
        self.tokens.read().unwrap().access_token.clone()
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)]) -> Result<T> {
        let resp = self.send_get(path, params, &self.access_token()).await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh().await?;
            let retry = self.send_get(path, params, &self.access_token()).await?;
            return Self::decode(retry).await;
        }
        Self::decode(resp).await
    }

    async fn send_get(
        &self,
        path: &str,
        params: &[(&str, String)],
        token: &str,
    ) -> Result<reqwest::Response> {
        let mut query: Vec<(&str, String)> = vec![("countryCode", self.session.country_code.clone())];
        query.extend(params.iter().cloned());
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        Ok(self.http.get(url).bearer_auth(token).query(&query).send().await?)
    }

    async fn try_refresh(&self) -> Result<()> {
        let refresh_token = self
            .tokens
            .read()
            .unwrap()
            .refresh_token
            .clone()
            .ok_or(Error::Unauthorized)?;
        let new_tokens = auth::refresh(&self.http, &self.token_url, &refresh_token).await?;
        *self.tokens.write().unwrap() = new_tokens.clone();
        self.store.save(&new_tokens)?;
        Ok(())
    }

    async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        match resp.status() {
            s if s.is_success() => Ok(resp.json().await?),
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
    use wiremock::matchers::{method, path};
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
}
