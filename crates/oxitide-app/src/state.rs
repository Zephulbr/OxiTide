use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use oxitide_audio::cache::DiskCache;
use oxitide_audio::engine::Engine;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::Pkce;
use oxitide_tidal::token_store::{KeyringStatus, TokenStore};
use oxitide_tidal::Error;
use tauri::{AppHandle, Emitter};

use crate::artwork::ArtworkService;
use crate::commands::LoginOutcome;
use crate::config::Config;
use crate::lyrics::LyricsService;
use crate::scrobble::ScrobblerService;
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
    pub scrobbler: Mutex<Option<Arc<ScrobblerService>>>,
    /// Persists the playback session across restarts; `None` when the app has
    /// no writable data directory.
    pub session_saver: Mutex<Option<Arc<crate::session::SessionSaver>>>,
    /// Local playback history; `None` when the app has no writable data
    /// directory.
    pub history: Mutex<Option<Arc<crate::history::HistoryStore>>>,
    /// Content-blocking flag cache; `None` only before startup wiring.
    pub track_flags: Mutex<Option<Arc<crate::track_flags::TrackFlagsStore>>>,
    /// System tray handle; `None` when the tray could not be created.
    pub tray: Mutex<Option<tauri::tray::TrayIcon>>,
    /// The process-wide token store chosen at startup (keyring or session-only).
    pub token_store: RwLock<Option<Arc<dyn TokenStore>>>,
    /// Result of the startup keyring probe, exposed to the UI.
    pub keyring_status: Mutex<Option<KeyringStatus>>,
    pub media: Mutex<Option<crate::media_controls::MediaSession>>,
    pub discord: Mutex<Option<crate::discord::DiscordPresence>>,
    /// Hi-Fi Rack level meter emitter; idle unless the UI enables it.
    pub meter: crate::meter::MeterEmitter,
    /// CatJam is mounted: the engine analyzes beats (applied to every player).
    beat_tracking: AtomicBool,
    /// The phone remote server; `None` while disabled (Track 11).
    pub remote: Mutex<Option<Arc<crate::remote::RemoteService>>>,
    /// Why an enabled remote is not listening (bind failure).
    pub remote_error: Mutex<Option<String>>,
    /// Serializes remote start/stop.
    pub remote_lifecycle: tokio::sync::Mutex<()>,
    /// Paired phones (`remote.json`), loaded on first use.
    pub remote_devices: std::sync::OnceLock<Arc<crate::remote::pairing::DeviceStore>>,
    /// The release `check_for_update` found, kept for `install_update`.
    pub pending_update: Mutex<Option<tauri_plugin_updater::Update>>,
    /// Album release dates for track sorting, opened on first use.
    album_dates: std::sync::OnceLock<Arc<crate::album_dates::AlbumDatesStore>>,
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
        player.engine.set_beat_tracking(self.beat_tracking.load(Ordering::SeqCst));
        *self.player.write().unwrap() = Some(Arc::new(player));
    }

    /// Turn beat analysis on or off now and for any player built later.
    pub fn set_beat_tracking(&self, enabled: bool) {
        self.beat_tracking.store(enabled, Ordering::SeqCst);
        if let Some(player) = self.player() {
            player.engine.set_beat_tracking(enabled);
        }
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

    pub fn set_scrobbler(&self, service: Arc<ScrobblerService>) {
        *self.scrobbler.lock().unwrap() = Some(service);
    }

    pub fn scrobbler(&self) -> Option<Arc<ScrobblerService>> {
        self.scrobbler.lock().unwrap().clone()
    }

    pub fn set_session_saver(&self, saver: Arc<crate::session::SessionSaver>) {
        *self.session_saver.lock().unwrap() = Some(saver);
    }

    pub fn session_saver(&self) -> Option<Arc<crate::session::SessionSaver>> {
        self.session_saver.lock().unwrap().clone()
    }

    pub fn set_history(&self, store: Arc<crate::history::HistoryStore>) {
        *self.history.lock().unwrap() = Some(store);
    }

    pub fn history(&self) -> Option<Arc<crate::history::HistoryStore>> {
        self.history.lock().unwrap().clone()
    }

    pub fn set_track_flags(&self, store: Arc<crate::track_flags::TrackFlagsStore>) {
        *self.track_flags.lock().unwrap() = Some(store);
    }

    pub fn track_flags(&self) -> Option<Arc<crate::track_flags::TrackFlagsStore>> {
        self.track_flags.lock().unwrap().clone()
    }

    pub fn album_dates(&self) -> Arc<crate::album_dates::AlbumDatesStore> {
        self.album_dates
            .get_or_init(|| {
                Arc::new(crate::album_dates::AlbumDatesStore::start(
                    crate::album_dates::album_dates_path(),
                ))
            })
            .clone()
    }

    pub fn set_tray(&self, tray: tauri::tray::TrayIcon) {
        *self.tray.lock().unwrap() = Some(tray);
    }

    pub fn tray(&self) -> Option<tauri::tray::TrayIcon> {
        self.tray.lock().unwrap().clone()
    }

    pub fn set_token_store(&self, store: Arc<dyn TokenStore>) {
        *self.token_store.write().unwrap() = Some(store);
    }

    pub fn token_store(&self) -> Option<Arc<dyn TokenStore>> {
        self.token_store.read().unwrap().clone()
    }

    pub fn set_keyring_status(&self, status: KeyringStatus) {
        *self.keyring_status.lock().unwrap() = Some(status);
    }

    /// The startup probe result; persistence is assumed until main sets it.
    pub fn keyring_status(&self) -> KeyringStatus {
        self.keyring_status
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(KeyringStatus::persistent)
    }

    pub fn set_media(&self, session: crate::media_controls::MediaSession) {
        *self.media.lock().unwrap() = Some(session);
    }

    pub fn take_media(&self) -> Option<crate::media_controls::MediaSession> {
        self.media.lock().unwrap().take()
    }

    pub fn set_discord(&self, service: crate::discord::DiscordPresence) {
        *self.discord.lock().unwrap() = Some(service);
    }

    pub fn take_discord(&self) -> Option<crate::discord::DiscordPresence> {
        self.discord.lock().unwrap().take()
    }

    pub fn remote(&self) -> Option<Arc<crate::remote::RemoteService>> {
        self.remote.lock().unwrap().clone()
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
        crate::remote::host::session_changed(self, crate::remote::events::SessionKind::Expired);
    }

    /// A favorite changed on any surface: tell the desktop UI (`favorite_changed`)
    /// and connected phones, so a heart toggled on one shows on the other.
    pub fn favorite_changed(&self, kind: &'static str, id: u64, favorite: bool) {
        let change = crate::remote::events::FavoriteChange { kind, id, favorite };
        if let Some(handle) = self.handle.lock().unwrap().as_ref() {
            let _ = handle.emit(crate::remote::host::FAVORITE_CHANGED_EVENT, change);
        }
        if let Some(remote) = self.remote() {
            remote.favorite_changed(change);
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
        let _ = self.take_discord();
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
    fn scrobbler_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.scrobbler().is_none());
        let service = crate::scrobble::ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            std::sync::Arc::new(crate::scrobble::credentials::MemoryCredentialStore::default()),
            String::new(),
            String::new(),
            false,
            false,
        );
        state.set_scrobbler(service);
        assert!(state.scrobbler().is_some());
    }

    #[test]
    fn session_saver_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.session_saver().is_none());
        let dir = std::env::temp_dir().join(format!(
            "oxitide-state-saver-{}",
            std::process::id()
        ));
        let saver = Arc::new(crate::session::SessionSaver::start(
            dir.join("session.json"),
            std::time::Duration::from_secs(60),
        ));
        state.set_session_saver(saver);
        assert!(state.session_saver().is_some());
    }

    #[test]
    fn history_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.history().is_none());
        let dir = std::env::temp_dir().join(format!("oxitide-state-history-{}", std::process::id()));
        let store = Arc::new(crate::history::HistoryStore::start(
            dir.join("history.jsonl"),
            100,
            true,
        ));
        state.set_history(store);
        assert!(state.history().is_some());
    }

    #[test]
    fn track_flags_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.track_flags().is_none());
        let store = Arc::new(crate::track_flags::TrackFlagsStore::start(None));
        state.set_track_flags(store);
        assert!(state.track_flags().is_some());
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

    #[test]
    fn keyring_status_round_trips() {
        let state = AppState::default();
        assert!(state.keyring_status().persistent);
        state.set_keyring_status(KeyringStatus::unavailable("no secret service"));
        let status = state.keyring_status();
        assert!(!status.persistent);
        assert_eq!(status.reason.as_deref(), Some("no secret service"));
    }

    #[test]
    fn selected_token_store_is_returned_by_later_reads() {
        use oxitide_tidal::token_store::{MemoryStore, TokenSet};
        let state = AppState::default();
        assert!(state.token_store().is_none());
        let store: Arc<dyn TokenStore> = Arc::new(MemoryStore::default());
        let tokens = TokenSet {
            access_token: "a".into(),
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_at: None,
        };
        store.save(&tokens).unwrap();
        state.set_token_store(store);
        let loaded = state.token_store().unwrap().load().unwrap().unwrap();
        assert_eq!(loaded.access_token, "a");
    }
}
