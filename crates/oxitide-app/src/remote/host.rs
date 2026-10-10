//! Tauri glue: the `RemoteSession` over `AppState`, the embedded page assets,
//! desktop events, and the enable/disable lifecycle behind the commands.

use std::sync::Arc;

use oxitide_audio::engine::PlaybackState;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::events::SessionKind;
use super::net::{self, InterfaceDto};
use super::pairing::{devices_path, DeviceStore, PairRequestDto};
use super::rpc::{self, RemoteCommand};
use super::{qr, Asset, AssetSource, BoxFuture, RemoteEvents, RemoteOptions, RemoteService, RemoteSession};
use crate::config::RemoteConfig;
use crate::state::AppState;

pub const REMOTE_STATUS_EVENT: &str = "remote_status";
pub const REMOTE_PAIR_REQUEST_EVENT: &str = "remote_pair_request";
/// Desktop event: a favorite changed (possibly from a phone).
pub const FAVORITE_CHANGED_EVENT: &str = "favorite_changed";

#[derive(Debug, Clone, Serialize)]
pub struct RemoteStatusDto {
    /// The configured switch (`[remote] enabled`).
    pub enabled: bool,
    pub listening: bool,
    /// The port actually bound (may differ from the configured one).
    pub port: Option<u16>,
    /// The address shown in the QR code.
    pub address: Option<String>,
    /// `http://<address>:<port>/remote` (no pairing token).
    pub url: Option<String>,
    /// Why the server is not listening although enabled.
    pub error: Option<String>,
    /// Open phone connections.
    pub connected: usize,
    pub interfaces: Vec<InterfaceDto>,
    pub settings: RemoteConfig,
    /// Phones waiting for approval.
    pub pending: Vec<PairRequestDto>,
    /// Seconds left on the outstanding pairing QR, if any.
    pub pairing_expires_in_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingDto {
    pub url: String,
    pub qr_svg: String,
    pub expires_in_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteDeviceDto {
    pub id: String,
    pub name: String,
    pub created_ms: u64,
    pub last_seen_ms: u64,
    pub connected: bool,
}

struct TauriSession<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> RemoteSession for TauriSession<R> {
    fn logged_in(&self) -> bool {
        self.app.state::<AppState>().is_logged_in()
    }

    fn playback(&self) -> Option<PlaybackState> {
        self.app.state::<AppState>().player().map(|p| p.engine.state())
    }

    fn explicit(&self, track_id: u64) -> Option<bool> {
        self.app.state::<AppState>().track_flags()?.get(track_id)?.explicit
    }

    fn command(&self, command: RemoteCommand) -> BoxFuture<'static, Result<Value, String>> {
        let app = self.app.clone();
        Box::pin(async move { rpc::execute(&app, command).await })
    }
}

/// Release builds serve the embedded `ui/dist`; a debug build with `devUrl`
/// proxies the Vite dev server first (Tauri's dev resolver would read a
/// possibly stale `ui/dist`).
struct TauriAssets<R: Runtime> {
    app: AppHandle<R>,
    dev: Option<(reqwest::Client, url::Url)>,
}

/// The remote entry's mount point; anything else answering for
/// `remote.html` is not our page.
const REMOTE_MARKER: &str = "id=\"remote\"";

/// Tauri's resolver falls back to `index.html` (the desktop shell) for any
/// unknown path; only exact hits are served.
fn embedded_asset<R: Runtime>(app: &AppHandle<R>, file: &str) -> Option<Asset> {
    let asset = app.asset_resolver().get(file.to_string())?;
    let mime = asset.mime_type().to_string();
    let bytes = asset.bytes().to_vec();
    let is_html = mime.starts_with("text/html");
    let wants_html = file.ends_with(".html");
    if is_html != wants_html {
        return None;
    }
    if file == "remote.html" && !String::from_utf8_lossy(&bytes).contains(REMOTE_MARKER) {
        return None;
    }
    Some(Asset { bytes, mime })
}

impl<R: Runtime> AssetSource for TauriAssets<R> {
    fn get(&self, path: &str) -> BoxFuture<'static, Option<Asset>> {
        let file = path.split(['?', '#']).next().unwrap_or(path).to_string();
        let Some((client, base)) = self.dev.clone() else {
            let found = embedded_asset(&self.app, &file);
            return Box::pin(async move { found });
        };
        let path = path.to_string();
        Box::pin(async move {
            let url = base.join(&path).ok()?;
            let response = client.get(url).send().await.ok()?;
            if !response.status().is_success() {
                return None;
            }
            let mime = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            // Vite's SPA fallback answers unknown paths with the shell.
            if mime.starts_with("text/html") != file.ends_with(".html") {
                return None;
            }
            let bytes = response.bytes().await.ok()?.to_vec();
            Some(Asset { bytes, mime })
        })
    }
}

struct TauriEvents<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> RemoteEvents for TauriEvents<R> {
    fn changed(&self) {
        let _ = self.app.emit(REMOTE_STATUS_EVENT, status(&self.app));
    }

