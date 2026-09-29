use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use oxitide_audio::engine::Engine;
use windows::core::{factory, HSTRING};
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
    SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

use super::{cover_url, to_engine_cmd, ControlCommand, PlayState, SessionSender, SessionUpdate, TrackMeta};

const THUMBNAIL_SIZE: u32 = 320;
const CHANNEL_CAPACITY: usize = 64;

pub struct MediaSession {
    tx: Option<SyncSender<SessionUpdate>>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MediaSession {
    pub fn spawn(engine: Engine, hwnd: isize, enabled: bool) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<SessionUpdate>(CHANNEL_CAPACITY);
        let flag = Arc::new(AtomicBool::new(enabled));
        let worker_flag = flag.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-smtc".into())
            .spawn(move || worker(engine, hwnd, rx, worker_flag, worker_shutdown))
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

struct Smtc {
    controls: SystemMediaTransportControls,
    updater: windows::Media::SystemMediaTransportControlsDisplayUpdater,
    timeline: SystemMediaTransportControlsTimelineProperties,
}

fn setup(engine: &Engine, hwnd: isize) -> windows::core::Result<Smtc> {
    let interop: ISystemMediaTransportControlsInterop =
        factory::<SystemMediaTransportControls, _>()?;
    let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(HWND(hwnd as _))? };
    controls.SetIsPlayEnabled(true)?;
    controls.SetIsPauseEnabled(true)?;
    controls.SetIsNextEnabled(true)?;
    controls.SetIsPreviousEnabled(true)?;
    let updater = controls.DisplayUpdater()?;
    updater.SetType(MediaPlaybackType::Music)?;

    let buttons = engine.clone();
    let button_handler = TypedEventHandler::new(
        move |_, args: windows::core::Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
            let args = args.ok()?;
            dispatch_button(&buttons, args.Button()?);
            Ok(())
        },
    );
    controls.ButtonPressed(&button_handler)?;

    let seeker = engine.clone();
    let position_handler = TypedEventHandler::new(
        move |_, args: windows::core::Ref<'_, PlaybackPositionChangeRequestedEventArgs>| {
            let args = args.ok()?;
            let requested = Duration::from(args.RequestedPlaybackPosition()?);
            let _ = seeker.handle(to_engine_cmd(ControlCommand::Seek(requested.as_millis() as u64)));
            Ok(())
        },
    );
    controls.PlaybackPositionChangeRequested(&position_handler)?;

    Ok(Smtc {
        controls,
        updater,
        timeline: SystemMediaTransportControlsTimelineProperties::new()?,
    })
}

fn dispatch_button(engine: &Engine, button: SystemMediaTransportControlsButton) {
    let command = if button == SystemMediaTransportControlsButton::Play
        || button == SystemMediaTransportControlsButton::Pause
    {
        ControlCommand::PlayPause
    } else if button == SystemMediaTransportControlsButton::Next {
        ControlCommand::Next
    } else if button == SystemMediaTransportControlsButton::Previous {
        ControlCommand::Prev
    } else {
        return;
    };
    let _ = engine.handle(to_engine_cmd(command));
}

fn worker(
    engine: Engine,
    hwnd: isize,
    rx: Receiver<SessionUpdate>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
    let mut smtc: Option<Smtc> = None;
    let mut shown = false;
    let mut last_track: Option<TrackMeta> = None;
    let mut last_play = PlayState::Stopped;
    let mut last_position = (0u64, 0u64);
    let mut msg = MSG::default();

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let want = enabled.load(Ordering::Relaxed);
        if want && smtc.is_none() {
            match setup(&engine, hwnd) {
                Ok(created) => smtc = Some(created),
                Err(e) => {
                    tracing::warn!("SMTC unavailable: {e}");
                    enabled.store(false, Ordering::Relaxed);
                }
            }
        }
        if let Some(s) = smtc.as_mut() {
            if want != shown {
                if let Err(e) = s.controls.SetIsEnabled(want) {
                    tracing::warn!("SMTC enable failed: {e}");
                }
                shown = want;
                if want {
                    let _ = apply_track(s, last_track.as_ref());
                    let _ = s.controls.SetPlaybackStatus(playback_status(last_play));
                    let _ = apply_position(s, last_position.0, last_position.1);
                } else {
                    let _ = s.updater.ClearAll();
                }
            }
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(update) => {
                if let (Some(s), true) = (smtc.as_mut(), want) {
                    match &update {
                        SessionUpdate::Track(track) => {
                            if let Err(e) = apply_track(s, track.as_ref()) {
                                tracing::warn!("SMTC metadata failed: {e}");
                            }
                            if let Some(track) = track {
                                if let Err(e) = apply_position(s, 0, track.duration_ms) {
                                    tracing::warn!("SMTC timeline failed: {e}");
                                }
                            }
                        }
                        SessionUpdate::Playback(state) => {
                            if let Err(e) = s.controls.SetPlaybackStatus(playback_status(*state)) {
                                tracing::warn!("SMTC status failed: {e}");
                            }
                        }
                        SessionUpdate::Position { position_ms, duration_ms } => {
                            if let Err(e) = apply_position(s, *position_ms, *duration_ms) {
                                tracing::warn!("SMTC timeline failed: {e}");
                            }
                        }
                    }
                }
                match update {
                    SessionUpdate::Track(track) => last_track = track,
                    SessionUpdate::Playback(state) => last_play = state,
                    SessionUpdate::Position { position_ms, duration_ms } => {
                        last_position = (position_ms, duration_ms);
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    if let Some(s) = smtc.as_ref() {
        let _ = s.controls.SetIsEnabled(false);
        let _ = s.updater.ClearAll();
    }
}

fn apply_track(smtc: &mut Smtc, track: Option<&TrackMeta>) -> windows::core::Result<()> {
    smtc.updater.ClearAll()?;
    let Some(track) = track else {
        return smtc.updater.Update();
    };
    smtc.updater.SetType(MediaPlaybackType::Music)?;
    let properties = smtc.updater.MusicProperties()?;
    properties.SetTitle(&HSTRING::from(track.title.as_str()))?;
    properties.SetArtist(&HSTRING::from(track.artist.as_str()))?;
    properties.SetAlbumTitle(&HSTRING::from(track.album.as_str()))?;
    if let Some(url) = track.cover.as_deref().and_then(|cover| cover_url(cover, THUMBNAIL_SIZE)) {
        if let Ok(uri) = Uri::CreateUri(&HSTRING::from(url.as_str())) {
            if let Ok(stream) = RandomAccessStreamReference::CreateFromUri(&uri) {
                let _ = smtc.updater.SetThumbnail(&stream);
            }
        }
    }
    smtc.updater.Update()
}

fn apply_position(smtc: &mut Smtc, position_ms: u64, duration_ms: u64) -> windows::core::Result<()> {
    let start = TimeSpan::default();
    let end = TimeSpan::from(Duration::from_millis(duration_ms));
    let position = TimeSpan::from(Duration::from_millis(position_ms));
    smtc.timeline.SetStartTime(start)?;
    smtc.timeline.SetMinSeekTime(start)?;
    smtc.timeline.SetEndTime(end)?;
    smtc.timeline.SetMaxSeekTime(end)?;
    smtc.timeline.SetPosition(position)?;
    smtc.controls.UpdateTimelineProperties(&smtc.timeline)
}

fn playback_status(state: PlayState) -> MediaPlaybackStatus {
    match state {
        PlayState::Playing => MediaPlaybackStatus(3),
        PlayState::Paused => MediaPlaybackStatus(4),
        PlayState::Stopped => MediaPlaybackStatus(2),
    }
}
