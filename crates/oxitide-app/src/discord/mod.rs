use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps};
use discord_rich_presence::error::Error;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::media_controls::{PlayState, SessionSender, SessionUpdate};

mod presence;

use presence::{plan_action, Action, ActivitySpec, Desired, Sent};

pub const DISCORD_CLIENT_ID: &str = "1554830262083264532";

const CHANNEL_CAPACITY: usize = 64;
const LOOP_TIMEOUT: Duration = Duration::from_millis(500);
const BACKOFF_START: Duration = Duration::from_secs(5);
const BACKOFF_CAP: Duration = Duration::from_secs(60);
const SEND_FAILURE_DELAY: Duration = Duration::from_secs(1);
const BUTTON_LABEL: &str = "Listen on TIDAL";

pub struct DiscordPresence {
    tx: Option<SyncSender<SessionUpdate>>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl DiscordPresence {
    pub fn spawn(enabled: bool) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<SessionUpdate>(CHANNEL_CAPACITY);
        let flag = Arc::new(AtomicBool::new(enabled));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_flag = flag.clone();
        let worker_shutdown = shutdown.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-discord".into())
            .spawn(move || worker(rx, worker_flag, worker_shutdown))
            .map_err(|e| e.to_string())?;
        Ok(Self { tx: Some(tx), enabled: flag, shutdown, handle: Some(handle) })
    }

    pub fn sender(&self) -> SessionSender {
        SessionSender::new(self.tx.as_ref().expect("live sender").clone())
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

impl Drop for DiscordPresence {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn apply(desired: &mut Desired, update: SessionUpdate) {
    match update {
        SessionUpdate::Track(track) => {
            desired.track = track;
            desired.position_ms = 0;
            if desired.track.is_none() {
                desired.play = PlayState::Stopped;
            }
        }
        SessionUpdate::Playback(play) => desired.play = play,
        SessionUpdate::Position { position_ms, .. } => desired.position_ms = position_ms,
    }
}

fn drain_one(client: &mut DiscordIpcClient) -> Result<(), Error> {
    for _ in 0..3 {
        match client.recv()? {
            (3, payload) => client.send(payload, 4)?,
            _ => return Ok(()),
        }
    }
    Ok(())
}

fn set_activity(
    client: &mut DiscordIpcClient,
    spec: &ActivitySpec,
) -> Result<(), Error> {
    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .details(spec.details.as_str())
        .state(spec.state.as_str())
        .assets(
            Assets::new()
                .large_image(spec.large_image.as_str())
                .large_text(spec.large_text.as_str())
                .small_image(spec.small_image)
                .small_text(spec.small_text),
        )
        .buttons(vec![Button::new(BUTTON_LABEL, spec.button_url.as_str())]);
    if let Some((start, end)) = spec.timestamps {
        activity = activity.timestamps(Timestamps::new().start(start).end(end));
    }
    client.set_activity(activity)?;
    drain_one(client)
}

fn worker(rx: Receiver<SessionUpdate>, enabled: Arc<AtomicBool>, shutdown: Arc<AtomicBool>) {
    if DISCORD_CLIENT_ID.is_empty() {
        let mut warned = false;
        while !shutdown.load(Ordering::Relaxed) {
            if !warned && enabled.load(Ordering::Relaxed) {
                tracing::warn!("Discord presence disabled: client ID not configured");
                warned = true;
            }
            std::thread::sleep(LOOP_TIMEOUT);
        }
        return;
    }

    let mut client: Option<DiscordIpcClient> = None;
    let mut desired = Desired { track: None, play: PlayState::Stopped, position_ms: 0 };
    let mut sent = Sent::NEVER;
    let mut backoff = BACKOFF_START;
    let mut next_connect = Instant::now();

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let want = enabled.load(Ordering::Relaxed);
        if !want && sent.shown {
            match client.as_mut().map(|c| c.clear_activity()) {
                Some(Ok(())) => sent = Sent::NEVER,
                Some(Err(e)) => {
                    tracing::debug!("Discord presence clear failed: {e}");
                    client = None;
                    next_connect = Instant::now() + SEND_FAILURE_DELAY;
                }
                None => sent = Sent::NEVER,
            }
        }
        if want && client.is_none() && Instant::now() >= next_connect {
            let mut fresh = DiscordIpcClient::new(DISCORD_CLIENT_ID);
            match fresh.connect() {
                Ok(()) => {
                    tracing::info!("Discord presence connected");
                    client = Some(fresh);
                    sent = Sent::NEVER;
                    backoff = BACKOFF_START;
                }
                Err(e) => {
                    tracing::debug!("Discord presence unavailable: {e}");
                    next_connect = Instant::now() + backoff;
                    backoff = (backoff * 2).min(BACKOFF_CAP);
                }
            }
        }

        match rx.recv_timeout(LOOP_TIMEOUT) {
            Ok(update) => {
                apply(&mut desired, update);
                while let Ok(update) = rx.try_recv() {
                    apply(&mut desired, update);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if want && client.is_some() {
            let now = now_ms();
            match plan_action(&desired, &sent, now) {
                Action::None => {}
                Action::Clear => {
                    match client.as_mut().map(|c| c.clear_activity()) {
                        Some(Ok(())) => sent = Sent::NEVER,
                        Some(Err(e)) => {
                            tracing::debug!("Discord presence clear failed: {e}");
                            client = None;
                            next_connect = Instant::now() + SEND_FAILURE_DELAY;
                        }
                        None => {}
                    }
                }
                Action::Send(spec) => {
                    match client.as_mut().map(|c| set_activity(c, &spec)) {
                        Some(Ok(())) => {
                            sent = Sent {
                                track_id: desired.track.as_ref().map(|track| track.track_id),
                                play: desired.play,
                                start_ms: spec
                                    .timestamps
                                    .map(|(start, _)| start)
                                    .unwrap_or(now - desired.position_ms as i64),
                                at_ms: now,
                                shown: true,
                            };
                        }
                        Some(Err(e)) => {
                            tracing::debug!("Discord presence update failed: {e}");
                            client = None;
                            next_connect = Instant::now() + SEND_FAILURE_DELAY;
                        }
                        None => {}
                    }
                }
            }
        }
    }

    if let Some(client) = client.as_mut() {
        if sent.shown {
            let _ = client.clear_activity();
        }
        let _ = client.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_service_spawns_sends_and_drops() {
        let service = DiscordPresence::spawn(false).expect("spawn");
        service.sender().send(SessionUpdate::Playback(PlayState::Paused));
        let started = Instant::now();
        drop(service);
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