    fn pair_request(&self, request: &PairRequestDto) {
        let _ = self.app.emit(REMOTE_PAIR_REQUEST_EVENT, request);
        // The approval prompt lives in the window; surface it if the app sits
        // in the tray (the user started pairing moments ago).
        if let Some(window) = self.app.get_webview_window("main") {
            if !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false) {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
    }
}

/// The device store, loaded on first use (never on the startup path).
pub fn devices<R: Runtime>(app: &AppHandle<R>) -> Arc<DeviceStore> {
    app.state::<AppState>()
        .remote_devices
        .get_or_init(|| Arc::new(DeviceStore::load(devices_path())))
        .clone()
}

pub fn status<R: Runtime>(app: &AppHandle<R>) -> RemoteStatusDto {
    let state = app.state::<AppState>();
    let settings = state.config().remote;
    let service = state.remote();
    let interfaces = net::list_interfaces();
    let address = service
        .as_ref()
        .map(|_| net::pick(&interfaces, &settings.interface, net::default_route_ip()));
    let error = state.remote_error.lock().unwrap().clone();
    RemoteStatusDto {
        enabled: settings.enabled,
        listening: service.is_some(),
        port: service.as_ref().map(|s| s.port()),
        address: address.map(|ip| ip.to_string()),
        url: service.as_ref().zip(address).map(|(s, ip)| qr::base_url(ip, s.port())),
        error,
        connected: service.as_ref().map(|s| s.connected()).unwrap_or(0),
        interfaces,
        pending: service.as_ref().map(|s| s.pending_requests()).unwrap_or_default(),
        pairing_expires_in_secs: service
            .as_ref()
            .and_then(|s| s.pairing_remaining())
            .map(|d| d.as_secs()),
        settings,
    }
}

pub fn emit_status<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(REMOTE_STATUS_EVENT, status(app));
}

/// Start the server from `[remote]` if it is not running. A bind failure is
/// recorded for the status (never a panic).
pub async fn start<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _lifecycle = state.remote_lifecycle.lock().await;
    if state.remote().is_some() {
        return Ok(());
    }
    let settings = state.config().remote;
    let dev = if cfg!(debug_assertions) {
        app.config().build.dev_url.clone().map(|url| (reqwest::Client::new(), url))
    } else {
        None
    };
    let started = RemoteService::start(
        RemoteOptions { port: settings.port, approve_new_devices: settings.approve_new_devices },
        Arc::new(TauriSession { app: app.clone() }),
        Arc::new(TauriAssets { app: app.clone(), dev }),
        Arc::new(TauriEvents { app: app.clone() }),
        devices(app),
    )
    .await;
    let result = match started {
        Ok(service) => {
            *state.remote_error.lock().unwrap() = None;
            *state.remote.lock().unwrap() = Some(service);
            Ok(())
        }
        Err(e) => {
            tracing::warn!("phone remote unavailable: {e}");
            *state.remote_error.lock().unwrap() = Some(e.clone());
            Err(e)
        }
    };
    drop(_lifecycle);
    emit_status(app);
    result
}

/// Stop the server (disable, quit). The device store persists.
pub async fn stop<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let _lifecycle = state.remote_lifecycle.lock().await;
    stop_now(app);
}

/// Stop without the lifecycle lock (exit path, where nothing else runs).
pub fn stop_now<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let service = state.remote.lock().unwrap().take();
    *state.remote_error.lock().unwrap() = None;
    if let Some(service) = service {
        service.stop();
        emit_status(app);
    }
}

pub fn begin_pairing<R: Runtime>(app: &AppHandle<R>) -> Result<PairingDto, String> {
    let state = app.state::<AppState>();
    let service = state.remote().ok_or_else(|| "turn the phone remote on first".to_string())?;
    let token = service.begin_pairing()?;
    let interfaces = net::list_interfaces();
    let ip = net::pick(&interfaces, &state.config().remote.interface, net::default_route_ip());
    let url = qr::pairing_url(ip, service.port(), &token);
    let qr_svg = qr::qr_svg(&url)?;
    let expires_in_secs = service.pairing_remaining().map(|d| d.as_secs()).unwrap_or(0);
    Ok(PairingDto { url, qr_svg, expires_in_secs })
}

pub fn list_devices<R: Runtime>(app: &AppHandle<R>) -> Vec<RemoteDeviceDto> {
    let connected = app
        .state::<AppState>()
        .remote()
        .map(|s| s.connected_devices())
        .unwrap_or_default();
    devices(app)
        .list()
        .into_iter()
        .map(|d| RemoteDeviceDto {
            connected: connected.contains(&d.id),
            id: d.id,
            name: d.name,
            created_ms: d.created_ms,
            last_seen_ms: d.last_seen_ms,
        })
        .collect()
}

pub fn rename_device<R: Runtime>(app: &AppHandle<R>, id: &str, name: &str) -> Result<(), String> {
    let name = super::guard::sanitize_device_name(name);
    if !devices(app).rename(id, name)? {
        return Err("unknown device".to_string());
    }
    emit_status(app);
    Ok(())
}

pub fn revoke_device<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), String> {
    devices(app).revoke(id)?;
    if let Some(service) = app.state::<AppState>().remote() {
        service.device_revoked(id);
    }
    emit_status(app);
    Ok(())
}

/// Tell connected phones about a login, logout, or expiry.
pub fn session_changed(state: &AppState, kind: SessionKind) {
    if let Some(service) = state.remote() {
        service.notify_session(kind);
    }
}
