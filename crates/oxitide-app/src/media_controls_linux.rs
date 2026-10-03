//! Linux MPRIS media controls (spec §4.3). This is the only D-Bus code; the
//! session-update fold, seek classification, and control gating live in
//! `media_controls.rs` so they are unit-tested on Windows.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mpris_server::zbus;
use mpris_server::{Metadata, PlaybackStatus, Player, Time, TrackId};
use oxitide_audio::engine::Engine;
use tauri::Manager;

use super::{
    apply_session_update, control_for_pause, control_for_play, control_for_stop, cover_url,
    seek_target, to_engine_cmd, track_path, ControlCommand, MprisEffect, MprisState, PlayState,
    SessionSender, SessionUpdate, TrackMeta,
};

const CHANNEL_CAPACITY: usize = 64;
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const THUMBNAIL_SIZE: u32 = 320;
const BUS_NAME_SUFFIX: &str = "OxiTide";
const IDENTITY: &str = "OxiTide";

/// Adapter-originated MPRIS signals, drained by the async loop so the sync
/// method callbacks never need to await.
enum AdapterEffect {
    Seeked(u64),
}

pub struct MediaSession {
    tx: Option<SyncSender<SessionUpdate>>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MediaSession {
    pub fn spawn(engine: Engine, enabled: bool, raise: tauri::AppHandle) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<SessionUpdate>(CHANNEL_CAPACITY);
        let flag = Arc::new(AtomicBool::new(enabled));
        let worker_flag = flag.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-mpris".into())
            .spawn(move || worker(engine, raise, rx, worker_flag, worker_shutdown))
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

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn worker(
    engine: Engine,
    raise: tauri::AppHandle,
    rx: Receiver<SessionUpdate>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::warn!("MPRIS disabled: {e}");
            enabled.store(false, Ordering::Relaxed);
            return;
        }
    };
    let failure = enabled.clone();
    let local_set = tokio::task::LocalSet::new();
    let serve = local_set.run_until(serve(engine, raise, rx, enabled, shutdown));
    if let Err(e) = runtime.block_on(serve) {
        tracing::warn!("MPRIS stopped: {e}");
        failure.store(false, Ordering::Relaxed);
    }
}

