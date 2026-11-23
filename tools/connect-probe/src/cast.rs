//! One scripted TIDAL Connect session, mirroring what TIDAL desktop 2.48.0
//! sends (see `docs/superpowers/plans/notes/2026-11-23-tidal-connect-sender-spike.md`).

use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

use crate::discover;
use crate::interrupt;
use crate::log::Log;
use crate::scrub::summarize;
use crate::tidal::{self, CloudQueue, Session, Track};
use crate::tls;

/// TIDAL's controller pings every 15 s and gives up after 31 s of silence.
const PING_EVERY: Duration = Duration::from_secs(15);

pub struct CastOptions {
    pub device: Option<String>,
    pub tracks: Vec<u64>,
    pub keyring_service: String,
    pub quality: String,
    pub discover_seconds: u64,
}

/// What the session showed; printed as the summary at the end.
#[derive(Default, Debug)]
pub struct Findings {
    pub device: Option<String>,
    pub tls_ok: Option<bool>,
    pub session_started: bool,
    pub session_id: Option<String>,
    pub states: Vec<String>,
    pub media_ids: Vec<String>,
    pub server_errors: Vec<String>,
    pub request_results: Vec<String>,
    pub session_errors: Vec<String>,
    pub closed_by_device: bool,
    pub autoplay: Option<bool>,
    /// `endSession` (or the final `stop`) was sent.
    pub end_sent: bool,
    pub interrupted: bool,
    /// Each transport command and whether the device's state followed it.
    pub transport: Vec<(&'static str, bool)>,
    pub first_track: Option<String>,
    pub second_track: Option<String>,
    pub queue_created: bool,
    pub queue_deleted: bool,
}

impl Findings {
    pub fn saw_state(&self, state: &str) -> bool {
        self.states.iter().any(|s| s.eq_ignore_ascii_case(state))
    }
}

struct Endpoint {
    host: String,
    port: u16,
    label: String,
}

async fn pick_device(log: &Log, opts: &CastOptions) -> Result<Endpoint> {
    if let Some(spec) = &opts.device {
        if let Some((host, port)) = spec.rsplit_once(':') {
            if let Ok(port) = port.parse::<u16>() {
                return Ok(Endpoint {
                    host: host.to_string(),
                    port,
                    label: format!("{host}:{port}"),
                });
            }
        }
    }
    let devices = discover::discover(log, opts.discover_seconds).await?;
    let wanted = opts.device.as_deref().map(str::to_lowercase);
    let matching: Vec<_> = devices
        .iter()
        .filter(|d| match &wanted {
            Some(w) => d.name.to_lowercase().contains(w),
            None => true,
        })
        .collect();
    match matching.as_slice() {
        [one] => Ok(Endpoint {
            host: one.host.clone(),
            port: one.port,
            label: one.label(),
        }),
        [] if devices.is_empty() => bail!(
            "no TIDAL Connect device found. Check that this PC is on the same network as the \
             device (not a guest network), that the device is on, and that Windows Firewall \
             allowed this program on private networks."
        ),
        [] => bail!("no device matches --device; see the list above"),
        _ => {
            bail!("more than one device found; rerun with --device \"<name>\" (see the list above)")
        }
    }
}

pub async fn run(log: &Log, opts: &CastOptions, findings: &mut Findings) -> Result<()> {
    let login = tidal::load_login(&opts.keyring_service)?;
    log.scrubber.add(&login.access_token, "token");
    match login.minutes_left {
        Some(m) => log.line(format!("OxiTide login found (valid for {m} more minutes).")),
        None => log.line("OxiTide login found."),
    }
    let http = tidal::http();
    let session = tidal::fetch_session(&http, &login.access_token).await?;
    log.scrubber.add(&session.user_id.to_string(), "user-id");
    log.scrubber.add(&session.session_id, "session-id");
    log.line("TIDAL account session OK.");

    let endpoint = pick_device(log, opts).await?;
    findings.device = Some(endpoint.label.clone());

    let tracks = tidal::pick_tracks(
        &http,
        &login.access_token,
        &session.country_code,
        &opts.tracks,
    )
    .await?;
    for t in &tracks {
        log.line(format!(
            "  track {}: {} - {}",
            t.id,
            t.title,
            t.artists.join(", ")
        ));
    }
    let queue =
        tidal::create_queue(&http, &login.access_token, &session.country_code, &tracks).await?;
    findings.queue_created = true;
    let matches = queue.items.len() == tracks.len()
        && queue
            .items
            .iter()
            .zip(&tracks)
            .all(|(item, t)| item.media_id == t.id.to_string());
    log.line(format!(
        "Cloud queue created ({} items, position {}, items match the tracks: {}).",
        queue.items.len(),
        queue.position,
        if matches { "yes" } else { "no" }
    ));

    // An interrupt makes `drive` skip ahead to `endSession`; the queue is
    // deleted below either way.
    let outcome = if interrupt::is_set() {
        Ok(())
    } else {
        drive(
            log,
            &endpoint,
            &login.access_token,
            &session,
            &tracks,
            &queue,
            opts,
            findings,
        )
        .await
    };
    if interrupt::is_set() {
        findings.interrupted = true;
        log.line("Interrupted; cleaned up the device session, deleting the cloud queue.");
    }

    match tidal::delete_queue(&http, &login.access_token, &session.country_code, &queue.id).await {
        Ok(()) => {
            findings.queue_deleted = true;
            log.line("Cloud queue deleted.");
        }
        Err(e) => log.line(format!("WARNING: could not delete the cloud queue: {e}")),
    }
    outcome
}

type Sink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type Stream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

struct Conn<'a> {
    log: &'a Log,
    sink: Sink,
    stream: Stream,
    next_request: u64,
    next_ping: Instant,
    closed: bool,
    /// Set for the closing `endSession`, which must run after an interrupt.
    ignore_interrupt: bool,
}

