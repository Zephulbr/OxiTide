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
