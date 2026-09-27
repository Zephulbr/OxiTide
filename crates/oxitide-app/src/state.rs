use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use oxitide_audio::cache::DiskCache;
use oxitide_audio::engine::Engine;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::Pkce;
use oxitide_tidal::Error;
use tauri::{AppHandle, Emitter};

use crate::artwork::ArtworkService;
use crate::config::Config;

pub struct Player {
    pub engine: Engine,
}

#[derive(Default)]
pub struct AppState {
    pub pending_pkce: Mutex<Option<Pkce>>,
    pub client: RwLock<Option<TidalClient>>,
    pub player: RwLock<Option<Arc<Player>>>,
    pub config: Mutex<Config>,
    pub cache: Mutex<Option<Arc<DiskCache>>>,
    pub artwork: Mutex<Option<Arc<ArtworkService>>>,
    session_expired: AtomicBool,
    handle: Mutex<Option<AppHandle>>,
}

impl AppState {
    pub fn set_pending(&self, pkce: Pkce) {
        *self.pending_pkce.lock().unwrap() = Some(pkce);
    }

    pub fn take_pending(&self) -> Option<Pkce> {
        self.pending_pkce.lock().unwrap().take()
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
        *self.client.write().unwrap() = None;
        *self.player.write().unwrap() = None;
        self.clear_session_expired();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_pkce_is_taken_once() {
        let state = AppState::default();
        state.set_pending(Pkce::from_parts("v".into(), "k".into()));
        assert!(state.take_pending().is_some());
        assert!(state.take_pending().is_none());
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
}
