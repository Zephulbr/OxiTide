use std::sync::Arc;

use oxitide_audio::cache::DiskCache;
use oxitide_audio::dsp::{DspSettings, TransitionStyle};
use oxitide_audio::engine::{Engine, EngineCmd, EngineEvent};
use oxitide_audio::fetch::HttpFetcher;
use oxitide_audio::output::{ExclusiveFallback, OutputMode};
use oxitide_audio::queue::{ArtistRef, QueueItem, RepeatMode};
use oxitide_tidal::auth::{self, SessionInfo};
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::later::{LaterEntry, LaterIds, LaterKind};
use oxitide_tidal::library::FavoriteIds;
use oxitide_tidal::shares::{PlaylistInvite, PlaylistMember};
use oxitide_tidal::pkce::{
    random_state, Pkce, PKCE_CLIENT_ID, PKCE_REDIRECT_DEEP_LINK, PKCE_REDIRECT_PASTE, TOKEN_URL,
};
use oxitide_tidal::token_store::{KeyringStatus, TokenSet, TokenStore};
use oxitide_tidal::Error;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::artwork::PaletteDto;
use crate::config::{
    clamp_catjam_scale, clamp_queue_width, CatjamCoat, Config, CoverShape, Density, DynamicColor, InstrumentTone,
    PanelStyle, QualityDisplay, RackConfig, Theme, Typeface, UiMode,
};
use crate::lyrics::{LyricsDto, LyricsRequest};
use crate::player::{devices, OutputDeviceDto, QualityDto};
use crate::state::{AppState, PendingLogin, Player};

pub const API_BASE: &str = "https://api.tidal.com/v1/";

#[derive(Serialize)]
pub struct LoginPrompt {
    pub url: String,
}

#[derive(Serialize, Clone)]
pub struct SessionInfoDto {
    pub country_code: String,
    pub user_id: u64,
}

/// Outcome of a completed deep-link login, buffered so a cold-start result is
/// never missed by the UI.
#[derive(Clone)]
pub enum LoginOutcome {
    Success(SessionInfoDto),
    Error(String),
}

/// Which redirect an in-progress login uses: `Deep` (primary) or `Paste`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Deep,
    Paste,
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

/// Verify the echoed `state` against the pending value. A present-but-wrong
/// value is always rejected; a missing value is rejected only when
/// `require_present` (the automatic deep-link path, where TIDAL always echoes
/// `state`). The manual paste redirect tolerates a missing `state`.
pub fn verify_login_state(
    expected: &str,
    redirect: &auth::RedirectOutcome,
    require_present: bool,
) -> Result<(), String> {
    match redirect.state.as_deref() {
        Some(got) if got == expected => Ok(()),
        Some(_) => Err("login state mismatch".to_string()),
        None if require_present => Err("login state missing".to_string()),
        None => Ok(()),
    }
}

/// Parse a redirect for the mode its pending login used. Deep links must be
/// `tidal://login/auth`; the paste fallback accepts the HTTPS Oops URL.
fn parse_and_check(
    login: &PendingLogin,
    redirect_url: &str,
) -> Result<auth::RedirectOutcome, String> {
    let require_state = login.redirect_uri == PKCE_REDIRECT_DEEP_LINK;
    let outcome = if require_state {
        auth::validate_redirect(redirect_url).map_err(|e| e.to_string())?
    } else {
        auth::RedirectOutcome {
            code: auth::extract_code(redirect_url).map_err(|e| e.to_string())?,
            state: auth::extract_state(redirect_url),
        }
    };
    verify_login_state(&login.state, &outcome, require_state)?;
    Ok(outcome)
}

/// Take the pending login and validate the redirect. A rejected redirect puts
/// the pending login back, so a stray/forged link cannot destroy an in-flight
/// login (spec §7).
fn take_and_validate(
    state: &AppState,
    redirect_url: &str,
) -> Result<(PendingLogin, auth::RedirectOutcome), String> {
    let login = state
        .take_pending()
        .ok_or_else(|| "no login in progress".to_string())?;
    match parse_and_check(&login, redirect_url) {
        Ok(outcome) => Ok((login, outcome)),
        Err(error) => {
            state.set_pending(login);
            Err(error)
        }
    }
}

/// The shared completion path: validate the redirect, exchange the code for
/// tokens, persist them, build the client/player, and return the session.
/// Used by both the paste-back command and the deep-link handler.
pub async fn finish_login(
    app: &tauri::AppHandle,
    state: &AppState,
    redirect_url: String,
) -> Result<SessionInfoDto, String> {
    let (login, _redirect) = take_and_validate(state, &redirect_url)?;
    let http = oxitide_tidal::http_client();
    let tokens = auth::exchange_code(
        &http,
        TOKEN_URL,
        &login.pkce,
        &login.redirect_uri,
        &redirect_url,
    )
    .await
    .map_err(|e| e.to_string())?;
    let session = auth::fetch_session(&http, API_BASE, &tokens.access_token)
        .await
        .map_err(|e| e.to_string())?;
    let store = state
        .token_store()
        .ok_or_else(|| "token store unavailable".to_string())?;
    store.save(&tokens).map_err(|e| e.to_string())?;
    save_session(saved_session_path().as_deref(), &session);
    if let (Some(live), Some(parked)) = (crate::session::session_path(), crate::session::parked_path()) {
        crate::session::unpark(&live, &parked, session.user_id);
    }
    let client = TidalClient::new(http, API_BASE, TOKEN_URL, session.clone(), tokens, store);
    state.set_client(client.clone());
    let config = state.config();
    let cache = state
        .cache()
        .unwrap_or_else(|| crate::player::build_cache(&config));
    state.set_player(replace_player(app, state, client, &config, cache)?);
    crate::remote::host::session_changed(state, crate::remote::events::SessionKind::Ok);
    Ok(SessionInfoDto {
        country_code: session.country_code,
        user_id: session.user_id,
    })
}

#[tauri::command]
pub fn auth_begin(
    state: State<'_, AppState>,
    mode: Option<AuthMode>,
) -> Result<LoginPrompt, String> {
    let redirect_uri = match mode.unwrap_or(AuthMode::Deep) {
        AuthMode::Deep => PKCE_REDIRECT_DEEP_LINK,
        AuthMode::Paste => PKCE_REDIRECT_PASTE,
    };
    let pkce = Pkce::generate();
    let login_state = random_state();
    let url = pkce.login_url(PKCE_CLIENT_ID, &login_state, redirect_uri);
    state.set_pending(PendingLogin {
        pkce,
        state: login_state,
        redirect_uri: redirect_uri.to_string(),
    });
    state.clear_last_login();
    Ok(LoginPrompt { url })
}

#[tauri::command]
pub async fn auth_complete(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    redirect_url: String,
) -> Result<SessionInfoDto, String> {
    finish_login(&app, &state, redirect_url).await
}

#[tauri::command]
pub fn auth_take_result(state: State<'_, AppState>) -> Result<Option<SessionInfoDto>, String> {
    match state.take_last_login() {
        None => Ok(None),
        Some(LoginOutcome::Success(session)) => Ok(Some(session)),
        Some(LoginOutcome::Error(error)) => Err(error),
    }
}

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> bool {
    state.is_logged_in()
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> Result<(), String> {
    // Set the queue aside for this account before the engine clears it; the
    // same account gets it back at its next login (`finish_login`).
    let owner = state.client.read().unwrap().as_ref().map(|client| client.session().user_id);
    if let (Some(saver), Some(parked)) = (state.session_saver(), crate::session::parked_path()) {
        saver.park(parked, owner);
    }
    if let Some(player) = state.player() {
        let _ = player
            .engine
            .handle(oxitide_audio::engine::EngineCmd::Clear);
        player.engine.shutdown();
    }
    if let Some(store) = state.token_store() {
        store.clear().map_err(|e| e.to_string())?;
    }
    if let Some(path) = saved_session_path() {
        let _ = std::fs::remove_file(path);
    }
    if let Some(history) = state.history() {
        history.finalize_now();
    }
    state.logout();
    if let Some(saver) = state.session_saver() {
        saver.clear();
    }
    // The remote keeps running so a phone shows "Sign in on the PC".
    crate::remote::host::session_changed(&state, crate::remote::events::SessionKind::LoggedOut);
    Ok(())
}

/// The startup credential-store probe result, for the UI notice.
#[tauri::command]
pub fn get_keyring_status(state: State<'_, AppState>) -> Result<KeyringStatus, String> {
    Ok(state.keyring_status())
}

/// Load a persisted session from the store, refreshing on `401`.
///
/// Returns `Ok(None)` when there is no stored token or it can no longer be
/// used, so the UI can fall back to the login screen without a hard error.
/// `config\tidal_session.json`: the account's country and user id from the
/// last online sign-in or restore (no secrets; tokens stay in the keyring),
/// so the app can start offline and play cached tracks.
fn saved_session_path() -> Option<std::path::PathBuf> {
    crate::config::config_path().map(|path| path.with_file_name("tidal_session.json"))
}

fn load_saved_session(path: Option<&std::path::Path>) -> Option<SessionInfo> {
    let text = std::fs::read_to_string(path?).ok()?;
    serde_json::from_str(&text).ok()
}

fn save_session(path: Option<&std::path::Path>, session: &SessionInfo) {
    let Some(path) = path else { return };
    match serde_json::to_vec(session) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(path, bytes) {
                tracing::warn!("could not save the session info: {e}");
            }
        }
        Err(e) => tracing::warn!("could not encode the session info: {e}"),
    }
}

/// How a stored login was restored.
#[derive(Debug, PartialEq)]
enum Restored {
    Online(SessionInfo, TokenSet),
    /// TIDAL was unreachable: the saved session info with the stored
    /// tokens. The client refreshes them on its first 401 once online.
    Offline(SessionInfo, TokenSet),
}

async fn restore_tokens(
    http: &reqwest::Client,
    base_url: &str,
    token_url: &str,
    store: &dyn TokenStore,
    saved: Option<SessionInfo>,
) -> Result<Option<Restored>, String> {
    let tokens = match store.load() {
        Ok(Some(tokens)) => tokens,
        Ok(None) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    // Only a network failure falls back to the saved session; a rejected
    // login must still lead to the sign-in screen.
    let offline = |e: &Error, tokens: TokenSet| {
        if e.is_transient() || matches!(e, Error::Io(_)) {
            tracing::warn!("TIDAL unreachable at startup ({e}); starting offline");
            saved.clone().map(|session| Restored::Offline(session, tokens))
        } else {
            None
        }
    };
    match auth::fetch_session(http, base_url, &tokens.access_token).await {
        Ok(session) => Ok(Some(Restored::Online(session, tokens))),
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
                Err(e) => return Ok(offline(&e, tokens)),
            };
            store.save(&refreshed).map_err(|e| e.to_string())?;
            match auth::fetch_session(http, base_url, &refreshed.access_token).await {
                Ok(session) => Ok(Some(Restored::Online(session, refreshed))),
                Err(e) => Ok(offline(&e, refreshed)),
            }
        }
        Err(e) => Ok(offline(&e, tokens)),
    }
}

