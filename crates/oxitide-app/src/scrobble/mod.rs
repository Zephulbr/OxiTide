//! Scrobbling to Last.fm and ListenBrainz (Track 5 spec).
//!
//! `ScrobblerService` is fed raw playback events, turns them into now-playing
//! and scrobble intents with the pure `ScrobbleTracker`, and fans them out to
//! both services. All HTTP happens on a dedicated worker (Tasks 8–9).

pub mod credentials;
pub mod lastfm;
pub mod listenbrainz;
pub mod queue;
pub mod tracker;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxitide_audio::engine::EngineEvent;
use serde::Serialize;
use tokio::sync::mpsc;

use credentials::{CredentialStore, StoredCredential};
use lastfm::LastFmClient;
use listenbrainz::ListenBrainzClient;
use queue::{QueuedScrobble, ScrobbleQueue};
use tracker::{ScrobbleIntent, ScrobbleTracker};

pub const LASTFM_SERVICE: &str = "lastfm";
pub const LISTENBRAINZ_SERVICE: &str = "listenbrainz";

pub type StatusSink = Arc<dyn Fn(ScrobbleStatusDto) + Send + Sync>;
pub type AuthErrorSink = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScrobbleServiceStatus {
    pub id: String,
    pub enabled: bool,
    pub connected: bool,
    pub username: Option<String>,
    pub queued: usize,
    pub needs_reconnect: bool,
    pub last_error: Option<String>,
    pub configured: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScrobbleStatusDto {
    pub lastfm: ScrobbleServiceStatus,
    pub listenbrainz: ScrobbleServiceStatus,
}

#[derive(Default)]
struct Backoff {
    failures: u32,
    next_at: Option<Instant>,
}

impl Backoff {
    fn ready(&self) -> bool {
        self.next_at.map(|at| Instant::now() >= at).unwrap_or(true)
    }

    fn fail(&mut self) {
        self.failures = (self.failures + 1).min(7);
        let seconds = (5u64 << self.failures).min(300);
        self.next_at = Some(Instant::now() + Duration::from_secs(seconds));
    }

    fn reset(&mut self) {
        self.failures = 0;
        self.next_at = None;
    }
}

#[derive(Default)]
struct Inner {
    tracker: ScrobbleTracker,
    lastfm_enabled: bool,
    listenbrainz_enabled: bool,
    lastfm: Option<StoredCredential>,
    listenbrainz: Option<StoredCredential>,
    lastfm_queue: ScrobbleQueue,
    listenbrainz_queue: ScrobbleQueue,
    lastfm_error: Option<String>,
    listenbrainz_error: Option<String>,
    lastfm_needs_reconnect: bool,
    listenbrainz_needs_reconnect: bool,
    lastfm_backoff: Backoff,
    listenbrainz_backoff: Backoff,
    lastfm_pending_token: Option<String>,
}

pub fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

pub struct ScrobblerService {
    lastfm: LastFmClient,
    listenbrainz: ListenBrainzClient,
    store: Arc<dyn CredentialStore>,
    lastfm_configured: bool,
    tx: mpsc::UnboundedSender<ScrobbleIntent>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<ScrobbleIntent>>>,
    inner: Mutex<Inner>,
    on_status: Mutex<Option<StatusSink>>,
    on_auth_error: Mutex<Option<AuthErrorSink>>,
}

impl ScrobblerService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        http: reqwest::Client,
        lastfm_base: impl Into<String>,
        listenbrainz_base: impl Into<String>,
        store: Arc<dyn CredentialStore>,
        lastfm_api_key: String,
        lastfm_api_secret: String,
        lastfm_enabled: bool,
        listenbrainz_enabled: bool,
    ) -> Arc<Self> {
        let lastfm = LastFmClient::new(
            http.clone(),
            lastfm_base,
            lastfm_api_key.clone(),
            lastfm_api_secret.clone(),
        );
        let listenbrainz = ListenBrainzClient::new(http, listenbrainz_base);
        let lastfm_credential = store.load(LASTFM_SERVICE).ok().flatten();
        let listenbrainz_credential = store.load(LISTENBRAINZ_SERVICE).ok().flatten();
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            lastfm,
            listenbrainz,
            store,
            lastfm_configured: !lastfm_api_key.is_empty() && !lastfm_api_secret.is_empty(),
            tx,
            rx: Mutex::new(Some(rx)),
            inner: Mutex::new(Inner {
                lastfm_enabled,
                listenbrainz_enabled,
                lastfm: lastfm_credential,
                listenbrainz: listenbrainz_credential,
                ..Inner::default()
            }),
            on_status: Mutex::new(None),
            on_auth_error: Mutex::new(None),
        })
    }

    pub fn set_status_sink(&self, sink: StatusSink) {
        *self.on_status.lock().unwrap() = Some(sink);
    }

    pub fn set_auth_error_sink(&self, sink: AuthErrorSink) {
        *self.on_auth_error.lock().unwrap() = Some(sink);
    }

    fn emit_status(&self) {
        if let Some(sink) = self.on_status.lock().unwrap().as_ref() {
            sink(self.status());
        }
    }

    fn notify_auth_error(&self, service: &str) {
        if let Some(sink) = self.on_auth_error.lock().unwrap().as_ref() {
            sink(service.to_string());
        }
        self.emit_status();
    }

    pub fn status(&self) -> ScrobbleStatusDto {
        let inner = self.inner.lock().unwrap();
        ScrobbleStatusDto {
            lastfm: ScrobbleServiceStatus {
                id: LASTFM_SERVICE.to_string(),
                enabled: inner.lastfm_enabled,
                connected: inner.lastfm.is_some(),
                username: inner.lastfm.as_ref().map(|c| c.username.clone()),
                queued: inner.lastfm_queue.len(),
                needs_reconnect: inner.lastfm_needs_reconnect,
                last_error: inner.lastfm_error.clone(),
                configured: self.lastfm_configured,
            },
            listenbrainz: ScrobbleServiceStatus {
                id: LISTENBRAINZ_SERVICE.to_string(),
                enabled: inner.listenbrainz_enabled,
                connected: inner.listenbrainz.is_some(),
                username: inner.listenbrainz.as_ref().map(|c| c.username.clone()),
                queued: inner.listenbrainz_queue.len(),
                needs_reconnect: inner.listenbrainz_needs_reconnect,
                last_error: inner.listenbrainz_error.clone(),
                configured: true,
            },
        }
    }

    pub fn set_enabled(&self, service: &str, enabled: bool) {
        {
            let mut inner = self.inner.lock().unwrap();
            match service {
                LASTFM_SERVICE => inner.lastfm_enabled = enabled,
                LISTENBRAINZ_SERVICE => inner.listenbrainz_enabled = enabled,
                _ => return,
            }
        }
        self.emit_status();
    }

    pub fn on_player_event(&self, event: &EngineEvent) {
        let intents = {
            let mut inner = self.inner.lock().unwrap();
            inner.tracker.on_event(event, now_epoch())
        };
        for intent in intents {
            let _ = self.tx.send(intent);
        }
    }

    fn enabled_flags(&self) -> (bool, bool) {
        let inner = self.inner.lock().unwrap();
        (inner.lastfm_enabled, inner.listenbrainz_enabled)
    }

    fn lastfm_session(&self) -> Option<String> {
        self.inner.lock().unwrap().lastfm.as_ref().map(|c| c.secret.clone())
    }

    fn listenbrainz_token(&self) -> Option<String> {
        self.inner.lock().unwrap().listenbrainz.as_ref().map(|c| c.secret.clone())
    }

    pub async fn handle_intent(&self, intent: ScrobbleIntent) {
        match intent {
            ScrobbleIntent::NowPlaying(item) => {
                let (lastfm, listenbrainz) = self.enabled_flags();
                if lastfm {
                    if let Some(session) = self.lastfm_session() {
                        if let Err(error) = self.lastfm.now_playing(&session, &item).await {
                            tracing::debug!("last.fm now-playing failed: {error}");
                        }
                    }
                }
                if listenbrainz {
                    if let Some(token) = self.listenbrainz_token() {
                        let _ = self.listenbrainz.playing_now(&token, &item).await;
                    }
                }
            }
            ScrobbleIntent::Scrobble { item, started_at } => {
                let (lastfm, listenbrainz) = self.enabled_flags();
                let entry = QueuedScrobble { item, started_at };
                let mut changed = false;
                {
                    let mut inner = self.inner.lock().unwrap();
                    if lastfm {
                        inner.lastfm_queue.push(entry.clone());
                        changed = true;
                    }
                    if listenbrainz {
                        inner.listenbrainz_queue.push(entry);
                        changed = true;
                    }
                }
                if changed {
                    self.emit_status();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrobble::credentials::MemoryCredentialStore;
    use oxitide_audio::queue::QueueItem;

    fn service(store: Arc<dyn CredentialStore>) -> Arc<ScrobblerService> {
        ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            false,
            false,
        )
    }

    fn queue_item() -> QueueItem {
        QueueItem {
            track_id: 7,
            title: "Song".into(),
            artist: "Band".into(),
            album: "Record".into(),
            duration_secs: 200,
            cover: None,
        }
    }

    #[test]
    fn status_defaults_are_disconnected() {
        let service = service(Arc::new(MemoryCredentialStore::default()));
        let status = service.status();
        assert!(!status.lastfm.connected);
        assert!(!status.lastfm.enabled);
        assert!(status.lastfm.configured, "key+secret are present");
        assert_eq!(status.lastfm.queued, 0);
        assert!(!status.listenbrainz.connected);
    }

    #[test]
    fn missing_key_is_not_configured() {
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            Arc::new(MemoryCredentialStore::default()),
            String::new(),
            String::new(),
            false,
            false,
        );
        assert!(!service.status().lastfm.configured);
    }

    #[test]
    fn toggles_are_independent() {
        let service = service(Arc::new(MemoryCredentialStore::default()));
        service.set_enabled(LASTFM_SERVICE, true);
        assert!(service.status().lastfm.enabled);
        assert!(!service.status().listenbrainz.enabled);
        service.set_enabled(LISTENBRAINZ_SERVICE, true);
        assert!(service.status().listenbrainz.enabled);
    }

    #[tokio::test]
    async fn a_scrobble_intent_enqueues_for_enabled_services() {
        let service = service(Arc::new(MemoryCredentialStore::default()));
        service.set_enabled(LASTFM_SERVICE, true);
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        assert_eq!(service.status().lastfm.queued, 1);
        assert_eq!(service.status().listenbrainz.queued, 0, "disabled service must not enqueue");
    }
}
