//! The remote event hub (spec §6.4, RM-D8). One engine subscriber converts
//! events into a slim state (never the queue) and fans them out on a
//! `broadcast` channel to SSE clients. With no client connected it returns
//! before doing any work, so the engine thread never pays for the remote.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::Stream;
use oxitide_audio::engine::{EngineEvent, PlaybackState};
use oxitide_audio::queue::{QueueItem, RepeatMode};
use serde::Serialize;
use tokio::sync::{broadcast, watch};

use super::RemoteSession;

/// State events are coalesced to at most 10 per second.
pub const STATE_INTERVAL: Duration = Duration::from_millis(100);
/// Position events are sent at most once per second.
pub const POSITION_INTERVAL: Duration = Duration::from_secs(1);
/// A client further behind than this is dropped and resyncs on reconnect.
const CHANNEL_CAPACITY: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTrack {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: Option<String>,
    pub duration_secs: u32,
    pub explicit: Option<bool>,
    pub audio_quality: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStream {
    pub codec: Option<String>,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub bitrate_kbps: Option<u32>,
}

/// The phone's view of playback: everything Now Playing needs, never the
/// queue itself (`queueLen` + `queueRev` stand in for it).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteState {
    pub logged_in: bool,
    pub playing: bool,
    pub buffering: bool,
    pub current: Option<RemoteTrack>,
    pub current_index: Option<usize>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub queue_len: usize,
    pub queue_rev: u64,
    pub stream: Option<RemoteStream>,
    pub bit_perfect: bool,
    pub dsp_stages: Vec<String>,
}

impl RemoteState {
    /// What a phone sees while the desktop is signed out (or has no player).
    pub fn idle(logged_in: bool) -> Self {
        Self {
            logged_in,
            playing: false,
            buffering: false,
            current: None,
            current_index: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            queue_len: 0,
            queue_rev: 0,
            stream: None,
            bit_perfect: false,
            dsp_stages: Vec::new(),
        }
    }
}

fn quality_name(state: &PlaybackState) -> Option<String> {
    let quality = state.stream.as_ref()?.quality?;
    serde_json::to_value(quality).ok()?.as_str().map(str::to_string)
}

fn track(item: &QueueItem, explicit: Option<bool>, audio_quality: Option<String>) -> RemoteTrack {
    RemoteTrack {
        track_id: item.track_id,
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        cover: item.cover.clone(),
        duration_secs: item.duration_secs,
        explicit,
        audio_quality,
    }
}

/// Convert an engine snapshot; `explicit` is the cached flag for the current
/// track (queue items carry none).
pub fn slim(state: &PlaybackState, explicit: Option<bool>, queue_rev: u64) -> RemoteState {
    RemoteState {
        logged_in: true,
        playing: state.playing,
        buffering: state.buffering,
        current: state.current.as_ref().map(|item| track(item, explicit, quality_name(state))),
        current_index: state.current_index,
        position_ms: state.position_ms,
        duration_ms: state.duration_ms,
        volume: state.volume,
        shuffle: state.shuffle,
        repeat: state.repeat,
        queue_len: state.queue.len(),
        queue_rev,
        stream: state.stream.as_ref().map(|s| RemoteStream {
            codec: s.codec.clone(),
            sample_rate: s.sample_rate,
            bit_depth: s.bit_depth,
            bitrate_kbps: s.bitrate_kbps,
        }),
        bit_perfect: state.bit_perfect,
        dsp_stages: state.dsp_stages.iter().map(|s| s.to_string()).collect(),
    }
}

/// Length + order-sensitive hash of the queue's track ids.
pub fn fingerprint(queue: &[QueueItem]) -> (usize, u64) {
    let mut hasher = DefaultHasher::new();
    for item in queue {
        item.track_id.hash(&mut hasher);
    }
    (queue.len(), hasher.finish())
}

/// `queueRev`: bumps whenever an observed fingerprint differs from the last.
#[derive(Debug, Default)]
pub struct QueueRev {
    last: Option<(usize, u64)>,
    rev: u64,
}

impl QueueRev {
    pub fn observe(&mut self, fingerprint: (usize, u64)) -> u64 {
        if self.last != Some(fingerprint) {
            if self.last.is_some() {
                self.rev += 1;
            }
            self.last = Some(fingerprint);
        }
        self.rev
    }
}

