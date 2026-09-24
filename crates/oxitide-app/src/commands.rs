use std::sync::Arc;

use oxitide_audio::cache::DiskCache;
use oxitide_audio::engine::{Engine, EngineCmd, EngineEvent, PlaybackState};
use oxitide_audio::fetch::HttpFetcher;
use oxitide_audio::output::{ExclusiveFallback, OutputMode};
use oxitide_audio::queue::{QueueItem, RepeatMode};
use oxitide_tidal::auth::{self, SessionInfo};
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::library::FavoriteIds;
use oxitide_tidal::pkce::{Pkce, PKCE_CLIENT_ID, TOKEN_URL};
use oxitide_tidal::token_store::{KeyringStore, TokenSet, TokenStore};
use oxitide_tidal::Error;
use serde::Serialize;
use tauri::{Manager, State};

use crate::config::{Config, QualityDisplay};
use crate::player::{devices, OutputDeviceDto, QualityDto};
use crate::state::{AppState, Player};

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

#[derive(Serialize)]
pub struct AudioStatusDto {
    pub output_mode: OutputMode,
    pub bit_perfect: bool,
    pub notice: Option<String>,
}

#[derive(Serialize)]
pub struct CacheStatusDto {
    pub dir: String,
    pub used_bytes: u64,
    /// In-flight `.part` bytes; not counted against the cap.
    pub pending_bytes: u64,
    pub max_bytes: u64,
    pub usable: bool,
    /// The raw configured `cache.dir` ("" = default), so the UI can tell when
    /// the configured directory was rejected and a fallback is in use.
    pub configured_dir: String,
}

#[derive(Serialize)]
pub struct TrackPageDto {
    pub items: Vec<Track>,
    pub total: u32,
    pub offset: u32,
    pub limit: u32,
}