impl Conn<'_> {
    async fn send(&mut self, mut msg: Value, describe: &str, with_request_id: bool) -> Result<()> {
        if with_request_id {
            msg["requestId"] = json!(self.next_request);
            self.log
                .line(format!("-> {describe} (requestId {})", self.next_request));
            self.next_request += 1;
        } else {
            self.log.line(format!("-> {describe}"));
        }
        self.sink.send(Message::text(msg.to_string())).await?;
        Ok(())
    }

    /// Reads device messages for up to `limit`, or until `done` holds.
    async fn observe(
        &mut self,
        limit: Duration,
        findings: &mut Findings,
        done: impl Fn(&Findings) -> bool,
    ) -> Result<()> {
        let deadline = Instant::now() + limit;
        let interrupted = |c: &Self| !c.ignore_interrupt && interrupt::is_set();
        while !self.closed && !done(findings) && !interrupted(self) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            if now >= self.next_ping {
                self.sink.send(Message::Ping(Default::default())).await?;
                self.next_ping = now + PING_EVERY;
            }
            let wait = deadline.min(self.next_ping) - now;
            let next = if self.ignore_interrupt {
                tokio::time::timeout(wait, self.stream.next()).await
            } else {
                tokio::select! {
                    r = tokio::time::timeout(wait, self.stream.next()) => r,
                    _ = interrupt::wait() => break,
                }
            };
            let Ok(next) = next else {
                continue;
            };
            match next {
                None => {
                    self.log.line("<- connection closed");
                    self.closed = true;
                    findings.closed_by_device = true;
                }
                Some(Err(e)) => {
                    self.log.line(format!("<- connection error: {e}"));
                    self.closed = true;
                    findings.closed_by_device = true;
                }
                Some(Ok(Message::Text(text))) => self.on_text(text.as_str(), findings),
                Some(Ok(Message::Close(frame))) => {
                    self.log.line(format!(
                        "<- close frame {:?}",
                        frame.map(|f| (u16::from(f.code), f.reason.to_string()))
                    ));
                    self.closed = true;
                    findings.closed_by_device = true;
                }
                Some(Ok(Message::Binary(b))) => self
                    .log
                    .line(format!("<- binary message ({} bytes)", b.len())),
                Some(Ok(_)) => {} // ping/pong frames
            }
        }
        Ok(())
    }

    fn on_text(&self, text: &str, findings: &mut Findings) {
        let Ok(msg) = serde_json::from_str::<Value>(text) else {
            self.log
                .line(format!("<- non-JSON text message ({} bytes)", text.len()));
            return;
        };
        let command = msg.get("command").and_then(Value::as_str).unwrap_or("");
        if command == "notifySessionStarted" {
            if let Some(id) = msg.get("sessionId") {
                let id = match id {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                self.log.scrubber.add(&id, "connect-session");
                findings.session_id = Some(id);
            }
            findings.session_started = true;
        }
        self.log.line(format!("<- {}", summarize(&msg)));
        match command {
            "notifyPlayerStatusChanged" => {
                if let Some(state) = msg.get("playerState").and_then(Value::as_str) {
                    findings.states.push(state.to_string());
                }
            }
            "notifyMediaChanged" => {
                if let Some(id) = msg.pointer("/mediaInfo/mediaId") {
                    findings.media_ids.push(match id {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    });
                }
            }
            "notifyContentServerError" | "notifyQueueServerError" => {
                findings
                    .server_errors
                    .push(format!("{command}: {}", summarize(&msg)));
            }
            "notifySessionError" => findings.session_errors.push(summarize(&msg).to_string()),
            "notifyRequestResult" => findings.request_results.push(summarize(&msg).to_string()),
            _ => {}
        }
    }
}

