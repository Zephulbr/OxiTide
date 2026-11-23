//! Stand-in host for TIDAL's own receiver binary (`tidal_connect_application.exe`,
//! shipped with TIDAL desktop): connects to its plain host socket on :9000 and
//! answers like TIDAL desktop's `RemoteDesktopController` would, so the probe
//! can be pointed at the binary on `127.0.0.1:2019` without hardware.
//! Local testing only; nothing here is meant for the tester.

use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::log::Log;
use crate::scrub::summarize;

pub async fn run(log: &Log, port: u16) -> Result<()> {
    let url = format!("ws://127.0.0.1:{port}");
    let started = Instant::now();
    let ws = loop {
        match tokio_tungstenite::connect_async(url.as_str()).await {
            Ok((ws, _)) => break ws,
            Err(e) if started.elapsed() < Duration::from_secs(60) => {
                log.line(format!("bench: {url} not ready ({e}); retrying"));
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => bail!("bench: could not reach the receiver binary at {url}: {e}"),
        }
    };
    log.line(format!(
        "bench: connected to the receiver binary's host socket {url}"
    ));
    let (mut sink, mut stream) = ws.split();
    let mut ping = tokio::time::interval(Duration::from_secs(15));
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut playing = false;
    let mut progress_ms: u64 = 0;

    loop {
        let reply: Vec<Value> = tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log.line("bench: stopping (Ctrl+C)");
                let _ = sink.send(Message::Close(None)).await;
                return Ok(());
            }
            _ = ping.tick() => {
                sink.send(Message::Ping(Default::default())).await?;
                continue;
            }
            _ = tick.tick() => {
                if !playing { continue; }
                progress_ms += 1000;
                vec![json!({"command": "onProgressUpdated", "duration": 0, "progress": progress_ms, "type": "media"})]
            }
            next = stream.next() => match next {
                None => { log.line("bench: host socket closed"); return Ok(()); }
                Some(Err(e)) => { log.line(format!("bench: host socket error: {e}")); return Ok(()); }
                Some(Ok(Message::Text(text))) => {
                    let Ok(msg) = serde_json::from_str::<Value>(text.as_str()) else { continue; };
                    log.line(format!("bench <- {}", summarize(&msg)));
                    let status = |s: &str| json!({"command": "onStatusUpdated", "playerState": s, "type": "media"});
                    match msg.get("command").and_then(Value::as_str).unwrap_or("") {
                        "requestResources" => vec![json!({"command": "grantResources"})],
                        "prepare" => {
                            let id = msg.pointer("/mediaInfo/mediaId").cloned().unwrap_or(Value::Null);
                            log.line(format!("bench: PREPARE track {id}"));
                            progress_ms = 0;
                            vec![status("buffering"), status("paused")]
                        }
                        "play" => { playing = true; vec![status("playing")] }
                        "pause" => { playing = false; vec![status("paused")] }
                        "stop" => { playing = false; progress_ms = 0; vec![status("idle")] }
                        "seek" => {
                            progress_ms = msg.get("position").and_then(Value::as_u64).unwrap_or(0);
                            vec![]
                        }
                        _ => vec![],
                    }
                }
                Some(Ok(_)) => continue,
            },
        };
        for msg in reply {
            if msg["command"] != "onProgressUpdated" {
                log.line(format!("bench -> {}", summarize(&msg)));
            }
            sink.send(Message::text(msg.to_string())).await?;
        }
    }
}
