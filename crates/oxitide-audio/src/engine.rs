use std::sync::{Arc, Mutex};

use crate::decode::{PcmBuffer, PcmFormat, PcmSink, StreamInfo};
use crate::error::{Error, Result};
use crate::fetch::{self, Fetcher, StreamBuffer};
use crate::output::{AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode, OutputStatus};
use crate::queue::{Queue, QueueItem};
use crate::source::StreamPlan;
use oxitide_tidal::stream::Quality;

pub trait StreamResolver: Send + Sync + 'static {
    fn resolve(&self, track_id: u64, quality: Quality) -> Result<StreamPlan>;
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlaybackState {
    pub playing: bool,
    pub buffering: bool,
    pub current: Option<QueueItem>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: crate::queue::RepeatMode,
    pub preferred_quality: Quality,
    pub queue: Vec<QueueItem>,
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
    PlayPause,
    Next,
    Prev,
    Seek(u64),
    SetVolume(f32),
    SetShuffle(bool),
    SetRepeat(crate::queue::RepeatMode),
    Reorder { from: usize, to: usize },
    Remove(usize),
    Clear,
}

type Listener = Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// Creates a fresh `AudioOutput` for each track. The app passes a factory that
/// builds `WasapiOutput`; tests pass one that builds `NullOutput`.
pub trait OutputFactory: Send + Sync + 'static {
    fn create(&self) -> Box<dyn AudioOutput>;
}

impl<F> OutputFactory for F
where
    F: Fn() -> Box<dyn AudioOutput> + Send + Sync + 'static,
{
    fn create(&self) -> Box<dyn AudioOutput> {
        self()
    }
}

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