/// The `contentServerInfo` / `queueServerInfo` object of `loadCloudQueue`.
/// No refresh token, as TIDAL desktop sends it.
fn server_info(server_url: &str, token: &str, query: Value) -> Value {
    json!({
        "authInfo": {
            "oauthServerInfo": {
                "authInfo": {
                    "headerAuth": format!("Bearer {token}"),
                    "oauthParameters": {"accessToken": token, "refreshToken": ""},
                },
                "formParameters": {"grant_type": "switch_client", "scope": "r_usr"},
                "httpHeaderFields": [],
                "serverUrl": tidal::AUTH_SERVER_URL,
            }
        },
        "httpHeaderFields": [],
        "queryParameters": query,
        "serverUrl": server_url,
    })
}

fn current_media_info(queue: &CloudQueue, tracks: &[Track]) -> Value {
    let pos = queue.position.min(tracks.len().saturating_sub(1));
    let track = &tracks[pos];
    let item_id = queue
        .items
        .get(pos)
        .map(|i| i.id.clone())
        .unwrap_or_default();
    let mut metadata = json!({
        "albumTitle": track.album_title,
        "artists": track.artists,
        "duration": track.duration_s * 1000,
        "title": track.title,
    });
    let images = track.images();
    if !images.is_null() {
        metadata["images"] = images;
    }
    json!({
        "itemId": item_id,
        "mediaId": track.id.to_string(),
        "mediaType": 0,
        "metadata": metadata,
    })
}

#[allow(clippy::too_many_arguments)]
async fn drive(
    log: &Log,
    endpoint: &Endpoint,
    token: &str,
    session: &Session,
    tracks: &[Track],
    queue: &CloudQueue,
    opts: &CastOptions,
    findings: &mut Findings,
) -> Result<()> {
    let (config, verifier) = tls::client_config()?;
    let url = format!("wss://{}:{}", endpoint.host, endpoint.port);
    log.line(format!(
        "Connecting to {} (TLS, trusting only TIDAL's Connect CA)...",
        endpoint.label
    ));
    let connect = tokio_tungstenite::connect_async_tls_with_config(
        url.as_str(),
        None,
        false,
        Some(Connector::Rustls(config)),
    );
    let ws = match tokio::time::timeout(Duration::from_secs(10), connect).await {
        Err(_) => bail!("timed out connecting to the device"),
        Ok(Err(e)) => {
            if let Some(failure) = verifier.failure() {
                findings.tls_ok = Some(false);
                log.line(failure);
                log.line("The device presented this certificate chain:");
                for cert in verifier.presented() {
                    log.plain(tls::to_pem(&cert));
                }
                bail!("TLS verification failed; nothing was sent to the device");
            }
            bail!("could not connect to the device: {e}");
        }
        Ok(Ok((ws, _response))) => ws,
    };
    findings.tls_ok = Some(true);
    log.line(format!(
        "TLS OK: the device certificate chains to TIDAL's CA ({} certificate(s) presented).",
        verifier.presented().len()
    ));
    let (sink, stream) = ws.split();
    let mut conn = Conn {
        log,
        sink,
        stream,
        next_request: 0,
        next_ping: Instant::now() + PING_EVERY,
        closed: false,
        ignore_interrupt: false,
    };

    let steps = session_steps(&mut conn, token, session, tracks, queue, opts, findings).await;

    // 4. End the session and stop playback on the device, also after an
    // interrupt or a failed step.
    if !conn.closed {
        conn.ignore_interrupt = true;
        findings.end_sent = true;
        let sent = match findings.session_id.clone() {
            Some(id) => {
                conn.send(
                    json!({"command": "endSession", "sessionId": id, "stopCasting": true}),
                    "endSession (stopCasting)",
                    false,
                )
                .await
            }
            None => conn.send(json!({"command": "stop"}), "stop", true).await,
        };
        if sent.is_ok() {
            let wait = if interrupt::is_set() { 1 } else { 3 };
            let _ = conn
                .observe(Duration::from_secs(wait), findings, |_| false)
                .await;
        }
        let _ = conn.sink.send(Message::Close(None)).await;
    }
    steps
}

