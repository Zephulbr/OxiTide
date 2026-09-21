use serde::Deserialize;

use crate::error::{Error, Result};
use crate::pkce::{Pkce, PKCE_CLIENT_ID, PKCE_REDIRECT_URI};
use crate::token_store::TokenSet;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SessionInfo {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "countryCode")]
    pub country_code: String,
    #[serde(rename = "userId")]
    pub user_id: u64,
}

#[derive(Deserialize)]
struct RawToken {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default = "bearer")]
    token_type: String,
    #[serde(default)]
    expires_in: Option<i64>,
}

fn bearer() -> String {
    "Bearer".into()
}

fn raw_to_tokens(raw: RawToken, previous_refresh: Option<&str>) -> TokenSet {
    let expires_at = raw.expires_in.map(|s| now_unix() + s);
    let refresh_token = raw
        .refresh_token
        .or_else(|| previous_refresh.map(|s| s.to_string()));
    TokenSet {
        access_token: raw.access_token,
        refresh_token,
        token_type: raw.token_type,
        expires_at,
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn exchange_code(
    http: &reqwest::Client,
    token_url: &str,
    pkce: &Pkce,
    redirect_url: &str,
) -> Result<TokenSet> {
    let code = extract_code(redirect_url)?;
    let params = [
        ("code", code.as_str()),
        ("client_id", PKCE_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("redirect_uri", PKCE_REDIRECT_URI),
        ("scope", "r_usr w_usr w_sub"),
        ("code_verifier", pkce.code_verifier.as_str()),
        ("client_unique_key", pkce.client_unique_key.as_str()),
    ];
    let resp = http.post(token_url).form(&params).send().await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(Error::Unauthorized);
    }
    if !status.is_success() {
        let message = resp.text().await.unwrap_or_default();
        return Err(Error::Api { status: status.as_u16(), message });
    }
    let raw: RawToken = resp.json().await?;
    if raw.access_token.is_empty() {
        return Err(Error::Auth("empty access token".into()));
    }
    Ok(raw_to_tokens(raw, None))
}

pub async fn refresh(
    http: &reqwest::Client,
    token_url: &str,
    refresh_token: &str,
    previous_refresh: Option<&str>,
) -> Result<TokenSet> {
    let params = [
        ("client_id", PKCE_CLIENT_ID),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", "r_usr w_usr w_sub"),
    ];
    let resp = http.post(token_url).form(&params).send().await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(Error::Unauthorized);
    }
    if !status.is_success() {
        let message = resp.text().await.unwrap_or_default();
        return Err(Error::Api { status: status.as_u16(), message });
    }
    let raw: RawToken = resp.json().await?;
    Ok(raw_to_tokens(raw, previous_refresh.or(Some(refresh_token))))
}

pub async fn fetch_session(
    http: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<SessionInfo> {
    let url = format!("{base_url}sessions");
    let resp = http.get(url).bearer_auth(access_token).send().await?;
    match resp.status() {
        s if s.is_success() => Ok(resp.json().await?),
        reqwest::StatusCode::UNAUTHORIZED => Err(Error::Unauthorized),
        reqwest::StatusCode::FORBIDDEN => Err(Error::Forbidden),
        s => Err(Error::Api {
            status: s.as_u16(),
            message: resp.text().await.unwrap_or_default(),
        }),
    }
}

pub fn extract_code(redirect_url: &str) -> Result<String> {
    let parsed = url::Url::parse(redirect_url)
        .map_err(|e| Error::Auth(format!("invalid redirect url: {e}")))?;
    parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| Error::Auth("redirect url has no code".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn extract_code_reads_query_param() {
        let url = "https://tidal.com/android/login/auth?code=ABC123&state=na";
        assert_eq!(extract_code(url).unwrap(), "ABC123");
    }

    #[test]
    fn extract_code_errors_without_code() {
        let e = extract_code("https://tidal.com/android/login/auth?state=na").unwrap_err();
        assert!(matches!(e, Error::Auth(_)));
    }

    #[tokio::test]
    async fn exchange_code_parses_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT", "refresh_token": "RT", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let tokens = exchange_code(&http, &format!("{}/token", server.uri()), &pkce,
            "https://tidal.com/android/login/auth?code=XYZ").await.unwrap();
        assert_eq!(tokens.access_token, "AT");
        assert_eq!(tokens.refresh_token.as_deref(), Some("RT"));
        assert!(tokens.expires_at.is_some());
    }

    #[tokio::test]
    async fn exchange_code_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        let http = reqwest::Client::new();
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let err = exchange_code(&http, &format!("{}/token", server.uri()), &pkce,
            "https://tidal.com/android/login/auth?code=XYZ").await.unwrap_err();
        assert!(matches!(err, Error::Unauthorized));
    }

    #[tokio::test]
    async fn refresh_keeps_previous_refresh_token_when_omitted() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let tokens = refresh(&http, &format!("{}/token", server.uri()), "old-rt", Some("old-rt"))
            .await
            .unwrap();
        assert_eq!(tokens.access_token, "AT2");
        assert_eq!(tokens.refresh_token.as_deref(), Some("old-rt"));
    }

    #[tokio::test]
    async fn refresh_replaces_refresh_token_when_present() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT3", "refresh_token": "new-rt", "token_type": "Bearer"
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let tokens = refresh(&http, &format!("{}/token", server.uri()), "old-rt", Some("old-rt"))
            .await
            .unwrap();
        assert_eq!(tokens.refresh_token.as_deref(), Some("new-rt"));
    }

    #[tokio::test]
    async fn fetch_session_parses_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "sid", "countryCode": "GR", "userId": 191812568
            })))
            .mount(&server).await;
        let http = reqwest::Client::new();
        let info = fetch_session(&http, &(server.uri() + "/"), "AT").await.unwrap();
        assert_eq!(info.session_id, "sid");
        assert_eq!(info.country_code, "GR");
        assert_eq!(info.user_id, 191812568);
    }
}
