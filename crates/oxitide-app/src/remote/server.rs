//! The HTTP layer (spec §5, §6.1–§6.4): listener with LAN-only admission,
//! Host/Origin/header guards, pairing, the RPC endpoint, the SSE stream, and
//! the embedded page.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::connect_info::Connected;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::serve::IncomingStream;
use axum::Router;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};

use super::events::{self, Hub};
use super::guard::{self, RateLimiter, COOKIE_NAME};
use super::pairing::{now_ms, Approvals, DeviceRecord, DeviceStore, PairError, Pairing, APPROVAL_TIMEOUT};
use super::rpc::{self, RemoteCommand, RpcError, RpcRequest, RpcResponse, MAX_RESPONSE_BYTES};
use super::{AssetSource, RemoteEvents, RemoteSession};

/// Request bodies above this are refused (`413`).
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Concurrent event streams across all phones.
pub const MAX_STREAMS: usize = 8;
/// Concurrent RPC calls.
pub const MAX_RPC_IN_FLIGHT: usize = 32;
const KEEP_ALIVE: Duration = Duration::from_secs(15);
const PORT_ATTEMPTS: u16 = 10;

/// The remote page's own CSP (spec §5.2). The webview CSP is unaffected.
pub const PAGE_CSP: &str = "default-src 'self'; img-src 'self' https://resources.tidal.com data:; \
connect-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; \
object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'none'";

/// Open SSE streams, per device.
#[derive(Default)]
pub struct Connections {
    by_device: Mutex<HashMap<String, usize>>,
}

impl Connections {
    pub fn total(&self) -> usize {
        self.by_device.lock().unwrap().values().sum()
    }

    pub fn devices(&self) -> HashSet<String> {
        self.by_device.lock().unwrap().keys().cloned().collect()
    }

    fn enter(self: &Arc<Self>, device: &str, events: Arc<dyn RemoteEvents>) -> Option<StreamGuard> {
        let mut map = self.by_device.lock().unwrap();
        if map.values().sum::<usize>() >= MAX_STREAMS {
            return None;
        }
        *map.entry(device.to_string()).or_default() += 1;
        Some(StreamGuard { connections: self.clone(), device: device.to_string(), events })
    }
}

/// Counts one stream while alive; dropping it (stream end or client gone)
/// updates the desktop's connected state.
pub struct StreamGuard {
    connections: Arc<Connections>,
    device: String,
    events: Arc<dyn RemoteEvents>,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        {
            let mut map = self.connections.by_device.lock().unwrap();
            if let Some(count) = map.get_mut(&self.device) {
                *count -= 1;
                if *count == 0 {
                    map.remove(&self.device);
                }
            }
        }
        self.events.changed();
    }
}

pub struct Ctx {
    pub session: Arc<dyn RemoteSession>,
    pub hub: Arc<Hub>,
    pub devices: Arc<DeviceStore>,
    pub pairing: Pairing,
    pub approvals: Approvals,
    pub approve_new: AtomicBool,
    pub limiter: Mutex<RateLimiter>,
    pub connections: Arc<Connections>,
    pub rpc_slots: Arc<Semaphore>,
    pub assets: Arc<dyn AssetSource>,
    pub events: Arc<dyn RemoteEvents>,
    pub port: u16,
    pub shutdown: watch::Receiver<bool>,
}

impl Ctx {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: Arc<dyn RemoteSession>,
        hub: Arc<Hub>,
        devices: Arc<DeviceStore>,
        assets: Arc<dyn AssetSource>,
        events: Arc<dyn RemoteEvents>,
        port: u16,
        approve_new: bool,
        shutdown: watch::Receiver<bool>,
    ) -> Self {
        Self {
            session,
            hub,
            devices,
            pairing: Pairing::default(),
            approvals: Approvals::default(),
            approve_new: AtomicBool::new(approve_new),
            limiter: Mutex::new(RateLimiter::standard()),
            connections: Arc::new(Connections::default()),
            rpc_slots: Arc::new(Semaphore::new(MAX_RPC_IN_FLIGHT)),
            assets,
            events,
            port,
            shutdown,
        }
    }
}