/// Minimum spacing between sends.
#[derive(Debug)]
pub struct Throttle {
    interval: Duration,
    last: Option<Instant>,
}

impl Throttle {
    pub fn new(interval: Duration) -> Self {
        Self { interval, last: None }
    }

    /// `Ok` (and record the send) when allowed now, else how long to wait.
    pub fn ready(&mut self, now: Instant) -> Result<(), Duration> {
        match self.last {
            Some(last) if now.saturating_duration_since(last) < self.interval => {
                Err(self.interval - now.saturating_duration_since(last))
            }
            _ => {
                self.last = Some(now);
                Ok(())
            }
        }
    }
}

/// `session` event payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionKind {
    LoggedOut,
    Expired,
    Ok,
}

/// `favorite` event payload: `kind` is `track`, `album`, or `artist`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FavoriteChange {
    pub kind: &'static str,
    pub id: u64,
    pub favorite: bool,
}

#[derive(Debug, Clone)]
pub enum HubEvent {
    State(Arc<RemoteState>),
    Position { position_ms: u64, duration_ms: u64 },
    Error { code: String, message: String },
    Session(SessionKind),
    /// A favorite changed on any surface (desktop or a phone).
    Favorite(FavoriteChange),
    /// `None` = every device (remote disabled); `Some(id)` = one device.
    Revoked(Option<String>),
}

/// A paged slice of the engine queue.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuePage {
    pub items: Vec<QueueItem>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub current_index: Option<usize>,
    pub queue_rev: u64,
}

struct HubInner {
    rev: QueueRev,
    latest: Option<RemoteState>,
    state_throttle: Throttle,
    position_throttle: Throttle,
    flush_scheduled: bool,
}

pub struct Hub {
    session: Arc<dyn RemoteSession>,
    tx: broadcast::Sender<HubEvent>,
    inner: Mutex<HubInner>,
    runtime: Option<tokio::runtime::Handle>,
}

impl Hub {
    /// `runtime` runs the deferred send of a coalesced state; it is captured
    /// at start because engine events arrive on non-runtime threads.
    pub fn new(session: Arc<dyn RemoteSession>, runtime: Option<tokio::runtime::Handle>) -> Arc<Self> {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Arc::new(Self {
            session,
            tx,
            inner: Mutex::new(HubInner {
                rev: QueueRev::default(),
                latest: None,
                state_throttle: Throttle::new(STATE_INTERVAL),
                position_throttle: Throttle::new(POSITION_INTERVAL),
                flush_scheduled: false,
            }),
            runtime,
        })
    }