#[tauri::command]
pub async fn auth_restore(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<SessionInfoDto>, String> {
    let http = oxitide_tidal::http_client();
    let store = state
        .token_store()
        .ok_or_else(|| "token store unavailable".to_string())?;
    let saved_path = saved_session_path();
    let saved = load_saved_session(saved_path.as_deref());
    let restored = restore_tokens(&http, API_BASE, TOKEN_URL, store.as_ref(), saved).await?;
    let (session, tokens) = match restored {
        None => return Ok(None),
        Some(Restored::Online(session, tokens)) => {
            save_session(saved_path.as_deref(), &session);
            (session, tokens)
        }
        Some(Restored::Offline(session, tokens)) => (session, tokens),
    };
    let client = TidalClient::new(http, API_BASE, TOKEN_URL, session.clone(), tokens, store);
    state.set_client(client.clone());
    let config = state.config();
    let cache = state
        .cache()
        .unwrap_or_else(|| crate::player::build_cache(&config));
    state.set_player(replace_player(&app, &state, client, &config, cache)?);
    crate::remote::host::session_changed(&state, crate::remote::events::SessionKind::Ok);
    crate::perf::mark("auth_restored");
    Ok(Some(SessionInfoDto {
        country_code: session.country_code,
        user_id: session.user_id,
    }))
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
    engine.set_dsp(config.dsp.clone());
    engine
        .handle(EngineCmd::SetVolume(config.audio.volume))
        .map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let _ = state.take_media();
    let _ = state.take_discord();
    #[cfg(windows)]
    let smtc_sender = {
        let mut found = None;
        match app.get_webview_window("main").map(|window| window.hwnd()) {
            Some(Ok(hwnd)) => {
                match crate::media_controls::MediaSession::spawn(
                    engine.clone(),
                    hwnd.0 as isize,
                    config.integrations.smtc,
                ) {
                    Ok(session) => {
                        found = Some(session.sender());
                        state.set_media(session);
                    }
                    Err(e) => tracing::warn!("SMTC disabled: {e}"),
                }
            }
            Some(Err(e)) => tracing::warn!("SMTC disabled (no HWND): {e}"),
            None => tracing::warn!("SMTC disabled (no main window)"),
        }
        found
    };
    #[cfg(target_os = "linux")]
    let smtc_sender = {
        let mut found = None;
        match crate::media_controls::MediaSession::spawn(
            engine.clone(),
            config.integrations.smtc,
            app.clone(),
        ) {
            Ok(session) => {
                found = Some(session.sender());
                state.set_media(session);
            }
            Err(e) => tracing::warn!("MPRIS disabled: {e}"),
        }
        found
    };
    #[cfg(not(any(windows, target_os = "linux")))]
    let smtc_sender: Option<crate::media_controls::SessionSender> = None;
    let discord_sender = match crate::discord::DiscordPresence::spawn(config.integrations.discord) {
        Ok(service) => {
            let sender = service.sender();
            state.set_discord(service);
            Some(sender)
        }
        Err(e) => {
            tracing::warn!("Discord presence disabled: {e}");
            None
        }
    };

    let handle = app.clone();
    let last_track = std::sync::Mutex::new(None);
    let queue_gate = crate::player::QueueGate::default();
    // The tray tooltip only names the track; skip the shell call for state
    // events that keep it (volume, seek, flags).
    let tooltip_track: std::sync::Mutex<Option<Option<u64>>> = std::sync::Mutex::new(None);
    let set_tooltip = move |handle: &tauri::AppHandle, item: Option<&oxitide_audio::queue::QueueItem>| {
        let id = item.map(|i| i.track_id);
        let mut last = tooltip_track.lock().unwrap();
        if *last != Some(id) {
            *last = Some(id);
            crate::tray::update_tooltip(handle, item);
        }
    };
    engine.subscribe(Arc::new(move |event| {
        let app_state = handle.state::<AppState>();
        if app_state.player().is_some() {
            if let Some(saver) = app_state.session_saver() {
                saver.record(&event);
            }
        }
        if let EngineEvent::Error { code, .. } = &event {
            if code == "auth" {
                handle.state::<AppState>().notify_session_expired();
            }
        }
        match &event {
            EngineEvent::NowPlaying(item) => {
                set_tooltip(&handle, item.as_ref());
            }
            EngineEvent::State(snapshot) => {
                set_tooltip(&handle, snapshot.current.as_ref());
                let view = crate::thumbbar::View {
                    playing: snapshot.playing,
                    has_track: snapshot.current.is_some(),
                };
                crate::thumbbar::set_view(&handle, view);
            }
            _ => {}
        }
        if smtc_sender.is_some() || discord_sender.is_some() {
            let mut last = last_track.lock().unwrap();
            let (updates, next) = crate::media_controls::plan(&event, *last);
            *last = next;
            for update in updates {
                if let Some(sender) = &smtc_sender {
                    sender.send(update.clone());
                }
                if let Some(sender) = &discord_sender {
                    sender.send(update);
                }
            }
        }
        crate::perf::emit_counted(&handle, "playback_event", queue_gate.dto(event));
    }));
    let scrobble_handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        if let Some(scrobbler) = scrobble_handle.state::<AppState>().scrobbler() {
            scrobbler.on_player_event(&event);
        }
    }));
    let history_handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        if let Some(history) = history_handle.state::<AppState>().history() {
            history.record(&event);
        }
    }));
    // Autoplay: a no-op unless the last queued track is current.
    let autoplay_handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        autoplay_handle.state::<AppState>().autoplay.on_event(&event, &autoplay_handle);
    }));
    // Phone remote (Track 11): a no-op unless the remote is on and a phone
    // is connected.
    let remote_handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        if let Some(remote) = remote_handle.state::<AppState>().remote() {
            remote.on_engine_event(&event);
        }
    }));
    engine.set_perf_hook(std::sync::Arc::new(crate::perf::audio_event));
    crate::perf::mark("player_built");
    Ok(Player { engine })
}

use oxitide_tidal::feed::FeedActivity;
use oxitide_tidal::folders::{CollectionEntry, CollectionFolder};
use oxitide_tidal::models::{
    Album, AlbumInfo, Artist, ArtistDiscography, ArtistProfile, Mix, Playlist, SearchResults, Track,
    TrackCredits,
};
use oxitide_tidal::pages::{MixDetail, PageItem, PageSection};

