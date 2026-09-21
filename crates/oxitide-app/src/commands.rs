use std::sync::Arc;

use oxitide_tidal::auth;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::{Pkce, PKCE_CLIENT_ID, TOKEN_URL};
use oxitide_tidal::token_store::{KeyringStore, TokenStore};
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
    require_client(&state)?.album_tracks(id, 100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist(state: State<'_, AppState>, id: String) -> Result<Playlist, String> {
    require_client(&state)?.playlist(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist_tracks(state: State<'_, AppState>, id: String) -> Result<Vec<Track>, String> {
    require_client(&state)?.playlist_tracks(&id, 100, 0).await.map_err(|e| e.to_string())
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
    require_client(&state)?.favorite_tracks(100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    require_client(&state)?.favorite_albums(100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_artists(state: State<'_, AppState>) -> Result<Vec<Artist>, String> {
    require_client(&state)?.favorite_artists(100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    require_client(&state)?.user_playlists(100, 0).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