struct Inner {
    queue: Mutex<Queue>,
    resolver: Arc<dyn StreamResolver>,
    fetcher: Arc<dyn Fetcher>,
    output: Arc<dyn OutputFactory>,
    quality: Mutex<Quality>,
    device_id: Mutex<Option<String>>,
    output_mode: Mutex<OutputMode>,
    exclusive_fallback: Mutex<ExclusiveFallback>,
    output_status: Arc<Mutex<OutputStatus>>,
    listeners: Mutex<Vec<Listener>>,
    /// Serializes the generation bump + spawn of a new track with the advance
    /// decision of a completing playback thread, so a command and an auto-advance
    /// cannot both move the queue. Lock ordering: always take `playback_lock`
    /// before `queue`, and never hold `queue` while taking `playback_lock`.
    playback_lock: Mutex<()>,
    /// Incremented on every new track; a playback thread whose generation is
    /// stale stops writing.
    generation: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    volume: Arc<AtomicU32>, // f32 bits
    position_ms: Arc<AtomicU64>,
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

impl Engine {
    pub fn new(
        resolver: Arc<dyn StreamResolver>,
        fetcher: Arc<dyn Fetcher>,
        output: Arc<dyn OutputFactory>,
        quality: Quality,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                queue: Mutex::new(Queue::default()),
                resolver,
                fetcher,
                output,
                quality: Mutex::new(quality),
                device_id: Mutex::new(None),
                output_mode: Mutex::new(OutputMode::Shared),
                exclusive_fallback: Mutex::new(ExclusiveFallback::Shared),
                output_status: Arc::new(Mutex::new(OutputStatus::default())),
                listeners: Mutex::new(Vec::new()),
                playback_lock: Mutex::new(()),
                generation: Arc::new(AtomicU64::new(0)),
                paused: Arc::new(AtomicBool::new(false)),
                volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
                position_ms: Arc::new(AtomicU64::new(0)),
            }),
        }
    }

    pub fn subscribe(&self, listener: Listener) {
        self.inner.listeners.lock().unwrap().push(listener);
    }

    pub fn set_quality(&self, quality: Quality) {
        *self.inner.quality.lock().unwrap() = quality;
    }

    /// Select the output device for future tracks. `None` means the system
    /// default. Already-playing tracks keep their device.
    pub fn set_output_device(&self, device: Option<String>) {
        *self.inner.device_id.lock().unwrap() = device;
    }

    /// The mode/bit-perfect state the output last reported.
    pub fn audio_status(&self) -> OutputStatus {
        self.inner.output_status.lock().unwrap().clone()
    }

    /// Select shared or exclusive output. Applies to the next track opened;
    /// Task 6 makes this apply to a playing track.
    pub fn set_output_mode(&self, mode: OutputMode) {
        *self.inner.output_mode.lock().unwrap() = mode;
    }

    /// Select what to do when exclusive output is unavailable.
    pub fn set_exclusive_fallback(&self, fallback: ExclusiveFallback) {
        *self.inner.exclusive_fallback.lock().unwrap() = fallback;
    }

    fn output_device(&self) -> Option<String> {
        self.inner.device_id.lock().unwrap().clone()
    }

    pub fn state(&self) -> PlaybackState {
        let q = self.inner.queue.lock().unwrap();
        PlaybackState {
            playing: q.is_playing(),
            buffering: false,
            current: q.current().cloned(),
            position_ms: self.inner.position_ms.load(Ordering::SeqCst),
            duration_ms: q.current().map(|i| i.duration_secs as u64 * 1000).unwrap_or(0),
            volume: q.volume(),
            shuffle: q.shuffle(),
            repeat: q.repeat(),
            preferred_quality: *self.inner.quality.lock().unwrap(),
            queue: q.items().to_vec(),
        }
    }

    fn emit(&self, event: EngineEvent) {
        for listener in self.inner.listeners.lock().unwrap().iter() {
            listener(event.clone());
        }
    }

    /// Run a queue-mutating playback command under `playback_lock`, bumping the
    /// generation first so any completing playback thread is invalidated before it
    /// can advance the queue. The lock is released before the caller starts the new
    /// track (via `start_playback`), which re-acquires it.
    fn mutate_playing_queue<T>(&self, mutate: impl FnOnce(&mut Queue) -> T) -> T {
        let _guard = self.inner.playback_lock.lock().unwrap();
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        let mut queue = self.inner.queue.lock().unwrap();
        mutate(&mut queue)
    }

    /// Like `mutate_playing_queue`, but only bumps the generation (invalidating a
    /// running playback thread) when `mutate` actually moved the current index or
    /// stopped playback. A no-op `Prev` at the first track must leave playback
    /// untouched so `state().playing` keeps matching the audio. Returns the
    /// resulting current item and whether the index moved.
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

    pub fn handle(&self, cmd: EngineCmd) -> Result<()> {
        match cmd {
            EngineCmd::PlayTracks { items, start } => {
                let item = self.mutate_playing_queue(|q| {
                    q.play_context(items, start);
                    q.current().cloned()
                });
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if let Some(item) = item {
                    self.start_playback(item);
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::PlayPause => {
                // Only toggle when a track is current. On an empty queue
                // `toggle_play_pause` is a no-op, so flipping `paused` here would
                // latch it true and silence the next `PlayTracks`.
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
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Next => {
                let (item, changed) = self.mutate_queue_if_changed(|q| {
                    q.next();
                });
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if changed {
                    if let Some(item) = item {
                        self.start_playback(item);
                    }
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Prev => {
                let (item, changed) = self.mutate_queue_if_changed(|q| {
                    q.prev();
                });
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if changed {
                    if let Some(item) = item {
                        self.start_playback(item);
                    }
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Seek(_) => { /* position tracking is a Plan 3 concern */ }
            EngineCmd::SetVolume(v) => {
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
            EngineCmd::Clear => {
                self.mutate_playing_queue(|q| q.clear());
                self.inner.paused.store(false, Ordering::SeqCst);
                self.emit(EngineEvent::NowPlaying(None));
                self.emit(EngineEvent::State(self.state()));
            }
        }
        Ok(())
    }

    /// Resolve, download, decode, and write `item` through a fresh output. Runs on
    /// the calling thread; used directly by tests and by the per-track thread.
    pub fn play_track_into(&self, item: &QueueItem, output: &mut dyn AudioOutput) -> Result<()> {
        let quality = *self.inner.quality.lock().unwrap();
        let plan = self.inner.resolver.resolve(item.track_id, quality)?;
        let buffer = StreamBuffer::new();
        let fetcher = self.inner.fetcher.clone();
        let download_buffer = buffer.clone();
        let downloader = std::thread::spawn(move || fetch::download(download_buffer, fetcher, plan));
        let source = crate::demux::StreamBufferSource::new(buffer);
        let mut sink = OutputSink {
            output,
            device_id: self.output_device(),
            mode: *self.inner.output_mode.lock().unwrap(),
            fallback: *self.inner.exclusive_fallback.lock().unwrap(),
            status: self.inner.output_status.clone(),
            generation: None,
            paused: self.inner.paused.clone(),
            volume: self.inner.volume.clone(),
            position_ms: self.inner.position_ms.clone(),
            frames: 0,
            rate: 0,
            channels: 2,
            last_volume: 1.0,
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = output.stop();
        let _ = downloader.join();
        result
    }

    /// Spawn a playback thread for `item`, taking `playback_lock` first. On success
    /// it advances the queue and starts the next track; a stale generation (a newer
    /// command superseded it) aborts silently.
    fn start_playback(&self, item: QueueItem) {
        let _guard = self.inner.playback_lock.lock().unwrap();
        self.start_playback_locked(item);
    }

    /// The body of `start_playback`. Assumes the caller holds `playback_lock` so
    /// the generation bump + spawn is atomic with a completing thread's advance.
    fn start_playback_locked(&self, item: QueueItem) {
        let generation = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.inner.position_ms.store(0, Ordering::SeqCst);
        // A fresh track always starts unpaused.
        self.inner.paused.store(false, Ordering::SeqCst);
        let engine = self.clone();
        let spawned = std::thread::Builder::new()
            .name("oxitide-playback".into())
            .spawn(move || {
                let result = engine.run_track(&item, generation);
                // Serialize the advance decision with new commands and with other
                // completing threads. Held only around the generation check, queue
                // advance, and next spawn — never across decode/join.
                let _guard = engine.inner.playback_lock.lock().unwrap();
                if engine.inner.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                match result {
                    Ok(()) => {
                        let next = {
                            let mut q = engine.inner.queue.lock().unwrap();
                            q.on_track_ended().and_then(|i| q.items().get(i).cloned())
                        };
                        engine.emit(EngineEvent::NowPlaying(next.clone()));
                        engine.emit(EngineEvent::State(engine.state()));
                        match next {
                            Some(next) => engine.start_playback_locked(next),
                            // End of queue: bump the generation so the position
                            // ticker for this track exits instead of looping forever.
                            None => {
                                engine.inner.generation.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                    Err(e) => {
                        {
                            let mut q = engine.inner.queue.lock().unwrap();
                            q.stop();
                        }
                        // Stop this track's position ticker and unblock auto-advance.
                        engine.inner.generation.fetch_add(1, Ordering::SeqCst);
                        engine.emit(EngineEvent::Error {
                            code: "playback".into(),
                            message: e.to_string(),
                        });
                        engine.emit(EngineEvent::State(engine.state()));
                    }
                }
            });
        if let Err(e) = spawned {
            self.emit(EngineEvent::Error { code: "playback".into(), message: e.to_string() });
            return;
        }

        // Emit throttled position updates while this track is current.
        let ticker = self.clone();
        std::thread::Builder::new()
            .name("oxitide-position".into())
            .spawn(move || {
                while ticker.inner.generation.load(Ordering::SeqCst) == generation {
                    let position_ms = ticker.inner.position_ms.load(Ordering::SeqCst);
                    let duration_ms = ticker
                        .inner
                        .queue
                        .lock()
                        .unwrap()
                        .current()
                        .map(|i| i.duration_secs as u64 * 1000)
                        .unwrap_or(0);
                    ticker.emit(EngineEvent::Position { position_ms, duration_ms });
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            })
            .ok();
    }

    fn run_track(&self, item: &QueueItem, generation: u64) -> Result<()> {
        let quality = *self.inner.quality.lock().unwrap();
        let plan = self.inner.resolver.resolve(item.track_id, quality)?;
        let buffer = StreamBuffer::new();
        let fetcher = self.inner.fetcher.clone();
        let download_buffer = buffer.clone();
        let downloader = std::thread::spawn(move || fetch::download(download_buffer, fetcher, plan));
        let source = crate::demux::StreamBufferSource::new(buffer);
        let mut output = self.inner.output.create();
        let mut sink = OutputSink {
            output: output.as_mut(),
            device_id: self.output_device(),
            mode: *self.inner.output_mode.lock().unwrap(),
            fallback: *self.inner.exclusive_fallback.lock().unwrap(),
            status: self.inner.output_status.clone(),
            generation: Some((self.inner.generation.clone(), generation)),
            paused: self.inner.paused.clone(),
            volume: self.inner.volume.clone(),
            position_ms: self.inner.position_ms.clone(),
            frames: 0,
            rate: 0,
            channels: 2,
            last_volume: 1.0,
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = output.stop();
        let _ = downloader.join();
        result
    }
}

struct OutputSink<'a> {
    output: &'a mut dyn AudioOutput,
    device_id: Option<String>,
    mode: OutputMode,
    fallback: ExclusiveFallback,
    status: Arc<Mutex<OutputStatus>>,
    last_volume: f32,
    generation: Option<(Arc<AtomicU64>, u64)>,
    paused: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    position_ms: Arc<AtomicU64>,
    frames: u64,
    rate: u32,
    channels: u16,
}

impl PcmSink for OutputSink<'_> {
    fn preferred_format(&self) -> PcmFormat {
        PcmFormat::F32
    }

    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.rate = info.sample_rate;
        self.channels = info.channels;
        let spec = AudioOutputSpec {
            device_id: self.device_id.clone(),
            info: *info,
            mode: self.mode,
            exclusive_fallback: self.fallback,
        };
        self.output.open(&spec)?;
        self.output.start()?;
        self.last_volume = f32::from_bits(self.volume.load(Ordering::SeqCst));
        self.output.set_volume(self.last_volume)?;
        *self.status.lock().unwrap() = self.output.status();
        Ok(())
    }

    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
        if let Some((generation, mine)) = &self.generation {
            if generation.load(Ordering::SeqCst) != *mine {
                return Err(Error::Output("playback superseded".into()));
            }
        }
        if self.paused.load(Ordering::SeqCst) {
            // Keep the decode loop from racing ahead while paused.
            std::thread::sleep(std::time::Duration::from_millis(20));
            return Ok(());
        }
        let volume = f32::from_bits(self.volume.load(Ordering::SeqCst));
        if (volume - self.last_volume).abs() >= f32::EPSILON {
            self.output.set_volume(volume)?;
            self.last_volume = volume;
        }
        self.output.write(buf)?;
        if self.rate > 0 {
            self.frames += buf.frames(self.channels);
            self.position_ms
                .store(self.frames.saturating_mul(1000) / self.rate as u64, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::NullOutput;
    use crate::source::StreamPlan;
    use std::collections::HashMap;

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

    fn fixture_plan() -> (StreamPlan, HashMap<String, Vec<u8>>) {
        let bytes = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let plan = StreamPlan {
            init_url: None,
            media_urls: vec!["mem://tone".into()],
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
        };
        let mut files = HashMap::new();
        files.insert("mem://tone".to_string(), bytes);
        (plan, files)
    }

    fn item(id: u64) -> QueueItem {
        QueueItem { track_id: id, title: format!("T{id}"), artist: "A".into(), album: "B".into(), duration_secs: 2, cover: None }
    }

    /// A factory that hands out clones of one `NullOutput`, so a test can read
    /// everything the engine wrote.
    fn null_factory() -> (Arc<dyn OutputFactory>, Arc<Mutex<NullOutput>>) {
        let handle = Arc::new(Mutex::new(NullOutput::new()));
        let clone = handle.clone();
        let factory: Arc<dyn OutputFactory> =
            Arc::new(move || -> Box<dyn AudioOutput> { Box::new(clone.lock().unwrap().clone()) });
        (factory, handle)
    }

    fn engine_with(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
    ) -> (Engine, Arc<Mutex<NullOutput>>) {
        let (factory, output) = null_factory();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            factory,
            Quality::Lossless,
        );
        (engine, output)
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
        // The playback thread may emit concurrently; assert a State within the
        // first few events rather than at a fixed index.
        assert!(events.iter().take(4).any(|e| matches!(e, EngineEvent::State(_))));
    }

    #[test]
    fn play_track_into_decodes_fixture() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let mut out = NullOutput::new();
        engine.play_track_into(&item(1), &mut out).unwrap();
        assert_eq!(out.samples().len(), 176_400);
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
    fn playback_thread_writes_samples_and_advances() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        // Wait up to 5s for the first track (2s of audio) to reach the sink.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while output.lock().unwrap().samples().len() < 176_400 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(output.lock().unwrap().samples().len() >= 176_400);
    }

    /// Regression: `PlayPause` on an empty queue must not latch `paused` true and
    /// silence the next `PlayTracks`.
    #[test]
    fn play_pause_on_empty_queue_does_not_silence_later_playback() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayPause).unwrap();
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while output.lock().unwrap().samples().len() < 176_400 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(output.lock().unwrap().samples().len() >= 176_400);
    }

    /// Regression: the position ticker must stop once the queue ends instead of
    /// emitting `Position` forever.
    #[test]
    fn position_ticker_stops_when_queue_ends() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while engine.state().playing && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!engine.state().playing, "single track did not finish");

        let positions = || {
            seen.lock()
                .unwrap()
                .iter()
                .filter(|e| matches!(e, EngineEvent::Position { .. }))
                .count()
        };
        // Let any in-flight ticker emission land, then snapshot.
        std::thread::sleep(std::time::Duration::from_millis(600));
        let before = positions();
        // A still-running ticker would emit ~2 more in this window.
        std::thread::sleep(std::time::Duration::from_millis(1200));
        assert_eq!(positions(), before, "position ticker kept emitting after the queue ended");
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !order.lock().unwrap().contains(&id) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(order.lock().unwrap().contains(&id), "track {id} was never resolved");
    }

    /// Regression: a `Next` issued while a track is playing must not cause a
    /// double advance (duplicate or skipped track). The gated resolver makes the
    /// interleaving deterministic.
    #[test]
    fn next_while_playing_does_not_duplicate_or_skip() {
        let (plan, files) = fixture_plan();
        let gate = Arc::new(Gate::new());
        let order = Arc::new(Mutex::new(Vec::new()));
        let (factory, _output) = null_factory();
        let engine = Engine::new(
            Arc::new(GatedResolver { plan, gate: gate.clone(), order: order.clone() }),
            Arc::new(crate::fetch::MapFetcher { files }),
            factory,
            Quality::Lossless,
        );

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

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while engine.state().playing && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(*order.lock().unwrap(), vec![1, 2, 3]);
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(3));
    }

    /// Regression: a failed `run_track` must stop the queue and the position
    /// ticker, not just emit an error.
    #[test]
    fn playback_error_stops_and_reports() {
        let (factory, _output) = null_factory();
        let engine = Engine::new(
            Arc::new(FailingResolver),
            Arc::new(crate::fetch::MapFetcher::default()),
            factory,
            Quality::Lossless,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. }))
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })),
            "no error event emitted"
        );
        assert!(!engine.state().playing, "queue still reports playing after an error");

        let positions = || {
            seen.lock()
                .unwrap()
                .iter()
                .filter(|e| matches!(e, EngineEvent::Position { .. }))
                .count()
        };
        // Let any in-flight ticker emission land, then snapshot.
        std::thread::sleep(std::time::Duration::from_millis(600));
        let before = positions();
        // A still-running ticker would emit ~2 more in this window.
        std::thread::sleep(std::time::Duration::from_millis(1200));
        assert_eq!(positions(), before, "position ticker kept emitting after a playback error");
    }

    /// Regression: `Next` at the end of the queue with repeat off must not
    /// restart the last track. Audio plays iff `state().playing`.
    #[test]
    fn next_at_end_with_repeat_off_does_not_restart() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while engine.state().playing && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!engine.state().playing, "single track did not finish");

        let before = output.lock().unwrap().samples().len();
        assert!(before >= 176_400);
        engine.handle(EngineCmd::Next).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            output.lock().unwrap().samples().len(),
            before,
            "Next at the end restarted playback"
        );
        assert!(!engine.state().playing, "Next at the end left the queue playing");
    }

    /// Regression: `Prev` at the first track is a no-op and must leave the
    /// running playback (and its generation) untouched.
    #[test]
    fn prev_at_first_track_does_not_restart() {
        let (plan, files) = fixture_plan();
        let gate = Arc::new(Gate::new());
        let (factory, output) = null_factory();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(GatedFetcher { files, gate: gate.clone(), id: 1 }),
            factory,
            Quality::Lossless,
        );
        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();
        // Playback is parked in the fetcher, so nothing has been written yet.
        let before_gen = engine.inner.generation.load(Ordering::SeqCst);
        let before_samples = output.lock().unwrap().samples().len();

        engine.handle(EngineCmd::Prev).unwrap();

        assert_eq!(
            engine.inner.generation.load(Ordering::SeqCst),
            before_gen,
            "no-op Prev bumped the generation"
        );
        assert!(engine.state().playing, "no-op Prev stopped playback");
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(1));
        assert_eq!(output.lock().unwrap().samples().len(), before_samples);
        gate.release(1);
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

    /// Regression: the selected output device must reach `AudioOutput::open`.
    #[test]
    fn set_output_device_is_passed_to_open() {
        let (plan, files) = fixture_plan();
        let specs = Arc::new(Mutex::new(Vec::new()));
        let specs_ref = specs.clone();
        let factory: Arc<dyn OutputFactory> = Arc::new(move || -> Box<dyn AudioOutput> {
            Box::new(RecordingOutput { specs: specs_ref.clone() })
        });
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            factory,
            Quality::Lossless,
        );
        engine.set_output_device(Some("device-x".into()));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while specs.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(specs.lock().unwrap()[0].device_id.as_deref(), Some("device-x"));
    }
}