#[tauri::command]
pub async fn get_home(state: State<'_, AppState>) -> Result<Vec<PageSection>, String> {
    require_client(&state)?
        .home_page()
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_explore(state: State<'_, AppState>) -> Result<Vec<PageSection>, String> {
    require_client(&state)?
        .explore_page()
        .await
        .map_err(|e| state.fail_tidal(e))
}

/// A page a page link opens (Explore's genres, moods, decades). The path is
/// re-checked here: it comes back from the webview, not straight from TIDAL.
#[tauri::command]
pub async fn get_page(state: State<'_, AppState>, path: String) -> Result<Vec<PageSection>, String> {
    if !oxitide_tidal::pages::is_page_path(&path) {
        return Err("invalid page path".into());
    }
    require_client(&state)?
        .page(&path)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_feed(state: State<'_, AppState>) -> Result<Vec<FeedActivity>, String> {
    require_client(&state)?
        .feed_activities()
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_lyrics(
    state: State<'_, AppState>,
    track_id: u64,
    title: String,
    artist: String,
    album: Option<String>,
    duration_secs: u64,
) -> Result<Option<LyricsDto>, String> {
    let service = state
        .lyrics()
        .ok_or_else(|| "lyrics unavailable".to_string())?;
    let client = require_client(&state)?;
    let req = LyricsRequest {
        track_id,
        title,
        artist,
        album,
        duration_secs,
    };
    Ok(service.get(&client, &req).await)
}

fn require_scrobbler(
    state: &State<'_, AppState>,
) -> Result<std::sync::Arc<crate::scrobble::ScrobblerService>, String> {
    state.scrobbler().ok_or_else(|| "scrobbler unavailable".to_string())
}

#[tauri::command]
pub fn get_scrobble_status(
    state: State<'_, AppState>,
) -> Result<crate::scrobble::ScrobbleStatusDto, String> {
    Ok(require_scrobbler(&state)?.status())
}

#[tauri::command]
pub async fn lastfm_begin_auth(state: State<'_, AppState>) -> Result<String, String> {
    require_scrobbler(&state)?.lastfm_begin_auth().await
}

#[tauri::command]
pub async fn lastfm_complete_auth(
    state: State<'_, AppState>,
) -> Result<crate::scrobble::ScrobbleStatusDto, String> {
    require_scrobbler(&state)?.lastfm_complete_auth().await
}

#[tauri::command]
pub fn lastfm_cancel_auth(state: State<'_, AppState>) -> Result<(), String> {
    require_scrobbler(&state)?.lastfm_cancel_auth();
    Ok(())
}

#[tauri::command]
pub fn lastfm_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    require_scrobbler(&state)?.lastfm_disconnect()?;
    Ok(())
}

#[tauri::command]
pub async fn listenbrainz_connect(
    state: State<'_, AppState>,
    token: String,
) -> Result<crate::scrobble::ScrobbleStatusDto, String> {
    require_scrobbler(&state)?.listenbrainz_connect(token).await
}

#[tauri::command]
pub fn listenbrainz_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    require_scrobbler(&state)?.listenbrainz_disconnect()?;
    Ok(())
}

#[tauri::command]
pub fn set_scrobble_enabled(
    state: State<'_, AppState>,
    service: String,
    enabled: bool,
) -> Result<(), String> {
    let scrobbler = require_scrobbler(&state)?;
    if service != crate::scrobble::LASTFM_SERVICE
        && service != crate::scrobble::LISTENBRAINZ_SERVICE
    {
        return Err("unknown scrobble service".to_string());
    }
    scrobbler.set_enabled(&service, enabled);
    let mut config = state.config();
    if service == crate::scrobble::LASTFM_SERVICE {
        config.scrobbling.lastfm_enabled = enabled;
    } else {
        config.scrobbling.listenbrainz_enabled = enabled;
    }
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub async fn get_favorite_mixes(state: State<'_, AppState>) -> Result<Vec<Mix>, String> {
    require_client(&state)?
        .all_favorite_mixes()
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_mix(state: State<'_, AppState>, id: String) -> Result<MixDetail, String> {
    require_client(&state)?
        .mix_page(&id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_for_you(state: State<'_, AppState>) -> Result<Vec<PageSection>, String> {
    require_client(&state)?
        .for_you_page()
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_track_radio(
    state: State<'_, AppState>,
    track_id: u64,
) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .track_radio(track_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_radio(
    state: State<'_, AppState>,
    artist_id: u64,
) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .artist_radio(artist_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_similar_artists(
    state: State<'_, AppState>,
    artist_id: u64,
) -> Result<Vec<Artist>, String> {
    require_client(&state)?
        .similar_artists(artist_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_collection_items(
    state: State<'_, AppState>,
    folder_id: Option<String>,
    order: Option<String>,
) -> Result<Vec<CollectionEntry>, String> {
    let folder = folder_id.unwrap_or_else(|| "root".to_string());
    require_client(&state)?
        .collection_items(&folder, order.as_deref())
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<CollectionFolder, String> {
    let parent = parent_id.unwrap_or_else(|| "root".to_string());
    require_client(&state)?
        .create_folder(&name, &parent)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn rename_folder(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    require_client(&state)?
        .rename_folder(&id, &name)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn remove_collection_entry(
    state: State<'_, AppState>,
    kind: String,
    id: String,
) -> Result<(), String> {
    require_client(&state)?
        .remove_collection_entry(&kind, &id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn move_playlists_to_folder(
    state: State<'_, AppState>,
    ids: Vec<String>,
    folder_id: String,
) -> Result<(), String> {
    require_client(&state)?
        .move_playlists(&ids, &folder_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_purchases(state: State<'_, AppState>) -> Result<Vec<PageItem>, String> {
    match require_client(&state)?.purchases(100, 0).await {
        Ok((items, _total)) => Ok(items),
        Err(Error::NotFound(_)) => Ok(Vec::new()),
        Err(e) => Err(state.fail_tidal(e)),
    }
}

#[tauri::command]
pub async fn get_album(state: State<'_, AppState>, id: u64) -> Result<Album, String> {
    require_client(&state)?
        .album(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_album_tracks(state: State<'_, AppState>, id: u64) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .album_tracks_all(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_album_info(state: State<'_, AppState>, id: u64) -> Result<AlbumInfo, String> {
    require_client(&state)?
        .album_info(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_track(state: State<'_, AppState>, id: u64) -> Result<Track, String> {
    require_client(&state)?
        .track(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_track_credits(
    state: State<'_, AppState>,
    id: u64,
) -> Result<TrackCredits, String> {
    require_client(&state)?
        .track_credits(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_playlist(state: State<'_, AppState>, id: String) -> Result<Playlist, String> {
    require_client(&state)?
        .playlist(&id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_playlist_tracks(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .playlist_tracks_all(&id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist(state: State<'_, AppState>, id: u64) -> Result<Artist, String> {
    require_client(&state)?
        .artist(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

/// Only web URLs with a host can be handed to the system browser.
pub(crate) fn external_url(raw: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(raw).map_err(|_| "invalid URL".to_string())?;
    match parsed.scheme() {
        "http" | "https" if parsed.host_str().is_some() => Ok(parsed),
        _ => Err("only http(s) URLs with a host can be opened".to_string()),
    }
}

#[tauri::command]
pub fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let parsed = external_url(&url)?;
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_artist_profile(
    state: State<'_, AppState>,
    id: u64,
) -> Result<ArtistProfile, String> {
    require_client(&state)?
        .artist_profile(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_top_tracks(
    state: State<'_, AppState>,
    artist_id: u64,
    limit: u32,
) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .artist_top_tracks(artist_id, limit)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_albums(
    state: State<'_, AppState>,
    artist_id: u64,
    filter: ArtistDiscography,
    limit: u32,
    offset: u32,
) -> Result<Vec<Album>, String> {
    require_client(&state)?
        .artist_albums(artist_id, filter, limit, offset)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_top_tracks_page(
    state: State<'_, AppState>,
    artist_id: u64,
    limit: u32,
    offset: u32,
) -> Result<TrackPageDto, String> {
    let (items, total) = require_client(&state)?
        .artist_top_tracks_page(artist_id, limit, offset)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(TrackPageDto {
        items,
        total,
        offset,
        limit,
    })
}

#[tauri::command]
pub async fn get_artist_albums_all(
    state: State<'_, AppState>,
    artist_id: u64,
    filter: ArtistDiscography,
) -> Result<Vec<Album>, String> {
    require_client(&state)?
        .artist_albums_all(artist_id, filter)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_page(
    state: State<'_, AppState>,
    artist_id: u64,
) -> Result<Vec<PageSection>, String> {
    require_client(&state)?
        .artist_page(artist_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Result<SearchResults, String> {
    require_client(&state)?
        .search(&query, &["tracks", "albums", "artists", "playlists"], 20, 0)
        .await
        .map_err(|e| state.fail_tidal(e))
}

/// One page of a single result type (Explore's filter chips): `kind` is
/// `tracks`, `albums`, `artists` or `playlists`; the other groups come back
/// empty.
#[tauri::command]
pub async fn search_type(
    state: State<'_, AppState>,
    query: String,
    kind: String,
    offset: u32,
) -> Result<SearchResults, String> {
    const KINDS: [&str; 4] = ["tracks", "albums", "artists", "playlists"];
    let Some(kind) = KINDS.iter().find(|k| **k == kind) else {
        return Err(format!("unknown search type: {kind}"));
    };
    let mut results = require_client(&state)?
        .search(&query, &[kind], 50, offset)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    results.top_hit = None;
    Ok(results)
}

#[tauri::command]
pub async fn get_favorite_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .all_favorite_tracks()
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    require_client(&state)?
        .all_favorite_albums()
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_artists(state: State<'_, AppState>) -> Result<Vec<Artist>, String> {
    require_client(&state)?
        .all_favorite_artists()
        .await
        .map_err(|e| state.fail_tidal(e))
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
        .map_err(|e| state.fail_tidal(e))?;
    state.favorite_changed("track", track_id, favorite);
    Ok(())
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
        .map_err(|e| state.fail_tidal(e))?;
    state.favorite_changed("album", album_id, favorite);
    Ok(())
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
        .map_err(|e| state.fail_tidal(e))?;
    state.favorite_changed("artist", artist_id, favorite);
    Ok(())
}

#[tauri::command]
pub async fn get_favorite_ids(state: State<'_, AppState>) -> Result<FavoriteIds, String> {
    require_client(&state)?
        .favorite_ids()
        .await
        .map_err(|e| state.fail_tidal(e))
}

/// Every Save for Later id, by kind (videos only counted).
#[tauri::command]
pub async fn get_save_for_later_ids(state: State<'_, AppState>) -> Result<LaterIds, String> {
    require_client(&state)?
        .later_ids()
        .await
        .map_err(|e| state.fail_tidal(e))
}

/// One Save for Later page, newest first, hydrated into the usual models.
#[derive(Serialize)]
pub struct LaterPageDto {
    pub items: Vec<LaterEntry>,
    pub cursor: Option<String>,
    /// Videos on this page, which OxiTide does not show.
    pub videos: u32,
}

#[tauri::command]
pub async fn get_save_for_later_page(
    state: State<'_, AppState>,
    cursor: Option<String>,
) -> Result<LaterPageDto, String> {
    let client = require_client(&state)?;
    let page = client
        .later_page(cursor.as_deref())
        .await
        .map_err(|e| state.fail_tidal(e))?;
    let videos = page
        .items
        .iter()
        .filter(|item| item.kind == LaterKind::Video)
        .count() as u32;
    let items = client
        .later_entries(page.items)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(LaterPageDto {
        items,
        cursor: page.cursor,
        videos,
    })
}

/// Save to or remove from Save for Later. TIDAL skipping an item that is
/// already saved counts as success (spec SL-D7).
#[tauri::command]
pub async fn set_save_for_later(
    state: State<'_, AppState>,
    kind: LaterKind,
    id: String,
    saved: bool,
) -> Result<(), String> {
    let client = require_client(&state)?;
    let items = [(kind, id)];
    let result = if saved {
        client.later_add(&items).await.map(|_| ())
    } else {
        client.later_remove(&items).await
    };
    result.map_err(|e| match e {
        Error::Api { status: 409, message } if message.contains("TOO_MANY_ITEMS") => {
            "Save for Later is full".to_string()
        }
        e => state.fail_tidal(e),
    })
}

/// The playlist's collaborator invitation, reused for the session until it
/// nears expiry (spec CO-D3). `renew` deletes the current link first, so it
/// stops working.
#[tauri::command]
pub async fn playlist_invite(
    state: State<'_, AppState>,
    uuid: String,
    renew: bool,
) -> Result<PlaylistInvite, String> {
    let client = require_client(&state)?;
    if renew {
        if let Some(old) = state.invites.take(&uuid) {
            client
                .delete_share(&old.share_id)
                .await
                .map_err(|e| state.fail_tidal(e))?;
        }
    } else if let Some(invite) = state.invites.get(&uuid) {
        return Ok(invite);
    }
    let invite = client
        .create_playlist_invite(&uuid)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    state.invites.insert(&uuid, invite.clone());
    Ok(invite)
}

/// The owner, then the collaborators.
#[tauri::command]
pub async fn playlist_members(
    state: State<'_, AppState>,
    uuid: String,
) -> Result<Vec<PlaylistMember>, String> {
    require_client(&state)?
        .playlist_members(&uuid)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> Result<Playlist, String> {
    require_client(&state)?
        .create_playlist(&name, &description)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn set_playlist_public(
    state: State<'_, AppState>,
    id: String,
    is_public: bool,
) -> Result<(), String> {
    require_client(&state)?
        .set_playlist_public(&id, is_public)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn rename_playlist(
    state: State<'_, AppState>,
    id: String,
    title: String,
    description: String,
) -> Result<(), String> {
    require_client(&state)?
        .rename_playlist(&id, &title, &description)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, id: String) -> Result<(), String> {
    require_client(&state)?
        .delete_playlist(&id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn add_playlist_tracks(
    state: State<'_, AppState>,
    id: String,
    track_ids: Vec<u64>,
    allow_duplicates: bool,
) -> Result<Vec<u64>, String> {
    require_client(&state)?
        .add_playlist_tracks(&id, &track_ids, allow_duplicates)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn remove_playlist_items(
    state: State<'_, AppState>,
    id: String,
    indices: Vec<u32>,
) -> Result<(), String> {
    require_client(&state)?
        .remove_playlist_items(&id, &indices)
        .await
        .map_err(|e| state.fail_tidal(e))
}

/// One removed playlist row, as it was before the removal (undo).
#[derive(Debug, Clone, Deserialize)]
pub struct RemovedPlaylistRow {
    pub index: u32,
    pub track_id: u64,
}

/// Undo of `remove_playlist_items`: put the rows back at their indices.
#[tauri::command]
pub async fn restore_playlist_items(
    state: State<'_, AppState>,
    id: String,
    rows: Vec<RemovedPlaylistRow>,
) -> Result<(), String> {
    let rows: Vec<(u32, u64)> = rows.iter().map(|row| (row.index, row.track_id)).collect();
    require_client(&state)?
        .restore_playlist_items(&id, &rows)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn move_playlist_items(
    state: State<'_, AppState>,
    id: String,
    indices: Vec<u32>,
    to_index: u32,
) -> Result<(), String> {
    require_client(&state)?
        .move_playlist_items(&id, &indices, to_index)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_user_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    require_client(&state)?
        .all_user_playlists()
        .await
        .map_err(|e| state.fail_tidal(e))
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
    Ok(TrackPageDto {
        items,
        total,
        offset,
        limit,
    })
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
    Ok(TrackPageDto {
        items,
        total,
        offset,
        limit,
    })
}

/// One page-feed track as the UI sends it for instant playback: embedded
/// metadata is used when complete, otherwise the id is resolved server-side.
#[derive(Debug, Clone, Deserialize)]
pub struct PageTrackDto {
    pub id: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artists: Option<Vec<String>>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub explicit: Option<bool>,
}

impl PageTrackDto {
    /// The queue item when the payload carried enough metadata; `None` asks
    /// the caller to resolve the track by id.
    fn embedded(&self) -> Option<QueueItem> {
        let title = self.title.as_deref().filter(|title| !title.is_empty())?;
        let artists = self.artists.as_ref().filter(|artists| !artists.is_empty())?;
        let duration = self.duration.filter(|duration| *duration > 0)?;
        Some(QueueItem {
            track_id: self.id,
            title: title.to_string(),
            artist: artists.join(", "),
            album: self.album.clone().unwrap_or_default(),
            duration_secs: duration,
            cover: self.cover.clone(),
            artist_refs: Vec::new(),
            album_id: None,
            autoplay: false,
        })
    }
}

/// Build queue items for `ids`, preserving input order, with at most four
/// TIDAL requests in flight. Fails on the first fetch error. `flags` receives
/// every observed `explicit` value so session saves and later filters can use it.
async fn queue_items_from_ids(
    client: &TidalClient,
    ids: &[u64],
    flags: Option<&crate::track_flags::TrackFlagsStore>,
) -> Result<Vec<QueueItem>, Error> {
    let mut results: Vec<Option<QueueItem>> = vec![None; ids.len()];
    let mut pending = ids.iter().copied().enumerate();
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let Some((index, id)) = pending.next() else {
            break;
        };
        let client = client.clone();
        set.spawn(async move { (index, client.track(id).await) });
    }
    while let Some(joined) = set.join_next().await {
        let (index, result) = joined.map_err(|e| Error::Task(e.to_string()))?;
        let track = result?;
        if let Some(store) = flags {
            store.record_explicit(track.id, track.explicit);
        }
        results[index] = Some(queue_item_from_track(&track));
        if let Some((next_index, next_id)) = pending.next() {
            let client = client.clone();
            set.spawn(async move { (next_index, client.track(next_id).await) });
        }
    }
    Ok(results.into_iter().flatten().collect())
}

/// Queue metadata for a track, matching the UI's `trackToQueueItem`.
pub(crate) fn queue_item_from_track(track: &Track) -> QueueItem {
    QueueItem {
        track_id: track.id,
        title: track.title.clone(),
        artist: track
            .artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        album: track
            .album
            .as_ref()
            .map(|a| a.title.clone())
            .unwrap_or_default(),
        duration_secs: track.duration,
        cover: track.album.as_ref().and_then(|a| a.cover.clone()),
        artist_refs: track
            .artists
            .iter()
            .map(|a| ArtistRef {
                id: a.id,
                name: a.name.clone(),
            })
            .collect(),
        album_id: track.album.as_ref().map(|a| a.id),
        autoplay: false,
    }
}

/// Queue items for page-feed tracks, preserving order. Complete embedded
/// metadata skips the per-track API call (spec §5.4); incomplete entries fall
/// back to `queue_items_from_ids`, so a partial payload still plays.
async fn queue_items_from_page_items(
    client: &TidalClient,
    items: &[PageTrackDto],
    flags: Option<&crate::track_flags::TrackFlagsStore>,
) -> Result<Vec<QueueItem>, Error> {
    let mut seen = std::collections::HashSet::new();
    let missing: Vec<u64> = items
        .iter()
        .filter(|item| item.embedded().is_none())
        .map(|item| item.id)
        .filter(|id| seen.insert(*id))
        .collect();
    let resolved: std::collections::HashMap<u64, QueueItem> = if missing.is_empty() {
        std::collections::HashMap::new()
    } else {
        queue_items_from_ids(client, &missing, flags)
            .await?
            .into_iter()
            .map(|item| (item.track_id, item))
            .collect()
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if let Some(entry) = item.embedded() {
            if let (Some(store), Some(explicit)) = (flags, item.explicit) {
                store.record_explicit(item.id, explicit);
            }
            out.push(entry);
        } else if let Some(entry) = resolved.get(&item.id).cloned() {
            out.push(entry);
        } else {
            return Err(Error::NotFound(format!("tracks/{}", item.id)));
        }
    }
    Ok(out)
}

/// Fetch the requested tracks and filter them; kept for the id-only path.
async fn resolve_playable_items(
    client: &TidalClient,
    ids: &[u64],
    flags: Option<&crate::track_flags::TrackFlagsStore>,
    content: &crate::config::ContentConfig,
) -> Result<(Vec<QueueItem>, Vec<u64>), Error> {
    let items = queue_items_from_ids(client, ids, flags).await?;
    filter_playable_items(client, ids, items, flags, content).await
}

/// Apply the active content filters to already-resolved queue items. Unknown
/// flags never block; AI enrichment uses `ids` to batch-fetch flags.
pub(crate) async fn filter_playable_items(
    client: &TidalClient,
    ids: &[u64],
    items: Vec<QueueItem>,
    flags: Option<&crate::track_flags::TrackFlagsStore>,
    content: &crate::config::ContentConfig,
) -> Result<(Vec<QueueItem>, Vec<u64>), Error> {
    if !content.block_explicit && !content.block_ai {
        let kept_ids = items.iter().map(|item| item.track_id).collect();
        return Ok((items, kept_ids));
    }
    let known: std::collections::HashMap<u64, crate::track_flags::TrackFlagEntry> = match flags {
        Some(store) => {
            if content.block_ai {
                store.enrich(client, ids).await?;
            }
            store.known(ids).into_iter().collect()
        }
        None => std::collections::HashMap::new(),
    };
    let mut kept_items = Vec::new();
    let mut kept_ids = Vec::new();
    for item in items {
        let entry = known.get(&item.track_id);
        let blocked = crate::track_flags::is_blocked(
            entry.and_then(|entry| entry.explicit),
            entry.and_then(|entry| entry.ai),
            content,
        );
        if !blocked {
            kept_ids.push(item.track_id);
            kept_items.push(item);
        }
    }
    Ok((kept_items, kept_ids))
}

/// Play a list of track ids (resolved server-side), replacing the queue.
/// Content filters are enforced here as a backstop for every UI path.
#[tauri::command]
pub async fn play_track_ids(
    state: State<'_, AppState>,
    ids: Vec<u64>,
    start: usize,
    shuffle: Option<bool>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Err("no tracks".to_string());
    }
    let client = require_client(&state)?;
    let content = state.config().content;
    let store = state.track_flags();
    let (items, kept_ids) =
        resolve_playable_items(&client, &ids, store.as_deref(), &content)
            .await
            .map_err(|e| state.fail_tidal(e))?;
    if items.is_empty() {
        return Err("all tracks are hidden by your content filters".to_string());
    }
    let start = crate::track_flags::remap_start(&ids, &kept_ids, start);
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let cmd = if shuffle.unwrap_or(false) {
        EngineCmd::PlayShuffled { items }
    } else {
        EngineCmd::PlayTracks { items, start }
    };
    crate::perf::note_play_command();
    player.engine.handle(cmd).map_err(|e| e.to_string())
}

/// Play page-feed tracks, using embedded metadata and resolving only the
/// items whose payload was incomplete. Content filters are enforced here as
/// the backstop for every UI path.
#[tauri::command]
pub async fn play_page_items(
    state: State<'_, AppState>,
    items: Vec<PageTrackDto>,
    start: usize,
    shuffle: Option<bool>,
) -> Result<(), String> {
    if items.is_empty() {
        return Err("no tracks".to_string());
    }
    let client = require_client(&state)?;
    let content = state.config().content;
    let store = state.track_flags();
    let ids: Vec<u64> = items.iter().map(|item| item.id).collect();
    let queue = queue_items_from_page_items(&client, &items, store.as_deref())
        .await
        .map_err(|e| state.fail_tidal(e))?;
    let (queue, kept_ids) = filter_playable_items(&client, &ids, queue, store.as_deref(), &content)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    if queue.is_empty() {
        return Err("all tracks are hidden by your content filters".to_string());
    }
    let start = crate::track_flags::remap_start(&ids, &kept_ids, start);
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let cmd = if shuffle.unwrap_or(false) {
        EngineCmd::PlayShuffled { items: queue }
    } else {
        EngineCmd::PlayTracks { items: queue, start }
    };
    crate::perf::note_play_command();
    player.engine.handle(cmd).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn play_tracks(
    state: State<'_, AppState>,
    items: Vec<QueueItem>,
    start: usize,
    shuffle: Option<bool>,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let cmd = if shuffle.unwrap_or(false) {
        EngineCmd::PlayShuffled { items }
    } else {
        EngineCmd::PlayTracks { items, start }
    };
    crate::perf::note_play_command();
    player.engine.handle(cmd).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    crate::perf::note_play_command();
    player
        .engine
        .handle(EngineCmd::PlayPause)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn next(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::Next)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn prev(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::Prev)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn seek(state: State<'_, AppState>, position_ms: u64) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::Seek(position_ms))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_volume(state: State<'_, AppState>, volume: f32, persist: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::SetVolume(volume))
        .map_err(|e| e.to_string())?;
    if persist {
        let mut config = state.config();
        config.audio.volume = volume.clamp(0.0, 1.0);
        config.save()?;
        state.set_config(config);
    }
    Ok(())
}

#[tauri::command]
pub fn fade_and_pause(state: State<'_, AppState>, duration_ms: u64) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::FadeAndPause { duration_ms })
        .map_err(|e| e.to_string())
}

/// One page of the local history log, newest first.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryPageDto {
    pub items: Vec<crate::history::HistoryRecord>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[tauri::command(async)]
pub fn get_history_page(
    state: State<'_, AppState>,
    offset: usize,
    limit: usize,
) -> Result<HistoryPageDto, String> {
    let limit = limit.clamp(1, 500);
    let (items, total) = match state.history() {
        Some(store) => store.page(offset, limit),
        None => (Vec::new(), 0),
    };
    Ok(HistoryPageDto { items, total, offset, limit })
}

#[tauri::command(async)]
pub fn get_stats(state: State<'_, AppState>, offset_minutes: i32) -> Result<crate::history::Stats, String> {
    Ok(match state.history() {
        Some(store) => store.stats(offset_minutes),
        None => crate::history::Stats::default(),
    })
}

#[tauri::command(async)]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(store) = state.history() {
        store.clear();
    }
    Ok(())
}

#[tauri::command]
pub fn set_history_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    if let Some(store) = state.history() {
        store.set_enabled(enabled);
    }
    let mut config = state.config();
    config.history.enabled = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_history_max_entries(state: State<'_, AppState>, max_entries: usize) -> Result<(), String> {
    let max_entries = max_entries.clamp(1, 1_000_000);
    if let Some(store) = state.history() {
        store.set_max_entries(max_entries);
    }
    let mut config = state.config();
    config.history.max_entries = max_entries;
    config.save()?;
    state.set_config(config);
    Ok(())
}

/// Local play counts for track sorting; every requested id is answered (0
/// when unplayed or when history is unavailable).
#[tauri::command(async)]
pub fn get_play_counts(state: State<'_, AppState>, ids: Vec<u64>) -> Result<Vec<PlayCountDto>, String> {
    let counts = state.history().map(|store| store.play_counts(&ids)).unwrap_or_default();
    Ok(ids
        .into_iter()
        .map(|track_id| PlayCountDto { track_id, plays: counts.get(&track_id).copied().unwrap_or(0) })
        .collect())
}

/// Album release dates for track sorting, cached on disk; unknown ids are
/// omitted (fail open). Offline or signed out, only cached dates answer.
#[tauri::command]
pub async fn get_album_release_dates(
    state: State<'_, AppState>,
    ids: Vec<u64>,
) -> Result<Vec<AlbumDateDto>, String> {
    let store = state.album_dates();
    let dates = match require_client(&state) {
        Ok(client) => store.enrich(&client, &ids).await.map_err(|e| state.fail_tidal(e))?,
        Err(_) => store.known(&ids),
    };
    Ok(dates
        .into_iter()
        .map(|(album_id, release_date)| AlbumDateDto { album_id, release_date })
        .collect())
}

#[tauri::command]
pub async fn get_track_flags(
    state: State<'_, AppState>,
    ids: Vec<u64>,
) -> Result<Vec<TrackFlagDto>, String> {
    let Some(store) = state.track_flags() else {
        return Ok(Vec::new());
    };
    let entries = match require_client(&state) {
        Ok(client) => store
            .enrich(&client, &ids)
            .await
            .map_err(|e| state.fail_tidal(e))?,
        // Not logged in: cached flags only; unknown stays unknown.
        Err(_) => store.known(&ids),
    };
    Ok(entries
        .into_iter()
        .map(|(track_id, entry)| TrackFlagDto {
            track_id,
            explicit: entry.explicit,
            ai: entry.ai,
        })
        .collect())
}

#[tauri::command]
pub fn set_block_explicit(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.content.block_explicit = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

/// Autoplay on or off. Off drops the upcoming suggestions (the current track
/// keeps playing); on fills at once when the last track is already current.
#[tauri::command]
pub fn set_autoplay(app: tauri::AppHandle, state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.playback.autoplay = enabled;
    config.save()?;
    state.set_config(config);
    state.autoplay.set_enabled(enabled);
    if let Some(player) = state.player() {
        if enabled {
            state.autoplay.consider(&player.engine.state(), &app);
        } else {
            state.autoplay.prune(&player.engine);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_block_ai(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.content.block_ai = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_show_labels(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.content.show_labels = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_shuffle(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::SetShuffle(enabled))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_repeat(state: State<'_, AppState>, mode: RepeatMode) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::SetRepeat(mode))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_reorder(state: State<'_, AppState>, from: usize, to: usize) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::Reorder { from, to })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_remove(state: State<'_, AppState>, index: usize) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::Remove(index))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_clear(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::Clear)
        .map_err(|e| e.to_string())
}

/// Enforce a just-changed content filter on the live queue: the explicit
/// toggle clears it (official parity), the AI toggle removes only blocked
/// items and advances playback when the current track is removed. No-op when
/// the scope's filter is off or nothing is blocked.
#[tauri::command]
pub async fn apply_content_filters(
    state: State<'_, AppState>,
    scope: String,
) -> Result<(), String> {
    let content = state.config().content;
    match crate::track_flags::filter_action(&scope, &content) {
        crate::track_flags::FilterAction::Noop => return Ok(()),
        crate::track_flags::FilterAction::Clear => {
            let Some(player) = state.player() else {
                return Ok(());
            };
            return player
                .engine
                .handle(EngineCmd::Clear)
                .map_err(|e| e.to_string());
        }
        crate::track_flags::FilterAction::Prune => {}
    }
    let Some(player) = state.player() else {
        return Ok(());
    };
    let snapshot = player.engine.state();
    if snapshot.queue.is_empty() {
        return Ok(());
    }
    let ids: Vec<u64> = snapshot.queue.iter().map(|item| item.track_id).collect();
    let entries = match state.track_flags() {
        Some(store) => match require_client(&state) {
            Ok(client) => store
                .enrich(&client, &ids)
                .await
                .map_err(|e| state.fail_tidal(e))?,
            Err(_) => store.known(&ids),
        },
        None => Vec::new(),
    };
    let known: std::collections::HashMap<u64, crate::track_flags::TrackFlagEntry> =
        entries.into_iter().collect();
    let blocked: Vec<bool> = snapshot
        .queue
        .iter()
        .map(|item| {
            let entry = known.get(&item.track_id);
            crate::track_flags::is_blocked(
                entry.and_then(|entry| entry.explicit),
                entry.and_then(|entry| entry.ai),
                &content,
            )
        })
        .collect();
    let outcome = crate::track_flags::plan_prune(snapshot.queue.len(), snapshot.current_index, &blocked);
    if outcome.remove.is_empty() {
        return Ok(());
    }
    if outcome.current_removed {
        let keep: Vec<QueueItem> = snapshot
            .queue
            .iter()
            .zip(blocked.iter())
            .filter(|(_, blocked)| !**blocked)
            .map(|(item, _)| item.clone())
            .collect();
        if keep.is_empty() {
            return player
                .engine
                .handle(EngineCmd::Clear)
                .map_err(|e| e.to_string());
        }
        let start = outcome.new_current.unwrap_or(0);
        let cmd = if snapshot.playing {
            EngineCmd::PlayTracks { items: keep, start }
        } else {
            EngineCmd::RestoreSession {
                items: keep,
                start,
                position_ms: 0,
                shuffle: snapshot.shuffle,
                repeat: snapshot.repeat,
            }
        };
        player.engine.handle(cmd).map_err(|e| e.to_string())?;
    } else {
        for index in outcome.remove {
            player
                .engine
                .handle(EngineCmd::Remove(index))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn retry(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.retry().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_add(state: State<'_, AppState>, items: Vec<QueueItem>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::Enqueue { items })
        .map_err(|e| e.to_string())
}

/// "Play next": insert right after the current track.
#[tauri::command]
pub fn queue_play_next(state: State<'_, AppState>, items: Vec<QueueItem>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::EnqueueNext { items })
        .map_err(|e| e.to_string())
}

/// Insert at queue slot `index` (a drag dropped on the queue rail).
#[tauri::command]
pub fn queue_insert(
    state: State<'_, AppState>,
    index: usize,
    items: Vec<QueueItem>,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::EnqueueAt { index, items })
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn get_playback_state(state: State<'_, AppState>) -> Result<crate::player::StateDto, String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    Ok(crate::player::StateDto::full(player.engine.state()))
}

/// One track's cached flags as the UI enrichment store expects them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackFlagDto {
    pub track_id: u64,
    pub explicit: Option<bool>,
    pub ai: Option<bool>,
}

/// Local play count for one track (track sorting, multi-sort spec §4).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayCountDto {
    pub track_id: u64,
    pub plays: u32,
}

/// An album's release date (`YYYY-MM-DD`), `None` when TIDAL has none.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlbumDateDto {
    pub album_id: u64,
    pub release_date: Option<String>,
}

/// The persisted session as the UI should apply it, or `None` when no usable
/// file exists. Playback is applied paused by `restore_session`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestoredSessionDto {
    pub queue: Vec<QueueItem>,
    pub current_index: usize,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub play_source: Option<String>,
    /// What started the queue (the player bar's "From …" line).
    pub play_from: Option<crate::session::PlayFrom>,
    /// Last known stream for the restored current track (detailed quality
    /// display before playback opens a stream).
    pub stream: Option<oxitide_audio::engine::StreamMeta>,
}

#[tauri::command(async)]
pub fn get_restored_session(state: State<'_, AppState>) -> Result<Option<RestoredSessionDto>, String> {
    let Some(path) = crate::session::session_path() else {
        return Ok(None);
    };
    let Some(session) = crate::session::load(&path) else {
        return Ok(None);
    };
    let content = state.config().content;
    let filtered = crate::session::filter_session_queue(
        session.queue,
        session.current_index,
        session.position_ms,
        &content,
    );
    Ok(Some(RestoredSessionDto {
        current_index: filtered.current_index.unwrap_or(0),
        position_ms: filtered.position_ms,
        shuffle: session.shuffle,
        repeat: session.repeat,
        play_source: session.play_source,
        play_from: session.play_from,
        stream: session.stream,
        queue: filtered.items.into_iter().map(QueueItem::from).collect(),
    }))
}

#[tauri::command]
pub fn restore_session(
    state: State<'_, AppState>,
    items: Vec<QueueItem>,
    start: usize,
    position_ms: u64,
    shuffle: bool,
    repeat: RepeatMode,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::RestoreSession { items, start, position_ms, shuffle, repeat })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_play_source(
    state: State<'_, AppState>,
    id: Option<String>,
    from: Option<crate::session::PlayFrom>,
) -> Result<(), String> {
    if let Some(saver) = state.session_saver() {
        saver.set_play_source(id, from);
    }
    Ok(())
}

#[tauri::command]
pub fn clear_session(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(saver) = state.session_saver() {
        saver.clear();
    }
    Ok(())
}

#[tauri::command]
pub fn perf_mark(name: String) {
    crate::perf::mark(&name);
}

/// Perf runs end through the normal exit path (session flush, history
/// finalize) instead of a forced kill. A no-op outside a perf session.
#[tauri::command]
pub fn perf_quit(app: tauri::AppHandle) -> Result<(), String> {
    if crate::perf::session().is_none() {
        return Err("no perf session".into());
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn get_perf_snapshot() -> crate::perf::PerfSnapshot {
    crate::perf::snapshot()
}

#[tauri::command]
pub fn perf_submit_ui_metrics(metrics: serde_json::Value) {
    crate::perf::ui_metrics(metrics);
}

/// Perf-session-only: builds a 10k-track play context so the S7 gate can
/// measure queue construction without touching playback. Errors outside a
/// session, so normal runs cannot reach it.
#[tauri::command]
pub fn perf_queue_build(items: Vec<QueueItem>, start: usize) -> Result<(), String> {
    perf_queue_build_items(items, start)
}

fn perf_queue_build_items(items: Vec<QueueItem>, start: usize) -> Result<(), String> {
    if crate::perf::session().is_none() {
        return Err("no perf session".into());
    }
    let mut queue = oxitide_audio::queue::Queue::default();
    queue.play_context(items, start);
    Ok(())
}

#[tauri::command(async)]
pub fn get_output_devices() -> Vec<OutputDeviceDto> {
    devices()
}

#[tauri::command]
pub fn set_output_device(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let device = if device_id.is_empty() {
        None
    } else {
        Some(device_id.clone())
    };
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

/// Replace the sound-processing settings (EQ, ReplayGain, crossfade). Applies
/// live to the playing track and persists the sanitized values.
#[tauri::command]
pub fn set_dsp_settings(
    state: State<'_, AppState>,
    settings: DspSettings,
) -> Result<DspSettings, String> {
    let settings = settings.sanitized();
    if let Some(player) = state.player() {
        player.engine.set_dsp(settings.clone());
    }
    let mut config = state.config();
    config.dsp = settings.clone();
    config.save()?;
    state.set_config(config);
    Ok(settings)
}

/// AutoMix replay lab: the last transition re-planned and mixed in `style`
/// with the current crossfade settings, rendered in memory (never written to
/// disk). The payload is `[u32 LE header length][JSON header][16-bit WAV]`;
/// the header holds the report and where the overlap sits in the audio.
#[tauri::command(async)]
pub fn automix_render(
    state: State<'_, AppState>,
    style: TransitionStyle,
) -> Result<tauri::ipc::Response, String> {
    let player = state.player().ok_or("Playback is not ready")?;
    let mut settings = state.config().dsp.crossfade;
    settings.style = style;
    let rendered = player.engine.render_last_transition(&settings).ok_or(
        "Nothing to replay yet: let a track end on its own with AutoMix on.",
    )?;
    let header = serde_json::json!({
        "report": rendered.report,
        "overlap_start_ms": rendered.overlap_start_ms,
        "overlap_end_ms": rendered.overlap_end_ms,
    })
    .to_string();
    let wav = wav_pcm16(&rendered.samples, rendered.channels, rendered.rate);
    let mut payload = Vec::with_capacity(4 + header.len() + wav.len());
    payload.extend_from_slice(&(header.len() as u32).to_le_bytes());
    payload.extend_from_slice(header.as_bytes());
    payload.extend_from_slice(&wav);
    Ok(tauri::ipc::Response::new(payload))
}

/// A canonical 16-bit PCM WAV of interleaved normalized samples.
fn wav_pcm16(samples: &[f32], channels: usize, rate: u32) -> Vec<u8> {
    let channels = channels.max(1) as u16;
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    wav.extend_from_slice(&(channels * 2).to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        wav.extend_from_slice(&v.to_le_bytes());
    }
    wav
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    // The renderer must never receive the Last.fm API secret; the UI uses
    // neither the key nor the secret. `Config::save` keeps the real value.
    let mut config = state.config();
    config.scrobbling.lastfm_api_secret = String::new();
    Ok(config)
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

#[tauri::command(async)]
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
pub fn set_ui_mode(state: State<'_, AppState>, mode: UiMode) -> Result<(), String> {
    let mut config = state.config();
    config.ui.mode = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, theme: Theme) -> Result<(), String> {
    let mut config = state.config();
    config.ui.theme = theme;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_instrument_tone(state: State<'_, AppState>, tone: InstrumentTone) -> Result<(), String> {
    let mut config = state.config();
    config.ui.instrument_tone = tone;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_typeface(state: State<'_, AppState>, typeface: Typeface) -> Result<(), String> {
    let mut config = state.config();
    config.ui.typeface = typeface;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_custom_typeface(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let mut config = state.config();
    config.ui.custom_typeface = name;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_density(state: State<'_, AppState>, density: Density) -> Result<(), String> {
    let mut config = state.config();
    config.ui.density = density;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_cover_shape(state: State<'_, AppState>, shape: CoverShape) -> Result<(), String> {
    let mut config = state.config();
    config.ui.cover_shape = shape;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_dynamic_color(state: State<'_, AppState>, mode: DynamicColor) -> Result<(), String> {
    let mut config = state.config();
    config.ui.dynamic_color = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}

/// Hi-Fi Rack options (Track 9.5). The whole section is written at once; the
/// stored (clamped) value is returned so the UI can reconcile.
#[tauri::command]
pub fn set_rack_settings(
    state: State<'_, AppState>,
    settings: RackConfig,
) -> Result<RackConfig, String> {
    let mut config = state.config();
    config.rack = settings.clamped();
    let stored = config.rack.clone();
    config.save()?;
    state.set_config(config);
    Ok(stored)
}

/// Start or stop the Rack level meters (RK-D7). While enabled, the current
/// engine's tap measures written audio and `level_frame` is emitted ~30 Hz.
/// The tap is re-enabled every tick so an engine rebuilt after re-login is
/// picked up.
#[tauri::command]
pub fn set_level_meter(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    use tauri::Manager;
    if enabled {
        let mut gate = crate::meter::SilenceGate::default();
        state.meter.start(move || {
            let state = app.state::<AppState>();
            let Some(player) = state.player() else {
                return false;
            };
            let tap = player.engine.level_tap();
            if !tap.is_enabled() {
                tap.set_enabled(true);
            }
            let frame = tap.read();
            if gate.admit(&frame) {
                let dto = crate::meter::LevelFrameDto::from(frame);
                crate::perf::emit_counted(&app, crate::meter::LEVEL_FRAME_EVENT, dto);
            }
            !gate.idle()
        });
    } else {
        state.meter.stop();
        if let Some(player) = state.player() {
            player.engine.level_tap().set_enabled(false);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_queue_width(state: State<'_, AppState>, px: u32) -> Result<(), String> {
    let mut config = state.config();
    config.ui.queue_width = clamp_queue_width(px);
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_mini_mode(
    state: State<'_, AppState>,
    window: tauri::WebviewWindow,
    active: bool,
) -> Result<(), String> {
    let current = state.pre_mini();
    // Idempotent (spec §5.3): a repeated enter keeps the original stash.
    if active && current.is_some() {
        return Ok(());
    }
    // The stash changes only after the window op succeeds (window::mini_stash_after):
    // a failed op leaves it authoritative, so a retry re-attempts the resize and
    // close-while-mini still persists the full geometry (MP-D7).
    let next = if active {
        let geometry = crate::window::read_geometry(&window).map_err(|e| e.to_string())?;
        crate::window::mini_stash_after(true, current, Some(geometry), || {
            crate::window::enter_mini(&window, geometry)
        })
        .map_err(|e| e.to_string())?
    } else {
        crate::window::mini_stash_after(false, current, None, || {
            crate::window::exit_mini(&window, current)
        })
        .map_err(|e| e.to_string())?
    };
    match next {
        Some(geometry) => state.set_pre_mini(geometry),
        None => {
            state.take_pre_mini();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_start_in_mini(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.start_in_mini = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_oneko(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.oneko = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_catjam(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.catjam = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_catjam_coat(state: State<'_, AppState>, coat: CatjamCoat) -> Result<(), String> {
    let mut config = state.config();
    config.ui.catjam_coat = coat;
    config.save()?;
    state.set_config(config);
    Ok(())
}

/// Stores the cat's size clamped to 100–200 % and returns what was stored.
#[tauri::command]
pub fn set_catjam_scale(state: State<'_, AppState>, percent: u32) -> Result<u32, String> {
    let mut config = state.config();
    config.ui.catjam_scale = clamp_catjam_scale(percent);
    let stored = config.ui.catjam_scale;
    config.save()?;
    state.set_config(config);
    Ok(stored)
}

/// CatJam: analyze the playing audio for beats (`beat` events) while the
/// jamming cat is mounted. Kept on `AppState` so an engine rebuilt after
/// re-login starts with the same setting.
#[tauri::command]
pub fn set_beat_tracking(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state.set_beat_tracking(enabled);
    Ok(())
}

#[tauri::command]
pub fn set_hover_panels(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.hover_panels = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_row_separators(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.row_separators = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_player_bar_style(state: State<'_, AppState>, style: PanelStyle) -> Result<(), String> {
    let mut config = state.config();
    config.ui.player_bar_style = style;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_queue_style(state: State<'_, AppState>, style: PanelStyle) -> Result<(), String> {
    let mut config = state.config();
    config.ui.queue_style = style;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_smtc(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.integrations.smtc = enabled;
    config.save()?;
    state.set_config(config);
    if let Some(session) = state.media.lock().unwrap().as_ref() {
        session.set_enabled(enabled);
    }
    Ok(())
}

#[tauri::command]
pub fn set_discord(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.integrations.discord = enabled;
    config.save()?;
    state.set_config(config);
    if let Some(service) = state.discord.lock().unwrap().as_ref() {
        service.set_enabled(enabled);
    }
    Ok(())
}

#[tauri::command]
pub fn set_close_to_tray(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.close_to_tray = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

/// Bind (or clear, with an empty accelerator) one global hotkey. `replace`
/// names another action whose conflicting binding is cleared in the same
/// mutation (the Settings reassign flow). Registration failures restore the
/// previous config and binding, then surface an error.
#[tauri::command(async)]
pub fn set_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: String,
    accelerator: String,
    replace: Option<String>,
) -> Result<(), String> {
    crate::hotkeys::with_mutation_lock(|| {
        if !crate::hotkeys::is_action(&action) {
            return Err(format!("unknown hotkey action: {action}"));
        }
        let accelerator = accelerator.trim().to_string();
        if !accelerator.is_empty() {
            crate::hotkeys::parse(&accelerator)?;
        }
        let previous = state.config();
        let mut next = previous.clone();
        crate::hotkeys::apply_binding(&mut next.hotkeys, &action, &accelerator, replace.as_deref())?;
        next.save()?;
        state.set_config(next.clone());
        if let Err(e) = crate::hotkeys::register_all(&app, &next.hotkeys) {
            let _ = previous.save();
            state.set_config(previous.clone());
            let _ = crate::hotkeys::register_all(&app, &previous.hotkeys);
            return Err(e);
        }
        Ok(())
    })
}

#[tauri::command(async)]
pub fn set_hotkey_capture(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    active: bool,
) -> Result<(), String> {
    crate::hotkeys::with_mutation_lock(|| {
        let config = state.config();
        crate::hotkeys::set_capture(&app, &config.hotkeys, active)
    })
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
pub fn get_remote_status(app: tauri::AppHandle) -> crate::remote::host::RemoteStatusDto {
    crate::remote::host::status(&app)
}

/// Turn the phone remote on or off (persisted). Enabling reports a bind
/// failure through the status rather than an error.
#[tauri::command]
pub async fn set_remote_enabled(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<crate::remote::host::RemoteStatusDto, String> {
    let mut config = state.config();
    config.remote.enabled = enabled;
    config.save()?;
    state.set_config(config);
    if enabled {
        let _ = crate::remote::host::start(&app).await;
    } else {
        crate::remote::host::stop(&app).await;
    }
    Ok(crate::remote::host::status(&app))
}

/// Port, QR interface, and the approval switch. A port change restarts a
/// running server.
#[tauri::command]
pub async fn set_remote_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    port: u16,
    interface: String,
    approve_new_devices: bool,
) -> Result<crate::remote::host::RemoteStatusDto, String> {
    let mut config = state.config();
    let port = crate::config::clamp_remote_port(port);
    let restart = config.remote.port != port && state.remote().is_some();
    config.remote.port = port;
    config.remote.interface = interface.trim().to_string();
    config.remote.approve_new_devices = approve_new_devices;
    config.save()?;
    state.set_config(config);
    if let Some(service) = state.remote() {
        service.set_approve_new_devices(approve_new_devices);
    }
    if restart {
        crate::remote::host::stop(&app).await;
        let _ = crate::remote::host::start(&app).await;
    }
    let status = crate::remote::host::status(&app);
    crate::remote::host::emit_status(&app);
    Ok(status)
}

#[tauri::command]
pub fn begin_remote_pairing(app: tauri::AppHandle) -> Result<crate::remote::host::PairingDto, String> {
    let pairing = crate::remote::host::begin_pairing(&app)?;
    crate::remote::host::emit_status(&app);
    Ok(pairing)
}

#[tauri::command]
pub fn cancel_remote_pairing(app: tauri::AppHandle, state: State<'_, AppState>) {
    if let Some(service) = state.remote() {
        service.cancel_pairing();
    }
    crate::remote::host::emit_status(&app);
}

#[tauri::command]
pub fn list_remote_devices(app: tauri::AppHandle) -> Vec<crate::remote::host::RemoteDeviceDto> {
    crate::remote::host::list_devices(&app)
}

/// Rename a paired phone (shown in Settings only).
#[tauri::command]
pub fn rename_remote_device(app: tauri::AppHandle, id: String, name: String) -> Result<(), String> {
    crate::remote::host::rename_device(&app, &id, &name)
}

#[tauri::command]
pub fn revoke_remote_device(app: tauri::AppHandle, id: String) -> Result<(), String> {
    crate::remote::host::revoke_device(&app, &id)
}

/// Approve or decline a waiting phone. `false` when the request is gone.
#[tauri::command]
pub fn resolve_remote_pair_request(
    state: State<'_, AppState>,
    id: String,
    approve: bool,
) -> Result<bool, String> {
    let service = state.remote().ok_or_else(|| "the phone remote is off".to_string())?;
    Ok(service.resolve_pair_request(&id, approve))
}

/// Failures propagate so the UI keeps its current colors instead of resetting
/// to brand teal (spec §5.2).
#[tauri::command]
pub async fn analyze_cover(
    state: State<'_, AppState>,
    uuid: String,
    theme: crate::palette::Room,
) -> Result<PaletteDto, String> {
    let Some(service) = state.artwork() else {
        return Err("artwork service unavailable".into());
    };
    service.palette(&uuid, theme).await.map(PaletteDto::from)
}

/// Whether the window runs undecorated with the UI's own caption buttons.
#[tauri::command]
pub fn get_custom_titlebar() -> bool {
    crate::titlebar::custom()
}

/// Where the UI's maximize button is, for the Snap Layouts overlay; `None`
/// hides the overlay. A sync command, so it runs on the main thread.
#[tauri::command]
pub fn set_snap_target(target: Option<crate::titlebar::SnapTarget>) {
    crate::titlebar::set_target(target);
}

#[tauri::command]
pub fn get_update_support(state: State<'_, AppState>) -> crate::updater::UpdateSupport {
    crate::updater::support(state.config().updates.check_on_start)
}

#[tauri::command]
pub async fn check_for_update(
    app: tauri::AppHandle,
) -> Result<Option<crate::updater::UpdateInfo>, String> {
    crate::updater::check(&app).await
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    crate::updater::install(&app).await
}

#[tauri::command]
pub fn set_update_check_on_start(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.updates.check_on_start = enabled;
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

/// Settings → Troubleshooting → Copy diagnostics: a redacted plain-text report
/// (`diagnostics.rs`). Async: it reads the log and runs `ver` on Windows.
#[tauri::command(async)]
pub fn get_diagnostics(state: State<'_, AppState>) -> Result<String, String> {
    use crate::diagnostics::{self, Audio, Inputs};
    let config = state.config();
    let audio = state.player().map(|player| {
        let playback = player.engine.state();
        let status = player.engine.audio_status();
        let device = match config.audio.output_device.as_str() {
            "" => "system default".to_string(),
            id if oxitide_audio::output::is_asio_device(id) => "an ASIO driver".to_string(),
            _ => "a selected device".to_string(),
        };
        let stream = playback.stream.as_ref().map(|s| {
            let codec = s.codec.as_deref().unwrap_or("unknown codec");
            let bitrate = s.bitrate_kbps.map(|kbps| format!(", {kbps} kbps")).unwrap_or_default();
            format!(
                "{codec} {:.1} kHz / {}-bit, {} ch{bitrate}",
                s.sample_rate as f64 / 1000.0,
                s.bit_depth,
                s.channels
            )
        });
        Audio {
            output: format!("{:?}", status.mode),
            device,
            bit_perfect: status.bit_perfect,
            notice: status.notice,
            quality: format!("{:?}", playback.preferred_quality),
            stream,
            dsp_stages: playback.dsp_stages.clone(),
            underruns: player.engine.underrun_count(),
        }
    });
    let inputs = Inputs {
        version: env!("CARGO_PKG_VERSION").to_string(),
        debug_build: cfg!(debug_assertions),
        os: diagnostics::os_description(),
        webview: tauri::webview_version().unwrap_or_else(|_| "unknown".into()),
        signed_in: audio.is_some(),
        audio,
        switches: vec![
            ("Exclusive mode", config.audio.exclusive),
            ("Media controls", config.integrations.smtc),
            ("Discord", config.integrations.discord),
            ("Last.fm", config.scrobbling.lastfm_enabled),
            ("ListenBrainz", config.scrobbling.listenbrainz_enabled),
            ("Phone remote", config.remote.enabled),
            ("History", config.history.enabled),
            ("Close to tray", config.ui.close_to_tray),
            ("Block explicit", config.content.block_explicit),
            ("Block AI", config.content.block_ai),
            ("Update check on start", config.updates.check_on_start),
        ],
        ui: format!("{:?}, {:?} theme", config.ui.mode, config.ui.theme),
        recent_problems: diagnostics::recent_problems_from_logs(),
    };
    Ok(diagnostics::report(&inputs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_tidal::token_store::MemoryStore;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Commands allowed to run on the main thread (Track 13, EF-D6). A plain
    /// `#[tauri::command] pub fn` blocks the window's message loop while it
    /// runs; these stay sync on purpose, because sync commands run in arrival
    /// order:
    /// - transport and queue edits (play, add, reorder, volume…) must apply
    ///   in the order the user made them;
    /// - settings do a read-modify-write of the whole config, so two
    ///   concurrent saves could drop one of them;
    /// - window/titlebar, auth-flow and perf hooks are trivial or main-thread
    ///   by nature.
    /// Anything that blocks (disk scans, device enumeration, CPU work, long
    /// waits) and does not depend on ordering belongs in `async` instead.
    const SYNC_COMMANDS: &[&str] = &[
        "auth_begin", "auth_take_result", "auth_status", "auth_logout", "get_keyring_status",
        "get_scrobble_status", "lastfm_cancel_auth", "lastfm_disconnect", "listenbrainz_disconnect",
        "set_scrobble_enabled", "open_external", "play_tracks", "play_pause", "next", "prev", "seek",
        "set_volume", "fade_and_pause", "set_history_enabled", "set_history_max_entries",
        "set_block_explicit", "set_block_ai", "set_show_labels", "set_shuffle", "set_repeat",
        // A config writer; turning it off removes queue items, ordered with
        // the user's own queue edits.
        "set_autoplay",
        "queue_reorder", "queue_remove", "queue_clear", "retry", "queue_add", "queue_play_next",
        "queue_insert", "restore_session", "set_play_source", "clear_session", "perf_mark", "perf_quit",
        "get_perf_snapshot", "perf_submit_ui_metrics", "perf_queue_build", "set_output_device",
        "set_preferred_quality", "set_exclusive", "set_exclusive_fallback", "set_dsp_settings",
        "get_config", "get_audio_status", "get_cache_status", "set_allow_fallback",
        "set_quality_display", "set_ui_mode", "set_theme", "set_instrument_tone", "set_typeface",
        "set_custom_typeface",
        "set_density", "set_cover_shape", "set_dynamic_color", "set_rack_settings",
        "set_level_meter", "set_queue_width", "set_mini_mode", "set_start_in_mini", "set_oneko",
        "set_catjam", "set_catjam_coat", "set_catjam_scale",
        // An atomic store plus one engine call; ordered with CatJam's mount
        // and unmount so a quick remount cannot end with tracking off.
        "set_beat_tracking",
        "set_hover_panels", "set_row_separators", "set_player_bar_style", "set_queue_style", "set_smtc", "set_discord", "set_close_to_tray", "set_cache_max_size_mb",
        "set_cache_clear_on_exit", "get_remote_status", "begin_remote_pairing",
        "cancel_remote_pairing", "list_remote_devices", "rename_remote_device",
        "revoke_remote_device", "resolve_remote_pair_request", "get_custom_titlebar",
        "set_snap_target", "get_update_support", "set_update_check_on_start", "get_logs_dir",
        "open_logs_folder",
    ];

    /// Names of the plain (main-thread) `#[tauri::command] pub fn`s in `source`.
    fn sync_commands(source: &str) -> Vec<String> {
        let lines: Vec<&str> = source.lines().map(str::trim).collect();
        let mut names = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if *line != "#[tauri::command]" {
                continue;
            }
            let next = lines[i + 1..]
                .iter()
                .find(|l| !l.starts_with("#[") && !l.starts_with("///") && !l.is_empty());
            if let Some(name) = next.and_then(|l| l.strip_prefix("pub fn ")) {
                names.push(name.split(['(', '<']).next().unwrap_or("").to_string());
            }
        }
        names
    }

    #[test]
    fn every_main_thread_command_is_deliberate() {
        let found = sync_commands(include_str!("commands.rs"));
        let unexpected: Vec<&String> =
            found.iter().filter(|n| !SYNC_COMMANDS.contains(&n.as_str())).collect();
        assert!(
            unexpected.is_empty(),
            "new main-thread commands {unexpected:?}: make them #[tauri::command(async)] or add them to SYNC_COMMANDS with a reason"
        );
        let stale: Vec<&&str> = SYNC_COMMANDS.iter().filter(|n| !found.iter().any(|f| f == *n)).collect();
        assert!(stale.is_empty(), "SYNC_COMMANDS lists commands that are no longer sync: {stale:?}");
    }

    #[test]
    fn sync_command_parsing_skips_async_and_attributes() {
        let source = "#[tauri::command]\npub fn a(x: u8) {}\n#[tauri::command(async)]\npub fn b() {}\n#[tauri::command]\npub async fn c() {}\n#[tauri::command]\n#[allow(dead_code)]\npub fn d<T>() {}\n";
        assert_eq!(sync_commands(source), vec!["a", "d"]);
    }

    #[test]
    fn replay_wav_has_a_canonical_pcm16_header() {
        let wav = wav_pcm16(&[0.0, 1.0, -1.0, 0.5], 2, 48_000);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2, "channels");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 48_000);
        assert_eq!(&wav[36..40], b"data");
        let data: Vec<i16> = wav[44..].chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        assert_eq!(data, vec![0, i16::MAX, -i16::MAX, 16_384]);
    }

    #[test]
    fn queue_item_from_track_carries_name_link_ids() {
        let track: Track = serde_json::from_value(serde_json::json!({
            "id": 5,
            "title": "Song",
            "duration": 200,
            "artists": [{ "id": 1, "name": "Crosby, Stills" }, { "id": 2, "name": "Nash" }],
            "album": { "id": 9, "title": "LP", "cover": "c" }
        }))
        .unwrap();
        let item = queue_item_from_track(&track);
        assert_eq!(item.artist, "Crosby, Stills, Nash");
        assert_eq!(
            item.artist_refs,
            vec![
                ArtistRef { id: 1, name: "Crosby, Stills".into() },
                ArtistRef { id: 2, name: "Nash".into() },
            ]
        );
        assert_eq!(item.album_id, Some(9));
    }

    #[test]
    fn queue_item_ids_are_optional_on_the_wire() {
        let item: QueueItem =
            serde_json::from_value(serde_json::json!({ "track_id": 1, "title": "T" })).unwrap();
        assert!(item.artist_refs.is_empty());
        assert_eq!(item.album_id, None);
        let value = serde_json::to_value(&item).unwrap();
        assert!(value.get("artist_refs").is_none() && value.get("album_id").is_none());
    }

    fn outcome(state: Option<&str>) -> auth::RedirectOutcome {
        auth::RedirectOutcome {
            code: "c".into(),
            state: state.map(str::to_string),
        }
    }

    fn pending(redirect_uri: &str, state: &str) -> PendingLogin {
        PendingLogin {
            pkce: Pkce::from_parts("v".into(), "k".into()),
            state: state.into(),
            redirect_uri: redirect_uri.into(),
        }
    }

    #[test]
    fn verify_login_state_accepts_a_match() {
        assert!(verify_login_state("s", &outcome(Some("s")), true).is_ok());
        assert!(verify_login_state("s", &outcome(Some("s")), false).is_ok());
    }

    #[test]
    fn verify_login_state_rejects_mismatch() {
        assert!(verify_login_state("s", &outcome(Some("other")), true).is_err());
        assert!(verify_login_state("s", &outcome(Some("other")), false).is_err());
    }

    #[test]
    fn verify_login_state_requires_presence_only_when_asked() {
        assert!(verify_login_state("s", &outcome(None), true).is_err());
        assert!(verify_login_state("s", &outcome(None), false).is_ok());
    }

    #[test]
    fn paste_redirect_is_accepted_by_the_shared_completion() {
        let login = pending(PKCE_REDIRECT_PASTE, "s");
        let out = parse_and_check(
            &login,
            "https://tidal.com/android/login/auth?code=XYZ&state=s",
        )
        .unwrap();
        assert_eq!(out.code, "XYZ");
    }

    #[test]
    fn deep_link_redirect_requires_the_tidal_scheme() {
        let login = pending(PKCE_REDIRECT_DEEP_LINK, "s");
        assert!(parse_and_check(
            &login,
            "https://tidal.com/android/login/auth?code=XYZ&state=s"
        )
        .is_err());
        let out = parse_and_check(&login, "tidal://login/auth?code=XYZ&state=s").unwrap();
        assert_eq!(out.code, "XYZ");
    }

    #[test]
    fn missing_state_is_required_for_deep_links_but_not_for_paste() {
        assert!(parse_and_check(
            &pending(PKCE_REDIRECT_DEEP_LINK, "s"),
            "tidal://login/auth?code=XYZ"
        )
        .is_err());
        assert!(parse_and_check(
            &pending(PKCE_REDIRECT_PASTE, "s"),
            "https://tidal.com/android/login/auth?code=XYZ"
        )
        .is_ok());
    }

    #[test]
    fn a_rejected_redirect_does_not_clear_the_pending_login() {
        let state = AppState::default();
        state.set_pending(pending(PKCE_REDIRECT_DEEP_LINK, "s"));
        assert!(take_and_validate(&state, "https://evil.example/cb?code=x").is_err());
        assert!(
            state.take_pending().is_some(),
            "pending login must survive a rejected redirect"
        );
    }

    #[test]
    fn a_valid_redirect_consumes_the_pending_login() {
        let state = AppState::default();
        state.set_pending(pending(PKCE_REDIRECT_DEEP_LINK, "s"));
        assert!(take_and_validate(&state, "tidal://login/auth?code=x&state=s").is_ok());
        assert!(state.take_pending().is_none());
    }

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
        let dto = LoginPrompt {
            url: "https://example.test/login".to_string(),
        };
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
        let dto = TrackPageDto {
            items: Vec::new(),
            total: 7,
            offset: 100,
            limit: 50,
        };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["total"], 7);
        assert_eq!(value["offset"], 100);
        assert_eq!(value["limit"], 50);
        assert!(value["items"].is_array());
    }

    #[test]
    fn history_page_dto_serializes_fields() {
        let dto = HistoryPageDto { items: Vec::new(), total: 3, offset: 1, limit: 100 };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["total"], 3);
        assert_eq!(value["offset"], 1);
        assert_eq!(value["limit"], 100);
        assert!(value["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn session_info_dto_serializes_fields() {
        let dto = SessionInfoDto {
            country_code: "GR".to_string(),
            user_id: 42,
        };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["country_code"], "GR");
        assert_eq!(value["user_id"], 42);
    }

    fn online(restored: Restored) -> (SessionInfo, TokenSet) {
        match restored {
            Restored::Online(session, tokens) => (session, tokens),
            other => panic!("expected an online restore, got {other:?}"),
        }
    }

    fn saved_session() -> SessionInfo {
        SessionInfo { session_id: "saved".into(), country_code: "GR".into(), user_id: 42 }
    }

    /// A base URL nothing listens on: every request fails to connect.
    fn unreachable_base() -> String {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        format!("http://127.0.0.1:{port}/")
    }

    #[tokio::test]
    async fn an_unreachable_tidal_restores_the_saved_session_offline() {
        let store = MemoryStore::default();
        store.save(&sample_tokens("good", Some("rt"))).unwrap();
        let base = unreachable_base();
        let restored = restore_tokens(
            &reqwest::Client::new(),
            &base,
            &format!("{base}token"),
            &store,
            Some(saved_session()),
        )
        .await
        .unwrap();
        match restored {
            Some(Restored::Offline(session, tokens)) => {
                assert_eq!(session.user_id, 42);
                assert_eq!(tokens.access_token, "good");
            }
            other => panic!("expected an offline restore, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_unreachable_tidal_without_a_saved_session_restores_nothing() {
        let store = MemoryStore::default();
        store.save(&sample_tokens("good", Some("rt"))).unwrap();
        let base = unreachable_base();
        let restored =
            restore_tokens(&reqwest::Client::new(), &base, &format!("{base}token"), &store, None)
                .await
                .unwrap();
        assert!(restored.is_none());
    }

    #[tokio::test]
    async fn a_revoked_login_never_starts_offline() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&server)
            .await;
        let store = MemoryStore::default();
        store.save(&sample_tokens("stale", Some("revoked"))).unwrap();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(
            &reqwest::Client::new(),
            &base,
            &format!("{}/token", server.uri()),
            &store,
            Some(saved_session()),
        )
        .await
        .unwrap();
        assert!(restored.is_none(), "a revoked login must lead to sign-in");
    }

    #[test]
    fn the_saved_session_round_trips() {
        let dir = std::env::temp_dir().join(format!("oxitide-saved-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tidal_session.json");
        save_session(Some(&path), &saved_session());
        assert_eq!(load_saved_session(Some(&path)), Some(saved_session()));
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(load_saved_session(Some(&path)), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn restore_tokens_returns_none_without_stored_token() {
        let server = MockServer::start().await;
        let store = MemoryStore::default();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store, None)
            .await
            .unwrap();
        assert!(restored.is_none());
    }

    #[tokio::test]
    async fn restore_tokens_uses_valid_stored_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sessions"))
            .and(header("authorization", "Bearer good"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "s", "countryCode": "GR", "userId": 7
            })))
            .mount(&server)
            .await;
        let store = MemoryStore::default();
        store.save(&sample_tokens("good", Some("rt"))).unwrap();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store, None)
            .await
            .unwrap()
            .map(online)
            .unwrap();
        assert_eq!(restored.0.user_id, 7);
        assert_eq!(restored.1.access_token, "good");
    }

    #[tokio::test]
    async fn restore_tokens_refreshes_on_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sessions"))
            .and(header("authorization", "Bearer stale"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/sessions"))
            .and(header("authorization", "Bearer fresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "s", "countryCode": "GR", "userId": 9
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "fresh", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let store = MemoryStore::default();
        store.save(&sample_tokens("stale", Some("old-rt"))).unwrap();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store, None)
            .await
            .unwrap()
            .map(online)
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
        Mock::given(method("GET"))
            .and(path("/sessions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let store = MemoryStore::default();
        store.save(&sample_tokens("stale", Some("bad-rt"))).unwrap();
        let http = reqwest::Client::new();
        let base = format!("{}/", server.uri());
        let restored = restore_tokens(&http, &base, &format!("{}/token", server.uri()), &store, None)
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

    #[test]
    fn playlist_dto_uses_camel_case_for_track_count() {
        let dto = oxitide_tidal::models::Playlist {
            uuid: "u".into(),
            title: "T".into(),
            description: None,
            number_of_tracks: 3,
            duration: 0,
            image: None,
            square_image: Some("sq".into()),
            created: Some("2026-01-05T00:00:00.000+0000".into()),
            last_item_added_at: None,
            last_updated: None,
            public_playlist: Some(true),
            number_of_followers: Some(4),
            creator: None,
            owned: true,
        };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["numberOfTracks"], serde_json::json!(3));
        assert_eq!(value["uuid"], serde_json::json!("u"));
        assert_eq!(value["squareImage"], serde_json::json!("sq"));
        assert_eq!(value["publicPlaylist"], serde_json::json!(true));
        assert_eq!(value["numberOfFollowers"], serde_json::json!(4));
        assert_eq!(
            value["created"],
            serde_json::json!("2026-01-05T00:00:00.000+0000")
        );
        assert_eq!(value["lastItemAddedAt"], serde_json::Value::Null);
        assert_eq!(value["lastUpdated"], serde_json::Value::Null);
    }

    #[test]
    fn perf_queue_build_requires_a_session() {
        let items = vec![QueueItem {
            track_id: 1,
            title: "T".into(),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 1,
            cover: None,
            artist_refs: Vec::new(),
            album_id: None,
            autoplay: false,
        }];
        let result = perf_queue_build_items(items, 0);
        assert!(result.is_err(), "the perf command must not run outside a session");
        assert_eq!(result.unwrap_err(), "no perf session");
    }

    fn tidal_client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo {
                session_id: "s".into(),
                country_code: "GR".into(),
                user_id: 1,
            },
            TokenSet {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                token_type: "Bearer".into(),
                expires_at: None,
            },
            Arc::new(MemoryStore::default()),
        )
    }

    #[test]
    fn track_flag_dto_serializes_camel_case_free_fields() {
        let dto = TrackFlagDto { track_id: 7, explicit: Some(true), ai: None };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json, serde_json::json!({"track_id": 7, "explicit": true, "ai": null}));
    }

    #[tokio::test]
    async fn queue_items_from_ids_preserves_order_and_maps_track_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1, "title": "One", "duration": 210,
                "artists": [{"id": 5, "name": "A"}, {"id": 6, "name": "B"}],
                "album": {"id": 9, "title": "Album", "cover": "c-1"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 2, "title": "Two", "duration": 90, "artists": []
            })))
            .mount(&server)
            .await;

        let items = queue_items_from_ids(&tidal_client(&server), &[2, 1], None)
            .await
            .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].track_id, 2);
        assert_eq!(items[0].album, "");
        assert_eq!(items[1].track_id, 1);
        assert_eq!(items[1].title, "One");
        assert_eq!(items[1].artist, "A, B");
        assert_eq!(items[1].album, "Album");
        assert_eq!(items[1].duration_secs, 210);
        assert_eq!(items[1].cover.as_deref(), Some("c-1"));
    }

    #[tokio::test]
    async fn queue_items_from_ids_fails_fast_on_a_mid_list_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1, "title": "One"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let err = queue_items_from_ids(&tidal_client(&server), &[1, 2], None)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Api { status: 500, .. }), "got {err:?}");
    }

    fn content(block_explicit: bool, block_ai: bool) -> crate::config::ContentConfig {
        crate::config::ContentConfig { block_explicit, block_ai, ..Default::default() }
    }

    fn v1_track(id: u64, explicit: bool) -> serde_json::Value {
        serde_json::json!({
            "id": id, "title": format!("T{id}"), "duration": 100,
            "artists": [{"id": 1, "name": "A"}], "explicit": explicit
        })
    }

    #[tokio::test]
    async fn resolve_playable_items_filters_explicit_without_enrichment() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, true)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(2, false)))
            .mount(&server)
            .await;

        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) = resolve_playable_items(
            &client,
            &[1, 2],
            Some(&store),
            &content(true, false),
        )
        .await
        .unwrap();

        assert_eq!(kept, vec![2]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].track_id, 2);
        assert_eq!(store.get(2).unwrap().explicit, Some(false));
    }

    #[tokio::test]
    async fn resolve_playable_items_does_not_enrich_when_filters_are_off() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, false)))
            .mount(&server)
            .await;
        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) =
            resolve_playable_items(&client, &[1], Some(&store), &content(false, false))
                .await
                .unwrap();
        assert_eq!(kept, vec![1]);
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn resolve_playable_items_filters_ai_after_enrichment() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, false)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(2, false)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "1", "type": "tracks", "attributes": {"explicit": false, "ai": true}},
                    {"id": "2", "type": "tracks", "attributes": {"explicit": false, "ai": false}}
                ]
            })))
            .mount(&server)
            .await;

        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) =
            resolve_playable_items(&client, &[1, 2], Some(&store), &content(false, true))
                .await
                .unwrap();

        assert_eq!(kept, vec![2]);
        assert_eq!(items[0].track_id, 2);
    }

    #[tokio::test]
    async fn resolve_playable_items_fails_open_when_enrichment_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, false)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) =
            resolve_playable_items(&client, &[1], Some(&store), &content(false, true))
                .await
                .unwrap();

        assert_eq!(kept, vec![1], "unknown ai must not block");
        assert_eq!(items.len(), 1);
    }

    fn page_track(id: u64, complete: bool) -> PageTrackDto {
        if complete {
            PageTrackDto {
                id,
                title: Some(format!("T{id}")),
                artists: Some(vec!["A".into()]),
                album: Some("B".into()),
                duration: Some(200),
                cover: Some("c".into()),
                explicit: Some(false),
            }
        } else {
            PageTrackDto {
                id,
                title: None,
                artists: None,
                album: None,
                duration: None,
                cover: None,
                explicit: None,
            }
        }
    }

    fn queue_row(id: u64) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 200,
            cover: None,
            artist_refs: Vec::new(),
            album_id: None,
            autoplay: false,
        }
    }

    #[tokio::test]
    async fn queue_items_from_page_items_skips_fetch_for_complete_metadata() {
        let server = MockServer::start().await;
        // No /tracks/{id} mock is mounted; any fetch would 404 and fail the call.
        let items =
            queue_items_from_page_items(&tidal_client(&server), &[page_track(1, true)], None)
                .await
                .unwrap();
        assert_eq!(items[0].track_id, 1);
        assert_eq!(items[0].title, "T1");
        assert_eq!(items[0].artist, "A");
        assert_eq!(items[0].album, "B");
        assert_eq!(items[0].duration_secs, 200);
        assert_eq!(items[0].cover.as_deref(), Some("c"));
    }

    #[tokio::test]
    async fn queue_items_from_page_items_fetches_only_incomplete_metadata() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 2, "title": "Fetched", "duration": 180,
                "artists": [{"id": 5, "name": "Artist"}],
                "album": {"id": 7, "title": "Album", "cover": "cover-2"}
            })))
            .mount(&server)
            .await;
        let items = queue_items_from_page_items(
            &tidal_client(&server),
            &[page_track(1, true), page_track(2, false)],
            None,
        )
        .await
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].track_id, 1);
        assert_eq!(items[1].track_id, 2);
        assert_eq!(items[1].title, "Fetched");
        assert_eq!(items[1].duration_secs, 180);
    }

    #[tokio::test]
    async fn queue_items_from_page_items_resolves_duplicate_incomplete_ids_once() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 2, "title": "Fetched", "duration": 180,
                "artists": [{"id": 5, "name": "Artist"}]
            })))
            .mount(&server)
            .await;
        let items = queue_items_from_page_items(
            &tidal_client(&server),
            &[page_track(2, false), page_track(2, false)],
            None,
        )
        .await
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].track_id, 2);
        assert_eq!(items[1].track_id, 2);
        assert_eq!(items[0].title, "Fetched");
        assert_eq!(items[1].title, "Fetched");
        let requests = server.received_requests().await.unwrap();
        let track_gets = requests
            .iter()
            .filter(|request| request.url.path() == "/tracks/2")
            .count();
        assert_eq!(track_gets, 1, "duplicate incomplete ids must cost one GET");
    }

    #[tokio::test]
    async fn queue_items_from_page_items_records_embedded_explicit() {
        let server = MockServer::start().await;
        let store = crate::track_flags::TrackFlagsStore::start(None);
        let mut track = page_track(1, true);
        track.explicit = Some(true);
        queue_items_from_page_items(&tidal_client(&server), &[track], Some(&store))
            .await
            .unwrap();
        let known = store.known(&[1]);
        assert_eq!(known[0].1.explicit, Some(true));
    }

    #[tokio::test]
    async fn filter_playable_items_blocks_and_reports_kept_ids() {
        let server = MockServer::start().await;
        let store = crate::track_flags::TrackFlagsStore::start(None);
        store.record_explicit(2, true);
        let (kept, ids) = filter_playable_items(
            &tidal_client(&server),
            &[1, 2],
            vec![queue_row(1), queue_row(2)],
            Some(&store),
            &content(true, false),
        )
        .await
        .unwrap();
        assert_eq!(ids, vec![1]);
        assert_eq!(kept.len(), 1);
    }

    #[tokio::test]
    async fn filter_playable_items_enriches_ai_flags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "1", "type": "tracks", "attributes": {"explicit": false, "ai": true}},
                    {"id": "2", "type": "tracks", "attributes": {"explicit": false, "ai": false}}
                ]
            })))
            .mount(&server)
            .await;
        let store = crate::track_flags::TrackFlagsStore::start(None);
        let (kept, ids) = filter_playable_items(
            &tidal_client(&server),
            &[1, 2],
            vec![queue_row(1), queue_row(2)],
            Some(&store),
            &content(false, true),
        )
        .await
        .unwrap();
        assert_eq!(ids, vec![2]);
        assert_eq!(kept[0].track_id, 2);
    }

    #[test]
    fn external_url_accepts_http_and_https() {
        assert_eq!(
            external_url("https://twitter.com/acdc").unwrap().as_str(),
            "https://twitter.com/acdc"
        );
        assert!(external_url("http://example.com/").is_ok());
    }

    #[test]
    fn external_url_rejects_unsafe_schemes_and_missing_hosts() {
        for raw in [
            "javascript:alert(1)",
            "file:///C:/secret",
            "data:text/html,x",
            "https://",
            "",
        ] {
            assert!(external_url(raw).is_err(), "{raw} should be rejected");
        }
    }
}
