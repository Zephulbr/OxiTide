//! Phone remote (Track 11, spec `2026-10-09-oxitide-track11-phone-remote-design.md`).
//!
//! An opt-in LAN server: a phone pairs by QR code, then drives playback, the
//! queue, search, and the library through one allowlisted RPC endpoint and
//! follows state over server-sent events. Disabled = no listener, no task,
//! no hub (`AppState::remote` is `None`).

pub mod events;
pub mod guard;
pub mod host;
pub mod net;
pub mod pairing;
pub mod qr;
pub mod rpc;
pub mod server;

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use oxitide_audio::engine::{EngineEvent, PlaybackState};
use serde_json::Value;
use tokio::sync::watch;

use events::{FavoriteChange, Hub, SessionKind};
use pairing::{DeviceStore, PairRequestDto};
use rpc::RemoteCommand;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The transport-agnostic session boundary (RM-D12): commands in, slim state
/// out. The web remote is the only implementation; a later TIDAL Connect
/// sidecar or listen-along transport can map its messages onto it without
/// touching the engine.
pub trait RemoteSession: Send + Sync + 'static {
    fn logged_in(&self) -> bool;
    /// The engine's current snapshot (`None` without a player).
    fn playback(&self) -> Option<PlaybackState>;
    /// The cached explicit flag for a track, if known.
    fn explicit(&self, track_id: u64) -> Option<bool>;
    fn command(&self, command: RemoteCommand) -> BoxFuture<'static, Result<Value, String>>;
}

/// A file of the remote page.
pub struct Asset {
    pub bytes: Vec<u8>,
    pub mime: String,
}

/// Serves the remote page's built files (`path` has no leading slash and may
/// carry a query string).
pub trait AssetSource: Send + Sync + 'static {
    fn get(&self, path: &str) -> BoxFuture<'static, Option<Asset>>;
}

/// Desktop-facing notifications (`remote_status`, `remote_pair_request`).
pub trait RemoteEvents: Send + Sync + 'static {
    /// Listening state, connections, devices, or pending approvals changed.
    fn changed(&self);
    fn pair_request(&self, request: &PairRequestDto);
}

pub struct RemoteOptions {
    pub port: u16,
    pub approve_new_devices: bool,
}

/// A running remote server. Dropping it does not stop it; call `stop`.
pub struct RemoteService {
    ctx: Arc<server::Ctx>,
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl RemoteService {
    /// Bind (trying `port … port+9`) and start serving on the current Tokio
    /// runtime.
    pub async fn start(
        options: RemoteOptions,
        session: Arc<dyn RemoteSession>,
        assets: Arc<dyn AssetSource>,
        events: Arc<dyn RemoteEvents>,
        devices: Arc<DeviceStore>,
    ) -> Result<Arc<Self>, String> {
        let (listener, port) = server::bind(options.port)?;
        let (shutdown, shutdown_rx) = watch::channel(false);
        let hub = Hub::new(session.clone(), Some(tokio::runtime::Handle::current()));
        let ctx = Arc::new(server::Ctx::new(
            session,
            hub,
            devices,
            assets,
            events,
            port,
            options.approve_new_devices,
            shutdown_rx.clone(),
        ));
        tokio::spawn(server::serve(listener, ctx.clone(), shutdown_rx));
        tracing::info!("phone remote listening on port {port}");
        Ok(Arc::new(Self { ctx, port, shutdown }))
    }

    /// Close every stream with a final `revoked` and stop accepting.
    pub fn stop(&self) {
        self.ctx.pairing.cancel();
        self.ctx.hub.revoke(None);
        let _ = self.shutdown.send(true);
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Open event streams across all phones.
    pub fn connected(&self) -> usize {
        self.ctx.connections.total()
    }

    pub fn connected_devices(&self) -> HashSet<String> {
        self.ctx.connections.devices()
    }

    pub fn begin_pairing(&self) -> Result<String, String> {
        self.ctx.pairing.begin(Instant::now())
    }

    pub fn cancel_pairing(&self) {
        self.ctx.pairing.cancel();
    }

    pub fn pairing_remaining(&self) -> Option<Duration> {
        self.ctx.pairing.remaining(Instant::now())
    }

    pub fn pending_requests(&self) -> Vec<PairRequestDto> {
        self.ctx.approvals.list()
    }

    pub fn resolve_pair_request(&self, id: &str, approve: bool) -> bool {
        self.ctx.approvals.resolve(id, approve)
    }

    pub fn set_approve_new_devices(&self, approve: bool) {
        self.ctx.approve_new.store(approve, Ordering::SeqCst);
    }

    /// Close a revoked device's streams now (its cookie already fails).
    pub fn device_revoked(&self, id: &str) {
        self.ctx.hub.revoke(Some(id.to_string()));
    }

    pub fn on_engine_event(&self, event: &EngineEvent) {
        self.ctx.hub.on_engine_event(event);
    }

    pub fn favorite_changed(&self, change: FavoriteChange) {
        self.ctx.hub.notify_favorite(change);
    }

    pub fn notify_session(&self, kind: SessionKind) {
        let hub = self.ctx.hub.clone();
        // Never touch the engine from the caller's thread (it may be an
        // engine listener): the snapshot runs on the runtime.
        tauri::async_runtime::spawn(async move { hub.notify_session(kind) });
    }
}