fn bind_dual_stack(port: u16) -> io::Result<std::net::TcpListener> {
    let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_only_v6(false)?;
    #[cfg(not(windows))]
    socket.set_reuse_address(true)?;
    socket.bind(&SocketAddr::from((Ipv6Addr::UNSPECIFIED, port)).into())?;
    socket.listen(128)?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

fn bind_v4(port: u16) -> io::Result<std::net::TcpListener> {
    let listener = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Listen on `[::]:port` (dual-stack, falling back to `0.0.0.0`), trying
/// `port … port+9`. Returns the listener and the port actually bound.
pub fn bind(port: u16) -> Result<(TcpListener, u16), String> {
    let mut last_error = String::new();
    for offset in 0..PORT_ATTEMPTS {
        let Some(candidate) = port.checked_add(offset) else {
            break;
        };
        // IPv4-only only when IPv6 itself is unavailable: Windows would let
        // `0.0.0.0` share a port another process holds dual-stack.
        let bound = bind_dual_stack(candidate).or_else(|e| match e.kind() {
            io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied => Err(e),
            _ => bind_v4(candidate),
        });
        match bound.and_then(TcpListener::from_std) {
            Ok(listener) => {
                let actual = listener.local_addr().map(|a| a.port()).unwrap_or(candidate);
                return Ok((listener, actual));
            }
            Err(e) => last_error = e.to_string(),
        }
        if port == 0 {
            break;
        }
    }
    Err(format!(
        "could not listen on ports {port}–{}: {last_error}",
        port.saturating_add(PORT_ATTEMPTS - 1)
    ))
}

/// Drops non-LAN peers at accept, before a byte is read (RM-D3).
pub struct LanListener(pub TcpListener);

impl axum::serve::Listener for LanListener {
    type Io = TcpStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (TcpStream, SocketAddr) {
        loop {
            match self.0.accept().await {
                Ok((stream, peer)) if guard::is_lan_peer(peer.ip()) => return (stream, peer),
                Ok((stream, peer)) => {
                    tracing::debug!("remote: refused non-LAN peer {}", peer.ip());
                    drop(stream);
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

/// Both ends of a connection: the peer (rate limits) and the local address
/// it arrived on (the only acceptable `Host`).
#[derive(Debug, Clone, Copy)]
pub struct ConnInfo {
    pub remote: SocketAddr,
    pub local: SocketAddr,
}

impl Connected<IncomingStream<'_, LanListener>> for ConnInfo {
    fn connect_info(stream: IncomingStream<'_, LanListener>) -> Self {
        let remote = *stream.remote_addr();
        let local = stream.io().local_addr().unwrap_or(remote);
        Self { remote, local }
    }
}

pub fn router(ctx: Arc<Ctx>) -> Router {
    Router::new()
        .route("/", get(|| async { Redirect::to("/remote") }))
        .route("/api/pair", post(pair))
        .route("/api/rpc", post(rpc_call))
        .route("/api/events", get(event_stream))
        .fallback(asset)
        .layer(middleware::from_fn_with_state(ctx.clone(), guard_layer))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(ctx)
}

pub async fn serve(listener: TcpListener, ctx: Arc<Ctx>, mut shutdown: watch::Receiver<bool>) {
    let app = router(ctx).into_make_service_with_connect_info::<ConnInfo>();
    let result = axum::serve(LanListener(listener), app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.wait_for(|stopped| *stopped).await;
        })
        .await;
    if let Err(e) = result {
        tracing::warn!("phone remote server stopped: {e}");
    }
}

fn error_response(status: StatusCode, error: &RpcError) -> Response {
    let mut response = (status, axum::Json(RpcResponse::err(error))).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn too_many(retry: Duration) -> Response {
    let mut response = error_response(
        StatusCode::TOO_MANY_REQUESTS,
        &RpcError::new("rate_limited", "too many failed attempts; try again later"),
    );
    if let Ok(value) = HeaderValue::from_str(&retry.as_secs().max(1).to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// Host/Origin on every request; JSON + `X-Oxi-Remote: 1` on every POST.
async fn guard_layer(
    State(ctx): State<Arc<Ctx>>,
    ConnectInfo(conn): ConnectInfo<ConnInfo>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let host = header_str(headers, header::HOST).unwrap_or("");
    if !guard::host_allowed(host, conn.local.ip(), ctx.port) {
        return error_response(StatusCode::FORBIDDEN, &RpcError::new("forbidden", "unexpected host"));
    }
    if !guard::origin_allowed(header_str(headers, header::ORIGIN), host) {
        return error_response(StatusCode::FORBIDDEN, &RpcError::new("forbidden", "cross-origin request"));
    }
    if request.method() == Method::POST {
        let json = header_str(headers, header::CONTENT_TYPE)
            .is_some_and(|ct| ct.split(';').next().is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json")));
        let marked = headers.get("x-oxi-remote").is_some_and(|v| v == "1");
        if !json || !marked {
            return error_response(StatusCode::FORBIDDEN, &RpcError::new("forbidden", "not a remote request"));
        }
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}

fn device_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| guard::cookie_value(v, COOKIE_NAME))
}

/// The paired device behind the request's cookie. A wrong cookie counts
/// against the rate limit; a missing one (an unpaired phone) does not.
fn authenticate(ctx: &Ctx, ip: IpAddr, headers: &HeaderMap) -> Result<DeviceRecord, Denied> {
    let now = Instant::now();
    if let Err(retry) = ctx.limiter.lock().unwrap().check(ip, now) {
        return Err(Denied::RateLimited(retry));
    }
    let Some(token) = device_cookie(headers) else {
        return Err(Denied::Unauthorized);
    };
    match ctx.devices.authenticate(token, now_ms()) {
        Some(device) => Ok(device),
        None => {
            ctx.limiter.lock().unwrap().record_failure(ip, now);
            Err(Denied::Unauthorized)
        }
    }
}

/// Why a request carried no usable device cookie.
enum Denied {
    RateLimited(Duration),
    Unauthorized,
}

impl IntoResponse for Denied {
    fn into_response(self) -> Response {
        match self {
            Denied::RateLimited(retry) => too_many(retry),
            Denied::Unauthorized => error_response(
                StatusCode::UNAUTHORIZED,
                &RpcError::new("unauthorized", "pair this phone first"),
            ),
        }
    }
}

async fn dispatch(ctx: &Ctx, request: RpcRequest) -> Result<Value, RpcError> {
    let command = rpc::parse(&request.method, request.params)?;
    let to_value = |v: Result<Value, serde_json::Error>| v.map_err(|e| RpcError::new("failed", e.to_string()));
    match command {
        RemoteCommand::GetRemoteState => to_value(serde_json::to_value(ctx.hub.snapshot())),
        _ if !ctx.session.logged_in() => {
            Err(RpcError::new("not_logged_in", "sign in on the PC"))
        }
        RemoteCommand::GetQueuePage { offset, limit } => {
            to_value(serde_json::to_value(ctx.hub.queue_page(offset, limit)))
        }
        command => ctx
            .session
            .command(command)
            .await
            .map_err(|message| RpcError::new("failed", message)),
    }
}

async fn rpc_call(
    State(ctx): State<Arc<Ctx>>,
    ConnectInfo(conn): ConnectInfo<ConnInfo>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(denied) = authenticate(&ctx, conn.remote.ip(), &headers) {
        return denied.into_response();
    }
    let Ok(_permit) = ctx.rpc_slots.clone().try_acquire_owned() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &RpcError::new("busy", "too many requests in flight"),
        );
    };
    let request = match serde_json::from_slice::<RpcRequest>(&body) {
        Ok(request) => request,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &RpcError::new("bad_params", e.to_string()))
        }
    };
    let body = match dispatch(&ctx, request).await {
        Ok(value) => RpcResponse::ok(value),
        Err(error) => RpcResponse::err(&error),
    };
    let mut bytes = serde_json::to_vec(&body).unwrap_or_default();
    if bytes.len() > MAX_RESPONSE_BYTES {
        let error = RpcError::new("too_large", "response too large; use a paged method");
        bytes = serde_json::to_vec(&RpcResponse::err(&error)).unwrap_or_default();
    }
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairBody {
    token: String,
    #[serde(default)]
    device_name: String,
}

fn pair_error(status: StatusCode, code: &'static str, message: &str) -> Response {
    error_response(status, &RpcError::new(code, message))
}

async fn pair(
    State(ctx): State<Arc<Ctx>>,
    ConnectInfo(conn): ConnectInfo<ConnInfo>,
    body: Bytes,
) -> Response {
    let ip = guard::canonical(conn.remote.ip());
    let now = Instant::now();
    if let Err(retry) = ctx.limiter.lock().unwrap().check(ip, now) {
        return too_many(retry);
    }
    let Ok(body) = serde_json::from_slice::<PairBody>(&body) else {
        return pair_error(StatusCode::BAD_REQUEST, "bad_params", "expected {token, deviceName}");
    };
    match ctx.pairing.reserve(&body.token, now) {
        Ok(()) => {}
        Err(PairError::Invalid) => {
            ctx.limiter.lock().unwrap().record_failure(ip, now);
            return pair_error(
                StatusCode::FORBIDDEN,
                "invalid_token",
                "this pairing code is invalid or expired; show a new QR code on the PC",
            );
        }
        Err(PairError::Busy) => {
            return pair_error(StatusCode::CONFLICT, "busy", "another phone is pairing with this code")
        }
    }
    let name = guard::sanitize_device_name(&body.device_name);
    if ctx.approve_new.load(Ordering::SeqCst) {
        let (request, decision) = match ctx.approvals.register(name.clone(), ip.to_string()) {
            Ok(pending) => pending,
            Err(e) => {
                ctx.pairing.release(&body.token);
                return pair_error(StatusCode::INTERNAL_SERVER_ERROR, "failed", &e);
            }
        };
        ctx.events.pair_request(&request);
        ctx.events.changed();
        let mut shutdown = ctx.shutdown.clone();
        let outcome = tokio::select! {
            decided = tokio::time::timeout(APPROVAL_TIMEOUT, decision) => match decided {
                Ok(Ok(approved)) => Some(approved),
                Ok(Err(_)) => Some(false),
                Err(_) => None,
            },
            _ = shutdown.wait_for(|stopped| *stopped) => Some(false),
        };
        ctx.approvals.remove(&request.id);
        ctx.events.changed();
        match outcome {
            Some(true) => {}
            Some(false) => {
                ctx.pairing.consume(&body.token);
                return pair_error(StatusCode::FORBIDDEN, "denied", "pairing was declined on the PC");
            }
            None => {
                ctx.pairing.release(&body.token);
                return pair_error(
                    StatusCode::REQUEST_TIMEOUT,
                    "timeout",
                    "no answer on the PC; try again",
                );
            }
        }
    }
    ctx.pairing.consume(&body.token);
    let (device, token) = match ctx.devices.create(name, now_ms()) {
        Ok(created) => created,
        Err(e) => return pair_error(StatusCode::INTERNAL_SERVER_ERROR, "failed", &e),
    };
    ctx.events.changed();
    let cookie = format!("{COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=31536000");
    let mut response = (
        StatusCode::OK,
        axum::Json(json!({"ok": true, "result": {"deviceId": device.id, "name": device.name}})),
    )
        .into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        headers.insert(header::SET_COOKIE, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn event_stream(
    State(ctx): State<Arc<Ctx>>,
    ConnectInfo(conn): ConnectInfo<ConnInfo>,
    headers: HeaderMap,
) -> Response {
    let device = match authenticate(&ctx, conn.remote.ip(), &headers) {
        Ok(device) => device,
        Err(denied) => return denied.into_response(),
    };
    let Some(guard) = ctx.connections.enter(&device.id, ctx.events.clone()) else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &RpcError::new("busy", "too many open remotes"),
        );
    };
    ctx.events.changed();
    let stream = events::client_stream(ctx.hub.clone(), device.id, ctx.shutdown.clone(), guard).map(
        |message| {
            Ok::<_, Infallible>(
                Event::default()
                    .event(message.event)
                    .id(message.id.to_string())
                    .data(message.data),
            )
        },
    );
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE).text("keep-alive"))
        .into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    response
}

/// The page and its assets. `/remote` is the entry; the desktop shell's own
/// `index.html` is not served.
async fn asset(State(ctx): State<Arc<Ctx>>, method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") || path.is_empty() || path == "index.html" || path.contains("..") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (lookup, is_entry) = match path {
        "remote" | "remote/" | "remote.html" => ("remote.html".to_string(), true),
        _ => (
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(path).trim_start_matches('/').to_string(),
            false,
        ),
    };
    let Some(file) = ctx.assets.get(&lookup).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut response = (StatusCode::OK, file.bytes).into_response();
    let headers = response.headers_mut();
    if let Ok(mime) = HeaderValue::from_str(&file.mime) {
        headers.insert(header::CONTENT_TYPE, mime);
    }
    let cache = if is_entry || file.mime.starts_with("text/html") {
        headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(PAGE_CSP));
        "no-store"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::events::tests::{item, playback, FakeSession};
    use crate::remote::pairing::PairRequestDto;
    use crate::remote::{Asset, BoxFuture, RemoteOptions, RemoteService};
    use std::sync::atomic::AtomicUsize;

    struct FakeAssets;

    impl AssetSource for FakeAssets {
        fn get(&self, path: &str) -> BoxFuture<'static, Option<Asset>> {
            let found = match path {
                "remote.html" => Some(Asset { bytes: b"<!doctype html>remote".to_vec(), mime: "text/html".into() }),
                "assets/app.js" => Some(Asset { bytes: b"js".to_vec(), mime: "text/javascript".into() }),
                _ => None,
            };
            Box::pin(async move { found })
        }
    }

    #[derive(Default)]
    struct FakeEvents {
        requests: Mutex<Vec<PairRequestDto>>,
        changes: AtomicUsize,
    }

    impl RemoteEvents for FakeEvents {
        fn changed(&self) {
            self.changes.fetch_add(1, Ordering::SeqCst);
        }
        fn pair_request(&self, request: &PairRequestDto) {
            self.requests.lock().unwrap().push(request.clone());
        }
    }

    struct Harness {
        service: Arc<RemoteService>,
        session: Arc<FakeSession>,
        events: Arc<FakeEvents>,
        devices: Arc<DeviceStore>,
        base: String,
        host: String,
        http: reqwest::Client,
    }

    async fn harness(approve: bool) -> Harness {
        let session = FakeSession::with(playback((1..=3).map(item).collect(), Some(0)));
        let events = Arc::new(FakeEvents::default());
        let devices = Arc::new(DeviceStore::load(None));
        let service = RemoteService::start(
            RemoteOptions { port: 0, approve_new_devices: approve },
            session.clone(),
            Arc::new(FakeAssets),
            events.clone(),
            devices.clone(),
        )
        .await
        .unwrap();
        let host = format!("127.0.0.1:{}", service.port());
        Harness {
            base: format!("http://{host}"),
            host,
            service,
            session,
            events,
            devices,
            http: reqwest::Client::new(),
        }
    }

    impl Harness {
        fn post(&self, path: &str, body: Value) -> reqwest::RequestBuilder {
            self.http
                .post(format!("{}{path}", self.base))
                .header("x-oxi-remote", "1")
                .header("origin", format!("http://{}", self.host))
                .json(&body)
        }

        async fn pair(&self) -> String {
            let token = self.service.begin_pairing().unwrap();
            let response = self
                .post("/api/pair", json!({"token": token, "deviceName": "Pixel"}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap().to_string();
            assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"), "{cookie}");
            cookie.split(';').next().unwrap().to_string()
        }

        async fn rpc(&self, cookie: &str, method: &str, params: Value) -> (StatusCode, Value) {
            let response = self
                .post("/api/rpc", json!({"method": method, "params": params}))
                .header(header::COOKIE, cookie)
                .send()
                .await
                .unwrap();
            let status = StatusCode::from_u16(response.status().as_u16()).unwrap();
            (status, response.json().await.unwrap_or(Value::Null))
        }
    }

    /// Read SSE frames until `count` named events have arrived.
    async fn read_events(response: &mut reqwest::Response, count: usize) -> Vec<(String, String)> {
        let mut buffer = String::new();
        let mut out = Vec::new();
        while out.len() < count {
            let chunk = tokio::time::timeout(Duration::from_secs(5), response.chunk())
                .await
                .expect("event in time")
                .unwrap();
            let Some(chunk) = chunk else { break };
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buffer.find("\n\n") {
                let frame: String = buffer.drain(..end + 2).collect();
                let mut name = None;
                let mut data = String::new();
                for line in frame.lines() {
                    if let Some(v) = line.strip_prefix("event:") {
                        name = Some(v.trim().to_string());
                    } else if let Some(v) = line.strip_prefix("data:") {
                        data.push_str(v.trim());
                    }
                }
                if let Some(name) = name {
                    out.push((name, data));
                }
            }
        }
        out
    }

    #[tokio::test]
    async fn pairing_happy_path_then_rpc() {
        let h = harness(false).await;
        let cookie = h.pair().await;
        assert_eq!(h.devices.list().len(), 1);
        let (status, body) = h.rpc(&cookie, "get_remote_state", Value::Null).await;
        assert_eq!(status, 200);
        assert_eq!(body["ok"], true);
        assert_eq!(body["result"]["queueLen"], 3);
        let (_, body) = h.rpc(&cookie, "play_pause", Value::Null).await;
        assert_eq!(body["ok"], true);
        assert_eq!(h.session.commands.lock().unwrap().as_slice(), ["PlayPause"]);
        let (_, body) = h.rpc(&cookie, "get_queue_page", json!({"offset": 1, "limit": 1})).await;
        assert_eq!(body["result"]["items"][0]["track_id"], 2);
        let (_, body) = h.rpc(&cookie, "get_config", Value::Null).await;
        assert_eq!(body["error"], "unknown_method: get_config");
        h.service.stop();
    }

    #[tokio::test]
    async fn bad_token_and_replay_are_rejected() {
        let h = harness(false).await;
        let token = h.service.begin_pairing().unwrap();
        let bad = h.post("/api/pair", json!({"token": "nope"})).send().await.unwrap();
        assert_eq!(bad.status(), 403);
        let ok = h.post("/api/pair", json!({"token": token})).send().await.unwrap();
        assert_eq!(ok.status(), 200);
        let replay = h.post("/api/pair", json!({"token": token})).send().await.unwrap();
        assert_eq!(replay.status(), 403);
        h.service.stop();
    }

    #[tokio::test]
    async fn repeated_bad_tokens_are_rate_limited() {
        let h = harness(false).await;
        h.service.begin_pairing().unwrap();
        for _ in 0..5 {
            let r = h.post("/api/pair", json!({"token": "nope"})).send().await.unwrap();
            assert_eq!(r.status(), 403);
        }
        let locked = h.post("/api/pair", json!({"token": "nope"})).send().await.unwrap();
        assert_eq!(locked.status(), 429);
        assert!(locked.headers().contains_key(header::RETRY_AFTER));
        h.service.stop();
    }

    #[tokio::test]
    async fn approval_gates_new_devices() {
        let h = harness(true).await;
        let token = h.service.begin_pairing().unwrap();
        let request = h.post("/api/pair", json!({"token": token, "deviceName": "iPhone"})).send();
        let pending = tokio::spawn(request);
        let id = loop {
            if let Some(r) = h.events.requests.lock().unwrap().first() {
                break r.id.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert_eq!(h.service.pending_requests().len(), 1);
        assert!(h.service.resolve_pair_request(&id, false));
        let denied = pending.await.unwrap().unwrap();
        assert_eq!(denied.status(), 403);
        assert!(h.devices.list().is_empty());
        let reuse = h.post("/api/pair", json!({"token": token})).send().await.unwrap();
        assert_eq!(reuse.status(), 403, "a denied token is burned");
        h.service.stop();
    }

    #[tokio::test]
    async fn missing_or_wrong_cookie_is_unauthorized() {
        let h = harness(false).await;
        let (status, body) = h.rpc("", "get_remote_state", Value::Null).await;
        assert_eq!(status, 401);
        assert_eq!(body["ok"], false);
        let (status, _) = h.rpc("oxi_remote=forged", "get_remote_state", Value::Null).await;
        assert_eq!(status, 401);
        h.service.stop();
    }

    #[tokio::test]
    async fn host_origin_and_header_guards() {
        let h = harness(false).await;
        let cookie = h.pair().await;
        let wrong_host = h
            .http
            .get(format!("{}/remote", h.base))
            .header(header::HOST, format!("evil.example:{}", h.service.port()))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong_host.status(), 403);
        let wrong_origin = h
            .http
            .post(format!("{}/api/rpc", h.base))
            .header("x-oxi-remote", "1")
            .header(header::ORIGIN, "http://evil.example")
            .header(header::COOKIE, &cookie)
            .json(&json!({"method": "next"}))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong_origin.status(), 403);
        let no_marker = h
            .http
            .post(format!("{}/api/rpc", h.base))
            .header(header::COOKIE, &cookie)
            .json(&json!({"method": "next"}))
            .send()
            .await
            .unwrap();
        assert_eq!(no_marker.status(), 403);
        let oversize = h
            .post("/api/rpc", json!({"method": "search", "params": {"query": "x".repeat(70_000)}}))
            .header(header::COOKIE, &cookie)
            .send()
            .await
            .unwrap();
        assert_eq!(oversize.status(), 413);
        h.service.stop();
    }

    #[tokio::test]
    async fn page_is_served_with_its_csp() {
        let h = harness(false).await;
        let page = h.http.get(format!("{}/remote", h.base)).send().await.unwrap();
        assert_eq!(page.status(), 200);
        let csp = page.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap().to_string();
        assert!(csp.contains("https://resources.tidal.com") && csp.contains("frame-ancestors 'none'"));
        let root = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(format!("{}/", h.base))
            .send()
            .await
            .unwrap();
        assert_eq!(root.status(), 303);
        let js = h.http.get(format!("{}/assets/app.js", h.base)).send().await.unwrap();
        assert_eq!(js.status(), 200);
        assert!(js.headers()[header::CACHE_CONTROL].to_str().unwrap().contains("immutable"));
        let shell = h.http.get(format!("{}/index.html", h.base)).send().await.unwrap();
        assert_eq!(shell.status(), 404);
        h.service.stop();
    }

    #[tokio::test]
    async fn not_logged_in_while_the_desktop_is_signed_out() {
        let h = harness(false).await;
        let cookie = h.pair().await;
        h.session.logged_in.store(false, Ordering::SeqCst);
        let (_, body) = h.rpc(&cookie, "next", Value::Null).await;
        assert_eq!(body["error"], "not_logged_in: sign in on the PC");
        let (_, body) = h.rpc(&cookie, "get_remote_state", Value::Null).await;
        assert_eq!(body["result"]["loggedIn"], false);
        h.service.stop();
    }

    #[tokio::test]
    async fn sse_snapshot_then_delta_then_revoke() {
        let h = harness(false).await;
        let cookie = h.pair().await;
        let mut stream = h
            .http
            .get(format!("{}/api/events", h.base))
            .header(header::COOKIE, &cookie)
            .send()
            .await
            .unwrap();
        assert_eq!(stream.status(), 200);
        let first = read_events(&mut stream, 1).await;
        assert_eq!(first[0].0, "snapshot");
        assert!(first[0].1.contains("\"queueLen\":3"));
        assert_eq!(h.service.connected(), 1);

        h.service.on_engine_event(&oxitide_audio::engine::EngineEvent::Error {
            code: "net".into(),
            message: "down".into(),
        });
        let delta = read_events(&mut stream, 1).await;
        assert_eq!(delta[0].0, "error");

        let device = h.devices.list()[0].id.clone();
        assert!(h.devices.revoke(&device).unwrap());
        h.service.device_revoked(&device);
        let last = read_events(&mut stream, 1).await;
        assert_eq!(last[0].0, "revoked");
        let (status, _) = h.rpc(&cookie, "next", Value::Null).await;
        assert_eq!(status, 401);
        h.service.stop();
    }

    #[tokio::test]
    async fn disabling_closes_the_stream() {
        let h = harness(false).await;
        let cookie = h.pair().await;
        let mut stream = h
            .http
            .get(format!("{}/api/events", h.base))
            .header(header::COOKIE, &cookie)
            .send()
            .await
            .unwrap();
        read_events(&mut stream, 1).await;
        h.service.stop();
        let last = read_events(&mut stream, 1).await;
        assert_eq!(last[0].0, "revoked");
        let end = tokio::time::timeout(Duration::from_secs(5), stream.chunk()).await.unwrap();
        assert!(matches!(end, Ok(None) | Err(_)), "the stream ends");
    }

    #[test]
    fn port_fallback_skips_a_taken_port() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (first, port) = bind(0).unwrap();
            let (second, next) = bind(port).unwrap();
            assert_ne!(port, next);
            assert!(next > port && next < port + PORT_ATTEMPTS);
            drop((first, second));
        });
    }
}