async fn session_steps(
    conn: &mut Conn<'_>,
    token: &str,
    session: &Session,
    tracks: &[Track],
    queue: &CloudQueue,
    opts: &CastOptions,
    findings: &mut Findings,
) -> Result<()> {
    let log = conn.log;
    let stop = |conn: &Conn<'_>| conn.closed || interrupt::is_set();

    // 1. Session. TIDAL desktop sends the user id as the session credential.
    conn.send(
        json!({
            "appId": "tidal",
            "appName": "tidal",
            "command": "startSession",
            "sessionCredential": session.user_id.to_string(),
        }),
        "startSession",
        false,
    )
    .await?;
    conn.observe(Duration::from_secs(10), findings, |f| f.session_started)
        .await?;
    if stop(conn) {
        return Ok(());
    }
    if !findings.session_started {
        log.line("No notifySessionStarted within 10 s; continuing anyway.");
    }

    // 2. The cloud queue, with server info the device uses to fetch it.
    let first = current_media_info(queue, tracks);
    findings.first_track = first
        .get("mediaId")
        .and_then(Value::as_str)
        .map(str::to_string);
    findings.second_track = tracks.get(1).map(|t| t.id.to_string());
    conn.send(
        json!({
            "autoplay": true,
            "command": "loadCloudQueue",
            "contentServerInfo": server_info(
                tidal::CONTENT_SERVER_URL,
                token,
                json!({"audiomode": "STEREO", "audioquality": opts.quality}),
            ),
            "currentMediaInfo": first,
            "position": 0,
            "queueInfo": {
                "maxAfterSize": 10,
                "maxBeforeSize": 10,
                "queueId": queue.id,
                "repeatMode": queue.repeat_mode,
                "shuffled": queue.shuffled,
            },
            "queueServerInfo": server_info(tidal::QUEUE_SERVER_URL, token, json!({})),
        }),
        &format!(
            "loadCloudQueue ({} tracks, first {}, quality {})",
            tracks.len(),
            tracks[0].id,
            opts.quality
        ),
        true,
    )
    .await?;
    conn.observe(Duration::from_secs(10), findings, |f| {
        f.saw_state("PLAYING")
    })
    .await?;
    if stop(conn) {
        return Ok(());
    }
    // TIDAL's own receiver binary loads the queue paused despite `autoplay`
    // in some runs (bench, 2026-11-23); an explicit play is what TIDAL's play
    // button sends.
    if !findings.saw_state("PLAYING") {
        log.line("Not playing after 10 s; sending an explicit play.");
        findings.autoplay = Some(false);
        conn.send(json!({"command": "play"}), "play", true).await?;
        conn.observe(Duration::from_secs(20), findings, |f| {
            f.saw_state("PLAYING")
        })
        .await?;
        if !findings.saw_state("PLAYING") {
            log.line("The device did not report PLAYING.");
        }
    } else if findings.saw_state("PLAYING") {
        findings.autoplay = Some(true);
    }
    conn.observe(Duration::from_secs(10), findings, |_| false)
        .await?;

    // 3. Transport, as described to the tester: pause, play, skip.
    for (command, wait) in [("pause", 6), ("play", 6), ("next", 15)] {
        if stop(conn) {
            break;
        }
        let states_before = findings.states.len();
        let media_before = findings.media_ids.len();
        conn.send(json!({"command": command}), command, true)
            .await?;
        conn.observe(Duration::from_secs(wait), findings, |_| false)
            .await?;
        let states_after = &findings.states[states_before..];
        let reached = |state: &str| states_after.iter().any(|s| s.eq_ignore_ascii_case(state));
        let worked = match command {
            "pause" => reached("PAUSED"),
            "play" => reached("PLAYING"),
            _ => findings.media_ids[media_before..]
                .iter()
                .any(|id| Some(id) == findings.second_track.as_ref()),
        };
        if !interrupt::is_set() {
            findings.transport.push((command, worked));
        }
    }
    Ok(())
}