    pub fn clients(&self) -> usize {
        self.tx.receiver_count()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HubEvent> {
        self.tx.subscribe()
    }

    fn send(&self, event: HubEvent) {
        let _ = self.tx.send(event);
    }

    /// A fresh full state from the engine (snapshot on connect / resync).
    pub fn snapshot(&self) -> RemoteState {
        let logged_in = self.session.logged_in();
        let Some(playback) = self.session.playback().filter(|_| logged_in) else {
            return RemoteState::idle(logged_in);
        };
        let explicit = playback.current.as_ref().and_then(|c| self.session.explicit(c.track_id));
        let mut inner = self.inner.lock().unwrap();
        let rev = inner.rev.observe(fingerprint(&playback.queue));
        let state = slim(&playback, explicit, rev);
        inner.latest = Some(state.clone());
        state
    }

    pub fn queue_page(&self, offset: usize, limit: usize) -> QueuePage {
        let playback = self.session.playback();
        let (queue, current_index) = match playback {
            Some(p) => (p.queue, p.current_index),
            None => (Default::default(), None),
        };
        let rev = self.inner.lock().unwrap().rev.observe(fingerprint(&queue));
        let items = queue.iter().skip(offset).take(limit).cloned().collect();
        QueuePage { items, total: queue.len(), offset, limit, current_index, queue_rev: rev }
    }

    /// The engine subscriber. Does nothing without a connected client.
    pub fn on_engine_event(self: &Arc<Self>, event: &EngineEvent) {
        if self.clients() == 0 {
            return;
        }
        match event {
            EngineEvent::State(playback) => {
                let explicit =
                    playback.current.as_ref().and_then(|c| self.session.explicit(c.track_id));
                let mut inner = self.inner.lock().unwrap();
                let rev = inner.rev.observe(fingerprint(&playback.queue));
                inner.latest = Some(slim(playback, explicit, rev));
                self.schedule_state(&mut inner);
            }
            EngineEvent::NowPlaying(item) => {
                let explicit = item.as_ref().and_then(|c| self.session.explicit(c.track_id));
                let mut inner = self.inner.lock().unwrap();
                if let Some(latest) = inner.latest.as_mut() {
                    latest.current = item.as_ref().map(|i| track(i, explicit, None));
                    latest.duration_ms = item.as_ref().map(|i| i.duration_secs as u64 * 1000).unwrap_or(0);
                    latest.position_ms = 0;
                    self.schedule_state(&mut inner);
                }
            }
            EngineEvent::Position { position_ms, duration_ms, .. } => {
                let mut inner = self.inner.lock().unwrap();
                if let Some(latest) = inner.latest.as_mut() {
                    latest.position_ms = *position_ms;
                    latest.duration_ms = *duration_ms;
                }
                if inner.position_throttle.ready(Instant::now()).is_ok() {
                    self.send(HubEvent::Position {
                        position_ms: *position_ms,
                        duration_ms: *duration_ms,
                    });
                }
            }
            EngineEvent::Error { code, message } => self.send(HubEvent::Error {
                code: code.clone(),
                message: message.clone(),
            }),
            // CatJam is desktop-only; the phone never gets beats.
            EngineEvent::Beat(_) => {}
        }
    }

    /// Send the latest state now, or once the coalescing window has passed.
    fn schedule_state(self: &Arc<Self>, inner: &mut HubInner) {
        match inner.state_throttle.ready(Instant::now()) {
            Ok(()) => {
                if let Some(latest) = inner.latest.clone() {
                    self.send(HubEvent::State(Arc::new(latest)));
                }
            }
            Err(wait) => {
                if inner.flush_scheduled {
                    return;
                }
                let Some(runtime) = &self.runtime else {
                    return;
                };
                inner.flush_scheduled = true;
                let hub = Arc::clone(self);
                runtime.spawn(async move {
                    tokio::time::sleep(wait).await;
                    let mut inner = hub.inner.lock().unwrap();
                    inner.flush_scheduled = false;
                    let _ = inner.state_throttle.ready(Instant::now());
                    if let Some(latest) = inner.latest.clone() {
                        hub.send(HubEvent::State(Arc::new(latest)));
                    }
                });
            }
        }
    }

    /// Login/logout/expiry. A new login also pushes a fresh state.
    pub fn notify_session(&self, kind: SessionKind) {
        if self.clients() == 0 {
            return;
        }
        self.send(HubEvent::Session(kind));
        self.send(HubEvent::State(Arc::new(self.snapshot())));
    }

    pub fn notify_favorite(&self, change: FavoriteChange) {
        if self.clients() > 0 {
            self.send(HubEvent::Favorite(change));
        }
    }

    pub fn revoke(&self, device: Option<String>) {
        self.send(HubEvent::Revoked(device));
    }
}

/// One SSE frame before it is rendered by the HTTP layer.
#[derive(Debug, Clone, PartialEq)]
pub struct SseMessage {
    pub event: &'static str,
    pub id: u64,
    pub data: String,
}

fn message<T: Serialize>(event: &'static str, id: u64, data: &T) -> SseMessage {
    SseMessage { event, id, data: serde_json::to_string(data).unwrap_or_else(|_| "null".into()) }
}

struct ClientState<G> {
    hub: Arc<Hub>,
    rx: broadcast::Receiver<HubEvent>,
    shutdown: watch::Receiver<bool>,
    device_id: String,
    next_id: u64,
    started: bool,
    done: bool,
    _guard: G,
}

impl<G> ClientState<G> {
    fn frame<T: Serialize>(&mut self, event: &'static str, data: &T) -> SseMessage {
        self.next_id += 1;
        message(event, self.next_id, data)
    }

