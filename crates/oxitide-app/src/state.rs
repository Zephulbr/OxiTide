use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use oxitide_audio::cache::DiskCache;
use oxitide_audio::engine::Engine;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::Pkce;
use oxitide_tidal::Error;
use tauri::{AppHandle, Emitter};

use crate::artwork::ArtworkService;
use crate::commands::LoginOutcome;
use crate::config::Config;
use crate::lyrics::LyricsService;
use crate::window::FullGeometry;

pub struct Player {
    pub engine: Engine,
}

/// A login that has started and is waiting for the browser redirect.
pub struct PendingLogin {
    pub pkce: Pkce,
    pub state: String,
    pub redirect_uri: String,
}

#[derive(Default)]
pub struct AppState {
    pub pending_login: Mutex<Option<PendingLogin>>,
    pub last_login: Mutex<Option<LoginOutcome>>,
    pub client: RwLock<Option<TidalClient>>,
    pub player: RwLock<Option<Arc<Player>>>,
    pub config: Mutex<Config>,
    /// The full-window geometry stashed while the mini player is active.
    pub pre_mini: Mutex<Option<FullGeometry>>,
    pub cache: Mutex<Option<Arc<DiskCache>>>,
    pub artwork: Mutex<Option<Arc<ArtworkService>>>,
    pub lyrics: Mutex<Option<Arc<LyricsService>>>,
    pub media: Mutex<Option<crate::media_controls::MediaSession>>,
    session_expired: AtomicBool,
    handle: Mutex<Option<AppHandle>>,
}

impl AppState {
    pub fn set_pending(&self, pending: PendingLogin) {
        *self.pending_login.lock().unwrap() = Some(pending);
    }

    pub fn take_pending(&self) -> Option<PendingLogin> {
        self.pending_login.lock().unwrap().take()
    }

    pub fn set_last_login(&self, outcome: LoginOutcome) {
        *self.last_login.lock().unwrap() = Some(outcome);
    }

    pub fn clear_last_login(&self) {
        *self.last_login.lock().unwrap() = None;
    }

    pub fn take_last_login(&self) -> Option<LoginOutcome> {
        self.last_login.lock().unwrap().take()
    }

    pub fn is_logged_in(&self) -> bool {
        self.client.read().unwrap().is_some()
    }

    pub fn set_client(&self, client: TidalClient) {
        *self.client.write().unwrap() = Some(client);
    }

    pub fn set_player(&self, player: Player) {
        *self.player.write().unwrap() = Some(Arc::new(player));
    }

    pub fn player(&self) -> Option<Arc<Player>> {
        self.player.read().unwrap().clone()
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    pub fn set_config(&self, config: Config) {
        *self.config.lock().unwrap() = config;
    }

    pub fn pre_mini(&self) -> Option<FullGeometry> {
        *self.pre_mini.lock().unwrap()
    }

    pub fn set_pre_mini(&self, geometry: FullGeometry) {
        *self.pre_mini.lock().unwrap() = Some(geometry);
    }

    pub fn take_pre_mini(&self) -> Option<FullGeometry> {
        self.pre_mini.lock().unwrap().take()
    }

    pub fn set_cache(&self, cache: Arc<DiskCache>) {
        *self.cache.lock().unwrap() = Some(cache);
    }

    pub fn cache(&self) -> Option<Arc<DiskCache>> {
        self.cache.lock().unwrap().clone()
    }

    pub fn set_artwork(&self, artwork: ArtworkService) {
        *self.artwork.lock().unwrap() = Some(Arc::new(artwork));
    }

    pub fn artwork(&self) -> Option<Arc<ArtworkService>> {
        self.artwork.lock().unwrap().clone()
    }

    pub fn set_lyrics(&self, service: LyricsService) {
        *self.lyrics.lock().unwrap() = Some(Arc::new(service));
    }

    pub fn lyrics(&self) -> Option<Arc<LyricsService>> {
        self.lyrics.lock().unwrap().clone()
    }

    pub fn set_media(&self, session: crate::media_controls::MediaSession) {
        *self.media.lock().unwrap() = Some(session);
    }

    pub fn take_media(&self) -> Option<crate::media_controls::MediaSession> {
        self.media.lock().unwrap().take()
    }

    /// Remember the app handle so `notify_session_expired` can reach the UI.
    pub fn set_handle(&self, handle: AppHandle) {
        *self.handle.lock().unwrap() = Some(handle);
    }

    #[allow(dead_code)]
    pub fn session_expired(&self) -> bool {
        self.session_expired.load(Ordering::SeqCst)
    }

    /// Flag the session as expired and tell the UI to show the login screen.
    /// Idempotent: the first signal wins.
    pub fn notify_session_expired(&self) {
        if self.session_expired.swap(true, Ordering::SeqCst) {
            return;
        }
        tracing::warn!("TIDAL session expired; waiting for a new login");
        if let Some(handle) = self.handle.lock().unwrap().as_ref() {
            let _ = handle.emit("session_expired", ());
        }
    }

    pub fn clear_session_expired(&self) {
        self.session_expired.store(false, Ordering::SeqCst);
    }

    /// Map a TIDAL client error to a command error string, signalling the UI
    /// when the session is no longer valid.
    pub fn fail_tidal(&self, error: Error) -> String {
        if matches!(error, Error::Unauthorized) {
            self.notify_session_expired();
        }
        error.to_string()
    }

    pub fn logout(&self) {
        let _ = self.take_media();
        *self.client.write().unwrap() = None;
        *self.player.write().unwrap() = None;
        self.clear_session_expired();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_login_is_taken_once() {
        let state = AppState::default();
        state.set_pending(PendingLogin {
            pkce: Pkce::from_parts("v".into(), "k".into()),
            state: "s".into(),
            redirect_uri: "tidal://login/auth".into(),
        });
        assert!(state.take_pending().is_some());
        assert!(state.take_pending().is_none());
    }

    #[test]
    fn last_login_is_taken_once() {
        let state = AppState::default();
        state.set_last_login(LoginOutcome::Error("x".into()));
        assert!(state.take_last_login().is_some());
        assert!(state.take_last_login().is_none());
    }

    #[test]
    fn logged_in_reflects_client_presence() {
        let state = AppState::default();
        assert!(!state.is_logged_in());
        state.logout();
        assert!(!state.is_logged_in());
    }

    #[test]
    fn fail_tidal_signals_only_unauthorized() {
        let state = AppState::default();
        assert_eq!(state.fail_tidal(Error::Unauthorized), "unauthorized");
        assert!(state.session_expired());

        let other = AppState::default();
        assert_eq!(other.fail_tidal(Error::RateLimited), "rate limited");
        assert!(!other.session_expired());
    }

    #[test]
    fn notify_session_expired_is_idempotent_without_a_handle() {
        let state = AppState::default();
        state.notify_session_expired();
        state.notify_session_expired();
        assert!(state.session_expired());
    }

    #[test]
    fn logout_clears_the_expired_flag() {
        let state = AppState::default();
        state.notify_session_expired();
        state.logout();
        assert!(!state.session_expired());
    }

    #[test]
    fn pre_mini_stash_is_set_and_taken_once() {
        let state = AppState::default();
        assert_eq!(state.pre_mini(), None);
        let geometry = crate::window::FullGeometry {
            width: 1200,
            height: 800,
            x: Some(1),
            y: Some(2),
            maximized: false,
        };
        state.set_pre_mini(geometry);
        assert_eq!(state.pre_mini(), Some(geometry));
        assert_eq!(state.take_pre_mini(), Some(geometry));
        assert_eq!(state.pre_mini(), None);
    }
}
