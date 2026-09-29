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
use lastfm::{LastFmClient, LastFmError};
use listenbrainz::{ListenBrainzClient, ListenBrainzError};
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

    fn delay_secs(failures: u32) -> u64 {
        (5u64 << failures.saturating_sub(1)).min(300)
    }

    /// Record a failure. When the server supplied a `Retry-After` value, seed
    /// the delay from it (clamped to 1–300 s so a `0` cannot retry immediately);
    /// otherwise use the 5 s→300 s schedule (spec §8).
    fn fail_with(&mut self, retry_after_secs: Option<u64>) {
        self.failures = (self.failures + 1).min(7);
        let seconds = match retry_after_secs {
            Some(secs) => secs.clamp(1, 300),
            None => Self::delay_secs(self.failures),
        };
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
                let (lastfm_reconnect, listenbrainz_reconnect) = {
                    let inner = self.inner.lock().unwrap();
                    (inner.lastfm_needs_reconnect, inner.listenbrainz_needs_reconnect)
                };
                if lastfm && !lastfm_reconnect {
                    if let Some(session) = self.lastfm_session() {
                        if let Err(error) = self.lastfm.now_playing(&session, &item).await {
                            match error {
                                LastFmError::Auth | LastFmError::NotAuthorized => {
                                    self.lastfm_auth_failed("last.fm needs reconnecting")
                                }
                                other => tracing::debug!("last.fm now-playing failed: {other}"),
                            }
                        }
                    }
                }
                if listenbrainz && !listenbrainz_reconnect {
                    if let Some(token) = self.listenbrainz_token() {
                        if let Err(error) = self.listenbrainz.playing_now(&token, &item).await {
                            if error == ListenBrainzError::Auth {
                                self.listenbrainz_auth_failed("listenbrainz needs reconnecting");
                            }
                        }
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

    pub fn start(self: &Arc<Self>) {
        let Some(mut rx) = self.rx.lock().unwrap().take() else {
            return;
        };
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    intent = rx.recv() => {
                        match intent {
                            Some(intent) => this.handle_intent(intent).await,
                            None => break,
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                }
                this.flush_queues().await;
            }
        });
    }

    fn lastfm_retryable(&self, message: &str, retry_after_secs: Option<u64>) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.lastfm_backoff.fail_with(retry_after_secs);
            inner.lastfm_error = Some(message.to_string());
        }
        self.emit_status();
    }

    fn lastfm_auth_failed(&self, message: &str) {
        let newly = {
            let mut inner = self.inner.lock().unwrap();
            let newly = !inner.lastfm_needs_reconnect;
            inner.lastfm_needs_reconnect = true;
            inner.lastfm_error = Some(message.to_string());
            newly
        };
        if newly {
            self.notify_auth_error(LASTFM_SERVICE);
        } else {
            self.emit_status();
        }
    }

    fn listenbrainz_retryable(&self, message: &str, retry_after_secs: Option<u64>) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.listenbrainz_backoff.fail_with(retry_after_secs);
            inner.listenbrainz_error = Some(message.to_string());
        }
        self.emit_status();
    }

    fn listenbrainz_auth_failed(&self, message: &str) {
        let newly = {
            let mut inner = self.inner.lock().unwrap();
            let newly = !inner.listenbrainz_needs_reconnect;
            inner.listenbrainz_needs_reconnect = true;
            inner.listenbrainz_error = Some(message.to_string());
            newly
        };
        if newly {
            self.notify_auth_error(LISTENBRAINZ_SERVICE);
        } else {
            self.emit_status();
        }
    }

    pub async fn flush_queues(&self) {
        self.flush_lastfm().await;
        self.flush_listenbrainz().await;
    }

    async fn flush_lastfm(&self) {
        let (session, batch) = {
            let inner = self.inner.lock().unwrap();
            if !inner.lastfm_enabled
                || inner.lastfm_needs_reconnect
                || inner.lastfm.is_none()
                || inner.lastfm_queue.is_empty()
                || !inner.lastfm_backoff.ready()
            {
                return;
            }
            let session = inner.lastfm.as_ref().map(|c| c.secret.clone()).unwrap();
            (session, inner.lastfm_queue.front_batch(50))
        };
        if batch.is_empty() {
            return;
        }
        let plays: Vec<(oxitide_audio::queue::QueueItem, i64)> =
            batch.iter().map(|entry| (entry.item.clone(), entry.started_at)).collect();
        match self.lastfm.scrobble(&session, &plays).await {
            Ok(()) => {
                {
                    let mut inner = self.inner.lock().unwrap();
                    inner.lastfm_queue.remove_front(batch.len());
                    inner.lastfm_backoff.reset();
                    inner.lastfm_error = None;
                }
                self.emit_status();
            }
            Err(LastFmError::Retryable) => {
                self.lastfm_retryable("last.fm is temporarily unavailable", None)
            }
            Err(LastFmError::RateLimited { retry_after_secs }) => self
                .lastfm_retryable("last.fm rate limited the request", retry_after_secs),
            Err(LastFmError::Auth) | Err(LastFmError::NotAuthorized) => {
                self.lastfm_auth_failed("last.fm needs reconnecting")
            }
            // `Permanent` now only comes from authoritative JSON error codes that
            // apply to the whole request, so dropping the batch is correct.
            // Item-level issues come back as HTTP 200 with per-item `ignored` flags.
            Err(LastFmError::Permanent) => {
                tracing::warn!("last.fm rejected {} scrobble(s); dropping", batch.len());
                {
                    let mut inner = self.inner.lock().unwrap();
                    inner.lastfm_queue.remove_front(batch.len());
                }
                self.emit_status();
            }
        }
    }

    async fn flush_listenbrainz(&self) {
        let token = {
            let inner = self.inner.lock().unwrap();
            if !inner.listenbrainz_enabled
                || inner.listenbrainz_needs_reconnect
                || inner.listenbrainz.is_none()
                || inner.listenbrainz_queue.is_empty()
                || !inner.listenbrainz_backoff.ready()
            {
                return;
            }
            inner.listenbrainz.as_ref().map(|c| c.secret.clone()).unwrap()
        };
        let mut sent = 0;
        // Cap each flush at 10 submissions and pace them at ~1 req/s (spec §4);
        // a large backlog drains across successive 30 s ticks. This bounds the
        // worker's worst-case block to ~10 s while staying polite to ListenBrainz.
        while sent < 10 {
            if sent > 0 {
                tokio::time::sleep(Duration::from_millis(1050)).await;
            }
            let entry = {
                let inner = self.inner.lock().unwrap();
                inner.listenbrainz_queue.front_batch(1).into_iter().next()
            };
            let Some(entry) = entry else { break };
            match self
                .listenbrainz
                .submit_listen(&token, &entry.item, entry.started_at)
                .await
            {
                Ok(()) => {
                    let mut inner = self.inner.lock().unwrap();
                    inner.listenbrainz_queue.remove_front(1);
                    inner.listenbrainz_backoff.reset();
                    inner.listenbrainz_error = None;
                    sent += 1;
                }
                Err(ListenBrainzError::Retryable) => {
                    self.listenbrainz_retryable(
                        "listenbrainz is temporarily unavailable",
                        None,
                    );
                    break;
                }
                Err(ListenBrainzError::RateLimited { retry_after_secs }) => {
                    self.listenbrainz_retryable(
                        "listenbrainz rate limited the request",
                        retry_after_secs,
                    );
                    break;
                }
                Err(ListenBrainzError::Auth) => {
                    self.listenbrainz_auth_failed("listenbrainz needs reconnecting");
                    break;
                }
                Err(ListenBrainzError::Permanent) => {
                    tracing::warn!("listenbrainz rejected a scrobble; dropping it");
                    let mut inner = self.inner.lock().unwrap();
                    inner.listenbrainz_queue.remove_front(1);
                    sent += 1;
                }
            }
        }
        if sent > 0 {
            self.emit_status();
        }
    }

    pub async fn lastfm_begin_auth(&self) -> Result<String, String> {
        if !self.lastfm_configured {
            return Err("Last.fm API key not configured".to_string());
        }
        let token = self.lastfm.get_token().await.map_err(|e| e.to_string())?;
        self.inner.lock().unwrap().lastfm_pending_token = Some(token.clone());
        Ok(self.lastfm.auth_url(&token))
    }

    pub async fn lastfm_complete_auth(&self) -> Result<ScrobbleStatusDto, String> {
        let token = self
            .inner
            .lock()
            .unwrap()
            .lastfm_pending_token
            .clone()
            .ok_or_else(|| "no authorization in progress".to_string())?;
        let session = match self.lastfm.get_session(&token).await {
            Ok(session) => session,
            Err(LastFmError::NotAuthorized) => {
                return Err("authorization is not complete yet".to_string())
            }
            Err(error) => return Err(error.to_string()),
        };
        let credential =
            StoredCredential { username: session.username, secret: session.key };
        self.store.save(LASTFM_SERVICE, &credential)?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.lastfm = Some(credential);
            inner.lastfm_pending_token = None;
            inner.lastfm_enabled = true;
            inner.lastfm_needs_reconnect = false;
            inner.lastfm_error = None;
            inner.lastfm_backoff.reset();
        }
        self.emit_status();
        Ok(self.status())
    }

    pub fn lastfm_cancel_auth(&self) {
        self.inner.lock().unwrap().lastfm_pending_token = None;
    }

    pub fn lastfm_disconnect(&self) -> Result<(), String> {
        self.store.clear(LASTFM_SERVICE)?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.lastfm = None;
            inner.lastfm_enabled = false;
            inner.lastfm_queue.clear();
            inner.lastfm_error = None;
            inner.lastfm_needs_reconnect = false;
            inner.lastfm_pending_token = None;
            inner.lastfm_backoff.reset();
        }
        self.emit_status();
        Ok(())
    }

    pub async fn listenbrainz_connect(
        &self,
        token: String,
    ) -> Result<ScrobbleStatusDto, String> {
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err("token is empty".to_string());
        }
        let username = self.listenbrainz.validate(&token).await.map_err(|e| e.to_string())?;
        let credential = StoredCredential { username, secret: token };
        self.store.save(LISTENBRAINZ_SERVICE, &credential)?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.listenbrainz = Some(credential);
            inner.listenbrainz_enabled = true;
            inner.listenbrainz_needs_reconnect = false;
            inner.listenbrainz_error = None;
            inner.listenbrainz_backoff.reset();
        }
        self.emit_status();
        Ok(self.status())
    }

    pub fn listenbrainz_disconnect(&self) -> Result<(), String> {
        self.store.clear(LISTENBRAINZ_SERVICE)?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.listenbrainz = None;
            inner.listenbrainz_enabled = false;
            inner.listenbrainz_queue.clear();
            inner.listenbrainz_error = None;
            inner.listenbrainz_needs_reconnect = false;
            inner.listenbrainz_backoff.reset();
        }
        self.emit_status();
        Ok(())
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

    #[tokio::test]
    async fn retryable_failure_keeps_the_scrobble_queued() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 16 })),
            )
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        let status = service.status();
        assert_eq!(status.lastfm.queued, 1, "16 must stay queued");
        assert!(!status.lastfm.needs_reconnect);
        assert!(status.lastfm.last_error.is_some());
    }

    #[tokio::test]
    async fn a_429_keeps_the_scrobble_queued_with_a_rate_limit_error() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "42"))
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        let status = service.status();
        assert_eq!(status.lastfm.queued, 1, "429 must stay queued");
        assert!(!status.lastfm.needs_reconnect);
        assert_eq!(status.lastfm.last_error.as_deref(), Some("last.fm rate limited the request"));
    }

    #[tokio::test]
    async fn a_successful_flush_drains_the_queue() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "scrobbles": {} })),
            )
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        assert_eq!(service.status().lastfm.queued, 0);
    }

    #[tokio::test]
    async fn an_invalid_session_marks_needs_reconnect_and_notifies_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 9 })))
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        service.set_auth_error_sink(Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        service.flush_queues().await;
        let status = service.status();
        assert!(status.lastfm.needs_reconnect);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "toast must fire once");
        assert!(!status.listenbrainz.needs_reconnect, "other service untouched");
    }

    #[tokio::test]
    async fn a_now_playing_auth_error_marks_needs_reconnect_and_stops_requests() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "error": 9 })))
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        service.set_auth_error_sink(Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        service.handle_intent(ScrobbleIntent::NowPlaying(queue_item())).await;
        assert!(service.status().lastfm.needs_reconnect);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "auth toast must fire once");

        service.handle_intent(ScrobbleIntent::NowPlaying(queue_item())).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no second toast");
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            1,
            "the reconnect flag must suppress further now-playing requests"
        );
    }

    #[test]
    fn backoff_delay_starts_at_five_seconds_and_caps_at_five_minutes() {
        let schedule: Vec<u64> = (1..=8).map(Backoff::delay_secs).collect();
        assert_eq!(schedule, vec![5, 10, 20, 40, 80, 160, 300, 300]);
    }

    #[test]
    fn retry_after_is_clamped_to_one_and_three_hundred_seconds() {
        let mut zero = Backoff::default();
        zero.fail_with(Some(0));
        assert!(!zero.ready(), "a zero Retry-After must not retry immediately");

        let mut huge = Backoff::default();
        huge.fail_with(Some(u64::MAX));
        assert!(huge.next_at.unwrap() <= Instant::now() + Duration::from_secs(300));
    }

    #[tokio::test]
    async fn lastfm_complete_auth_saves_the_session() {
        use wiremock::matchers::{method as http_method, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(query_param("method", "auth.getToken"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "token": "tok" })),
            )
            .mount(&server)
            .await;
        Mock::given(http_method("GET"))
            .and(query_param("method", "auth.getSession"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "session": { "name": "v", "key": "sk" } }),
            ))
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store.clone(),
            "key".into(),
            "secret".into(),
            false,
            false,
        );
        let url = service.lastfm_begin_auth().await.unwrap();
        assert!(url.contains("api_key=key"), "{url}");
        assert!(url.contains("token=tok"), "{url}");
        let status = service.lastfm_complete_auth().await.unwrap();
        assert!(status.lastfm.connected);
        assert!(status.lastfm.enabled, "connecting enables the service");
        assert_eq!(status.lastfm.username.as_deref(), Some("v"));
        assert_eq!(store.load(LASTFM_SERVICE).unwrap().unwrap().secret, "sk");
    }

    #[tokio::test]
    async fn lastfm_disconnect_clears_the_credential() {
        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = service(store.clone());
        service.lastfm_disconnect().unwrap();
        assert!(!service.status().lastfm.connected);
        assert!(store.load(LASTFM_SERVICE).unwrap().is_none());
    }

    struct FailingClearStore;

    impl CredentialStore for FailingClearStore {
        fn save(&self, _service: &str, _credential: &StoredCredential) -> Result<(), String> {
            Ok(())
        }

        fn load(&self, _service: &str) -> Result<Option<StoredCredential>, String> {
            Ok(Some(StoredCredential { username: "v".into(), secret: "sk".into() }))
        }

        fn clear(&self, _service: &str) -> Result<(), String> {
            Err("keyring delete failed".to_string())
        }
    }

    #[test]
    fn disconnect_propagates_a_keyring_clear_failure_and_stays_connected() {
        let service = service(Arc::new(FailingClearStore));
        assert!(service.status().lastfm.connected, "fixture starts connected");
        assert!(service.lastfm_disconnect().is_err());
        assert!(service.status().lastfm.connected, "state unchanged on clear failure");
    }

    #[tokio::test]
    async fn listenbrainz_connect_validates_and_stores() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .and(path("/validate-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "valid": true, "user_name": "v" })),
            )
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            server.uri(),
            store.clone(),
            "key".into(),
            "secret".into(),
            false,
            false,
        );
        let status = service.listenbrainz_connect("tok".into()).await.unwrap();
        assert!(status.listenbrainz.connected);
        assert!(status.listenbrainz.enabled);
        assert_eq!(status.listenbrainz.username.as_deref(), Some("v"));
        assert_eq!(store.load(LISTENBRAINZ_SERVICE).unwrap().unwrap().secret, "tok");
    }

    #[tokio::test]
    async fn listenbrainz_connect_rejects_an_invalid_token() {
        use wiremock::matchers::method as http_method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let service = ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            server.uri(),
            Arc::new(MemoryCredentialStore::default()),
            "key".into(),
            "secret".into(),
            false,
            false,
        );
        assert!(service.listenbrainz_connect("bad".into()).await.is_err());
        assert!(!service.status().listenbrainz.connected);
    }
}