fn require_cache(state: &State<'_, AppState>) -> Result<Arc<DiskCache>, String> {
    state.cache().ok_or_else(|| "cache unavailable".to_string())
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
    app: tauri::AppHandle,
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
    state.set_client(client.clone());
    let config = Config::load();
    state.set_config(config.clone());
    let cache = state
        .cache()
        .unwrap_or_else(|| crate::player::build_cache(&config));
    state.set_player(replace_player(&app, &state, client, &config, cache)?);
    Ok(SessionInfoDto { country_code: session.country_code, user_id: session.user_id })
}

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> bool {
    state.is_logged_in()
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(player) = state.player() {
        let _ = player.engine.handle(oxitide_audio::engine::EngineCmd::Clear);
        player.engine.shutdown();
    }
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
pub async fn auth_restore(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<SessionInfoDto>, String> {
    let http = reqwest::Client::new();
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore);
    let restored = restore_tokens(&http, API_BASE, TOKEN_URL, store.as_ref()).await?;
    match restored {
        None => Ok(None),
        Some((session, tokens)) => {
            let client =
                TidalClient::new(http, API_BASE, TOKEN_URL, session.clone(), tokens, store);
            state.set_client(client.clone());
            let config = Config::load();
            state.set_config(config.clone());
            let cache = state
                .cache()
                .unwrap_or_else(|| crate::player::build_cache(&config));
            state.set_player(replace_player(&app, &state, client, &config, cache)?);
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

/// Replace the running player: release the old engine's worker and output
/// first, and clear the expired-session flag now that a new login exists.
fn replace_player(
    app: &tauri::AppHandle,
    state: &AppState,
    client: TidalClient,
    config: &Config,
    cache: Arc<DiskCache>,
) -> Result<Player, String> {
    if let Some(old) = state.player() {
        old.engine.shutdown();
    }
    state.clear_session_expired();
    build_player(app, client, config, cache)
}

/// Build the playback engine for a freshly authenticated client and forward its
/// events to the UI on the `playback_event` channel.
fn build_player(
    app: &tauri::AppHandle,
    client: TidalClient,
    config: &Config,
    cache: Arc<DiskCache>,
) -> Result<Player, String> {
    let resolver = crate::player::TidalResolver::new(client).map_err(|e| e.to_string())?;
    let engine = Engine::new(
        Arc::new(resolver),
        || Arc::new(HttpFetcher::default()),
        crate::player::real_output(),
        config.streaming.quality.to_quality(),
        cache,
        config.streaming.allow_fallback,
    );
    engine.set_output_device(if config.audio.output_device.is_empty() {
        None
    } else {
        Some(config.audio.output_device.clone())
    });
    engine.set_output_mode(if config.audio.exclusive {
        OutputMode::Exclusive
    } else {
        OutputMode::Shared
    });
    engine.set_exclusive_fallback(config.audio.exclusive_fallback);
    engine.handle(EngineCmd::SetVolume(config.audio.volume)).map_err(|e| e.to_string())?;
    let handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        if let EngineEvent::Error { code, .. } = &event {
            if code == "auth" {
                handle.state::<AppState>().notify_session_expired();
            }
        }
        let _ = tauri::Emitter::emit(&handle, "playback_event", crate::player::to_dto(event));
    }));
    Ok(Player { engine })
}

use oxitide_tidal::home::HomeSection;
use oxitide_tidal::models::{Album, Artist, Playlist, SearchResults, Track};

#[tauri::command]
pub async fn get_home(state: State<'_, AppState>) -> Result<Vec<HomeSection>, String> {
    require_client(&state)?.home_page().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_album(state: State<'_, AppState>, id: u64) -> Result<Album, String> {
    require_client(&state)?.album(id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_album_tracks(state: State<'_, AppState>, id: u64) -> Result<Vec<Track>, String> {
    require_client(&state)?.album_tracks_all(id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_playlist(state: State<'_, AppState>, id: String) -> Result<Playlist, String> {
    require_client(&state)?.playlist(&id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_playlist_tracks(state: State<'_, AppState>, id: String) -> Result<Vec<Track>, String> {
    require_client(&state)?.playlist_tracks_all(&id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist(state: State<'_, AppState>, id: u64) -> Result<Artist, String> {
    require_client(&state)?.artist(id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Result<SearchResults, String> {
    require_client(&state)?
        .search(&query, &["tracks", "albums", "artists", "playlists"], 20, 0)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    require_client(&state)?.all_favorite_tracks().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    require_client(&state)?.all_favorite_albums().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_artists(state: State<'_, AppState>) -> Result<Vec<Artist>, String> {
    require_client(&state)?.all_favorite_artists().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn set_favorite_track(
    state: State<'_, AppState>,
    track_id: u64,
    favorite: bool,
) -> Result<(), String> {
    require_client(&state)?
        .set_favorite_track(track_id, favorite)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn set_favorite_album(
    state: State<'_, AppState>,
    album_id: u64,
    favorite: bool,
) -> Result<(), String> {
    require_client(&state)?
        .set_favorite_album(album_id, favorite)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn set_favorite_artist(
    state: State<'_, AppState>,
    artist_id: u64,
    favorite: bool,
) -> Result<(), String> {
    require_client(&state)?
        .set_favorite_artist(artist_id, favorite)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_ids(state: State<'_, AppState>) -> Result<FavoriteIds, String> {
    require_client(&state)?.favorite_ids().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_user_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    require_client(&state)?.all_user_playlists().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_tracks_page(
    state: State<'_, AppState>,
    offset: u32,
    limit: u32,
) -> Result<TrackPageDto, String> {
    let (items, total) = require_client(&state)?
        .favorite_tracks_page(limit, offset)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(TrackPageDto { items, total, offset, limit })
}

#[tauri::command]
pub async fn get_playlist_tracks_page(
    state: State<'_, AppState>,
    id: String,
    offset: u32,
    limit: u32,
) -> Result<TrackPageDto, String> {
    let (items, total) = require_client(&state)?
        .playlist_tracks_page(&id, limit, offset)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(TrackPageDto { items, total, offset, limit })
}

#[tauri::command]
pub fn play_tracks(
    state: State<'_, AppState>,
    items: Vec<QueueItem>,
    start: usize,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::PlayTracks { items, start })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::PlayPause).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn next(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Next).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn prev(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Prev).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn seek(state: State<'_, AppState>, position_ms: u64) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Seek(position_ms)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_volume(
    state: State<'_, AppState>,
    volume: f32,
    persist: bool,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::SetVolume(volume)).map_err(|e| e.to_string())?;
    if persist {
        let mut config = state.config();
        config.audio.volume = volume.clamp(0.0, 1.0);
        config.save()?;
        state.set_config(config);
    }
    Ok(())
}

#[tauri::command]
pub fn set_shuffle(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::SetShuffle(enabled)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_repeat(state: State<'_, AppState>, mode: RepeatMode) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::SetRepeat(mode)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_reorder(
    state: State<'_, AppState>,
    from: usize,
    to: usize,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Reorder { from, to }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_remove(state: State<'_, AppState>, index: usize) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Remove(index)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_clear(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Clear).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn retry(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.retry().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_add(state: State<'_, AppState>, items: Vec<QueueItem>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Enqueue { items }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_playback_state(state: State<'_, AppState>) -> Result<PlaybackState, String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    Ok(player.engine.state())
}

#[tauri::command]
pub fn get_output_devices() -> Vec<OutputDeviceDto> {
    devices()
}

#[tauri::command]
pub fn set_output_device(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let device = if device_id.is_empty() { None } else { Some(device_id.clone()) };
    player.engine.set_output_device(device);
    let mut config = state.config();
    config.audio.output_device = device_id;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_preferred_quality(
    state: State<'_, AppState>,
    quality: QualityDto,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_quality(quality.to_quality());
    let mut config = state.config();
    config.streaming.quality = quality;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_exclusive(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_output_mode(if enabled {
        OutputMode::Exclusive
    } else {
        OutputMode::Shared
    });
    let mut config = state.config();
    config.audio.exclusive = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_exclusive_fallback(
    state: State<'_, AppState>,
    mode: ExclusiveFallback,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_exclusive_fallback(mode);
    let mut config = state.config();
    config.audio.exclusive_fallback = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.config())
}

#[tauri::command]
pub fn get_audio_status(state: State<'_, AppState>) -> Result<AudioStatusDto, String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let status = player.engine.audio_status();
    Ok(AudioStatusDto {
        output_mode: status.mode,
        bit_perfect: status.bit_perfect,
        notice: status.notice,
    })
}

#[tauri::command]
pub fn get_cache_status(state: State<'_, AppState>) -> Result<CacheStatusDto, String> {
    let cache = require_cache(&state)?;
    Ok(CacheStatusDto {
        dir: cache.dir().display().to_string(),
        used_bytes: cache.size_bytes(),
        pending_bytes: cache.pending_bytes(),
        max_bytes: cache.max_bytes(),
        usable: cache.usable(),
        configured_dir: state.config().cache.dir.clone(),
    })
}

#[tauri::command]
pub fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    require_cache(&state)?.clear();
    Ok(())
}

#[tauri::command]
pub fn set_allow_fallback(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_allow_fallback(enabled);
    let mut config = state.config();
    config.streaming.allow_fallback = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_quality_display(state: State<'_, AppState>, mode: QualityDisplay) -> Result<(), String> {
    let mut config = state.config();
    config.ui.quality_display = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_cache_max_size_mb(state: State<'_, AppState>, mb: u64) -> Result<(), String> {
    require_cache(&state)?.set_max_bytes(mb.saturating_mul(1024 * 1024));
    let mut config = state.config();
    config.cache.max_size_mb = mb;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_cache_clear_on_exit(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.cache.clear_on_exit = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn get_logs_dir() -> String {
    crate::config::logs_dir()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default()
}

#[tauri::command]
pub fn open_logs_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = crate::config::logs_dir().ok_or_else(|| "no logs directory available".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
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
    fn cache_status_dto_serializes_pending_bytes() {
        let dto = CacheStatusDto {
            dir: "C:\\cache".into(),
            used_bytes: 10,
            pending_bytes: 5,
            max_bytes: 100,
            usable: true,
            configured_dir: String::new(),
        };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["used_bytes"], 10);
        assert_eq!(value["pending_bytes"], 5);
    }

    #[test]
    fn track_page_dto_serializes_fields() {
        let dto = TrackPageDto { items: Vec::new(), total: 7, offset: 100, limit: 50 };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["total"], 7);
        assert_eq!(value["offset"], 100);
        assert_eq!(value["limit"], 50);
        assert!(value["items"].is_array());
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

    #[test]
    fn favorite_ids_dto_serializes_arrays() {
        let dto = oxitide_tidal::library::FavoriteIds {
            tracks: vec![1, 2],
            albums: vec![3],
            artists: vec![],
        };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["tracks"], serde_json::json!([1, 2]));
        assert_eq!(value["albums"], serde_json::json!([3]));
        assert_eq!(value["artists"], serde_json::json!([]));
    }
}
