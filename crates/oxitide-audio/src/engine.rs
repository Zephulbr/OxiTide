use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::cache::{CacheKey, DiskCache};
use crate::decode::{PcmBuffer, PcmFormat, PcmSink, StreamInfo};
use crate::dsp::{CrossfadeSettings, DspSettings, Tail, TrackDsp, STAGE_CROSSFADE};
use crate::error::{Error, Result};
use crate::fetch::{self, Fetcher, StreamBuffer};
use crate::meter::LevelTap;
use crate::output::{AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode, OutputStatus};
use crate::perf::{AudioPerfEvent, PerfHook};
use crate::queue::{Queue, QueueItem, RepeatMode};
use crate::seek::{self, Discarder};
use crate::source::{is_he_aac, StreamPlan};
use oxitide_tidal::stream::{Quality, ReplayGain};

pub trait StreamResolver: Send + Sync + 'static {
    fn resolve(&self, track_id: u64, quality: Quality) -> Result<StreamPlan>;
}

/// The stream that was actually served and decoded for the current track.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StreamMeta {
    /// The served/actual quality (`None` when undecidable).
    pub quality: Option<Quality>,
    /// Codec name from the manifest.
    pub codec: Option<String>,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub channels: u16,
    /// Manifest bandwidth, else computed from the fetched bytes.
    pub bitrate_kbps: Option<u32>,
    /// The ReplayGain applied to this track, in dB, while ReplayGain is on.
    #[serde(default)]
    pub replay_gain_db: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlaybackState {
    pub playing: bool,
    pub buffering: bool,
    pub current: Option<QueueItem>,
    pub current_index: Option<usize>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub preferred_quality: Quality,
    pub output_mode: OutputMode,
    pub bit_perfect: bool,
    pub bit_perfect_notice: Option<String>,
    /// True while a DSP stage (EQ, ReplayGain, crossfade) alters samples;
    /// `bit_perfect` is then false.
    pub dsp_active: bool,
    /// The stages altering samples, for display (e.g. `["ReplayGain", "EQ"]`).
    pub dsp_stages: Vec<&'static str>,
    /// Configured crossfade length (0 = off): a crossfading track hands over
    /// this long before its end.
    pub crossfade_ms: u32,
    pub queue: Vec<QueueItem>,
    pub stream: Option<StreamMeta>,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    State(PlaybackState),
    NowPlaying(Option<QueueItem>),
    Position { position_ms: u64, duration_ms: u64 },
    Error { code: String, message: String },
}

#[derive(Debug, Clone)]
pub enum EngineCmd {
    PlayTracks { items: Vec<QueueItem>, start: usize },
    /// Replace the queue with a fully shuffled context (header Shuffle).
    PlayShuffled { items: Vec<QueueItem> },
    /// Replace the queue with a persisted session and start paused: no output
    /// is opened and no media is fetched until the user presses play. The
    /// saved queue order is preserved even when `shuffle` is true.
    RestoreSession {
        items: Vec<QueueItem>,
        start: usize,
        position_ms: u64,
        shuffle: bool,
        repeat: RepeatMode,
    },
    PlayPause,
    Next,
    Prev,
    Seek(u64),
    SetVolume(f32),
    SetShuffle(bool),
    SetRepeat(RepeatMode),
    Reorder { from: usize, to: usize },
    Remove(usize),
    Enqueue { items: Vec<QueueItem> },
    Clear,
    /// Fade the output to silence over `duration_ms`, then pause at the current
    /// position. The pre-fade volume is restored for the next play; a
    /// play/pause or volume change cancels the fade.
    FadeAndPause { duration_ms: u64 },
    /// Restart the current track at its last position (after an error).
    Retry,
}

type Listener = Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// Number of decoded PCM buffers a track's producer may run ahead of the
/// worker. Bounds the lookahead (typically a few seconds of audio) so a long
/// track never buffers in memory without limit.
const PREFETCH_BUFFERS: usize = 96;

/// An in-flight sleep-timer fade: the worker ramps the output volume from
/// `base_volume` to zero over `duration`, then pauses and restores the volume.
struct FadeState {
    started: Instant,
    duration: Duration,
    base_volume: f32,
}

struct Inner {
    queue: Mutex<Queue>,
    resolver: Arc<dyn StreamResolver>,
    fetcher: OnceLock<Arc<dyn Fetcher>>,
    cache: Arc<DiskCache>,
    quality: Mutex<Quality>,
    /// Whether a lower-than-requested served quality plays or errors.
    allow_fallback: AtomicBool,
    /// The stream actually served and decoded for the current track.
    stream_meta: Mutex<Option<StreamMeta>>,
    device_id: Mutex<Option<String>>,
    output_mode: Mutex<OutputMode>,
    exclusive_fallback: Mutex<ExclusiveFallback>,
    output_status: Mutex<OutputStatus>,
    /// Sound-processing settings; `dsp_version` bumps on every change so the
    /// worker re-applies them live.
    dsp: Mutex<DspSettings>,
    dsp_version: AtomicU64,
    /// DSP stages currently altering samples (empty = untouched).
    dsp_stages: Mutex<Vec<&'static str>>,
    listeners: Mutex<Vec<Listener>>,
    perf_hook: Mutex<Option<PerfHook>>,
    /// Read-only meter tap (Hi-Fi Rack): every written buffer is offered to it.
    level_tap: Arc<LevelTap>,
    /// Serializes queue mutation against the worker's advance decision.
    /// Lock ordering: always take `playback_lock` before `queue`.
    playback_lock: Mutex<()>,
    /// Incremented to invalidate the running session; the worker restarts.
    generation: AtomicU64,
    paused: AtomicBool,
    volume: AtomicU32, // f32 bits
    /// Active sleep-timer fade; the worker ramps the output volume to zero and
    /// then pauses.
    fade: Mutex<Option<FadeState>>,
    position_ms: AtomicU64,
    /// Track id the current `position_ms` belongs to (0 = none).
    position_track: AtomicU64,
    /// Pending seek; consumed by the worker.
    seek_target: Mutex<Option<PendingSeek>>,
    /// True while the worker is running a session (used by `PlayPause` to know
    /// whether a resume needs a fresh generation).
    worker_busy: AtomicBool,
    wake_lock: Mutex<()>,
    wake: Condvar,
    shutdown: AtomicBool,
}

impl Inner {
    /// Wake the worker without losing the wakeup against a concurrent wait:
    /// the waiter holds `wake_lock` across its predicate check and then waits
    /// on the same lock, so notifying while holding it guarantees the waiter
    /// either sees the new state or is parked and woken.
    fn notify_worker(&self) {
        let _guard = self.wake_lock.lock().unwrap();
        self.wake.notify_all();
    }
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

/// The output status with active DSP folded in: processed samples are never
/// bit-perfect. The notice stays the output's own (DSP is an opted-in choice,
/// surfaced through `dsp_stages`, not a warning).
fn effective_status(inner: &Inner) -> (OutputStatus, Vec<&'static str>) {
    let mut status = inner.output_status.lock().unwrap().clone();
    let stages = inner.dsp_stages.lock().unwrap().clone();
    if !stages.is_empty() {
        status.bit_perfect = false;
    }
    (status, stages)
}

fn build_state(inner: &Inner) -> PlaybackState {
    let (status, dsp_stages) = effective_status(inner);
    let q = inner.queue.lock().unwrap();
    PlaybackState {
        playing: q.is_playing(),
        buffering: false,
        current: q.current().cloned(),
        current_index: q.current_index(),
        position_ms: inner.position_ms.load(Ordering::SeqCst),
        duration_ms: q.current().map(|i| i.duration_secs as u64 * 1000).unwrap_or(0),
        volume: q.volume(),
        shuffle: q.shuffle(),
        repeat: q.repeat(),
        preferred_quality: *inner.quality.lock().unwrap(),
        output_mode: status.mode,
        bit_perfect: status.bit_perfect,
        bit_perfect_notice: status.notice,
        dsp_active: !dsp_stages.is_empty(),
        dsp_stages,
        crossfade_ms: inner.dsp.lock().unwrap().crossfade.duration_ms,
        queue: q.items().to_vec(),
        stream: inner.stream_meta.lock().unwrap().clone(),
    }
}

/// The volume the output should have right now and whether a sleep fade is
/// driving it. A fade overrides the user's volume until it completes or is
/// cancelled; the output-reopen path uses this too, so a format change
/// mid-fade never jumps back to full volume.
fn current_volume(inner: &Inner) -> (f32, bool) {
    let fade = inner.fade.lock().unwrap();
    match fade.as_ref() {
        Some(f) => {
            let duration = f.duration.as_secs_f32().max(0.001);
            let t = (f.started.elapsed().as_secs_f32() / duration).min(1.0);
            (f.base_volume * (1.0 - t), true)
        }
        None => (f32::from_bits(inner.volume.load(Ordering::SeqCst)), false),
    }
}

fn emit(inner: &Arc<Inner>, event: EngineEvent) {
    if let EngineEvent::Error { code, message } = &event {
        tracing::error!("playback error ({code}): {message}");
    }
    for listener in inner.listeners.lock().unwrap().iter() {
        listener(event.clone());
    }
}

fn fire_perf(inner: &Inner, event: AudioPerfEvent) {
    let hook = inner.perf_hook.lock().unwrap().clone();
    if let Some(hook) = hook {
        hook(event);
    }
}

impl Engine {
    /// `make_fetcher` runs on the playback worker thread, so a blocking fetcher
    /// (e.g. `HttpFetcher`'s `reqwest::blocking::Client`) is never built on the
    /// caller's thread — constructing one inside an async runtime context
    /// panics in debug builds.
    pub fn new(
        resolver: Arc<dyn StreamResolver>,
        make_fetcher: impl FnOnce() -> Arc<dyn Fetcher> + Send + 'static,
        output: Box<dyn AudioOutput>,
        quality: Quality,
        cache: Arc<DiskCache>,
        allow_fallback: bool,
    ) -> Self {
        let inner = Arc::new(Inner {
            queue: Mutex::new(Queue::default()),
            resolver,
            fetcher: OnceLock::new(),
            cache,
            quality: Mutex::new(quality),
            allow_fallback: AtomicBool::new(allow_fallback),
            stream_meta: Mutex::new(None),
            device_id: Mutex::new(None),
            output_mode: Mutex::new(OutputMode::Shared),
            exclusive_fallback: Mutex::new(ExclusiveFallback::Shared),
            output_status: Mutex::new(OutputStatus::default()),
            dsp: Mutex::new(DspSettings::default()),
            dsp_version: AtomicU64::new(0),
            dsp_stages: Mutex::new(Vec::new()),
            listeners: Mutex::new(Vec::new()),
            perf_hook: Mutex::new(None),
            level_tap: Arc::new(LevelTap::new()),
            playback_lock: Mutex::new(()),
            generation: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
            fade: Mutex::new(None),
            position_ms: AtomicU64::new(0),
            position_track: AtomicU64::new(0),
            seek_target: Mutex::new(None),
            worker_busy: AtomicBool::new(false),
            wake_lock: Mutex::new(()),
            wake: Condvar::new(),
            shutdown: AtomicBool::new(false),
        });
        let worker_inner = inner.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<()>(1);
        std::thread::Builder::new()
            .name("oxitide-worker".into())
            .spawn(move || worker_main(worker_inner, make_fetcher, ready_tx, output))
            .expect("spawn playback worker");
        ready_rx.recv().expect("playback worker failed to initialize");
        Self { inner }
    }

    pub fn subscribe(&self, listener: Listener) {
        self.inner.listeners.lock().unwrap().push(listener);
    }

    /// Stop the playback worker and release the output. Called on logout.
    pub fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        self.inner.notify_worker();
    }

    pub fn set_quality(&self, quality: Quality) {
        *self.inner.quality.lock().unwrap() = quality;
        self.restart_current_at_position();
    }

    /// Whether a lower-than-requested served quality plays or errors. Applies
    /// from the next track.
    pub fn set_allow_fallback(&self, allow: bool) {
        self.inner.allow_fallback.store(allow, Ordering::SeqCst);
    }

    /// Select the output device. `None` means the system default. Applies
    /// immediately (restarts the current track at its position).
    pub fn set_output_device(&self, device: Option<String>) {
        *self.inner.device_id.lock().unwrap() = device;
        self.restart_current_at_position();
    }

    /// Install a perf hook. Events are delivered on engine threads and must be
    /// cheap; the engine never waits on the hook.
    pub fn set_perf_hook(&self, hook: PerfHook) {
        *self.inner.perf_hook.lock().unwrap() = Some(hook);
    }

    fn output_device(&self) -> Option<String> {
        self.inner.device_id.lock().unwrap().clone()
    }

    /// The mode/bit-perfect state the output last reported, with active DSP
    /// folded in.
    /// The meter tap; enable it while a level display is visible.
    pub fn level_tap(&self) -> Arc<LevelTap> {
        self.inner.level_tap.clone()
    }

    pub fn audio_status(&self) -> OutputStatus {
        effective_status(&self.inner).0
    }

    /// Replace the sound-processing settings. Applies live: the playing track
    /// picks them up on its next buffer without restarting.
    pub fn set_dsp(&self, settings: DspSettings) {
        *self.inner.dsp.lock().unwrap() = settings.sanitized();
        self.inner.dsp_version.fetch_add(1, Ordering::SeqCst);
        if !self.inner.worker_busy.load(Ordering::SeqCst) {
            // Nothing is playing: no stage alters samples until a track starts
            // (a live session republishes its own stages).
            self.inner.dsp_stages.lock().unwrap().clear();
        }
        self.inner.notify_worker();
        self.emit(EngineEvent::State(self.state()));
    }

    /// Select shared or exclusive output. Applies immediately (restarts the
    /// current track at its position).
    pub fn set_output_mode(&self, mode: OutputMode) {
        *self.inner.output_mode.lock().unwrap() = mode;
        self.restart_current_at_position();
    }

    /// Select what to do when exclusive output is unavailable.
    pub fn set_exclusive_fallback(&self, fallback: ExclusiveFallback) {
        *self.inner.exclusive_fallback.lock().unwrap() = fallback;
        self.restart_current_at_position();
    }

    /// Restart the current track at its position so an output setting change
    /// applies immediately without losing the listener's place.
    fn restart_current_at_position(&self) {
        let current = {
            let q = self.inner.queue.lock().unwrap();
            if q.is_playing() {
                q.current().cloned()
            } else {
                None
            }
        };
        if let Some(item) = current {
            let same_track =
                self.inner.position_track.load(Ordering::SeqCst) == item.track_id;
            let position = if same_track {
                self.inner.position_ms.load(Ordering::SeqCst)
            } else {
                0
            };
            *self.inner.seek_target.lock().unwrap() = Some(PendingSeek {
                track_id: item.track_id,
                position_ms: position,
                auto_start: true,
            });
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
            self.inner.notify_worker();
        }
    }

    /// Restart the current track at its last position. Recovers a stream
    /// error, a device-loss pause, or a queue stopped at the end of a track.
    /// A no-op without a current item.
    pub fn retry(&self) -> Result<()> {
        let item = {
            let q = self.inner.queue.lock().unwrap();
            q.current().cloned()
        };
        if let Some(item) = item {
            self.inner.queue.lock().unwrap().resume();
            self.inner.paused.store(false, Ordering::SeqCst);
            // Only trust `position_ms` when it belongs to the current track.
            let position = if self.inner.position_track.load(Ordering::SeqCst) == item.track_id {
                self.inner.position_ms.load(Ordering::SeqCst)
            } else {
                0
            };
            *self.inner.seek_target.lock().unwrap() = Some(PendingSeek {
                track_id: item.track_id,
                position_ms: position,
                auto_start: true,
            });
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
            self.inner.notify_worker();
            self.emit(EngineEvent::State(self.state()));
        }
        Ok(())
    }

    pub fn state(&self) -> PlaybackState {
        build_state(&self.inner)
    }

    fn emit(&self, event: EngineEvent) {
        emit(&self.inner, event);
    }

    /// Cancel an in-flight sleep fade and restore the pre-fade volume. Called
    /// by every explicit transport or volume change so the user's action wins.
    fn cancel_fade(&self) {
        let base = self.inner.fade.lock().unwrap().take().map(|f| f.base_volume);
        if let Some(base) = base {
            self.inner.volume.store(base.to_bits(), Ordering::SeqCst);
            self.inner.queue.lock().unwrap().set_volume(base);
            self.inner.notify_worker();
        }
    }

    /// Run a queue-mutating playback command under `playback_lock`, bumping the
    /// generation first so the running worker session is invalidated.
    fn mutate_playing_queue<T>(&self, mutate: impl FnOnce(&mut Queue) -> T) -> T {
        let _guard = self.inner.playback_lock.lock().unwrap();
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        let mut queue = self.inner.queue.lock().unwrap();
        mutate(&mut queue)
    }