    fn revoked(&mut self) -> SseMessage {
        self.done = true;
        self.frame("revoked", &serde_json::json!({}))
    }
}

/// One phone's stream: a `snapshot` first, then deltas until the device is
/// revoked, the remote stops, or the client falls behind (then it ends, and
/// the phone reconnects for a fresh snapshot). `guard` lives as long as the
/// stream (connection accounting).
pub fn client_stream<G: Send + 'static>(
    hub: Arc<Hub>,
    device_id: String,
    shutdown: watch::Receiver<bool>,
    guard: G,
) -> impl Stream<Item = SseMessage> + Send + 'static {
    let rx = hub.subscribe();
    let state = ClientState {
        hub,
        rx,
        shutdown,
        device_id,
        next_id: 0,
        started: false,
        done: false,
        _guard: guard,
    };
    futures_util::stream::unfold(state, |mut s| async move {
        if s.done {
            return None;
        }
        if !s.started {
            s.started = true;
            let snapshot = s.hub.snapshot();
            let frame = s.frame("snapshot", &snapshot);
            return Some((frame, s));
        }
        loop {
            if *s.shutdown.borrow() {
                let frame = s.revoked();
                return Some((frame, s));
            }
            tokio::select! {
                changed = s.shutdown.changed() => {
                    if changed.is_err() || *s.shutdown.borrow() {
                        let frame = s.revoked();
                        return Some((frame, s));
                    }
                }
                received = s.rx.recv() => {
                    let frame = match received {
                        Ok(HubEvent::State(state)) => s.frame("state", state.as_ref()),
                        Ok(HubEvent::Position { position_ms, duration_ms }) => s.frame(
                            "position",
                            &serde_json::json!({"positionMs": position_ms, "durationMs": duration_ms}),
                        ),
                        Ok(HubEvent::Error { code, message }) => s.frame(
                            "error",
                            &serde_json::json!({"code": code, "message": message}),
                        ),
                        Ok(HubEvent::Session(kind)) => s.frame("session", &kind),
                        Ok(HubEvent::Favorite(change)) => s.frame("favorite", &change),
                        Ok(HubEvent::Revoked(None)) => s.revoked(),
                        Ok(HubEvent::Revoked(Some(id))) if id == s.device_id => s.revoked(),
                        Ok(HubEvent::Revoked(Some(_))) => continue,
                        // Lagged or closed: drop the client; it resyncs on reconnect.
                        Err(_) => return None,
                    };
                    return Some((frame, s));
                }
            }
        }
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::remote::rpc::RemoteCommand;
    use crate::remote::BoxFuture;
    use futures_util::StreamExt;
    use oxitide_audio::output::OutputMode;
    use oxitide_tidal::stream::Quality;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub fn item(id: u64) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 200,
            cover: Some("c".into()),
            artist_refs: Vec::new(),
            album_id: None,
        }
    }

    pub fn playback(queue: Vec<QueueItem>, current: Option<usize>) -> PlaybackState {
        PlaybackState {
            playing: true,
            buffering: false,
            current: current.and_then(|i| queue.get(i).cloned()),
            current_index: current,
            position_ms: 1500,
            duration_ms: 200_000,
            volume: 0.8,
            shuffle: false,
            repeat: RepeatMode::All,
            preferred_quality: Quality::Lossless,
            output_mode: OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            dsp_active: true,
            dsp_stages: vec!["EQ"],
            crossfade_ms: 0,
            last_transition: None,
            queue: Arc::new(queue),
            queue_rev: 1,
            stream: None,
        }
    }

    /// A session with a settable engine snapshot that counts lookups.
    #[derive(Default)]
    pub struct FakeSession {
        pub logged_in: std::sync::atomic::AtomicBool,
        pub playback: Mutex<Option<PlaybackState>>,
        pub lookups: AtomicUsize,
        pub commands: Mutex<Vec<String>>,
    }

    impl FakeSession {
        pub fn with(playback: PlaybackState) -> Arc<Self> {
            let session = Self::default();
            session.logged_in.store(true, Ordering::SeqCst);
            *session.playback.lock().unwrap() = Some(playback);
            Arc::new(session)
        }
    }

    impl RemoteSession for FakeSession {
        fn logged_in(&self) -> bool {
            self.logged_in.load(Ordering::SeqCst)
        }
        fn playback(&self) -> Option<PlaybackState> {
            self.lookups.fetch_add(1, Ordering::SeqCst);
            self.playback.lock().unwrap().clone()
        }
        fn explicit(&self, _track_id: u64) -> Option<bool> {
            self.lookups.fetch_add(1, Ordering::SeqCst);
            Some(true)
        }
        fn command(&self, command: RemoteCommand) -> BoxFuture<'static, Result<serde_json::Value, String>> {
            self.commands.lock().unwrap().push(format!("{command:?}"));
            Box::pin(async { Ok(serde_json::Value::Null) })
        }
    }

    #[test]
    fn slim_state_strips_the_queue() {
        let state = slim(&playback((1..=3).map(item).collect(), Some(1)), Some(true), 4);
        let value = serde_json::to_value(&state).unwrap();
        assert!(value.get("queue").is_none(), "{value}");
        assert_eq!(value["queueLen"], 3);
        assert_eq!(value["queueRev"], 4);
        assert_eq!(value["current"]["trackId"], 2);
        assert_eq!(value["current"]["explicit"], true);
        assert_eq!(value["currentIndex"], 1);
        assert_eq!(value["dspStages"], serde_json::json!(["EQ"]));
        assert_eq!(value["repeat"], "all");
        assert_eq!(value["loggedIn"], true);
    }

    #[test]
    fn queue_rev_changes_iff_the_queue_changes() {
        let mut rev = QueueRev::default();
        let a: Vec<_> = (1..=3).map(item).collect();
        assert_eq!(rev.observe(fingerprint(&a)), 0);
        assert_eq!(rev.observe(fingerprint(&a)), 0, "same queue, same rev");
        let reordered = vec![item(2), item(1), item(3)];
        assert_eq!(rev.observe(fingerprint(&reordered)), 1, "order matters");
        let longer: Vec<_> = (1..=4).map(item).collect();
        assert_eq!(rev.observe(fingerprint(&longer)), 2);
        assert_eq!(rev.observe(fingerprint(&longer)), 2);
    }

    #[test]
    fn throttle_spaces_sends() {
        let mut throttle = Throttle::new(Duration::from_secs(1));
        let t0 = Instant::now();
        assert!(throttle.ready(t0).is_ok());
        assert_eq!(throttle.ready(t0 + Duration::from_millis(300)), Err(Duration::from_millis(700)));
        assert!(throttle.ready(t0 + Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn nothing_is_computed_without_clients() {
        let session = FakeSession::with(playback((1..=3).map(item).collect(), Some(0)));
        let hub = Hub::new(session.clone(), None);
        hub.on_engine_event(&EngineEvent::State(playback((1..=3).map(item).collect(), Some(0))));
        hub.on_engine_event(&EngineEvent::Position { position_ms: 1, duration_ms: 2, lead_ms: 0 });
        assert_eq!(session.lookups.load(Ordering::SeqCst), 0);
        assert!(hub.inner.lock().unwrap().latest.is_none());
    }

    #[tokio::test]
    async fn states_are_coalesced_and_positions_throttled() {
        let session = FakeSession::with(playback(vec![item(1)], Some(0)));
        let hub = Hub::new(session, Some(tokio::runtime::Handle::current()));
        let mut rx = hub.subscribe();
        let state = playback(vec![item(1)], Some(0));
        hub.on_engine_event(&EngineEvent::State(state.clone()));
        hub.on_engine_event(&EngineEvent::State(state.clone()));
        hub.on_engine_event(&EngineEvent::State(state));
        hub.on_engine_event(&EngineEvent::Position { position_ms: 10, duration_ms: 20, lead_ms: 0 });
        hub.on_engine_event(&EngineEvent::Position { position_ms: 11, duration_ms: 20, lead_ms: 0 });

        assert!(matches!(rx.recv().await.unwrap(), HubEvent::State(_)), "first state is immediate");
        assert!(matches!(
            rx.recv().await.unwrap(),
            HubEvent::Position { position_ms: 10, .. }
        ));
        // The two later states collapse into one deferred send carrying the
        // latest position.
        match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await.unwrap().unwrap() {
            HubEvent::State(state) => assert_eq!(state.position_ms, 11),
            other => panic!("unexpected {other:?}"),
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(250), rx.recv()).await.is_err(),
            "exactly one deferred state and no second position within 1 s"
        );
    }

    #[tokio::test]
    async fn stream_starts_with_a_snapshot_then_deltas() {
        let session = FakeSession::with(playback((1..=2).map(item).collect(), Some(0)));
        let hub = Hub::new(session, None);
        let (_tx, shutdown) = watch::channel(false);
        let mut stream = Box::pin(client_stream(hub.clone(), "d1".into(), shutdown, ()));
        let first = stream.next().await.unwrap();
        assert_eq!((first.event, first.id), ("snapshot", 1));
        assert!(first.data.contains("\"queueLen\":2"));
        hub.on_engine_event(&EngineEvent::Error { code: "net".into(), message: "x".into() });
        let second = stream.next().await.unwrap();
        assert_eq!((second.event, second.id), ("error", 2));
    }

    #[tokio::test]
    async fn revoking_a_device_ends_only_its_stream() {
        let session = FakeSession::with(playback(vec![item(1)], Some(0)));
        let hub = Hub::new(session, None);
        let (_tx, shutdown) = watch::channel(false);
        let mut mine = Box::pin(client_stream(hub.clone(), "d1".into(), shutdown.clone(), ()));
        let mut other = Box::pin(client_stream(hub.clone(), "d2".into(), shutdown, ()));
        mine.next().await.unwrap();
        other.next().await.unwrap();
        hub.revoke(Some("d1".into()));
        assert_eq!(mine.next().await.unwrap().event, "revoked");
        assert!(mine.next().await.is_none());
        hub.on_engine_event(&EngineEvent::Error { code: "c".into(), message: "m".into() });
        assert_eq!(other.next().await.unwrap().event, "error", "d2 skips d1's revoke");
    }

    #[tokio::test]
    async fn shutdown_closes_every_stream() {
        let session = FakeSession::with(playback(vec![item(1)], Some(0)));
        let hub = Hub::new(session, None);
        let (tx, shutdown) = watch::channel(false);
        let mut stream = Box::pin(client_stream(hub, "d1".into(), shutdown, ()));
        stream.next().await.unwrap();
        tx.send(true).unwrap();
        assert_eq!(stream.next().await.unwrap().event, "revoked");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn a_lagging_client_is_dropped() {
        let session = FakeSession::with(playback(vec![item(1)], Some(0)));
        let hub = Hub::new(session, None);
        let (_tx, shutdown) = watch::channel(false);
        let mut stream = Box::pin(client_stream(hub.clone(), "d1".into(), shutdown, ()));
        stream.next().await.unwrap();
        for i in 0..(CHANNEL_CAPACITY as u64 + 5) {
            hub.on_engine_event(&EngineEvent::Error { code: i.to_string(), message: String::new() });
        }
        assert!(stream.next().await.is_none(), "lag ends the stream");
    }

    #[tokio::test]
    async fn favorite_changes_reach_connected_phones() {
        let session = FakeSession::with(playback(vec![item(1)], Some(0)));
        let hub = Hub::new(session, None);
        hub.notify_favorite(FavoriteChange { kind: "track", id: 9, favorite: true });
        let (_tx, shutdown) = watch::channel(false);
        let mut stream = Box::pin(client_stream(hub.clone(), "d1".into(), shutdown, ()));
        stream.next().await.unwrap();
        hub.notify_favorite(FavoriteChange { kind: "album", id: 7, favorite: false });
        let frame = stream.next().await.unwrap();
        assert_eq!(frame.event, "favorite");
        assert_eq!(frame.data, r#"{"kind":"album","id":7,"favorite":false}"#);
    }

    #[test]
    fn logged_out_snapshot_is_idle() {
        let session = FakeSession::with(playback(vec![item(1)], Some(0)));
        session.logged_in.store(false, Ordering::SeqCst);
        let hub = Hub::new(session, None);
        let state = hub.snapshot();
        assert!(!state.logged_in);
        assert!(state.current.is_none());
    }

    #[test]
    fn queue_pages_slice_the_engine_queue() {
        let session = FakeSession::with(playback((1..=10).map(item).collect(), Some(4)));
        let hub = Hub::new(session, None);
        let page = hub.queue_page(8, 5);
        assert_eq!(page.total, 10);
        assert_eq!(page.items.iter().map(|i| i.track_id).collect::<Vec<_>>(), vec![9, 10]);
        assert_eq!(page.current_index, Some(4));
    }
}
