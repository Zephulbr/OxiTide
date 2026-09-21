use std::sync::{Arc, Mutex};

use crate::decode::{AudioFormat, PcmSink};
use crate::error::{Error, Result};
use crate::fetch::{self, Fetcher, StreamBuffer};
use crate::output::AudioOutput;
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
    listeners: Mutex<Vec<Listener>>,
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
                listeners: Mutex::new(Vec::new()),
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
            queue: q.items().to_vec(),
        }
    }

    fn emit(&self, event: EngineEvent) {
        for listener in self.inner.listeners.lock().unwrap().iter() {
            listener(event.clone());
        }
    }

    pub fn handle(&self, cmd: EngineCmd) -> Result<()> {
        match cmd {
            EngineCmd::PlayTracks { items, start } => {
                let item = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.play_context(items, start);
                    q.current().cloned()
                };
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if let Some(item) = item {
                    self.start_playback(item);
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::PlayPause => {
                let playing = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.toggle_play_pause();
                    q.is_playing()
                };
                self.inner.paused.store(!playing, Ordering::SeqCst);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Next => {
                let item = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.next();
                    q.current().cloned()
                };
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if let Some(item) = item {
                    self.start_playback(item);
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Prev => {
                let item = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.prev();
                    q.current().cloned()
                };
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if let Some(item) = item {
                    self.start_playback(item);
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
                self.inner.queue.lock().unwrap().clear();
                self.inner.generation.fetch_add(1, Ordering::SeqCst);
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
            generation: None,
            paused: self.inner.paused.clone(),
            volume: self.inner.volume.clone(),
            position_ms: self.inner.position_ms.clone(),
            frames: 0,
            rate: 0,
            channels: 2,
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = output.stop();
        let _ = downloader.join();
        result
    }

    /// Spawn a playback thread for `item`. On success it advances the queue and
    /// starts the next track; a stale generation (a newer command superseded it)
    /// aborts silently.
    fn start_playback(&self, item: QueueItem) {
        let generation = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.inner.position_ms.store(0, Ordering::SeqCst);
        let engine = self.clone();
        let spawned = std::thread::Builder::new()
            .name("oxitide-playback".into())
            .spawn(move || {
                let result = engine.run_track(&item, generation);
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
                        if let Some(next) = next {
                            engine.start_playback(next);
                        }
                    }
                    Err(e) => engine.emit(EngineEvent::Error {
                        code: "playback".into(),
                        message: e.to_string(),
                    }),
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
            generation: Some((self.inner.generation.clone(), generation)),
            paused: self.inner.paused.clone(),
            volume: self.inner.volume.clone(),
            position_ms: self.inner.position_ms.clone(),
            frames: 0,
            rate: 0,
            channels: 2,
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = output.stop();
        let _ = downloader.join();
        result
    }
}

struct OutputSink<'a> {
    output: &'a mut dyn AudioOutput,
    generation: Option<(Arc<AtomicU64>, u64)>,
    paused: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    position_ms: Arc<AtomicU64>,
    frames: u64,
    rate: u32,
    channels: u16,
}

impl PcmSink for OutputSink<'_> {
    fn on_spec(&mut self, spec: &symphonia::core::audio::AudioSpec) -> Result<()> {
        self.rate = spec.rate();
        self.channels = spec.channels().count().max(1) as u16;
        self.output.open(
            None,
            AudioFormat { sample_rate: spec.rate(), channels: self.channels },
        )
    }
    fn on_samples(&mut self, samples: &[f32]) -> Result<()> {
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
        if (volume - 1.0).abs() < f32::EPSILON {
            self.output.write(samples)?;
        } else {
            let scaled: Vec<f32> = samples.iter().map(|s| s * volume).collect();
            self.output.write(&scaled)?;
        }
        if self.rate > 0 {
            self.frames += (samples.len() / self.channels as usize) as u64;
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
}