async fn serve(
    engine: Engine,
    raise: tauri::AppHandle,
    rx: Receiver<SessionUpdate>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> zbus::Result<()> {
    let player = Player::builder(BUS_NAME_SUFFIX)
        .can_play(true)
        .can_pause(true)
        .can_go_previous(true)
        .can_go_next(true)
        .build()
        .await?;
    let state = Arc::new(Mutex::new(MprisState::default()));
    let (effect_tx, effect_rx) = mpsc::channel::<AdapterEffect>();
    connect(&player, &engine, &raise, &state, &effect_tx, &enabled);
    tokio::task::spawn_local(player.run());
    player.set_identity(IDENTITY).await?;
    player.set_desktop_entry(IDENTITY).await?;
    player.set_can_raise(true).await?;
    player.set_can_seek(true).await?;

    let mut shown = true;
    let mut last_position_at = Instant::now();
    while !shutdown.load(Ordering::Relaxed) {
        let want = enabled.load(Ordering::Relaxed);
        if want != shown {
            set_enabled(&player, want, &state).await;
            shown = want;
            last_position_at = Instant::now();
        }
        while let Ok(update) = rx.try_recv() {
            let elapsed_ms = last_position_at.elapsed().as_millis() as u64;
            let effect = {
                let mut snapshot = state.lock().unwrap();
                apply_session_update(&mut snapshot, &update, elapsed_ms)
            };
            last_position_at = Instant::now();
            if want {
                push_effect(&player, &state, effect).await?;
            }
        }
        while let Ok(effect) = effect_rx.try_recv() {
            if want {
                match effect {
                    AdapterEffect::Seeked(position_ms) => {
                        let position = Time::from_millis(position_ms as i64);
                        player.set_position(position);
                        player.seeked(position).await?;
                    }
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    Ok(())
}

async fn push_effect(
    player: &Player,
    state: &Mutex<MprisState>,
    effect: MprisEffect,
) -> zbus::Result<()> {
    match effect {
        MprisEffect::Track => {
            let metadata = {
                let snapshot = state.lock().unwrap();
                match &snapshot.track {
                    Some(track) => track_metadata(track),
                    None => empty_metadata(),
                }
            };
            player.set_metadata(metadata).await?;
            player.set_position(Time::ZERO);
            player.seeked(Time::ZERO).await?;
        }
        MprisEffect::Playback => {
            let status = {
                let snapshot = state.lock().unwrap();
                status_of(snapshot.status)
            };
            player.set_playback_status(status).await?;
        }
        MprisEffect::Position { seeked } => {
            let position = {
                let snapshot = state.lock().unwrap();
                Time::from_millis(snapshot.position_ms as i64)
            };
            player.set_position(position);
            if seeked {
                player.seeked(position).await?;
            }
        }
    }
    Ok(())
}

async fn set_enabled(player: &Player, enabled: bool, state: &Mutex<MprisState>) {
    let _ = player.set_can_play(enabled).await;
    let _ = player.set_can_pause(enabled).await;
    let _ = player.set_can_seek(enabled).await;
    let _ = player.set_can_go_next(enabled).await;
    let _ = player.set_can_go_previous(enabled).await;
    let _ = player.set_can_raise(enabled).await;
    if enabled {
        let snapshot = state.lock().unwrap().clone();
        let metadata = match &snapshot.track {
            Some(track) => track_metadata(track),
            None => empty_metadata(),
        };
        let _ = player.set_metadata(metadata).await;
        let _ = player.set_playback_status(status_of(snapshot.status)).await;
        player.set_position(Time::from_millis(snapshot.position_ms as i64));
    } else {
        let _ = player.set_metadata(empty_metadata()).await;
        let _ = player.set_playback_status(PlaybackStatus::Stopped).await;
    }
}

fn connect(
    player: &Player,
    engine: &Engine,
    raise: &tauri::AppHandle,
    state: &Arc<Mutex<MprisState>>,
    effects: &mpsc::Sender<AdapterEffect>,
    enabled: &Arc<AtomicBool>,
) {
    {
        let engine = engine.clone();
        let enabled = enabled.clone();
        player.connect_play_pause(move |_| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            let _ = engine.handle(to_engine_cmd(ControlCommand::PlayPause));
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        let enabled = enabled.clone();
        player.connect_play(move |_| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            dispatch_gated(&engine, &state, control_for_play);
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        let enabled = enabled.clone();
        player.connect_pause(move |_| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            dispatch_gated(&engine, &state, control_for_pause);
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        let enabled = enabled.clone();
        player.connect_stop(move |_| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            dispatch_gated(&engine, &state, control_for_stop);
        });
    }
    {
        let engine = engine.clone();
        let enabled = enabled.clone();
        player.connect_next(move |_| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            let _ = engine.handle(to_engine_cmd(ControlCommand::Next));
        });
    }
    {
        let engine = engine.clone();
        let enabled = enabled.clone();
        player.connect_previous(move |_| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            let _ = engine.handle(to_engine_cmd(ControlCommand::Prev));
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        let effects = effects.clone();
        let enabled = enabled.clone();
        player.connect_seek(move |_player, offset| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            let target = {
                let snapshot = state.lock().unwrap();
                seek_target(&snapshot, offset.as_millis())
            };
            issue_seek(&engine, &state, &effects, target);
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        let effects = effects.clone();
        let enabled = enabled.clone();
        player.connect_set_position(move |_player, track_id, position| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            let current = {
                let snapshot = state.lock().unwrap();
                snapshot.track.as_ref().map(|track| track.track_id)
            };
            if current.map(track_path).as_deref() != Some(track_id.as_str()) {
                return;
            }
            issue_seek(&engine, &state, &effects, position.as_millis().max(0) as u64);
        });
    }
    {
        let raise = raise.clone();
        let enabled = enabled.clone();
        player.connect_raise(move |_| {
            if !enabled.load(Ordering::Relaxed) {
                return;
            }
            let app = raise.clone();
            let window_handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = window_handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });
        });
    }
}

fn dispatch_gated(
    engine: &Engine,
    state: &Mutex<MprisState>,
    pick: fn(&MprisState) -> Option<ControlCommand>,
) {
    let command = {
        let snapshot = state.lock().unwrap();
        pick(&snapshot)
    };
    if let Some(command) = command {
        let _ = engine.handle(to_engine_cmd(command));
    }
}

fn issue_seek(
    engine: &Engine,
    state: &Mutex<MprisState>,
    effects: &mpsc::Sender<AdapterEffect>,
    target_ms: u64,
) {
    {
        let mut snapshot = state.lock().unwrap();
        snapshot.position_ms = target_ms;
    }
    let _ = engine.handle(to_engine_cmd(ControlCommand::Seek(target_ms)));
    let _ = effects.send(AdapterEffect::Seeked(target_ms));
}

fn track_metadata(track: &TrackMeta) -> Metadata {
    let mut builder = Metadata::builder()
        .trackid(track_id(track.track_id))
        .title(track.title.clone())
        .artist([track.artist.clone()])
        .album(track.album.clone())
        .length(Time::from_millis(track.duration_ms as i64));
    if let Some(url) =
        track.cover.as_deref().and_then(|cover| cover_url(cover, THUMBNAIL_SIZE))
    {
        builder = builder.art_url(url);
    }
    builder.build()
}

fn empty_metadata() -> Metadata {
    Metadata::builder().trackid(TrackId::NO_TRACK).build()
}

fn track_id(track_id: u64) -> TrackId {
    TrackId::try_from(track_path(track_id)).unwrap_or(TrackId::NO_TRACK)
}

fn status_of(state: PlayState) -> PlaybackStatus {
    match state {
        PlayState::Playing => PlaybackStatus::Playing,
        PlayState::Paused => PlaybackStatus::Paused,
        PlayState::Stopped => PlaybackStatus::Stopped,
    }
}
