use serde::Deserialize;

use crate::client::decode_json;
use crate::error::{Error, Result};
use crate::pkce::{Pkce, PKCE_CLIENT_ID};
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
    redirect_uri: &str,
    redirect_url: &str,
) -> Result<TokenSet> {
    let code = extract_code(redirect_url)?;
    let params = [
        ("code", code.as_str()),
        ("client_id", PKCE_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
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
        return Err(Error::Api {
            status: status.as_u16(),
            message,
        });
    }
    let raw: RawToken = decode_json(resp).await?;
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
        return Err(Error::Api {
            status: status.as_u16(),
            message,
        });
    }
    let raw: RawToken = decode_json(resp).await?;
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
        s if s.is_success() => decode_json(resp).await,
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

/// The `state` query parameter, when present (used for the paste-back redirect,
/// which is not validated as a `tidal://` URL).
pub fn extract_state(redirect_url: &str) -> Option<String> {
    url::Url::parse(redirect_url)
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
}

/// A validated login redirect: the authorization code plus the optional
/// `state` TIDAL echoes back.
#[derive(Debug, PartialEq)]
pub struct RedirectOutcome {
    pub code: String,
    pub state: Option<String>,
}

/// Validate that a URL is the TIDAL login redirect for OxiTide and read its
/// `code`/`state`. Rejects anything that is not `tidal://login/auth`.
pub fn validate_redirect(url_str: &str) -> Result<RedirectOutcome> {
    let parsed =
        url::Url::parse(url_str).map_err(|e| Error::Auth(format!("invalid redirect url: {e}")))?;
    if parsed.scheme() != "tidal" || parsed.host_str() != Some("login") || parsed.path() != "/auth"
    {
        return Err(Error::Auth("unexpected redirect target".into()));
    }
    let mut code = None;
    let mut state = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            _ => {}
        }
    }
    let code = code.ok_or_else(|| Error::Auth("redirect url has no code".into()))?;
    Ok(RedirectOutcome { code, state })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pkce::PKCE_REDIRECT_PASTE;
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

    #[test]
    fn extract_state_reads_query_param() {
        assert_eq!(
            extract_state("https://tidal.com/android/login/auth?code=ABC&state=na").as_deref(),
            Some("na")
        );
        assert_eq!(
            extract_state("https://tidal.com/android/login/auth?code=ABC"),
            None
        );
    }

    #[test]
    fn validate_redirect_accepts_tidal_login() {
        let out = validate_redirect("tidal://login/auth?code=ABC&state=s1").unwrap();
        assert_eq!(out.code, "ABC");
        assert_eq!(out.state.as_deref(), Some("s1"));
    }

    #[test]
    fn validate_redirect_rejects_other_targets() {
        for url in [
            "https://tidal.com/android/login/auth?code=ABC",
            "oxitide://login/auth?code=ABC",
            "tidal://other/auth?code=ABC",
            "tidal://login/wrong?code=ABC",
            "tidal://login/auth?state=only",
        ] {
            assert!(validate_redirect(url).is_err(), "should reject {url}");
        }
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
        let tokens = exchange_code(
            &http,
            &format!("{}/token", server.uri()),
            &pkce,
            PKCE_REDIRECT_PASTE,
            "https://tidal.com/android/login/auth?code=XYZ",
        )
        .await
        .unwrap();
        assert_eq!(tokens.access_token, "AT");
        assert_eq!(tokens.refresh_token.as_deref(), Some("RT"));
        assert!(tokens.expires_at.is_some());
    }

    #[tokio::test]
    async fn exchange_code_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let err = exchange_code(
            &http,
            &format!("{}/token", server.uri()),
            &pkce,
            PKCE_REDIRECT_PASTE,
            "https://tidal.com/android/login/auth?code=XYZ",
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::Unauthorized));
    }

    #[tokio::test]
    async fn exchange_code_maps_invalid_json_body_to_json_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>login</html>"))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let err = exchange_code(
            &http,
            &format!("{}/token", server.uri()),
            &pkce,
            PKCE_REDIRECT_PASTE,
            "https://tidal.com/android/login/auth?code=XYZ",
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, Error::Json(_)),
            "expected Error::Json, got {err:?}"
        );
    }

    #[tokio::test]
    async fn refresh_maps_invalid_json_body_to_json_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>login</html>"))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let err = refresh(
            &http,
            &format!("{}/token", server.uri()),
            "old-rt",
            Some("old-rt"),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, Error::Json(_)),
            "expected Error::Json, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_session_maps_invalid_json_body_to_json_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>nope</html>"))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let err = fetch_session(&http, &(server.uri() + "/"), "AT")
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Json(_)),
            "expected Error::Json, got {err:?}"
        );
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
        let tokens = refresh(
            &http,
            &format!("{}/token", server.uri()),
            "old-rt",
            Some("old-rt"),
        )
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
        let tokens = refresh(
            &http,
            &format!("{}/token", server.uri()),
            "old-rt",
            Some("old-rt"),
        )
        .await
        .unwrap();
        assert_eq!(tokens.refresh_token.as_deref(), Some("new-rt"));
    }

    #[tokio::test]
    async fn fetch_session_parses_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "sid", "countryCode": "GR", "userId": 191812568
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let info = fetch_session(&http, &(server.uri() + "/"), "AT")
            .await
            .unwrap();
        assert_eq!(info.session_id, "sid");
        assert_eq!(info.country_code, "GR");
        assert_eq!(info.user_id, 191812568);
    }
}