    /// Like `mutate_playing_queue`, but only bumps the generation when `mutate`
    /// actually moved the current index or stopped playback (a no-op `Prev` at
    /// the first track must leave the running session untouched).
    fn mutate_queue_if_changed(&self, mutate: impl FnOnce(&mut Queue)) -> (Option<QueueItem>, bool) {
        let _guard = self.inner.playback_lock.lock().unwrap();
        let mut queue = self.inner.queue.lock().unwrap();
        let before = queue.current_index();
        let was_playing = queue.is_playing();
        mutate(&mut queue);
        let changed = queue.current_index() != before;
        if changed || (was_playing && !queue.is_playing()) {
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
        }
        (queue.current().cloned(), changed)
    }

    /// Replace the playback context via `mutate` and announce the new queue
    /// and current track to listeners.
    fn start_context(&self, mutate: impl FnOnce(&mut Queue)) {
        // Starting a new context is an explicit user action; end any fade.
        self.cancel_fade();
        *self.inner.stream_meta.lock().unwrap() = None;
        let item = self.mutate_playing_queue(|q| {
            mutate(q);
            q.current().cloned()
        });
        self.inner.notify_worker();
        self.emit(EngineEvent::NowPlaying(item.clone()));
        self.emit(EngineEvent::State(self.state()));
    }

