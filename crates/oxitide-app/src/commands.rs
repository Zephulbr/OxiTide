use std::sync::Arc;

use oxitide_tidal::auth::{self, SessionInfo};
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::{Pkce, PKCE_CLIENT_ID, TOKEN_URL};
use oxitide_tidal::token_store::{KeyringStore, TokenSet, TokenStore};
use oxitide_tidal::Error;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

pub const API_BASE: &str = "https://api.tidal.com/v1/";

#[derive(Serialize)]
pub struct LoginPrompt {
    pub url: String,
}

#[derive(Serialize)]
pub struct SessionInfoDto {
    pub country_code: String,
    pub user_id: u64,
}

#[tauri::command]
pub fn auth_begin(state: State<'_, AppState>) -> Result<LoginPrompt, String> {
    let pkce = Pkce::generate();
    let url = pkce.login_url(PKCE_CLIENT_ID);
    state.set_pending(pkce);
    Ok(LoginPrompt { url })
}

#[tauri::command]
pub async fn auth_complete(
    state: State<'_, AppState>,
    redirect_url: String,
) -> Result<SessionInfoDto, String> {
    let pkce = state.take_pending().ok_or_else(|| "no login in progress".to_string())?;
    let http = reqwest::Client::new();
    let tokens = auth::exchange_code(&http, TOKEN_URL, &pkce, &redirect_url)
        .await
        .map_err(|e| e.to_string())?;
    let session = auth::fetch_session(&http, API_BASE, &tokens.access_token)
        .await
        .map_err(|e| e.to_string())?;
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore);
    store.save(&tokens).map_err(|e| e.to_string())?;
    let client = TidalClient::new(
        http,
        API_BASE,
        TOKEN_URL,
        session.clone(),
        tokens,
        store,
    );
    state.set_client(client);
    Ok(SessionInfoDto { country_code: session.country_code, user_id: session.user_id })
}

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> bool {
    state.is_logged_in()
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> Result<(), String> {
    let store = KeyringStore;
    store.clear().map_err(|e| e.to_string())?;
    state.logout();
    Ok(())
}

/// Load a persisted session from the store, refreshing on `401`.
///
/// Returns `Ok(None)` when there is no stored token or it can no longer be
/// used, so the UI can fall back to the login screen without a hard error.
async fn restore_tokens(
    http: &reqwest::Client,
    base_url: &str,
    token_url: &str,
    store: &dyn TokenStore,
) -> Result<Option<(SessionInfo, TokenSet)>, String> {
    let tokens = match store.load() {
        Ok(Some(tokens)) => tokens,
        Ok(None) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    match auth::fetch_session(http, base_url, &tokens.access_token).await {
        Ok(session) => Ok(Some((session, tokens))),
        Err(Error::Unauthorized) => {
            let refresh_token = match tokens.refresh_token.clone() {
                Some(rt) => rt,
                None => return Ok(None),
            };
            let refreshed = match auth::refresh(
                http,
                token_url,
                &refresh_token,
                Some(refresh_token.as_str()),
            )
            .await
            {
                Ok(refreshed) => refreshed,
                Err(_) => return Ok(None),
            };
            store.save(&refreshed).map_err(|e| e.to_string())?;
            match auth::fetch_session(http, base_url, &refreshed.access_token).await {
                Ok(session) => Ok(Some((session, refreshed))),
                Err(_) => Ok(None),
            }
        }
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub async fn auth_restore(state: State<'_, AppState>) -> Result<Option<SessionInfoDto>, String> {
    let http = reqwest::Client::new();
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore);
    let restored = restore_tokens(&http, API_BASE, TOKEN_URL, store.as_ref()).await?;
    match restored {
        None => Ok(None),
        Some((session, tokens)) => {
            let client =
                TidalClient::new(http, API_BASE, TOKEN_URL, session.clone(), tokens, store);
            state.set_client(client);
            Ok(Some(SessionInfoDto {
                country_code: session.country_code,
                user_id: session.user_id,
            }))
        }
    }
}

fn require_client(state: &State<'_, AppState>) -> Result<TidalClient, String> {
    state
        .client
        .read()
        .map_err(|_| "state poisoned".to_string())?
        .clone()
        .ok_or_else(|| "not logged in".to_string())
}

use oxitide_tidal::home::HomeSection;
use oxitide_tidal::models::{Album, Artist, Playlist, SearchResults, Track};

#[tauri::command]
pub async fn get_home(state: State<'_, AppState>) -> Result<Vec<HomeSection>, String> {
    require_client(&state)?.home_page().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_album(state: State<'_, AppState>, id: u64) -> Result<Album, String> {
    require_client(&state)?.album(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_album_tracks(state: State<'_, AppState>, id: u64) -> Result<Vec<Track>, String> {
    require_client(&state)?.album_tracks_all(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist(state: State<'_, AppState>, id: String) -> Result<Playlist, String> {
    require_client(&state)?.playlist(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist_tracks(state: State<'_, AppState>, id: String) -> Result<Vec<Track>, String> {
    require_client(&state)?.playlist_tracks_all(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_artist(state: State<'_, AppState>, id: u64) -> Result<Artist, String> {
    require_client(&state)?.artist(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Result<SearchResults, String> {
    require_client(&state)?
        .search(&query, &["tracks", "albums", "artists", "playlists"], 20, 0)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    require_client(&state)?.all_favorite_tracks().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    require_client(&state)?.all_favorite_albums().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_artists(state: State<'_, AppState>) -> Result<Vec<Artist>, String> {
    require_client(&state)?.all_favorite_artists().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    require_client(&state)?.all_user_playlists().await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_tidal::token_store::MemoryStore;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_tokens(access: &str, refresh: Option<&str>) -> TokenSet {
        TokenSet {
            access_token: access.into(),
            refresh_token: refresh.map(|r| r.into()),
            token_type: "Bearer".into(),
            expires_at: None,
        }
    }

    #[test]
    fn login_prompt_serializes_url() {
        let dto = LoginPrompt { url: "https://example.test/login".to_string() };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["url"], "https://example.test/login");
    }

    #[test]
    fn session_info_dto_serializes_fields() {
        let dto = SessionInfoDto { country_code: "GR".to_string(), user_id: 42 };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["country_code"], "GR");
        assert_eq!(value["user_id"], 42);
    }

    #[tokio::test]
    async fn restore_tokens_returns_none_without_stored_token() {
        let server = MockServer::start().await;
        let store = MemoryStore::default();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store)
            .await
            .unwrap();
        assert!(restored.is_none());
    }

    #[tokio::test]
    async fn restore_tokens_uses_valid_stored_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/sessions"))
            .and(header("authorization", "Bearer good"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "s", "countryCode": "GR", "userId": 7
            })))
            .mount(&server).await;
        let store = MemoryStore::default();
        store.save(&sample_tokens("good", Some("rt"))).unwrap();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.0.user_id, 7);
        assert_eq!(restored.1.access_token, "good");
    }

    #[tokio::test]
    async fn restore_tokens_refreshes_on_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/sessions"))
            .and(header("authorization", "Bearer stale"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("GET")).and(path("/sessions"))
            .and(header("authorization", "Bearer fresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "s", "countryCode": "GR", "userId": 9
            })))
            .mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "fresh", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let store = MemoryStore::default();
        store.save(&sample_tokens("stale", Some("old-rt"))).unwrap();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.0.user_id, 9);
        assert_eq!(restored.1.access_token, "fresh");
        // Refresh response omitted refresh_token: the previous one is preserved
        // and persisted.
        assert_eq!(restored.1.refresh_token.as_deref(), Some("old-rt"));
        assert_eq!(store.load().unwrap().unwrap().access_token, "fresh");
    }

    #[tokio::test]
    async fn restore_tokens_returns_none_when_refresh_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/sessions"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        let store = MemoryStore::default();
        store.save(&sample_tokens("stale", Some("bad-rt"))).unwrap();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store)
            .await
            .unwrap();
        assert!(restored.is_none());
    }
}