    pub fn handle(&self, cmd: EngineCmd) -> Result<()> {
        match cmd {
            EngineCmd::PlayTracks { items, start } => {
                self.start_context(|q| q.play_context(items, start));
            }
            EngineCmd::PlayShuffled { items } => {
                self.start_context(|q| q.play_context_shuffled(items));
            }
            EngineCmd::RestoreSession { items, start, position_ms, shuffle, repeat } => {
                *self.inner.stream_meta.lock().unwrap() = None;
                let item = self.mutate_playing_queue(|q| {
                    q.restore(items, start, shuffle, repeat);
                    q.current().cloned()
                });
                self.inner.paused.store(true, Ordering::SeqCst);
                self.inner.position_ms.store(
                    if item.is_some() { position_ms } else { 0 },
                    Ordering::SeqCst,
                );
                self.inner.position_track.store(
                    item.as_ref().map(|i| i.track_id).unwrap_or(0),
                    Ordering::SeqCst,
                );
                // A non-auto-start seek keeps the worker idle at restore while
                // letting the first play resume from the saved position.
                *self.inner.seek_target.lock().unwrap() = item.as_ref().map(|i| PendingSeek {
                    track_id: i.track_id,
                    position_ms,
                    auto_start: false,
                });
                self.inner.notify_worker();
                self.emit(EngineEvent::NowPlaying(item));
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::PlayPause => {
                // An explicit transport action wins over a sleep fade.
                self.cancel_fade();
                // Only toggle when a track is current. On an empty queue
                // `toggle_play_pause` is a no-op, so flipping `paused` here
                // would latch it true and silence the next `PlayTracks`.
                let paused = {
                    let mut q = self.inner.queue.lock().unwrap();
                    if q.current().is_some() {
                        q.toggle_play_pause();
                        Some(!q.is_playing())
                    } else {
                        None
                    }
                };
                if let Some(paused) = paused {
                    self.inner.paused.store(paused, Ordering::SeqCst);
                    // Resuming while the worker is idle (queue ended, output
                    // error, device loss) needs a fresh generation to start a
                    // session; while a session is live, pause is an atomic
                    // check and must NOT invalidate the running session.
                    if !paused && !self.inner.worker_busy.load(Ordering::SeqCst) {
                        self.inner.generation.fetch_add(1, Ordering::SeqCst);
                    }
                }
                self.inner.notify_worker();
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Next => {
                let (item, changed) = self.mutate_queue_if_changed(|q| {
                    q.next();
                });
                self.inner.notify_worker();
                if changed {
                    self.emit(EngineEvent::NowPlaying(item.clone()));
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Prev => {
                let (item, changed) = self.mutate_queue_if_changed(|q| {
                    q.prev();
                });
                self.inner.notify_worker();
                if changed {
                    self.emit(EngineEvent::NowPlaying(item.clone()));
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Seek(ms) => {
                let current = {
                    let q = self.inner.queue.lock().unwrap();
                    q.current().cloned()
                };
                if let Some(item) = current {
                    *self.inner.seek_target.lock().unwrap() = Some(PendingSeek {
                        track_id: item.track_id,
                        position_ms: ms,
                        auto_start: true,
                    });
                    // Show the target immediately; the worker restarts there,
                    // staying paused if the queue is paused.
                    self.inner.position_ms.store(ms, Ordering::SeqCst);
                    self.inner.position_track.store(item.track_id, Ordering::SeqCst);
                    self.inner.generation.fetch_add(1, Ordering::SeqCst);
                    self.inner.notify_worker();
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::SetVolume(v) => {
                self.cancel_fade();
                let clamped = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.set_volume(v);
                    q.volume()
                };
                self.inner.volume.store(clamped.to_bits(), Ordering::SeqCst);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::SetShuffle(on) => {
                self.inner.queue.lock().unwrap().set_shuffle(on);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::SetRepeat(mode) => {
                self.inner.queue.lock().unwrap().set_repeat(mode);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Reorder { from, to } => {
                self.inner.queue.lock().unwrap().reorder(from, to);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Remove(index) => {
                self.inner.queue.lock().unwrap().remove(index);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Enqueue { items } => {
                if !items.is_empty() {
                    self.inner.queue.lock().unwrap().push(items);
                    self.emit(EngineEvent::State(self.state()));
                }
            }
            EngineCmd::Clear => {
                self.cancel_fade();
                *self.inner.stream_meta.lock().unwrap() = None;
                self.inner.dsp_stages.lock().unwrap().clear();
                self.mutate_playing_queue(|q| q.clear());
                self.inner.paused.store(false, Ordering::SeqCst);
                self.inner.notify_worker();
                self.emit(EngineEvent::NowPlaying(None));
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::FadeAndPause { duration_ms } => {
                let playing = self.inner.queue.lock().unwrap().is_playing();
                if playing {
                    let base = f32::from_bits(self.inner.volume.load(Ordering::SeqCst));
                    if duration_ms == 0 || base <= 0.0 {
                        // Nothing to ramp: pause now.
                        self.inner.queue.lock().unwrap().stop();
                        self.inner.paused.store(true, Ordering::SeqCst);
                        self.inner.notify_worker();
                    } else {
                        *self.inner.fade.lock().unwrap() = Some(FadeState {
                            started: Instant::now(),
                            duration: Duration::from_millis(duration_ms),
                            base_volume: base,
                        });
                        self.inner.notify_worker();
                    }
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Retry => {
                self.retry()?;
            }
        }
        Ok(())
    }

    /// Synchronous resolve/download/decode into `output`; used by tests.
    /// Ignores the queue and cancellation.
    pub fn play_track_into(&self, item: &QueueItem, output: &mut dyn AudioOutput) -> Result<()> {
        let mode = *self.inner.output_mode.lock().unwrap();
        let fallback = *self.inner.exclusive_fallback.lock().unwrap();
        let generation = self.inner.generation.load(Ordering::SeqCst);
        let mut stream = start_track(&self.inner, item, mode, generation, None);
        let start = wait_for_info(&self.inner, &mut stream, generation)?;
        *self.inner.stream_meta.lock().unwrap() = Some(start.meta.clone());
        let spec = AudioOutputSpec {
            device_id: self.output_device(),
            info: start.info,
            mode,
            exclusive_fallback: fallback,
        };
        output.open(&spec)?;
        output.start()?;
        output.set_volume(f32::from_bits(self.inner.volume.load(Ordering::SeqCst)))?;
        loop {
            match stream.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(StreamEvent::Samples(buf))) => output.write(&buf)?,
                Ok(Ok(StreamEvent::Info(_))) => {}
                Ok(Err(e)) => {
                    let _ = output.stop();
                    return Err(e);
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        output.stop()
    }
}

struct StreamStart {
    info: StreamInfo,
    start_frames: u64,
    meta: StreamMeta,
    replay_gain: ReplayGain,
}

/// One decoded chunk handed from a track's producer to the worker.
enum StreamEvent {
    Info(StreamStart),
    Samples(PcmBuffer),
}

/// A running resolve/fetch/decode pipeline for one track.
struct TrackStream {
    item: QueueItem,
    rx: Receiver<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
    cache: Arc<DiskCache>,
    /// The cache key this stream protects while it is alive, once resolved.
    cache_key: Arc<Mutex<Option<CacheKey>>>,
}

impl Drop for TrackStream {
    fn drop(&mut self) {
        // Stop the producer's fetch/decode promptly when the stream is
        // abandoned (next/prev/seek/clear or track change), and release the
        // cache protection that kept the playing/prefetched file on disk.
        self.buffer.cancel();
        if let Some(key) = self.cache_key.lock().unwrap().take() {
            self.cache.unprotect(&key);
        }
    }
}

fn start_track(
    inner: &Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
) -> TrackStream {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<StreamEvent>>(PREFETCH_BUFFERS);
    let buffer = StreamBuffer::new();
    let cache_key = Arc::new(Mutex::new(None));
    let stream = TrackStream {
        item: item.clone(),
        rx,
        buffer: buffer.clone(),
        cache: inner.cache.clone(),
        cache_key: cache_key.clone(),
    };
    let decode_inner = inner.clone();
    let item = item.clone();
    let spawned = std::thread::Builder::new()
        .name("oxitide-decode".into())
        .spawn(move || {
            decode_track(decode_inner, item, mode, generation, seek_ms, tx, buffer, cache_key)
        });
    if let Err(e) = spawned {
        // Dropping `tx` disconnects the channel; the worker treats it as an
        // ended track and advances. Wake it so it sees the disconnect without
        // waiting out the safety timeout.
        tracing::error!("failed to spawn decode thread: {e}");
        inner.notify_worker();
    }
    stream
}

/// Start decoding the queue item that `on_track_ended` would select next, so
/// the worker can chain into it without a gap.
fn start_prefetch(inner: &Arc<Inner>, mode: OutputMode, gen: u64) -> Option<TrackStream> {
    let next = {
        let q = inner.queue.lock().unwrap();
        q.peek_next_index().and_then(|i| q.items().get(i).cloned())
    };
    next.map(|item| start_track(inner, &item, mode, gen, None))
}

#[allow(clippy::too_many_arguments)]
fn decode_track(
    inner: Arc<Inner>,
    item: QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: SyncSender<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
    cache_key_slot: Arc<Mutex<Option<CacheKey>>>,
) {
    let quality = *inner.quality.lock().unwrap();
    fire_perf(&inner, AudioPerfEvent::ResolveStart { track_id: item.track_id });
    let plan = match inner.resolver.resolve(item.track_id, quality) {
        Ok(plan) => plan,
        Err(e) => {
            let _ = tx.send(Err(e));
            inner.notify_worker();
            return;
        }
    };
    tracing::debug!(
        "resolved track {}: codec={:?} rate={:?} bits={:?} timescale={} segments={} init={} first_media={:?}",
        item.track_id,
        plan.codec,
        plan.sample_rate,
        plan.bit_depth,
        plan.timescale,
        plan.segments.len(),
        plan.init_url.is_some(),
        plan.segments
            .first()
            .map(|s| s.url.split('?').next().unwrap_or("")),
    );
    let served = plan.quality;
    if let Some(served) = served {
        if !inner.allow_fallback.load(Ordering::SeqCst) && served < quality {
            let _ = tx.send(Err(Error::Unavailable(served.label().to_string())));
            inner.notify_worker();
            return;
        }
    }
    if plan.codec.as_deref().is_some_and(is_he_aac) {
        let _ = tx.send(Err(Error::Unsupported(
            "this track is only offered in HE-AAC (TIDAL Low), which the built-in decoder cannot play"
                .into(),
        )));
        inner.notify_worker();
        return;
    }
    fire_perf(&inner, AudioPerfEvent::DecodeStart { track_id: item.track_id });
    let cache_key = plan.manifest_hash.map(|manifest_hash| CacheKey {
        track_id: item.track_id,
        quality,
        manifest_hash,
    });
    if let Some(key) = cache_key {
        *cache_key_slot.lock().unwrap() = Some(key);
        inner.cache.protect(&key);
    }
    match cache_key {
        Some(key) => match inner.cache.lookup(&key) {
            Some(path) => {
                fire_perf(&inner, AudioPerfEvent::CacheHit { track_id: item.track_id });
                let result =
                    decode_from_file(&inner, &item, mode, generation, seek_ms, &tx, &path, &plan);
                match result {
                    Ok(()) => return,
                    Err(e) => {
                        if inner.generation.load(Ordering::SeqCst) != generation {
                            let _ = tx.send(Err(e));
                            inner.notify_worker();
                            return;
                        }
                        tracing::warn!(
                            "cache entry {} failed to decode ({e}); refetching",
                            key.file_name()
                        );
                        inner.cache.remove(&key);
                        fire_perf(&inner, AudioPerfEvent::CacheMiss { track_id: item.track_id });
                    }
                }
            }
            None => fire_perf(&inner, AudioPerfEvent::CacheMiss { track_id: item.track_id }),
        },
        None => fire_perf(&inner, AudioPerfEvent::CacheMiss { track_id: item.track_id }),
    }
    decode_from_network(inner, item, mode, generation, seek_ms, tx, buffer, plan, cache_key);
}

#[allow(clippy::too_many_arguments)]
fn decode_from_network(
    inner: Arc<Inner>,
    item: QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: SyncSender<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
    plan: StreamPlan,
    cache_key: Option<CacheKey>,
) {
    // Seek by segment index: media URLs are an ordered list, so restarting
    // at the segment covering the target needs no HTTP Range.
    let target = match seek_ms {
        Some(ms) if plan.sample_rate.is_some() => {
            Some(seek::target_segment(&plan, ms, plan.sample_rate.unwrap()))
        }
        _ => None,
    };
    let download_plan = match &target {
        Some(target) => plan.from_segment(target.segment_index),
        None => plan.clone(),
    };
    let source_start =
        target.as_ref().map(|t| t.position_frames - t.discard_frames).unwrap_or(0);
    // Only full-track plans are cached; a seek-truncated plan is a partial
    // track and would poison replays.
    let writer = match (seek_ms, cache_key) {
        (None, Some(key)) => inner.cache.begin_write(&key),
        _ => None,
    };
    let fetch_buffer = buffer.clone();
    let fetcher = inner.fetcher.get().expect("fetcher initialized by the worker").clone();
    let downloader =
        std::thread::spawn(move || fetch::download(fetch_buffer, fetcher, download_plan, writer));
    let source = crate::demux::StreamBufferSource::new(buffer.clone());
    let total_bytes = {
        let buffer = buffer.clone();
        Arc::new(move || buffer.total_pushed()) as Arc<dyn Fn() -> u64 + Send + Sync>
    };
    let result = decode_source(
        inner.clone(),
        &item,
        mode,
        generation,
        seek_ms,
        source_start,
        &tx,
        source,
        plan,
        total_bytes,
    );
    let _ = downloader.join();
    if let Err(e) = result {
        let _ = tx.send(Err(e));
        inner.notify_worker();
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_from_file(
    inner: &Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: &SyncSender<Result<StreamEvent>>,
    path: &std::path::Path,
    plan: &StreamPlan,
) -> Result<()> {
    let len = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let source = crate::demux::FileSource::open(path)?;
    let total_bytes = Arc::new(move || len) as Arc<dyn Fn() -> u64 + Send + Sync>;
    decode_source(
        inner.clone(),
        item,
        mode,
        generation,
        seek_ms,
        seek::source_start_frames(plan),
        tx,
        source,
        plan.clone(),
        total_bytes,
    )
}

#[allow(clippy::too_many_arguments)]
fn decode_source(
    inner: Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    source_start_frames: u64,
    tx: &SyncSender<Result<StreamEvent>>,
    source: impl symphonia::core::io::MediaSource + 'static,
    plan: StreamPlan,
    total_bytes: Arc<dyn Fn() -> u64 + Send + Sync>,
) -> Result<()> {
    let mut sink = ChannelSink {
        inner,
        generation,
        tx: tx.clone(),
        mode,
        out_format: PcmFormat::F32,
        channels: 2,
        plan,
        seek_ms,
        source_start_frames,
        discarder: Discarder::default(),
        start_frames: 0,
        duration_secs: item.duration_secs,
        total_bytes,
    };
    crate::decode::run(source, "mp4", &mut sink)
}

struct ChannelSink {
    inner: Arc<Inner>,
    generation: u64,
    tx: SyncSender<Result<StreamEvent>>,
    mode: OutputMode,
    out_format: PcmFormat,
    channels: u16,
    /// The original (untruncated) plan, for seek discard math.
    plan: StreamPlan,
    seek_ms: Option<u64>,
    /// The track frame the decoded stream actually begins at (0 for a cached
    /// file, the target segment's start for a seeked network stream).
    source_start_frames: u64,
    discarder: Discarder,
    start_frames: u64,
    /// The queue item's duration, for the bitrate fallback.
    duration_secs: u32,
    /// Total source bytes fetched so far (buffer for network, file length for a
    /// cache hit).
    total_bytes: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl ChannelSink {
    fn send(&self, event: Result<StreamEvent>) -> Result<()> {
        let sent = self
            .tx
            .send(event)
            .map_err(|_| Error::Output("playback stream closed".into()));
        self.inner.notify_worker();
        sent
    }

    /// Fallback bitrate for a single-file source: fetched bytes over the item's
    /// duration. Multi-segment manifests carry their own bandwidth.
    fn computed_bitrate_kbps(&self) -> Option<u32> {
        if self.plan.timescale != 0 || self.plan.segments.len() != 1 || self.duration_secs == 0 {
            return None;
        }
        let bytes = (self.total_bytes)();
        if bytes == 0 {
            return None;
        }
        Some((bytes.saturating_mul(8) / self.duration_secs as u64 / 1000) as u32)
    }
}

impl PcmSink for ChannelSink {
    fn preferred_format(&self) -> PcmFormat {
        self.out_format
    }

    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.out_format = match self.mode {
            OutputMode::Exclusive if info.is_bit_perfect_candidate() => info.format,
            _ => PcmFormat::F32,
        };
        self.channels = info.channels;
        if let Some(ms) = self.seek_ms {
            let target = seek::target_segment(&self.plan, ms, info.sample_rate);
            self.discarder =
                Discarder::new(seek::discard_frames_for(&target, self.source_start_frames));
            self.start_frames = target.position_frames;
        }
        let meta = StreamMeta {
            quality: self.plan.quality,
            codec: self.plan.codec.clone(),
            sample_rate: info.sample_rate,
            bit_depth: info.bits_per_sample,
            channels: info.channels,
            bitrate_kbps: self.plan.bitrate_kbps.or_else(|| self.computed_bitrate_kbps()),
            replay_gain_db: None,
        };
        self.send(Ok(StreamEvent::Info(StreamStart {
            info: *info,
            start_frames: self.start_frames,
            meta,
            replay_gain: self.plan.replay_gain,
        })))
    }

    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
        if self.inner.generation.load(Ordering::SeqCst) != self.generation {
            return Err(Error::Output("playback superseded".into()));
        }
        let buf = match buf {
            PcmBuffer::F32(v) => {
                let (offset, len) = self.discarder.apply(v.len(), self.channels);
                if len == 0 {
                    return Ok(());
                }
                PcmBuffer::F32(v[offset..offset + len].to_vec())
            }
            PcmBuffer::I32(v) => {
                let (offset, len) = self.discarder.apply(v.len(), self.channels);
                if len == 0 {
                    return Ok(());
                }
                PcmBuffer::I32(v[offset..offset + len].to_vec())
            }
        };
        self.send(Ok(StreamEvent::Samples(buf)))
    }
}

#[derive(Debug)]
enum StreamWait<T> {
    Event(T),
    Superseded,
    Shutdown,
    Woken,
    Disconnected,
}

enum SessionOver {
    Superseded,
    Shutdown,
}

/// Wait until the producer queues a stream event, the session predicate says
/// the session is over, or the engine is woken. The predicate runs while
/// `wake_lock` is held, so a command that bumps the generation and notifies
/// cannot slip between the check and the park. A bare wake returns
/// `StreamWait::Woken`; a disconnected producer returns `Disconnected`.
fn wait_for_stream_event<T>(
    wake_lock: &Mutex<()>,
    wake: &Condvar,
    rx: &std::sync::mpsc::Receiver<T>,
    session_over: impl Fn() -> Option<SessionOver>,
) -> StreamWait<T> {
    let guard = wake_lock.lock().unwrap();
    if let Some(over) = session_over() {
        return match over {
            SessionOver::Superseded => StreamWait::Superseded,
            SessionOver::Shutdown => StreamWait::Shutdown,
        };
    }
    match rx.try_recv() {
        Ok(event) => StreamWait::Event(event),
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            let _ = wake.wait_timeout(guard, Duration::from_secs(2));
            StreamWait::Woken
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => StreamWait::Disconnected,
    }
}

fn wait_for_info(inner: &Arc<Inner>, stream: &mut TrackStream, gen: u64) -> Result<StreamStart> {
    loop {
        let session_over = || {
            if inner.generation.load(Ordering::SeqCst) != gen {
                Some(SessionOver::Superseded)
            } else if inner.shutdown.load(Ordering::SeqCst) {
                Some(SessionOver::Shutdown)
            } else {
                None
            }
        };
        match wait_for_stream_event(&inner.wake_lock, &inner.wake, &stream.rx, session_over) {
            StreamWait::Superseded => return Err(Error::Output("playback superseded".into())),
            StreamWait::Shutdown => return Err(Error::Output("engine shut down".into())),
            StreamWait::Woken => continue,
            StreamWait::Disconnected => {
                return Err(Error::Decode("stream ended before any audio".into()))
            }
            StreamWait::Event(Ok(StreamEvent::Info(start))) => return Ok(start),
            StreamWait::Event(Ok(StreamEvent::Samples(_))) => continue,
            StreamWait::Event(Err(e)) => return Err(e),
        }
    }
}

fn emit_position_if_due(inner: &Arc<Inner>, frames: u64, rate: u32, last: &mut Instant) {
    if last.elapsed() < Duration::from_millis(500) {
        return;
    }
    *last = Instant::now();
    let position_ms = if rate == 0 { 0 } else { frames.saturating_mul(1000) / rate as u64 };
    let duration_ms = inner
        .queue
        .lock()
        .unwrap()
        .current()
        .map(|i| i.duration_secs as u64 * 1000)
        .unwrap_or(0);
    emit(inner, EngineEvent::Position { position_ms, duration_ms });
}

/// Record which DSP stages alter the current track's samples; `crossfading`
/// while the previous track's tail is being mixed in.
fn publish_dsp_stages(inner: &Inner, dsp: &TrackDsp, crossfading: bool) {
    let mut stages = dsp.active_stages();
    if crossfading {
        stages.push(STAGE_CROSSFADE);
    }
    *inner.dsp_stages.lock().unwrap() = stages;
}

/// How many frames of `current` to hold back for a crossfade into `next`; 0
/// when crossfade is off, nothing follows, or both share an album and albums
/// stay gapless.
fn crossfade_frames(
    settings: &CrossfadeSettings,
    current: &QueueItem,
    next: Option<&QueueItem>,
    rate: u32,
) -> u64 {
    let Some(next) = next else {
        return 0;
    };
    if settings.duration_ms == 0
        || (settings.skip_same_album && !current.album.is_empty() && current.album == next.album)
    {
        return 0;
    }
    settings.duration_ms as u64 * rate as u64 / 1000
}

/// A pending seek: the track it belongs to, the target, and whether it may
/// start a session by itself (a user seek) or must wait for a play press
/// (device-loss recovery).
#[derive(Debug, Clone, Copy)]
struct PendingSeek {
    track_id: u64,
    position_ms: u64,
    auto_start: bool,
}

enum SessionOutcome {
    Superseded,
    QueueEnded,
    OutputError {
        message: String,
        track_id: u64,
    },
    StreamError(Error),
    /// Every track reachable from the current one was unplayable and the
    /// engine stopped; `skipped` counts the tracks passed over.
    UnavailableChain {
        skipped: usize,
        last: Error,
    },
}

/// Play `first_item` and keep advancing through the queue until it ends, the
/// generation changes, or a device/stream error occurs.
#[allow(clippy::too_many_arguments)]
fn run_session(
    inner: &Arc<Inner>,
    output: &mut dyn AudioOutput,
    open_spec: &mut Option<AudioOutputSpec>,
    first_item: &QueueItem,
    mode: OutputMode,
    fallback: ExclusiveFallback,
    gen: u64,
    seek_ms: Option<u64>,
) -> SessionOutcome {
    // A seek must not play the audio still buffered in the device ring. Flush
    // the open device instead of releasing it: reopening re-negotiates the
    // endpoint and makes exclusive-mode DACs re-lock (audible click/hiss).
    if seek_ms.is_some() {
        if let Err(e) = output.flush() {
            return SessionOutcome::OutputError {
                message: e.to_string(),
                track_id: first_item.track_id,
            };
        }
    }
    let mut first_write_of_track;
    let mut last_write: Option<(u64, Instant)> = None;
    let mut stream = start_track(inner, first_item, mode, gen, seek_ms);
    let mut prefetch = start_prefetch(inner, mode, gen);
    // Unplayable tracks passed over since the last track that started; bounds
    // the skip chain so repeat-all/repeat-one cannot loop forever.
    let mut skipped: usize = 0;
    // The previous track's held-back end, waiting to cross into this one.
    let mut pending_tail: Option<Tail> = None;
    let mut frames: u64;
    let mut last_emit;
    loop {
        if inner.generation.load(Ordering::SeqCst) != gen {
            return SessionOutcome::Superseded;
        }
        *inner.stream_meta.lock().unwrap() = None;
        let start = match wait_for_info(inner, &mut stream, gen) {
            Ok(start) => start,
            Err(_) if inner.generation.load(Ordering::SeqCst) != gen => {
                return SessionOutcome::Superseded;
            }
            // An unavailable track (region-locked/removed, or a codec the
            // decoder cannot play) is skipped like the official client rather
            // than stopping the queue on it.
            Err(e) if e.is_track_unavailable() => {
                skipped += 1;
                let next = {
                    let _guard = inner.playback_lock.lock().unwrap();
                    if inner.generation.load(Ordering::SeqCst) != gen {
                        return SessionOutcome::Superseded;
                    }
                    let next = {
                        let mut q = inner.queue.lock().unwrap();
                        if skipped >= q.items().len() {
                            // Tried every track in the queue; give up.
                            None
                        } else {
                            q.skip_current().and_then(|i| q.items().get(i).cloned())
                        }
                    };
                    if next.is_some() {
                        emit(inner, EngineEvent::NowPlaying(next.clone()));
                        emit(inner, EngineEvent::State(build_state(inner)));
                    }
                    next
                };
                match next {
                    Some(item) => {
                        stream = match prefetch.take() {
                            Some(prefetched) if prefetched.item.track_id == item.track_id => {
                                prefetched
                            }
                            _ => start_track(inner, &item, mode, gen, None),
                        };
                        prefetch = start_prefetch(inner, mode, gen);
                        continue;
                    }
                    None => {
                        return SessionOutcome::UnavailableChain { skipped, last: e };
                    }
                }
            }
            Err(e) => return SessionOutcome::StreamError(e),
        };
        skipped = 0;
        let StreamStart { info, start_frames, mut meta, replay_gain } = start;
        fire_perf(inner, AudioPerfEvent::InfoReady { track_id: stream.item.track_id });
        let want = AudioOutputSpec {
            device_id: inner.device_id.lock().unwrap().clone(),
            info,
            mode,
            exclusive_fallback: fallback,
        };
        // The previous track's held-back end overlaps this one only when the
        // output stays as it is; across a reopen it fades out on its own.
        let mut incoming: Option<Tail> = None;
        if let Some(mut tail) = pending_tail.take() {
            if open_spec.as_ref() == Some(&want) {
                incoming = Some(tail);
            } else if let Some(previous) = open_spec.as_ref() {
                let faded = tail.fade_out_remaining();
                if let Err(e) = output.write(&faded) {
                    return SessionOutcome::OutputError {
                        message: e.to_string(),
                        track_id: stream.item.track_id,
                    };
                }
                inner.level_tap.push(&faded, previous.info.channels, previous.info.sample_rate);
            }
        }
        let mut dsp_version = inner.dsp_version.load(Ordering::SeqCst);
        let mut crossfade = {
            let settings = inner.dsp.lock().unwrap();
            settings.crossfade.clone()
        };
        let mut dsp = TrackDsp::new(&info, replay_gain, &inner.dsp.lock().unwrap());
        meta.replay_gain_db = dsp.replay_gain_db();
        publish_dsp_stages(inner, &dsp, incoming.is_some());
        *inner.stream_meta.lock().unwrap() = Some(meta);
        emit(inner, EngineEvent::State(build_state(inner)));
        // The stream's absolute starting frame (non-zero only for a seek).
        frames = start_frames;
        last_emit = Instant::now() - Duration::from_secs(1);
        inner.position_track.store(stream.item.track_id, Ordering::SeqCst);
        if info.sample_rate > 0 {
            inner
                .position_ms
                .store(frames.saturating_mul(1000) / info.sample_rate as u64, Ordering::SeqCst);
        }
        if open_spec.as_ref() != Some(&want) {
            if open_spec.is_some() {
                let _ = output.stop();
            }
            if let Err(e) = output.open(&want) {
                return SessionOutcome::OutputError {
                    message: e.to_string(),
                    track_id: stream.item.track_id,
                };
            }
            if let Err(e) = output.start() {
                return SessionOutcome::OutputError {
                    message: e.to_string(),
                    track_id: stream.item.track_id,
                };
            }
            fire_perf(inner, AudioPerfEvent::OutputOpened { track_id: stream.item.track_id });
            // A format change mid-fade must not jump back to the user volume.
            let (volume, _) = current_volume(inner);
            if let Err(e) = output.set_volume(volume) {
                return SessionOutcome::OutputError {
                    message: e.to_string(),
                    track_id: stream.item.track_id,
                };
            }
            *inner.output_status.lock().unwrap() = output.status();
            // Publish the fresh status: after a device-loss recovery the
            // previous notice would otherwise linger in clients until some
            // unrelated state event.
            emit(inner, EngineEvent::State(build_state(inner)));
            *open_spec = Some(want);
        }
        let channels = info.channels;
        let rate = info.sample_rate;
        let track_id = stream.item.track_id;
        first_write_of_track = true;
        let mut last_volume = f32::from_bits(inner.volume.load(Ordering::SeqCst));
        // Decoded audio read ahead of the output. Empty unless a crossfade is
        // armed, when it holds the crossfade length so that, once the producer
        // ends, it is exactly the track's tail.
        let mut fifo: VecDeque<PcmBuffer> = VecDeque::new();
        let mut fifo_frames: u64 = 0;
        let mut ended = false;
        let mut hand_off = false;
        loop {
            if inner.generation.load(Ordering::SeqCst) != gen {
                return SessionOutcome::Superseded;
            }
            let version = inner.dsp_version.load(Ordering::SeqCst);
            if version != dsp_version {
                dsp_version = version;
                {
                    let settings = inner.dsp.lock().unwrap();
                    dsp.update(&settings);
                    crossfade = settings.crossfade.clone();
                }
                if let Some(meta) = inner.stream_meta.lock().unwrap().as_mut() {
                    meta.replay_gain_db = dsp.replay_gain_db();
                }
                publish_dsp_stages(inner, &dsp, incoming.is_some());
                emit(inner, EngineEvent::State(build_state(inner)));
            }
            let (volume, fading) = current_volume(inner);
            // Fades step coarsely (endpoint volume calls are not free);
            // SetVolume changes are tracked exactly.
            let changed = if fading {
                volume <= f32::EPSILON || (volume - last_volume).abs() >= 0.01
            } else {
                (volume - last_volume).abs() >= f32::EPSILON
            };
            if changed {
                if let Err(e) = output.set_volume(volume) {
                    return SessionOutcome::OutputError {
                        message: e.to_string(),
                        track_id: stream.item.track_id,
                    };
                }
                last_volume = volume;
                *inner.output_status.lock().unwrap() = output.status();
            }
            if fading && volume <= f32::EPSILON {
                // Only complete when this iteration actually owns the fade: a
                // concurrent cancel (play/pause, volume, new context) took it
                // and must win, not be overwritten with a stale zero.
                if let Some(fade) = inner.fade.lock().unwrap().take() {
                    // Discard the audio staged at the faded volume before the
                    // user volume is restored: exclusive endpoint volume
                    // applies at render time, so a full ring would otherwise
                    // un-mute an audible tail after the pause.
                    if let Err(e) = output.flush() {
                        return SessionOutcome::OutputError {
                            message: e.to_string(),
                            track_id: stream.item.track_id,
                        };
                    }
                    // The fade finished: pause here, keep queue and position,
                    // and restore the user's volume so the next play is audible.
                    let base = fade.base_volume;
                    {
                        let mut q = inner.queue.lock().unwrap();
                        q.stop();
                        q.set_volume(base);
                    }
                    inner.volume.store(base.to_bits(), Ordering::SeqCst);
                    inner.paused.store(true, Ordering::SeqCst);
                    emit(inner, EngineEvent::State(build_state(inner)));
                }
            }
            if inner.paused.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
                if inner.generation.load(Ordering::SeqCst) != gen {
                    return SessionOutcome::Superseded;
                }
                emit_position_if_due(inner, frames, rate, &mut last_emit);
                continue;
            }
            // A sleep fade ends the session on this track; never cross into
            // the next one while it runs.
            let target = if fading {
                0
            } else {
                crossfade_frames(&crossfade, &stream.item, prefetch.as_ref().map(|p| &p.item), rate)
            };
            if fifo.is_empty() {
                if ended {
                    break;
                }
                let received = stream.rx.recv_timeout(Duration::from_millis(50));
                // A seek or track change bumps the generation. Once it moves,
                // this session is superseded and must not publish a stale
                // position or write buffered audio from the pre-seek offset.
                if inner.generation.load(Ordering::SeqCst) != gen {
                    return SessionOutcome::Superseded;
                }
                match received {
                    Ok(Ok(StreamEvent::Samples(buf))) => {
                        fifo_frames += buf.frames(channels);
                        fifo.push_back(buf);
                    }
                    Ok(Ok(StreamEvent::Info(_))) => continue,
                    Ok(Err(e)) => {
                        // The producer reports "playback superseded" once the
                        // generation moves; that case was caught just above.
                        return SessionOutcome::StreamError(e);
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        emit_position_if_due(inner, frames, rate, &mut last_emit);
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        ended = true;
                        continue;
                    }
                }
            }
            // Read ahead (the producer decodes faster than real time) until
            // the FIFO holds a crossfade's worth of audio.
            while target > 0 && !ended && fifo_frames < target {
                match stream.rx.try_recv() {
                    Ok(Ok(StreamEvent::Samples(buf))) => {
                        fifo_frames += buf.frames(channels);
                        fifo.push_back(buf);
                    }
                    Ok(Ok(StreamEvent::Info(_))) => {}
                    Ok(Err(e)) => {
                        if inner.generation.load(Ordering::SeqCst) != gen {
                            return SessionOutcome::Superseded;
                        }
                        return SessionOutcome::StreamError(e);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => ended = true,
                }
            }
            if ended && target > 0 {
                // The FIFO is the whole tail: hand it to the next track.
                hand_off = true;
                break;
            }
            let Some(buf) = fifo.pop_front() else {
                continue;
            };
            fifo_frames -= buf.frames(channels);
            let mut buf = dsp.process(buf);
            if let Some(tail) = incoming.as_mut() {
                buf = tail.mix_into(buf);
                if tail.is_done() {
                    incoming = None;
                    publish_dsp_stages(inner, &dsp, false);
                    emit(inner, EngineEvent::State(build_state(inner)));
                }
            }
            if let Err(e) = output.write(&buf) {
                return SessionOutcome::OutputError {
                    message: e.to_string(),
                    track_id: stream.item.track_id,
                };
            }
            inner.level_tap.push(&buf, channels, rate);
            frames += buf.frames(channels);
            if first_write_of_track {
                first_write_of_track = false;
                fire_perf(inner, AudioPerfEvent::FirstPcm { track_id });
                if let Some((previous, at)) = last_write {
                    if previous != track_id {
                        fire_perf(
                            inner,
                            AudioPerfEvent::Advance {
                                track_id,
                                gap_ms: at.elapsed().as_millis() as u64,
                            },
                        );
                    }
                }
            }
            last_write = Some((track_id, Instant::now()));
            if rate > 0 {
                inner
                    .position_ms
                    .store(frames.saturating_mul(1000) / rate as u64, Ordering::SeqCst);
                inner.position_track.store(track_id, Ordering::SeqCst);
            }
            emit_position_if_due(inner, frames, rate, &mut last_emit);
        }
        // A track shorter than the crossfade ends before the previous tail
        // finished; let the rest of that tail fade out on its own.
        if let Some(mut tail) = incoming.take() {
            let faded = tail.fade_out_remaining();
            if let Err(e) = output.write(&faded) {
                return SessionOutcome::OutputError { message: e.to_string(), track_id };
            }
            inner.level_tap.push(&faded, channels, rate);
        }
        let tail: Vec<PcmBuffer> = if hand_off {
            fifo.drain(..).map(|buf| dsp.process(buf)).collect()
        } else {
            Vec::new()
        };
        // Track ended: advance the queue under the playback lock, the same way
        // the old per-track thread did.
        let next = {
            let _guard = inner.playback_lock.lock().unwrap();
            if inner.generation.load(Ordering::SeqCst) != gen {
                return SessionOutcome::Superseded;
            }
            let next = {
                let mut q = inner.queue.lock().unwrap();
                q.on_track_ended().and_then(|i| q.items().get(i).cloned())
            };
            emit(inner, EngineEvent::NowPlaying(next.clone()));
            emit(inner, EngineEvent::State(build_state(inner)));
            next
        };
        match next {
            Some(item) => {
                // Use the prefetched stream when it is the right track; the
                // top of the loop decides whether the output can be reused
                // (matching StreamInfo) or must reopen (format change).
                stream = match prefetch.take() {
                    Some(prefetched) if prefetched.item.track_id == item.track_id => prefetched,
                    _ => start_track(inner, &item, mode, gen, None),
                };
                prefetch = start_prefetch(inner, mode, gen);
                if !tail.is_empty() {
                    pending_tail = Some(Tail::new(&tail, channels));
                }
            }
            None => {
                // Nothing follows after all: the held-back end plays as is.
                for buf in &tail {
                    if let Err(e) = output.write(buf) {
                        return SessionOutcome::OutputError { message: e.to_string(), track_id };
                    }
                    inner.level_tap.push(buf, channels, rate);
                }
                // The queue is over, so the worker is about to go idle. Bump
                // the generation here so a `PlayPause` that raced the worker
                // clearing `worker_busy` still starts a fresh session.
                inner.generation.fetch_add(1, Ordering::SeqCst);
                return SessionOutcome::QueueEnded;
            }
        }
    }
}

fn worker_main(
    inner: Arc<Inner>,
    make_fetcher: impl FnOnce() -> Arc<dyn Fetcher>,
    ready: SyncSender<()>,
    mut output: Box<dyn AudioOutput>,
) {
    let _ = inner.fetcher.set(make_fetcher());
    let _ = ready.send(());
    let mut open_spec: Option<AudioOutputSpec> = None;
    let mut handled_gen: u64 = 0;
    loop {
        if inner.shutdown.load(Ordering::SeqCst) {
            let _ = output.stop();
            return;
        }
        let gen_now = inner.generation.load(Ordering::SeqCst);
        let ready = {
            let q = inner.queue.lock().unwrap();
            let seeking = inner
                .seek_target
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|t| {
                    t.auto_start && q.current().is_some_and(|i| i.track_id == t.track_id)
                });
            gen_now != handled_gen && q.current().is_some() && (q.is_playing() || seeking)
        };
        if !ready {
            if open_spec.is_some() {
                let _ = output.stop();
                open_spec = None;
            }
            inner.worker_busy.store(false, Ordering::SeqCst);
            let guard = inner.wake_lock.lock().unwrap();
            let _ = inner.wake.wait_timeout(guard, Duration::from_millis(200)).unwrap();
            continue;
        }
        handled_gen = gen_now;
        inner.worker_busy.store(true, Ordering::SeqCst);

        let (item, seek_ms, was_playing) = {
            let _guard = inner.playback_lock.lock().unwrap();
            let q = inner.queue.lock().unwrap();
            let current = q.current().cloned();
            let target = inner.seek_target.lock().unwrap().take();
            let seek_ms = match (&current, target) {
                (Some(item), Some(t)) if t.track_id == item.track_id => Some(t.position_ms),
                _ => None,
            };
            (current, seek_ms, q.is_playing())
        };
        let Some(item) = item else {
            continue;
        };
        if seek_ms.is_none() {
            // A fresh track always starts unpaused.
            inner.paused.store(false, Ordering::SeqCst);
        } else if !was_playing {
            // A seek while the queue is stopped stays paused until the user
            // presses play, so no audio starts behind a "stopped" UI.
            inner.paused.store(true, Ordering::SeqCst);
        }
        // A seek during playback or pause keeps the current paused flag.
        inner.position_ms.store(seek_ms.unwrap_or(0), Ordering::SeqCst);
        inner.position_track.store(item.track_id, Ordering::SeqCst);
        let mode = crate::output::effective_mode(
            *inner.output_mode.lock().unwrap(),
            inner.device_id.lock().unwrap().as_deref(),
        );
        let fallback = *inner.exclusive_fallback.lock().unwrap();

        let outcome = run_session(
            &inner,
            output.as_mut(),
            &mut open_spec,
            &item,
            mode,
            fallback,
            handled_gen,
            seek_ms,
        );
        // The session is over; a resume from here must start a new one.
        inner.worker_busy.store(false, Ordering::SeqCst);
        match outcome {
            SessionOutcome::Superseded => {}
            SessionOutcome::QueueEnded => {
                let _ = output.stop();
                open_spec = None;
            }
            SessionOutcome::OutputError { message, track_id } => {
                {
                    let mut q = inner.queue.lock().unwrap();
                    q.stop();
                }
                *inner.stream_meta.lock().unwrap() = None;
                // Remember where the device failed so the next play press
                // resumes there; `auto_start: false` keeps the worker idle
                // until the user acts.
                *inner.seek_target.lock().unwrap() = Some(PendingSeek {
                    track_id,
                    position_ms: inner.position_ms.load(Ordering::SeqCst),
                    auto_start: false,
                });
                inner.generation.fetch_add(1, Ordering::SeqCst);
                let _ = output.stop();
                open_spec = None;
                {
                    let mut status = inner.output_status.lock().unwrap();
                    status.bit_perfect = false;
                    status.notice = Some(message.clone());
                }
                emit(&inner, EngineEvent::Error { code: "output".into(), message });
                emit(&inner, EngineEvent::State(build_state(&inner)));
            }
            SessionOutcome::UnavailableChain { skipped, last } => {
                {
                    let mut q = inner.queue.lock().unwrap();
                    q.stop();
                }
                *inner.stream_meta.lock().unwrap() = None;
                inner.generation.fetch_add(1, Ordering::SeqCst);
                let _ = output.stop();
                open_spec = None;
                let noun = if skipped == 1 { "track" } else { "tracks" };
                let message = format!("skipped {skipped} unavailable {noun} ({last})");
                emit(
                    &inner,
                    EngineEvent::Error {
                        code: "playback".into(),
                        message,
                    },
                );
                emit(&inner, EngineEvent::State(build_state(&inner)));
            }
            SessionOutcome::StreamError(e) => {
                {
                    let mut q = inner.queue.lock().unwrap();
                    q.stop();
                }
                *inner.stream_meta.lock().unwrap() = None;
                inner.generation.fetch_add(1, Ordering::SeqCst);
                let _ = output.stop();
                open_spec = None;
                let code = match &e {
                    Error::Tidal(oxitide_tidal::Error::Unauthorized) => "auth",
                    _ => "playback",
                };
                emit(&inner, EngineEvent::Error { code: code.into(), message: e.to_string() });
                emit(&inner, EngineEvent::State(build_state(&inner)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheKey, DiskCache};
    use crate::output::NullOutput;
    use crate::source::Segment;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicUsize;

    struct FakeResolver {
        plan: StreamPlan,
    }

    impl StreamResolver for FakeResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            Ok(self.plan.clone())
        }
    }

    struct FailingResolver;

    impl StreamResolver for FailingResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            Err(Error::Manifest("boom".into()))
        }
    }

    /// Fails the first resolve, then succeeds; simulates a transient network
    /// failure that a Retry recovers from.
    struct FlakyResolver {
        plan: StreamPlan,
        calls: AtomicUsize,
    }

    impl StreamResolver for FlakyResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(Error::Fetch("network down".into()))
            } else {
                Ok(self.plan.clone())
            }
        }
    }

    struct AuthFailingResolver;

    impl StreamResolver for AuthFailingResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            Err(Error::Tidal(oxitide_tidal::Error::Unauthorized))
        }
    }

    /// Serves `plan` for every track except the listed ids, which resolve to a
    /// TIDAL 403 like a region-locked or removed track.
    struct UnavailableResolver {
        plan: StreamPlan,
        unavailable: Vec<u64>,
    }

    impl StreamResolver for UnavailableResolver {
        fn resolve(&self, track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            if self.unavailable.contains(&track_id) {
                Err(Error::Tidal(oxitide_tidal::Error::Forbidden))
            } else {
                Ok(self.plan.clone())
            }
        }
    }

    fn fixture_plan() -> (StreamPlan, HashMap<String, Vec<u8>>) {
        let bytes = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let plan = StreamPlan {
            init_url: None,
            segments: vec![Segment { url: "mem://tone".into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: Default::default(),
        };
        let mut files = HashMap::new();
        files.insert("mem://tone".to_string(), bytes);
        (plan, files)
    }

    fn item(id: u64) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 2,
            cover: None,
        }
    }

    fn test_cache() -> Arc<DiskCache> {
        static CACHE_COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "oxitide-engine-cache-{}-{}",
            std::process::id(),
            CACHE_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        DiskCache::new(dir, 64 * 1024 * 1024)
    }

    fn build_engine(
        resolver: Arc<dyn StreamResolver>,
        fetcher: Arc<dyn Fetcher>,
        output: Box<dyn AudioOutput>,
        quality: Quality,
    ) -> Engine {
        Engine::new(resolver, move || fetcher, output, quality, test_cache(), true)
    }

    fn cached_plan(url: &str, manifest_hash: u64) -> StreamPlan {
        StreamPlan {
            init_url: None,
            segments: vec![Segment { url: url.into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            bitrate_kbps: None,
            quality: None,
            manifest_hash: Some(manifest_hash),
            replay_gain: Default::default(),
        }
    }

    struct CountingFetcher {
        files: HashMap<String, Vec<u8>>,
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Fetcher for CountingFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.files
                .get(url)
                .cloned()
                .ok_or_else(|| Error::Fetch(format!("no fixture for {url}")))
        }
    }

    fn engine_with(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
    ) -> (Engine, NullOutput) {
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        (engine, output)
    }

    fn engine_with_policy(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
        allow_fallback: bool,
    ) -> (Engine, NullOutput) {
        let output = NullOutput::new();
        let fetcher: Arc<dyn Fetcher> = Arc::new(crate::fetch::MapFetcher { files });
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            move || fetcher,
            Box::new(output.clone()),
            Quality::Lossless,
            test_cache(),
            allow_fallback,
        );
        (engine, output)
    }

    #[test]
    fn fallback_disabled_errors_on_a_lower_served_quality() {
        let (mut plan, files) = fixture_plan();
        plan.quality = Some(Quality::Low);
        let (engine, output) = engine_with_policy(plan, files, false);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| {
            seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. }))
        });
        let message = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                EngineEvent::Error { message, .. } => Some(message.clone()),
                _ => None,
            })
            .expect("no error event");
        assert!(
            message.contains("track unavailable at low"),
            "unexpected error message: {message}"
        );
        assert!(!engine.state().playing);
        assert_eq!(engine.state().queue.len(), 1, "the queue must be kept");
        assert!(output.samples().is_empty(), "a rejected stream must not write audio");
    }

    #[test]
    fn fallback_enabled_plays_and_reports_the_served_quality() {
        let (mut plan, files) = fixture_plan();
        plan.quality = Some(Quality::Low);
        let (engine, output) = engine_with_policy(plan, files, true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);

        let meta = engine.state().stream.expect("no stream metadata");
        assert_eq!(meta.quality, Some(Quality::Low));
        assert_eq!(meta.sample_rate, 44100);
        assert_eq!(meta.bit_depth, 16);
        assert_eq!(meta.channels, 2);
        assert_eq!(meta.codec.as_deref(), Some("flac"));
    }

    #[test]
    fn he_aac_streams_error_clearly_without_fetching() {
        let (mut plan, files) = fixture_plan();
        plan.codec = Some("mp4a.40.5".into());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(output.clone()),
            Quality::High,
            test_cache(),
            true,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })));
        let message = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                EngineEvent::Error { message, .. } => Some(message.clone()),
                _ => None,
            })
            .expect("no error event");
        assert!(message.contains("HE-AAC"), "unexpected message: {message}");
        assert_eq!(calls.load(Ordering::SeqCst), 0, "an unsupported stream must not be fetched");
        assert!(output.samples().is_empty());
        assert_eq!(engine.state().queue.len(), 1, "the queue must be kept");
    }

    #[test]
    fn he_aac_guard_precedes_the_cache_lookup() {
        let (mut plan, files) = fixture_plan();
        plan.codec = Some("mp4a.40.5".into());
        plan.manifest_hash = Some(0xabcd);
        let tone = files["mem://tone"].clone();
        let cache = test_cache();
        let key = CacheKey { track_id: 1, quality: Quality::High, manifest_hash: 0xabcd };
        {
            let mut writer = cache.begin_write(&key).unwrap();
            writer.write_all(&tone).unwrap();
            writer.commit();
        }
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(output.clone()),
            Quality::High,
            cache.clone(),
            true,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })));
        assert!(output.samples().is_empty(), "the cached entry must not be decoded");
        assert!(cache.lookup(&key).is_some(), "the guard must not touch the cache");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unknown_served_quality_plays() {
        let (mut plan, files) = fixture_plan();
        plan.quality = None;
        let (engine, output) = engine_with_policy(plan, files, false);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(output.samples().len(), 176_400);
        assert_eq!(engine.state().stream.and_then(|m| m.quality), None);
    }

    #[test]
    fn single_file_bitrate_is_computed_from_bytes_and_duration() {
        let (plan, files) = fixture_plan();
        let bytes = files["mem://tone"].len() as u64;
        let (engine, output) = engine_with_policy(plan, files, true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);

        let meta = engine.state().stream.expect("no stream metadata");
        assert_eq!(meta.bitrate_kbps, Some((bytes * 8 / 2 / 1000) as u32));
    }

    #[test]
    fn manifest_bitrate_wins_over_the_computed_one() {
        let (mut plan, files) = fixture_plan();
        plan.bitrate_kbps = Some(828);
        let (engine, output) = engine_with_policy(plan, files, true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(engine.state().stream.and_then(|m| m.bitrate_kbps), Some(828));
    }

    #[test]
    fn set_allow_fallback_applies_from_the_next_track() {
        let (mut plan, files) = fixture_plan();
        plan.quality = Some(Quality::Low);
        let (engine, output) = engine_with_policy(plan, files, false);
        engine.set_allow_fallback(true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(output.samples().len(), 176_400);
    }

    fn wait_for(mut ready: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !ready() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Regression: `HttpFetcher` builds a `reqwest::blocking::Client`, which
    /// panics in debug builds when constructed inside an async runtime context.
    /// The engine must build it on its own worker thread.
    #[tokio::test]
    async fn fetcher_is_built_off_the_async_runtime() {
        let output = NullOutput::new();
        let _engine = Engine::new(
            Arc::new(FailingResolver),
            || {
                assert!(
                    tokio::runtime::Handle::try_current().is_err(),
                    "fetcher was built on an async runtime thread"
                );
                Arc::new(crate::fetch::MapFetcher::default())
            },
            Box::new(output),
            Quality::Lossless,
            test_cache(),
            true,
        );
    }

    #[test]
    fn play_tracks_emits_now_playing_then_state() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        let events = seen.lock().unwrap();
        assert!(matches!(events[0], EngineEvent::NowPlaying(Some(_))));
        assert!(events.iter().take(4).any(|e| matches!(e, EngineEvent::State(_))));
    }

    #[test]
    fn play_track_into_decodes_fixture() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let mut out = NullOutput::new();
        engine.play_track_into(&item(1), &mut out).unwrap();
        assert_eq!(out.samples().len(), 176_400);
        assert_eq!(out.opens(), 1);
    }

    #[test]
    fn clear_emits_empty_now_playing() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::Clear).unwrap();
        let events = seen.lock().unwrap();
        assert!(matches!(events[0], EngineEvent::NowPlaying(None)));
    }

    #[test]
    fn enqueue_appends_without_restarting_playback() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !output.samples().is_empty());
        let opens = output.opens();

        engine.handle(EngineCmd::Enqueue { items: vec![item(2), item(3)] }).unwrap();

        let state = engine.state();
        assert_eq!(state.queue.len(), 3);
        assert_eq!(state.current.map(|i| i.track_id), Some(1));
        assert!(state.playing);
        assert_eq!(output.opens(), opens, "enqueue must not restart the session");
    }

    #[test]
    fn level_tap_measures_written_audio_only_while_enabled() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        let tap = engine.level_tap();
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !output.samples().is_empty());
        assert_eq!(tap.read(), crate::meter::LevelFrame::silent(), "disabled tap must stay silent");

        tap.set_enabled(true);
        let mut peak = 0.0f32;
        let started = std::time::Instant::now();
        while peak <= 0.0 && started.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(20));
            peak = tap.read().peak[0];
        }
        assert!(peak > 0.0, "an enabled tap must see the tone");
    }

    #[test]
    fn worker_writes_samples_and_advances() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for(|| output.samples().len() >= 176_400);
        assert!(output.samples().len() >= 176_400);
    }

    #[test]
    fn seek_restarts_at_the_requested_offset() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().position_ms > 100);

        engine.handle(EngineCmd::Seek(1000)).unwrap();
        // The handler seeds `position_ms`, so wait for the worker to actually
        // restart (flush) before checking the post-seek state.
        wait_for(|| output.flushes() >= 1);
        wait_for(|| engine.state().position_ms >= 1000);
        let state = engine.state();
        assert!(state.position_ms >= 1000, "seek did not land at 1000ms: {}", state.position_ms);
        assert!(state.playing);
        assert_eq!(
            output.opens(),
            1,
            "a same-format seek must flush the open device, not reopen it"
        );
    }

    /// Regression: a seek issued while paused must apply (position + slider)
    /// and playback must resume from the target, not from the old position.
    #[test]
    fn seek_while_paused_applies_and_resumes_from_the_target() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().position_ms > 100);

        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| !engine.state().playing);

        engine.handle(EngineCmd::Seek(1500)).unwrap();
        wait_for(|| engine.state().position_ms >= 1500);
        assert!(!engine.state().playing, "seek while paused must not resume playback");

        // Let the superseded session's in-flight buffer land, then require the
        // sample count to be stable: a paused seek must not write audio.
        let mut settled = output.samples().len();
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(150));
            let now = output.samples().len();
            if now == settled {
                break;
            }
            settled = now;
        }
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(
            output.samples().len(),
            settled,
            "paused seek must not write audio"
        );

        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| output.samples().len() > settled);
        assert!(engine.state().position_ms >= 1500);
    }

    /// Regression: seeking after the queue ended must not start audio while
    /// the UI still reports stopped.
    #[test]
    fn seek_after_queue_end_stays_paused_until_resumed() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);

        engine.handle(EngineCmd::Seek(1000)).unwrap();
        wait_for(|| engine.state().position_ms >= 1000);
        assert!(!engine.state().playing, "seek after queue end must not report playing");

        let mut settled = output.samples().len();
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(150));
            let now = output.samples().len();
            if now == settled {
                break;
            }
            settled = now;
        }
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(
            output.samples().len(),
            settled,
            "stopped seek must not write audio"
        );

        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| output.samples().len() > settled);
    }

    /// Regression: `PlayPause` on an empty queue must not latch `paused` true
    /// and silence the next `PlayTracks`.
    #[test]
    fn play_pause_on_empty_queue_does_not_silence_later_playback() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayPause).unwrap();
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| output.samples().len() >= 176_400);
        assert!(output.samples().len() >= 176_400);
    }

    /// Regression: `Position` events must stop once the queue ends instead of
    /// ticking forever.
    #[test]
    fn position_events_stop_when_queue_ends() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| !engine.state().playing);
        assert!(!engine.state().playing, "single track did not finish");

        let positions = || {
            seen.lock()
                .unwrap()
                .iter()
                .filter(|e| matches!(e, EngineEvent::Position { .. }))
                .count()
        };
        std::thread::sleep(Duration::from_millis(600));
        let before = positions();
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(positions(), before, "position events kept coming after the queue ended");
    }

    struct Gate {
        released: Mutex<std::collections::HashSet<u64>>,
        cond: std::sync::Condvar,
    }

    impl Gate {
        fn new() -> Self {
            Self { released: Mutex::new(std::collections::HashSet::new()), cond: std::sync::Condvar::new() }
        }
        fn wait(&self, id: u64) {
            let mut released = self.released.lock().unwrap();
            while !released.contains(&id) {
                released = self.cond.wait(released).unwrap();
            }
        }
        fn release(&self, id: u64) {
            self.released.lock().unwrap().insert(id);
            self.cond.notify_all();
        }
    }

    struct GatedResolver {
        plan: StreamPlan,
        gate: Arc<Gate>,
        order: Arc<Mutex<Vec<u64>>>,
    }

    impl StreamResolver for GatedResolver {
        fn resolve(&self, track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            self.order.lock().unwrap().push(track_id);
            self.gate.wait(track_id);
            Ok(self.plan.clone())
        }
    }

    fn wait_for_order(order: &Arc<Mutex<Vec<u64>>>, id: u64) {
        wait_for(|| order.lock().unwrap().contains(&id));
        assert!(order.lock().unwrap().contains(&id), "track {id} was never resolved");
    }

    /// Regression: a `Next` issued while a track is playing must not cause a
    /// double advance (duplicate or skipped track). With lookahead, a
    /// superseding `Next` abandons the in-flight prefetch and the new session
    /// may resolve the new current track again, so playback order is checked
    /// from `NowPlaying` events rather than resolver calls.
    #[test]
    fn next_while_playing_does_not_duplicate_or_skip() {
        let (plan, files) = fixture_plan();
        let gate = Arc::new(Gate::new());
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(GatedResolver { plan, gate: gate.clone(), order: order.clone() }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output),
            Quality::Lossless,
        );
        let played = Arc::new(Mutex::new(Vec::new()));
        let played_ref = played.clone();
        engine.subscribe(Arc::new(move |e| {
            if let EngineEvent::NowPlaying(Some(item)) = e {
                played_ref.lock().unwrap().push(item.track_id);
            }
        }));

        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2), item(3)], start: 0 })
            .unwrap();
        wait_for_order(&order, 1);
        // Let track 1 complete and auto-advance to track 2, then press Next.
        gate.release(1);
        wait_for_order(&order, 2);
        engine.handle(EngineCmd::Next).unwrap();
        wait_for_order(&order, 3);
        gate.release(2);
        gate.release(3);

        wait_for(|| !engine.state().playing);
        assert_eq!(*played.lock().unwrap(), vec![1, 2, 3]);
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(3));
    }

    /// Regression: a failed resolve/decode must stop the queue and report.
    #[test]
    fn playback_error_stops_and_reports() {
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FailingResolver),
            Arc::new(crate::fetch::MapFetcher::default()),
            Box::new(output),
            Quality::Lossless,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| {
            seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. }))
        });
        assert!(
            seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })),
            "no error event emitted"
        );
        assert!(!engine.state().playing, "queue still reports playing after an error");
    }

    #[test]
    fn retry_after_a_stream_error_restarts_the_current_track() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FlakyResolver { plan, calls: AtomicUsize::new(0) }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })));
        assert!(!engine.state().playing, "a stream error must stop the queue");

        engine.retry().unwrap();

        wait_for(|| output.samples().len() > 0);
        assert!(engine.state().playing, "retry did not resume playback");
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(1));
    }

    #[test]
    fn retry_resumes_at_the_recorded_position() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| output.samples().len() > 0);

        engine.handle(EngineCmd::Seek(1000)).unwrap();
        wait_for(|| engine.state().position_ms >= 1000);
        // Simulate the stop a stream error performs, keeping the position.
        engine.inner.queue.lock().unwrap().stop();
        let before = output.samples().len();

        engine.retry().unwrap();

        wait_for(|| output.samples().len() > before);
        assert!(engine.state().playing, "retry did not resume playback");
        assert!(
            engine.state().position_ms >= 1000,
            "retry restarted from the beginning: {}",
            engine.state().position_ms
        );
    }

    #[test]
    fn retry_without_a_current_item_is_a_noop() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        let before = engine.inner.generation.load(Ordering::SeqCst);

        engine.handle(EngineCmd::Retry).unwrap();

        assert_eq!(engine.inner.generation.load(Ordering::SeqCst), before);
        assert!(output.samples().is_empty());
        assert!(!engine.state().playing);
    }

    #[test]
    fn unauthorized_stream_error_is_coded_auth() {
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(AuthFailingResolver),
            Arc::new(crate::fetch::MapFetcher::default()),
            Box::new(output),
            Quality::Lossless,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })));
        let code = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                EngineEvent::Error { code, .. } => Some(code.clone()),
                _ => None,
            })
            .expect("no error event");
        assert_eq!(code, "auth");
    }

    fn engine_with_unavailable(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
        unavailable: Vec<u64>,
    ) -> (Engine, NullOutput) {
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(UnavailableResolver { plan, unavailable }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        (engine, output)
    }

    /// A region-locked / removed track (resolve 403) must be skipped so the
    /// rest of the queue keeps playing, without an error reaching the UI.
    #[test]
    fn unavailable_track_is_skipped_and_playback_continues() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with_unavailable(plan, files, vec![2]);
        let errors = Arc::new(Mutex::new(Vec::new()));
        let errors_ref = errors.clone();
        let played = Arc::new(Mutex::new(Vec::new()));
        let played_ref = played.clone();
        engine.subscribe(Arc::new(move |e| match e {
            EngineEvent::Error { message, .. } => errors_ref.lock().unwrap().push(message),
            EngineEvent::NowPlaying(Some(item)) => played_ref.lock().unwrap().push(item.track_id),
            _ => {}
        }));

        engine
            .handle(EngineCmd::PlayTracks {
                items: vec![item(1), item(2), item(3)],
                start: 0,
            })
            .unwrap();

        wait_for(|| !engine.state().playing);
        assert_eq!(
            output.samples().len(),
            2 * 176_400,
            "only the two playable tracks may decode"
        );
        assert!(
            errors.lock().unwrap().is_empty(),
            "a skipped track must not surface an error"
        );
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(3));
        let played = played.lock().unwrap();
        assert!(
            played.contains(&1) && played.contains(&3),
            "playback did not reach the later track: {played:?}"
        );
    }

    /// When nothing in the queue can play, stop once and report a single
    /// summary instead of stopping on the first unavailable track.
    #[test]
    fn all_unavailable_tracks_stop_with_a_summary() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with_unavailable(plan, files, vec![1, 2, 3]);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine
            .handle(EngineCmd::PlayTracks {
                items: vec![item(1), item(2), item(3)],
                start: 0,
            })
            .unwrap();

        wait_for(|| {
            seen.lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, EngineEvent::Error { .. }))
        });
        let message = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                EngineEvent::Error { message, .. } => Some(message.clone()),
                _ => None,
            })
            .expect("no error event");
        assert!(
            message.contains("skipped 3 unavailable tracks"),
            "unexpected message: {message}"
        );
        assert!(
            message.contains("forbidden"),
            "the reason must be preserved: {message}"
        );
        assert!(
            !engine.state().playing,
            "the queue must stop when nothing can play"
        );
        assert_eq!(engine.state().queue.len(), 3, "the queue must be kept");
        assert!(
            output.samples().is_empty(),
            "nothing playable must write no audio"
        );
    }

    /// Repeat-all must not turn an all-unavailable queue into an infinite skip
    /// loop; each track is tried once and then playback stops.
    #[test]
    fn all_unavailable_tracks_stop_even_under_repeat_all() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with_unavailable(plan, files, vec![1, 2]);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine
            .handle(EngineCmd::SetRepeat(RepeatMode::All))
            .unwrap();
        engine
            .handle(EngineCmd::PlayTracks {
                items: vec![item(1), item(2)],
                start: 0,
            })
            .unwrap();

        wait_for(|| {
            seen.lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, EngineEvent::Error { .. }))
        });
        let message = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                EngineEvent::Error { message, .. } => Some(message.clone()),
                _ => None,
            })
            .expect("no error event");
        assert!(
            message.contains("skipped 2 unavailable tracks"),
            "unexpected message: {message}"
        );
        assert!(!engine.state().playing);
        assert!(output.samples().is_empty());
    }

    /// `PlayShuffled` must load every item, turn the shuffle flag on, and
    /// start at the top of the shuffled order.
    #[test]
    fn play_shuffled_starts_the_queue_shuffled() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        engine
            .handle(EngineCmd::PlayShuffled {
                items: vec![item(1), item(2), item(3)],
            })
            .unwrap();
        let state = engine.state();
        assert!(state.shuffle, "the shuffle flag must turn on");
        assert!(state.playing);
        assert_eq!(
            state.current.map(|i| i.track_id),
            state.queue.first().map(|i| i.track_id),
            "playback must start at the top of the shuffled order"
        );
        let mut ids: Vec<u64> = state.queue.iter().map(|i| i.track_id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3], "the queue must keep every item");
    }

    #[test]
    fn restore_session_starts_paused_and_fetches_nothing() {
        let (plan, files) = fixture_plan();
        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = Arc::new(CountingFetcher { files, calls: calls.clone() });
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            fetcher,
            Box::new(NullOutput::new()),
            Quality::Lossless,
        );
        engine
            .handle(EngineCmd::RestoreSession {
                items: vec![item(1), item(2), item(3)],
                start: 1,
                position_ms: 4200,
                shuffle: true,
                repeat: RepeatMode::All,
            })
            .unwrap();
        let state = engine.state();
        assert!(!state.playing, "restore must not start playback");
        assert_eq!(state.current_index, Some(1));
        assert_eq!(state.position_ms, 4200);
        assert!(state.shuffle);
        assert_eq!(state.repeat, RepeatMode::All);
        assert_eq!(
            state.queue.iter().map(|i| i.track_id).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "restore must keep the saved order even with shuffle on"
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "restore must not fetch media");
    }

    #[test]
    fn restore_session_resumes_at_the_saved_position() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        engine
            .handle(EngineCmd::RestoreSession {
                items: vec![item(1), item(2)],
                start: 0,
                position_ms: 1200,
                shuffle: false,
                repeat: RepeatMode::Off,
            })
            .unwrap();
        engine.handle(EngineCmd::PlayPause).unwrap();
        let state = engine.state();
        assert!(state.playing);
        assert_eq!(state.current_index, Some(0));
    }

    /// Review Focus: the sleep fade must ramp through intermediate volumes,
    /// reach silence, pause, and put the user's volume back.
    #[test]
    fn fade_and_pause_ramps_to_zero_then_pauses_and_restores_volume() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        let events = Arc::new(Mutex::new(Vec::new()));
        engine.subscribe(Arc::new({
            let events = events.clone();
            move |event| events.lock().unwrap().push(event)
        }));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 150 }).unwrap();
        wait_for(|| !engine.state().playing);

        assert!(
            output.flushes() >= 1,
            "completion must discard the audio staged at the faded volume"
        );
        assert_eq!(engine.state().volume, 1.0, "pre-fade volume must be restored");
        wait_for(|| output.volumes().last() == Some(&1.0));
        let volumes = output.volumes();
        assert!(
            volumes.iter().any(|v| *v > 0.05 && *v < 0.95),
            "no intermediate ramp value: {volumes:?}"
        );
        assert!(
            volumes.iter().any(|v| v.abs() <= f32::EPSILON),
            "the ramp never reached silence: {volumes:?}"
        );
        assert_eq!(volumes.last(), Some(&1.0), "volume not restored after the fade");
        let done = events.lock().unwrap().iter().any(|event| {
            matches!(event, EngineEvent::State(state) if !state.playing && state.volume == 1.0)
        });
        assert!(done, "no completion State emitted");
    }

    /// Review Focus: play/pause during a fade cancels it and restores volume.
    #[test]
    fn play_pause_cancels_an_active_fade_and_restores_volume() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 60_000 }).unwrap();
        wait_for(|| output.volumes().iter().any(|v| *v < 0.95));

        engine.handle(EngineCmd::PlayPause).unwrap();
        assert!(!engine.state().playing);
        assert_eq!(engine.state().volume, 1.0);
        wait_for(|| output.volumes().last() == Some(&1.0));

        // Resuming must not inherit a leftover fade that pauses again.
        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| engine.state().playing);
        std::thread::sleep(Duration::from_millis(300));
        assert!(engine.state().playing, "the cancelled fade must not pause later");
        // A stale ramp step can land just after the cancel; the worker restores
        // the user volume on its next pass, so wait rather than race the sleep.
        wait_for(|| output.volumes().last() == Some(&1.0));
    }

    /// Review Focus: a volume change during a fade cancels it (the user wins).
    #[test]
    fn set_volume_cancels_an_active_fade() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 60_000 }).unwrap();
        wait_for(|| output.volumes().iter().any(|v| *v < 0.95));

        engine.handle(EngineCmd::SetVolume(0.25)).unwrap();
        wait_for(|| output.volumes().last() == Some(&0.25));
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(output.volumes().last(), Some(&0.25), "the fade kept ramping");
        assert!(engine.state().playing, "the fade must not pause after cancel");
    }

    /// Review Focus: FadeAndPause on an already-paused queue is inert.
    #[test]
    fn fade_and_pause_on_a_paused_queue_changes_nothing() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());
        engine.handle(EngineCmd::PlayPause).unwrap();
        assert!(!engine.state().playing);
        let before = output.volumes().len();

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 50 }).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        let volumes = output.volumes();
        assert_eq!(volumes.len(), before, "a paused queue must not fade: {volumes:?}");
        assert_eq!(volumes.last(), Some(&1.0));
        assert!(!engine.state().playing);
    }

    /// Review Focus: a zero-duration fade pauses immediately, with no ramp.
    #[test]
    fn fade_and_pause_with_zero_duration_pauses_immediately() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 0 }).unwrap();
        wait_for(|| !engine.state().playing);
        assert!(
            output.volumes().iter().all(|v| *v == 1.0),
            "zero duration must not ramp: {:?}",
            output.volumes()
        );
    }

    /// Review Focus: the effective output volume follows the fade, not the user
    /// volume, and cancelling restores the user volume.
    #[test]
    fn current_volume_reads_the_fade_target_while_fading() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        engine.handle(EngineCmd::SetVolume(0.8)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 60_000 }).unwrap();
        wait_for(|| current_volume(&engine.inner).0 < 0.8);
        let (volume, fading) = current_volume(&engine.inner);
        assert!(fading, "the fade must drive the output volume");
        assert!(volume < 0.8, "the fade target must win, got {volume}");

        engine.handle(EngineCmd::PlayPause).unwrap();
        let (volume, fading) = current_volume(&engine.inner);
        assert!(!fading, "play/pause must cancel the fade");
        assert!((volume - 0.8).abs() < f32::EPSILON, "user volume not restored: {volume}");
    }

    /// Test output that blocks on the zero-volume call so the test can cancel
    /// a fade exactly between the worker's fade read and its completion.
    struct GatedVolumeOutput {
        inner: NullOutput,
        entered: Arc<(Mutex<bool>, Condvar)>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    impl AudioOutput for GatedVolumeOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            self.inner.open(spec)
        }
        fn start(&mut self) -> Result<()> {
            self.inner.start()
        }
        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            self.inner.write(buf)
        }
        fn set_volume(&mut self, volume: f32) -> Result<()> {
            if volume <= f32::EPSILON {
                let (entered, cond) = &*self.entered;
                *entered.lock().unwrap() = true;
                cond.notify_all();
                let (release, cond) = &*self.release;
                let mut released = release.lock().unwrap();
                while !*released {
                    released = cond.wait(released).unwrap();
                }
            }
            self.inner.set_volume(volume)
        }
        fn flush(&mut self) -> Result<()> {
            self.inner.flush()
        }
        fn stop(&mut self) -> Result<()> {
            self.inner.stop()
        }
        fn status(&self) -> OutputStatus {
            self.inner.status()
        }
    }

    /// Review Focus: a cancel that races the fade's final step must not stop
    /// playback or zero the volume.
    #[test]
    fn cancelling_at_fade_completion_does_not_stop_playback_or_zero_the_volume() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let entered = Arc::new((Mutex::new(false), Condvar::new()));
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(GatedVolumeOutput {
                inner: output.clone(),
                entered: entered.clone(),
                release: release.clone(),
            }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 1 }).unwrap();
        {
            let (flag, cond) = &*entered;
            let mut blocked = flag.lock().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !*blocked && Instant::now() < deadline {
                let (guard, _) = cond.wait_timeout(blocked, Duration::from_millis(50)).unwrap();
                blocked = guard;
            }
            assert!(*blocked, "the fade never reached the output");
        }

        // Cancel while the worker is parked on the final zero-volume write.
        engine.handle(EngineCmd::SetVolume(0.5)).unwrap();
        {
            let (flag, cond) = &*release;
            *flag.lock().unwrap() = true;
            cond.notify_all();
        }

        std::thread::sleep(Duration::from_millis(200));
        assert!(engine.state().playing, "the cancelled fade must not stop playback");
        assert_eq!(engine.state().volume, 0.5, "the cancelled fade must not zero the volume");
    }

    /// Regression: `Next` at the end of the queue with repeat off must not
    /// restart the last track.
    #[test]
    fn next_at_end_with_repeat_off_does_not_restart() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing);
        assert!(!engine.state().playing, "single track did not finish");

        let before = output.samples().len();
        assert!(before >= 176_400);
        engine.handle(EngineCmd::Next).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(output.samples().len(), before, "Next at the end restarted playback");
        assert!(!engine.state().playing, "Next at the end left the queue playing");
    }

    struct GatedFetcher {
        files: HashMap<String, Vec<u8>>,
        gate: Arc<Gate>,
        id: u64,
    }

    impl Fetcher for GatedFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>> {
            self.gate.wait(self.id);
            self.files
                .get(url)
                .cloned()
                .ok_or_else(|| Error::Fetch(format!("no fixture for {url}")))
        }
    }

    /// Regression: `Prev` at the first track is a no-op and must leave the
    /// running session (and its generation) untouched.
    #[test]
    fn prev_at_first_track_does_not_restart() {
        let (plan, files) = fixture_plan();
        let gate = Arc::new(Gate::new());
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(GatedFetcher { files, gate: gate.clone(), id: 1 }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();
        // Playback is parked in the fetcher, so nothing has been written yet.
        std::thread::sleep(Duration::from_millis(200));
        let before_gen = engine.inner.generation.load(Ordering::SeqCst);
        let before_samples = output.samples().len();

        engine.handle(EngineCmd::Prev).unwrap();

        assert_eq!(
            engine.inner.generation.load(Ordering::SeqCst),
            before_gen,
            "no-op Prev bumped the generation"
        );
        assert!(engine.state().playing, "no-op Prev stopped playback");
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(1));
        assert_eq!(output.samples().len(), before_samples);
        gate.release(1);
        gate.release(2);
    }

    /// An output that records the spec passed to `open`.
    struct RecordingOutput {
        specs: Arc<Mutex<Vec<AudioOutputSpec>>>,
    }

    impl AudioOutput for RecordingOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            self.specs.lock().unwrap().push(spec.clone());
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            OutputStatus::default()
        }
    }

    #[test]
    fn set_output_device_is_passed_to_open() {
        let (plan, files) = fixture_plan();
        let specs = Arc::new(Mutex::new(Vec::new()));
        let specs_ref = specs.clone();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(RecordingOutput { specs: specs_ref }),
            Quality::Lossless,
        );
        engine.set_output_device(Some("device-x".into()));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| !specs.lock().unwrap().is_empty());
        assert_eq!(specs.lock().unwrap()[0].device_id.as_deref(), Some("device-x"));
        assert_eq!(specs.lock().unwrap()[0].mode, OutputMode::Shared);
    }

    #[test]
    fn asio_device_always_opens_exclusive() {
        let (plan, files) = fixture_plan();
        let specs = Arc::new(Mutex::new(Vec::new()));
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(RecordingOutput { specs: specs.clone() }),
            Quality::Lossless,
        );
        engine.set_output_device(Some("asio:Topping USB Audio Device".into()));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| !specs.lock().unwrap().is_empty());
        assert_eq!(specs.lock().unwrap()[0].mode, OutputMode::Exclusive);
    }

    #[test]
    fn perf_hook_receives_decode_cache_and_first_pcm_events() {
        let (plan, files) = fixture_plan();
        let events: Arc<Mutex<Vec<crate::perf::AudioPerfEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(NullOutput::new()),
            Quality::Lossless,
        );
        engine.set_perf_hook(Arc::new(move |event| sink.lock().unwrap().push(event)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| {
            events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, crate::perf::AudioPerfEvent::FirstPcm { track_id: 1 }))
        });
        let events = events.lock().unwrap().clone();
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::perf::AudioPerfEvent::DecodeStart { track_id: 1 })));
        // The fixture plan has no manifest hash, so it cannot be cached.
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::perf::AudioPerfEvent::CacheMiss { track_id: 1 })));
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::perf::AudioPerfEvent::ResolveStart { track_id: 1 })));
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::perf::AudioPerfEvent::InfoReady { track_id: 1 })));
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::perf::AudioPerfEvent::OutputOpened { track_id: 1 })));
    }

    /// An output that always reports the given status.
    struct FixedStatusOutput {
        status: OutputStatus,
    }

    impl AudioOutput for FixedStatusOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            self.status.clone()
        }
    }

    #[test]
    fn null_output_reports_exclusive_status() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.audio_status().mode == OutputMode::Exclusive);
        assert_eq!(engine.audio_status().mode, OutputMode::Exclusive);
        assert!(engine.audio_status().bit_perfect);
    }

    #[test]
    fn output_status_notice_flows_to_playback_state() {
        let (plan, files) = fixture_plan();
        let status = OutputStatus {
            mode: OutputMode::Exclusive,
            bit_perfect: false,
            notice: Some("volume applied in software; bit-perfect compromised".into()),
        };
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(FixedStatusOutput { status }),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.state().bit_perfect_notice.is_some());
        let state = engine.state();
        assert_eq!(state.output_mode, OutputMode::Exclusive);
        assert!(!state.bit_perfect);
        assert_eq!(
            state.bit_perfect_notice.as_deref(),
            Some("volume applied in software; bit-perfect compromised")
        );
    }

    /// Review Focus: an output that cannot open (e.g. exclusive unsupported
    /// with `warn_only`) must pause with a notice, keep the queue, and never
    /// panic.
    struct FailingOpenOutput;

    impl AudioOutput for FailingOpenOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            Err(Error::Output("exclusive mode is not supported by this output device".into()))
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            OutputStatus::default()
        }
    }

    #[test]
    fn output_open_failure_pauses_with_notice() {
        let (plan, files) = fixture_plan();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(FailingOpenOutput),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.state().bit_perfect_notice.is_some());
        let state = engine.state();
        assert!(!state.playing, "open failure must pause, not stop the queue");
        assert_eq!(state.current.map(|i| i.track_id), Some(1));
        assert_eq!(state.queue.len(), 1);
        assert!(state.bit_perfect_notice.is_some());
    }

    /// An output that fails every write until `failing` is cleared, then
    /// records samples. Simulates an unplugged device.
    struct DeviceLostOutput {
        failing: Arc<AtomicBool>,
        samples: Arc<Mutex<Vec<f32>>>,
        opens: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl AudioOutput for DeviceLostOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            if self.failing.load(Ordering::SeqCst) {
                return Err(Error::Output("output device unavailable".into()));
            }
            if let PcmBuffer::F32(v) = buf {
                self.samples.lock().unwrap().extend_from_slice(v);
            }
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            let mut status = OutputStatus::default();
            if self.failing.load(Ordering::SeqCst) {
                status.notice = Some("output device disconnected".into());
            }
            status
        }
    }

    /// Review Focus: losing the device mid-track pauses with a notice, keeps
    /// the queue, and recovers on the next play press.
    #[test]
    fn output_failure_pauses_keeps_queue_and_recovers() {
        let (plan, files) = fixture_plan();
        let failing = Arc::new(AtomicBool::new(true));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(DeviceLostOutput {
                failing: failing.clone(),
                samples: samples.clone(),
                opens: opens.clone(),
            }),
            Quality::Lossless,
        );
        // (failure_seen, cleared_after_failure): the UI hides the bar notice
        // while an error is active, so a recovered output must republish a
        // state whose notice is gone or the stale text lingers on screen.
        let watch = Arc::new(Mutex::new((false, false)));
        let watch_ref = watch.clone();
        engine.subscribe(Arc::new(move |event| {
            if let EngineEvent::State(state) = &event {
                let mut watch = watch_ref.lock().unwrap();
                if state.bit_perfect_notice.is_some() {
                    watch.0 = true;
                } else if watch.0 {
                    watch.1 = true;
                }
            }
        }));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.state().bit_perfect_notice.is_some());
        let state = engine.state();
        assert!(!state.playing, "device loss must pause playback");
        assert_eq!(state.current.map(|i| i.track_id), Some(1));
        assert_eq!(state.queue.len(), 1, "device loss must keep the queue");
        assert!(state.bit_perfect_notice.is_some());

        // The device comes back; the next user action recovers the track.
        failing.store(false, Ordering::SeqCst);
        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| samples.lock().unwrap().len() > 0);
        assert!(
            watch.lock().unwrap().1,
            "recovery must publish a state with the output notice cleared"
        );
        assert!(engine.state().playing, "recovery did not resume playback");
        wait_for(|| !engine.state().playing);
        assert_eq!(samples.lock().unwrap().len(), 176_400);
        assert!(opens.load(Ordering::SeqCst) >= 2, "recovery must reopen the output");
    }

    /// Writes until `remaining` samples are consumed, then fails every write.
    struct FailAfterOutput {
        remaining: Arc<Mutex<usize>>,
        samples: Arc<Mutex<Vec<f32>>>,
    }

    impl AudioOutput for FailAfterOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            let mut remaining = self.remaining.lock().unwrap();
            if *remaining == 0 {
                return Err(Error::Output("output device unavailable".into()));
            }
            if let PcmBuffer::F32(v) = buf {
                let take = (*remaining).min(v.len());
                self.samples.lock().unwrap().extend_from_slice(&v[..take]);
                *remaining -= take;
            }
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            OutputStatus::default()
        }
    }

    /// Regression: a device-loss resume target must never apply to a different
    /// track (e.g. a new album started after the failure).
    #[test]
    fn device_loss_target_does_not_leak_into_the_next_play_context() {
        let (plan, files) = fixture_plan();
        let remaining = Arc::new(Mutex::new(44_100)); // ~0.5s of stereo samples
        let samples = Arc::new(Mutex::new(Vec::new()));
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(FailAfterOutput { remaining: remaining.clone(), samples: samples.clone() }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().bit_perfect_notice.is_some());
        assert!(!engine.state().playing);
        let first = samples.lock().unwrap().len();
        assert!(first > 0, "the failing output wrote nothing before failing");

        // Device is back; a new play context must start at 0, not resume the
        // failed track's position.
        *remaining.lock().unwrap() = usize::MAX;
        engine.handle(EngineCmd::PlayTracks { items: vec![item(2)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && samples.lock().unwrap().len() > first);
        assert_eq!(
            samples.lock().unwrap().len(),
            first + 176_400,
            "a stale seek target dropped audio from the new track"
        );
    }

    /// Regression: device loss must pause, not auto-retry opening the output
    /// in a loop until the user presses play.
    #[test]
    fn device_loss_does_not_auto_retry_until_resume() {
        let (plan, files) = fixture_plan();
        let failing = Arc::new(AtomicBool::new(true));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(DeviceLostOutput {
                failing: failing.clone(),
                samples: samples.clone(),
                opens: opens.clone(),
            }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().bit_perfect_notice.is_some());

        let opens_at_notice = opens.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(
            opens.load(Ordering::SeqCst),
            opens_at_notice,
            "device loss must not auto-retry opening the output"
        );
        assert!(!engine.state().playing, "device loss must stay paused");
    }

    /// Opens once, then fails every open and every write while `failing` is
    /// set. Simulates a device that disappears after the first successful open.
    struct DisappearingOutput {
        failing: Arc<AtomicBool>,
        opens: Arc<std::sync::atomic::AtomicUsize>,
        samples: Arc<Mutex<Vec<f32>>>,
    }

    impl AudioOutput for DisappearingOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            let n = self.opens.fetch_add(1, Ordering::SeqCst);
            if n == 0 || !self.failing.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(Error::Output("output device unavailable".into()))
            }
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            if self.failing.load(Ordering::SeqCst) {
                return Err(Error::Output("output device unavailable".into()));
            }
            if let PcmBuffer::F32(v) = buf {
                self.samples.lock().unwrap().extend_from_slice(v);
            }
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            let mut status = OutputStatus::default();
            if self.failing.load(Ordering::SeqCst) {
                status.notice = Some("output device disconnected".into());
            }
            status
        }
    }

    /// Regression: while the device is gone the worker must not retry opening
    /// the output; it waits for a play press, then recovers.
    #[test]
    fn device_loss_does_not_retry_opens_while_gone() {
        let (plan, files) = fixture_plan();
        let failing = Arc::new(AtomicBool::new(true));
        let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(DisappearingOutput {
                failing: failing.clone(),
                opens: opens.clone(),
                samples: samples.clone(),
            }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().bit_perfect_notice.is_some());

        let opens_at_notice = opens.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(
            opens.load(Ordering::SeqCst),
            opens_at_notice,
            "device loss must not retry opening the output"
        );
        assert!(!engine.state().playing);

        failing.store(false, Ordering::SeqCst);
        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| samples.lock().unwrap().len() > 0);
        assert!(engine.state().playing, "recovery did not resume playback");
    }

    /// Regression: a device failure on a later track of a chained session must
    /// recover at that track's position, not the first track's.
    #[test]
    fn device_loss_on_a_later_track_recovers_at_that_tracks_position() {
        let (plan, files) = fixture_plan();
        // Track 1 (176,400 samples) plus 250 ms of track 2 (22,050 samples).
        let remaining = Arc::new(Mutex::new(176_400 + 22_050));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(FailAfterOutput { remaining: remaining.clone(), samples: samples.clone() }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for(|| engine.state().bit_perfect_notice.is_some());
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(2));
        assert!(!engine.state().playing);
        let first = samples.lock().unwrap().len();
        assert_eq!(first, 176_400 + 22_050, "the failure did not land mid-track-2");
        let failure_ms = engine.state().position_ms;
        assert!(failure_ms > 0, "the failure position was not recorded");

        *remaining.lock().unwrap() = usize::MAX;
        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| !engine.state().playing && samples.lock().unwrap().len() > first);
        // The output double overshoots the recorded failure by the tail of the
        // partially written buffer, and the seek target is quantized to
        // milliseconds, so the exact resume point derives from the recorded
        // position: recovery must discard exactly that seek target instead of
        // restarting the track from 0.
        let discard = (failure_ms * 44_100 / 1000) as usize * 2;
        assert_eq!(
            samples.lock().unwrap().len(),
            first + 176_400 - discard,
            "recovery must resume the playing track at its failure position"
        );
    }

    /// Wraps `NullOutput` and sleeps in `stop`, widening the window between a
    /// track ending and the worker going idle.
    struct SlowStopOutput {
        inner: NullOutput,
        stop_delay: Duration,
    }

    impl AudioOutput for SlowStopOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            self.inner.open(spec)
        }
        fn start(&mut self) -> Result<()> {
            self.inner.start()
        }
        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            self.inner.write(buf)
        }
        fn set_volume(&mut self, volume: f32) -> Result<()> {
            self.inner.set_volume(volume)
        }
        fn flush(&mut self) -> Result<()> {
            self.inner.flush()
        }
        fn stop(&mut self) -> Result<()> {
            std::thread::sleep(self.stop_delay);
            self.inner.stop()
        }
        fn status(&self) -> OutputStatus {
            self.inner.status()
        }
    }

    /// Regression: pressing play in the instant after the queue ends must
    /// restart playback instead of leaving a silent "playing" state.
    #[test]
    fn play_pause_immediately_after_queue_end_restarts_playback() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(SlowStopOutput {
                inner: output.clone(),
                stop_delay: Duration::from_millis(300),
            }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing);
        // The worker is still inside the slow stop; press play immediately.
        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| output.samples().len() > 176_400);
        assert!(
            output.samples().len() > 176_400,
            "resume after queue end did not restart playback"
        );
    }

    /// A resolver over per-track plans that records resolve order.
    struct PerTrackResolver {
        plans: HashMap<u64, StreamPlan>,
        order: Arc<Mutex<Vec<u64>>>,
    }

    impl StreamResolver for PerTrackResolver {
        fn resolve(&self, track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            self.order.lock().unwrap().push(track_id);
            self.plans
                .get(&track_id)
                .cloned()
                .ok_or_else(|| Error::Manifest(format!("no plan for {track_id}")))
        }
    }

    fn plan_for(url: &str, sample_rate: u32, bit_depth: u32) -> StreamPlan {
        StreamPlan {
            init_url: None,
            segments: vec![Segment { url: url.into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: Some("flac".into()),
            sample_rate: Some(sample_rate),
            bit_depth: Some(bit_depth),
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: Default::default(),
        }
    }

    #[test]
    fn matching_formats_chain_without_reopening() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();

        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        assert_eq!(output.samples().len(), 176_400 * 2);
        // One open for both tracks; a reopen between tracks would make this 2.
        assert_eq!(output.opens(), 1, "gapless playback must not reopen between matching tracks");
        assert_eq!(output.stops(), 1, "gapless playback must not stop between matching tracks");
    }

    #[test]
    fn format_change_reopens_at_the_boundary() {
        let tone16 = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let tone48 = std::fs::read("tests/fixtures/tone48.mp4").unwrap();
        let mut plans = HashMap::new();
        plans.insert(1, plan_for("mem://t16", 44100, 16));
        plans.insert(2, plan_for("mem://t48", 48000, 16));
        let mut files = HashMap::new();
        files.insert("mem://t16".to_string(), tone16);
        files.insert("mem://t48".to_string(), tone48);
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(PerTrackResolver { plans, order }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );

        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();

        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 + 192_000);
        // 2s @ 44.1k stereo + 2s @ 48k stereo.
        assert_eq!(output.samples().len(), 176_400 + 192_000);
        assert_eq!(output.opens(), 2, "a format change must reopen the output");
        assert_eq!(output.stops(), 2, "a format change must stop and reopen");
        assert_eq!(output.last_spec().map(|s| s.info.sample_rate), Some(48000));
    }

    #[test]
    fn prefetched_next_track_is_not_resolved_twice() {
        let (plan, files) = fixture_plan();
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut plans = HashMap::new();
        plans.insert(1, plan.clone());
        plans.insert(2, plan);
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(PerTrackResolver { plans, order: order.clone() }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        let resolved = order.lock().unwrap().clone();
        assert_eq!(
            resolved.iter().filter(|&&t| t == 1).count(),
            1,
            "track 1 resolved more than once: {resolved:?}"
        );
        assert_eq!(
            resolved.iter().filter(|&&t| t == 2).count(),
            1,
            "prefetched track 2 resolved more than once: {resolved:?}"
        );
    }

    /// Sleeps in `write` so playback takes real time, making mid-track state
    /// observable.
    struct ThrottledOutput {
        inner: NullOutput,
        delay: Duration,
    }

    impl AudioOutput for ThrottledOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            self.inner.open(spec)
        }
        fn start(&mut self) -> Result<()> {
            self.inner.start()
        }
        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            std::thread::sleep(self.delay);
            self.inner.write(buf)
        }
        fn set_volume(&mut self, volume: f32) -> Result<()> {
            self.inner.set_volume(volume)
        }
        fn flush(&mut self) -> Result<()> {
            self.inner.flush()
        }
        fn stop(&mut self) -> Result<()> {
            self.inner.stop()
        }
        fn status(&self) -> OutputStatus {
            self.inner.status()
        }
    }

    /// The approved refinement: a settings change while playing restarts the
    /// current track at its position, not from the start.
    #[test]
    fn settings_change_restarts_at_the_current_position() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(ThrottledOutput { inner: output.clone(), delay: Duration::from_millis(30) }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().position_ms >= 300 && engine.state().playing);
        let before = engine.state().position_ms;
        assert!(before < 1900, "track finished before the setting change: {before}");

        engine.set_output_mode(OutputMode::Exclusive);
        wait_for(|| output.opens() >= 2);
        assert!(
            engine.state().position_ms + 200 >= before,
            "position reset on restart: {before} -> {}",
            engine.state().position_ms
        );
        assert!(engine.state().playing);
    }

    /// Regression: a settings change in the window between auto-advance and the
    /// next track's stream must not apply the previous track's position to the
    /// next track.
    #[test]
    fn settings_change_at_the_queue_boundary_does_not_skip_the_next_track() {
        let (plan, files) = fixture_plan();
        let gate = Arc::new(Gate::new());
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(GatedResolver { plan, gate: gate.clone(), order: order.clone() }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for_order(&order, 1);
        gate.release(1);
        // Track 2's prefetch blocks inside resolve, so the worker is parked at
        // the boundary with current = track 2 and position_ms = track 1's end.
        wait_for(|| {
            output.samples().len() >= 176_400
                && engine.state().current.map(|i| i.track_id) == Some(2)
        });

        engine.set_output_mode(OutputMode::Exclusive);
        gate.release(2);

        // Post-change playback is bit-perfect exclusive, so the restarted track
        // lands in `i32_samples`; count both variants for the track total.
        let written = || output.samples().len() + output.i32_samples().len();
        wait_for(|| !engine.state().playing && written() >= 176_400 * 2);
        assert_eq!(
            written(),
            176_400 * 2,
            "a boundary settings change must not discard the next track"
        );
    }

    #[test]
    fn cache_miss_writes_the_entry_then_a_replay_makes_zero_fetcher_calls() {
        let (_, files) = fixture_plan();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let output = NullOutput::new();
        let cache = test_cache();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan: cached_plan("mem://tone", 0xabcd) }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: 0xabcd };

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert!(cache.lookup(&key).is_some(), "a cache miss did not write the entry");
        let first = calls.load(Ordering::SeqCst);
        assert_eq!(first, 1, "the first play should fetch exactly once");

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        assert_eq!(calls.load(Ordering::SeqCst), first, "a cache hit still fetched");
    }

    #[test]
    fn seek_plans_are_not_cached() {
        let (_, files) = fixture_plan();
        let output = NullOutput::new();
        let cache = test_cache();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan: cached_plan("mem://tone", 0xabcd) }),
            move || Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output),
            Quality::Lossless,
            cache.clone(),
            true,
        );
        let (tx, _rx) = std::sync::mpsc::sync_channel(512);
        let generation = engine.inner.generation.load(Ordering::SeqCst);
        let buffer = StreamBuffer::new();
        decode_track(
            engine.inner.clone(),
            item(1),
            OutputMode::Shared,
            generation,
            Some(500),
            tx,
            buffer,
            Arc::new(Mutex::new(None)),
        );
        assert_eq!(cache.size_bytes(), 0, "a seek plan must not be cached");
    }

    #[test]
    fn corrupt_cache_entry_is_removed_and_refetched() {
        let (_, files) = fixture_plan();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let output = NullOutput::new();
        let cache = test_cache();
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: 0xabcd };
        {
            let mut writer = cache.begin_write(&key).unwrap();
            writer.write_all(b"this is not an mp4").unwrap();
            writer.commit();
        }
        let engine = Engine::new(
            Arc::new(FakeResolver { plan: cached_plan("mem://tone", 0xabcd) }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(output.samples().len(), 176_400);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the network retry must run exactly once");
        let replaced = cache.lookup(&key).expect("the corrupt entry was not replaced");
        assert_eq!(
            std::fs::metadata(replaced).unwrap().len(),
            files["mem://tone"].len() as u64,
            "the refetched bytes were not re-cached"
        );
    }

    /// P4-3: the next queue item is downloaded into the cache while the current
    /// one plays (via the existing lookahead prefetch, now cache-teed), and a
    /// replay of the queue fetches nothing.
    #[test]
    fn next_track_is_prefetched_into_the_cache() {
        let tone = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let mut files = HashMap::new();
        files.insert("mem://t1".to_string(), tone.clone());
        files.insert("mem://t2".to_string(), tone);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut plans = HashMap::new();
        plans.insert(1, cached_plan("mem://t1", 0x1111));
        plans.insert(2, cached_plan("mem://t2", 0x2222));
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = NullOutput::new();
        let cache = test_cache();
        let engine = Engine::new(
            Arc::new(PerTrackResolver { plans, order }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(ThrottledOutput { inner: output.clone(), delay: Duration::from_millis(20) }),
            Quality::Lossless,
            cache.clone(),
            true,
        );
        let key1 = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: 0x1111 };
        let key2 = CacheKey { track_id: 2, quality: Quality::Lossless, manifest_hash: 0x2222 };

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for(|| cache.lookup(&key2).is_some());
        assert!(cache.lookup(&key2).is_some(), "the prefetch never filled track 2's entry");
        assert!(
            output.samples().len() < 176_400,
            "track 1 finished before the prefetch was observable"
        );
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        assert!(cache.lookup(&key1).is_some(), "track 1 was not cached when it finished");
        let after_first_play = calls.load(Ordering::SeqCst);
        assert_eq!(after_first_play, 2, "each track should be fetched exactly once");

        // Replaying the queue is a pure cache read.
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 4);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            after_first_play,
            "a replay of a cached queue still fetched"
        );
    }

    #[test]
    fn stream_wait_returns_queued_events_without_a_notify() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        tx.send(7).unwrap();
        let lock = Mutex::new(());
        let wake = Condvar::new();
        match wait_for_stream_event(&lock, &wake, &rx, || None) {
            StreamWait::Event(7) => {}
            other => panic!("expected the queued event, got {other:?}"),
        }
    }

    #[test]
    fn stream_wait_wakes_on_notify_without_an_event() {
        let (_tx, rx) = std::sync::mpsc::channel::<u32>();
        let lock = std::sync::Arc::new(Mutex::new(()));
        let wake = std::sync::Arc::new(Condvar::new());
        let (locker, waker) = (lock.clone(), wake.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let _guard = locker.lock().unwrap();
            waker.notify_all();
        });
        let started = std::time::Instant::now();
        let result = wait_for_stream_event(&lock, &wake, &rx, || None);
        assert!(matches!(result, StreamWait::Woken), "expected a bare wake, got {result:?}");
        assert!(started.elapsed() < Duration::from_millis(1000));
    }

    #[test]
    fn stream_wait_reports_a_disconnected_producer() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        drop(tx);
        let lock = Mutex::new(());
        let wake = Condvar::new();
        assert!(matches!(
            wait_for_stream_event(&lock, &wake, &rx, || None),
            StreamWait::Disconnected
        ));
    }

    /// A generation bump + notify that lands while the waiter is parked must be
    /// observed promptly (the `wait_for_info` loop re-checks the predicate after
    /// a bare wake), not at the 2 s safety timeout.
    #[test]
    fn stream_wait_sees_a_generation_bump_despite_the_park() {
        let (_tx, rx) = std::sync::mpsc::channel::<u32>();
        let lock = std::sync::Arc::new(Mutex::new(()));
        let wake = std::sync::Arc::new(Condvar::new());
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (locker, waker) = (lock.clone(), wake.clone());
        let bumper = generation.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            bumper.store(1, Ordering::SeqCst);
            let _guard = locker.lock().unwrap();
            waker.notify_all();
        });
        let session_over = || {
            if generation.load(Ordering::SeqCst) != 0 {
                Some(SessionOver::Superseded)
            } else {
                None
            }
        };
        let started = std::time::Instant::now();
        let mut result = wait_for_stream_event(&lock, &wake, &rx, session_over);
        while matches!(result, StreamWait::Woken) {
            result = wait_for_stream_event(&lock, &wake, &rx, session_over);
        }
        assert!(
            matches!(result, StreamWait::Superseded),
            "expected superseded, got {result:?}"
        );
        assert!(started.elapsed() < Duration::from_millis(1000));
    }

    #[test]
    fn stream_wait_predicate_wins_over_a_queued_event() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        tx.send(7).unwrap();
        let lock = Mutex::new(());
        let wake = Condvar::new();
        let result = wait_for_stream_event(&lock, &wake, &rx, || Some(SessionOver::Superseded));
        assert!(
            matches!(result, StreamWait::Superseded),
            "expected superseded, got {result:?}"
        );
    }

    /// Review Focus: a cached stream keeps its entry protected from the moment
    /// the key is known until the stream is dropped, so a commit for another
    /// track cannot evict the one being played or prefetched.
    #[test]
    fn a_cached_stream_protects_its_entry_for_its_lifetime() {
        const HASH: u64 = 0xabcd;
        let (tone_plan, files) = fixture_plan();
        let tone = files.get("mem://tone").cloned().expect("fixture bytes");
        let cache = test_cache();
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: HASH };
        let mut writer = cache.begin_write(&key).unwrap();
        writer.write_all(&tone).unwrap();
        writer.commit();

        let mut plan = cached_plan("mem://tone", HASH);
        plan.sample_rate = tone_plan.sample_rate;
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            {
                let files = files.clone();
                move || Arc::new(crate::fetch::MapFetcher { files }) as Arc<dyn Fetcher>
            },
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| cache.is_protected(&key));
        assert!(cache.is_protected(&key), "the playing stream must hold its protection");

        engine.handle(EngineCmd::Clear).unwrap();
        wait_for(|| !cache.is_protected(&key));
        assert!(!cache.is_protected(&key), "dropping the stream must release protection");
    }

    /// Review Focus: seeking a fully cached multi-segment track must discard
    /// relative to the file's actual start, not the target segment's start.
    /// The 2 s fixture seeked to 1500 ms leaves 0.5 s (44,100 stereo samples);
    /// the old math left 1.5 s (132,300) because it discarded only the
    /// segment-relative 500 ms.
    #[test]
    fn cached_seek_starts_at_the_target() {
        const HASH: u64 = 0x5eed;
        let (tone_plan, files) = fixture_plan();
        let tone = files.get("mem://tone").cloned().expect("fixture bytes");
        let cache = test_cache();
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: HASH };
        let mut writer = cache.begin_write(&key).unwrap();
        writer.write_all(&tone).unwrap();
        writer.commit();

        let mut plan = cached_plan("mem://tone", HASH);
        plan.timescale = 44100;
        plan.segments = vec![
            Segment { url: "mem://tone".into(), start_ticks: 0, duration_ticks: 44100 },
            Segment { url: "mem://tone".into(), start_ticks: 44100, duration_ticks: 44100 },
        ];
        plan.sample_rate = tone_plan.sample_rate;

        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            {
                let files = files.clone();
                move || Arc::new(crate::fetch::MapFetcher { files }) as Arc<dyn Fetcher>
            },
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| output.samples().len() > 0);
        let before = output.samples().len();
        engine.handle(EngineCmd::Seek(1500)).unwrap();
        wait_for(|| output.flushes() >= 1 && !engine.state().playing);
        std::thread::sleep(Duration::from_millis(100));

        let after = output.samples().len();
        assert!(
            after - before <= 60_000,
            "cached seek wrote {} samples after the target (expected <= 0.5 s)",
            after - before
        );
    }
    /// Collects decoded samples as left-aligned `i32`, exactly as the
    /// exclusive path receives them.
    struct ReferenceI32Sink(Vec<i32>);

    impl PcmSink for ReferenceI32Sink {
        fn preferred_format(&self) -> PcmFormat {
            PcmFormat::I32 { bits: 16 }
        }
        fn on_stream_info(&mut self, _info: &StreamInfo) -> Result<()> {
            Ok(())
        }
        fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
            self.0.extend_from_slice(buf.as_i32().expect("i32 samples"));
            Ok(())
        }
    }

    fn reference_i32() -> Vec<i32> {
        let bytes = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let mut sink = ReferenceI32Sink(Vec::new());
        crate::decode::run(crate::demux::BytesSource::new(bytes), "mp4", &mut sink).unwrap();
        sink.0
    }

    /// T7-D2: DSP settings that leave samples untouched (flat EQ, ReplayGain
    /// on a track without data at 0 dB) must keep exclusive output exact.
    #[test]
    fn identity_dsp_keeps_exclusive_output_sample_exact() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.set_output_mode(OutputMode::Exclusive);
        let mut settings = DspSettings::default();
        settings.eq.enabled = true;
        settings.replay_gain.mode = crate::dsp::ReplayGainMode::Track;
        engine.set_dsp(settings);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let reference = reference_i32();
        wait_for(|| output.i32_samples().len() >= reference.len());
        assert_eq!(output.i32_samples(), reference, "identity DSP altered samples");
        assert!(engine.audio_status().bit_perfect);
        assert!(!engine.state().dsp_active);
    }

    #[test]
    fn replay_gain_scales_exclusive_output_and_clears_bit_perfect() {
        let (mut plan, files) = fixture_plan();
        plan.replay_gain = ReplayGain { track_gain_db: Some(-6.0), ..Default::default() };
        let (engine, output) = engine_with(plan, files);
        let applied = Arc::new(Mutex::new(None));
        let applied_ref = applied.clone();
        engine.subscribe(Arc::new(move |event| {
            if let EngineEvent::State(state) = event {
                if let Some(db) = state.stream.and_then(|s| s.replay_gain_db) {
                    *applied_ref.lock().unwrap() = Some(db);
                }
            }
        }));
        engine.set_output_mode(OutputMode::Exclusive);
        let mut settings = DspSettings::default();
        settings.replay_gain.mode = crate::dsp::ReplayGainMode::Track;
        engine.set_dsp(settings);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let reference = reference_i32();
        wait_for(|| output.i32_samples().len() >= reference.len());
        let out = output.i32_samples();
        assert_eq!(out.len(), reference.len());
        let gain = 10f64.powf(-6.0 / 20.0);
        for (index, (&got, &src)) in out.iter().zip(&reference).enumerate() {
            let want = src as f64 * gain;
            assert!((got as f64 - want).abs() <= 1.0, "sample {index}: {got} vs {want}");
        }
        let status = engine.audio_status();
        assert!(!status.bit_perfect);
        assert_eq!(status.notice, None, "DSP is a choice, not an output warning");
        assert!(engine.state().dsp_active);
        assert_eq!(engine.state().dsp_stages, vec!["ReplayGain"]);
        let db = applied.lock().unwrap().expect("replay gain published in the stream meta");
        assert!((db + 6.0).abs() < 1e-3, "{db}");
    }

    /// T7-D3: a DSP change while playing applies without reopening or
    /// restarting, and bit-perfect follows it both ways.
    #[test]
    fn dsp_changes_apply_live_and_flip_bit_perfect() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(ThrottledOutput { inner: output.clone(), delay: Duration::from_millis(60) }),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().position_ms >= 200 && engine.state().playing);
        assert!(engine.state().bit_perfect);
        let opens = output.opens();

        let mut boosted = DspSettings::default();
        boosted.eq.enabled = true;
        boosted.eq.bands_db[5] = 6.0;
        engine.set_dsp(boosted);
        wait_for(|| !engine.state().bit_perfect);
        let state = engine.state();
        assert!(state.dsp_active);
        assert_eq!(state.dsp_stages, vec!["EQ"]);

        engine.set_dsp(DspSettings::default());
        wait_for(|| engine.state().bit_perfect);
        assert!(engine.state().bit_perfect, "flat settings must restore bit-perfect");
        assert!(engine.state().position_ms < 1900, "track ended before the checks finished");
        assert_eq!(output.opens(), opens, "a DSP change must not reopen the output");
    }
    fn album_item(id: u64, album: &str) -> QueueItem {
        QueueItem { album: album.into(), ..item(id) }
    }

    fn crossfade(duration_ms: u32, skip_same_album: bool) -> DspSettings {
        let mut settings = DspSettings::default();
        settings.crossfade.duration_ms = duration_ms;
        settings.crossfade.skip_same_album = skip_same_album;
        settings
    }

    /// Plays `items` to the end through a lightly throttled output (so the
    /// producer runs ahead like real playback) and returns the engine, output,
    /// and every `dsp_stages` list published.
    fn play_to_end(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
        settings: DspSettings,
        items: Vec<QueueItem>,
        expected_samples: usize,
    ) -> (Engine, NullOutput, Vec<Vec<&'static str>>) {
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(ThrottledOutput { inner: output.clone(), delay: Duration::from_millis(3) }),
            Quality::Lossless,
        );
        let stages = Arc::new(Mutex::new(Vec::new()));
        let stages_ref = stages.clone();
        engine.subscribe(Arc::new(move |event| {
            if let EngineEvent::State(state) = event {
                stages_ref.lock().unwrap().push(state.dsp_stages);
            }
        }));
        engine.set_dsp(settings);
        engine.handle(EngineCmd::PlayTracks { items, start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= expected_samples);
        std::thread::sleep(Duration::from_millis(50));
        let seen = stages.lock().unwrap().clone();
        (engine, output, seen)
    }

    /// T7-D6: a natural end overlaps the next track by about the crossfade
    /// length, on the same open output.
    #[test]
    fn crossfade_overlaps_tracks_from_different_albums() {
        let (plan, files) = fixture_plan();
        let overlap = 2 * 44_100; // 1 s of stereo samples
        let (engine, output, stages) = play_to_end(
            plan,
            files,
            crossfade(1000, true),
            vec![album_item(1, "A"), album_item(2, "B")],
            2 * 176_400 - overlap - 2 * 8192,
        );
        let total = output.samples().len();
        // The FIFO tops up a whole buffer at a time, so the overlap is the
        // target within one decoded packet (4096 frames).
        let slack = 2 * 4096;
        assert!(
            total + overlap <= 2 * 176_400 + slack && total + overlap + slack >= 2 * 176_400,
            "expected ~{} samples, got {total}",
            2 * 176_400 - overlap
        );
        assert_eq!(output.opens(), 1, "a crossfade must not reopen the output");
        assert!(stages.iter().any(|s| s.contains(&"crossfade")), "{stages:?}");
        assert!(engine.state().dsp_stages.is_empty(), "the crossfade stage must clear");
        assert!(output.samples().iter().all(|s| s.abs() <= 1.0));
    }

    #[test]
    fn crossfade_keeps_consecutive_album_tracks_gapless() {
        let (plan, files) = fixture_plan();
        let (_engine, output, stages) = play_to_end(
            plan,
            files,
            crossfade(1000, true),
            vec![album_item(1, "Same"), album_item(2, "Same")],
            2 * 176_400,
        );
        assert_eq!(output.samples().len(), 2 * 176_400);
        assert!(stages.iter().all(|s| !s.contains(&"crossfade")));
    }

    #[test]
    fn crossfade_can_include_album_tracks() {
        let (plan, files) = fixture_plan();
        let (_engine, output, _) = play_to_end(
            plan,
            files,
            crossfade(1000, false),
            vec![album_item(1, "Same"), album_item(2, "Same")],
            2 * 176_400 - 2 * 44_100 - 2 * 8192,
        );
        assert!(output.samples().len() < 2 * 176_400 - 2 * 44_100 + 2 * 4096);
    }

    #[test]
    fn crossfade_off_and_a_lone_track_play_every_sample() {
        let (plan, files) = fixture_plan();
        let (_engine, output, _) = play_to_end(
            plan.clone(),
            files.clone(),
            crossfade(0, false),
            vec![album_item(1, "A"), album_item(2, "B")],
            2 * 176_400,
        );
        assert_eq!(output.samples().len(), 2 * 176_400);
        let (_engine, output, _) =
            play_to_end(plan, files, crossfade(3000, false), vec![album_item(1, "A")], 176_400);
        assert_eq!(output.samples().len(), 176_400, "nothing follows, so nothing is held back");
    }

    /// Different output specs cannot overlap: the tail fades out, the output
    /// reopens, and no audio is dropped.
    #[test]
    fn crossfade_across_a_format_change_fades_out_instead() {
        let tone16 = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let tone48 = std::fs::read("tests/fixtures/tone48.mp4").unwrap();
        let mut plans = HashMap::new();
        plans.insert(1, plan_for("mem://t16", 44100, 16));
        plans.insert(2, plan_for("mem://t48", 48000, 16));
        let mut files = HashMap::new();
        files.insert("mem://t16".to_string(), tone16);
        files.insert("mem://t48".to_string(), tone48);
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(PerTrackResolver { plans, order: Arc::new(Mutex::new(Vec::new())) }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(ThrottledOutput { inner: output.clone(), delay: Duration::from_millis(3) }),
            Quality::Lossless,
        );
        engine.set_dsp(crossfade(1000, true));
        engine
            .handle(EngineCmd::PlayTracks {
                items: vec![album_item(1, "A"), album_item(2, "B")],
                start: 0,
            })
            .unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 + 192_000);
        let samples = output.samples();
        assert_eq!(samples.len(), 176_400 + 192_000);
        assert_eq!(output.opens(), 2);
        // The last frame of track 1 is faded to (near) silence.
        let last = samples[176_400 - 2].abs();
        assert!(last < 0.01, "tail not faded out: {last}");
    }
    /// A sleep fade (end-of-track sleep timer) must pause on the fading track,
    /// never cross into the next one.
    #[test]
    fn a_sleep_fade_disarms_the_crossfade() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(ThrottledOutput { inner: output.clone(), delay: Duration::from_millis(60) }),
            Quality::Lossless,
        );
        // 1.5 s of a 2 s track: armed, the hand-off would come at 0.5 s.
        engine.set_dsp(crossfade(1500, true));
        assert_eq!(engine.state().crossfade_ms, 1500);
        engine
            .handle(EngineCmd::PlayTracks {
                items: vec![album_item(1, "A"), album_item(2, "B")],
                start: 0,
            })
            .unwrap();
        wait_for(|| engine.state().position_ms >= 100);
        // ~33 writes of ~26 ms of audio each: the fade runs well past 0.5 s.
        engine.handle(EngineCmd::FadeAndPause { duration_ms: 2000 }).unwrap();
        wait_for(|| !engine.state().playing);
        let state = engine.state();
        assert!(!state.playing);
        assert_eq!(state.current.map(|i| i.track_id), Some(1), "the fade crossed into track 2");
        assert!(state.position_ms > 600, "paused before the would-be hand-off: {}", state.position_ms);
    }
    #[test]
    fn idle_engine_reports_no_dsp_stages() {
        let (mut plan, files) = fixture_plan();
        plan.replay_gain = ReplayGain { track_gain_db: Some(-6.0), ..Default::default() };
        let (engine, output) = engine_with(plan, files);
        let mut settings = DspSettings::default();
        settings.replay_gain.mode = crate::dsp::ReplayGainMode::Track;
        engine.set_dsp(settings);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        wait_for(|| !engine.inner.worker_busy.load(Ordering::SeqCst));
        assert_eq!(engine.state().dsp_stages, vec!["ReplayGain"], "the last track's stages stay visible");

        engine.set_dsp(DspSettings::default());
        assert!(engine.state().dsp_stages.is_empty());
        engine.handle(EngineCmd::Clear).unwrap();
        assert!(!engine.state().dsp_active);
    }
}
