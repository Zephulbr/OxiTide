use std::sync::Arc;

use oxitide_audio::cache::DiskCache;
use oxitide_audio::engine::{EngineEvent, PlaybackState, StreamResolver};
use oxitide_audio::queue::QueueItem;
use oxitide_audio::output::{self, AudioOutput, OutputDevice};
use oxitide_audio::source;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::stream::Quality;
use serde::{Deserialize, Serialize};

use crate::config::{default_cache_dir, Config};

/// Build the disk cache from config: the configured directory when usable,
/// else the default, else a temp directory. `DiskCache::new` is infallible.
/// The stale-`.part` sweep and cap enforcement (`cleanup`) run on the startup
/// background thread after the window is shown; see `main.rs`.
pub fn build_cache(config: &Config) -> Arc<DiskCache> {
    let max_bytes = config.cache.max_size_mb.saturating_mul(1024 * 1024);
    let configured = config.cache.dir.trim();
    if !configured.is_empty() {
        let cache = DiskCache::new(std::path::PathBuf::from(configured), max_bytes);
        if cache.usable() {
            return cache;
        }
        tracing::warn!("configured cache dir is unusable; falling back to the default");
    }
    let dir = default_cache_dir().unwrap_or_else(|| std::env::temp_dir().join("oxitide-cache"));
    DiskCache::new(dir, max_bytes)
}

/// Resolves a track's stream plan using a `TidalClient`. Runs the async client
/// on a dedicated Tokio runtime owned by this resolver, so it can be called from
/// the engine's worker thread.
pub struct TidalResolver {
    client: TidalClient,
    runtime: tokio::runtime::Runtime,
}

impl TidalResolver {
    pub fn new(client: TidalClient) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        Ok(Self { client, runtime })
    }
}

impl StreamResolver for TidalResolver {
    fn resolve(
        &self,
        track_id: u64,
        quality: Quality,
    ) -> oxitide_audio::Result<oxitide_audio::source::StreamPlan> {
        let info = self
            .runtime
            .block_on(self.client.playback_info(track_id, quality))?;
        // The manifest body carries signed CDN URLs, so only its size is logged.
        tracing::debug!(
            "playbackinfo track {track_id} ({quality:?}): mime={} manifest_len={}",
            info.manifest_mime,
            info.manifest.len()
        );
        source::plan_from_info(&info)
    }
}

/// The real output: WASAPI or ASIO (by device id) on Windows, an ALSA device
/// on Linux, silent `NullOutput` elsewhere (keeps the crate building on other
/// dev machines).
pub fn real_output() -> Box<dyn AudioOutput> {
    #[cfg(windows)]
    {
        Box::new(oxitide_audio::routed_output::RoutedOutput::windows())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(oxitide_audio::alsa_output::AlsaOutput::new())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Box::new(oxitide_audio::output::NullOutput::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityDto {
    Low,
    High,
    Lossless,
    Max,
}

impl QualityDto {
    pub fn to_quality(self) -> Quality {
        match self {
            QualityDto::Low => Quality::Low,
            QualityDto::High => Quality::High,
            QualityDto::Lossless => Quality::Lossless,
            QualityDto::Max => Quality::Max,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEventDto {
    State(StateDto),
    NowPlaying { item: Option<oxitide_audio::queue::QueueItem> },
    Position { position_ms: u64, duration_ms: u64 },
    Error { code: String, message: String },
}

/// `PlaybackState` for the desktop UI. `queue` is present when the UI may not
/// have this revision yet (every `get_playback_state`, and a state event whose
/// `queue_rev` moved); otherwise it is omitted and the UI keeps its copy.
/// Track 13 (EF-D1): a volume drag no longer re-sends a 10k-item queue.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateDto {
    #[serde(flatten)]
    pub state: PlaybackState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<Arc<Vec<QueueItem>>>,
}

impl StateDto {
    pub fn full(state: PlaybackState) -> Self {
        let queue = Some(state.queue.clone());
        Self { state, queue }
    }
}

/// Remembers the queue revision last sent to the desktop UI.
#[derive(Default)]
pub struct QueueGate {
    sent: std::sync::Mutex<Option<u64>>,
}

impl QueueGate {
    pub fn dto(&self, event: EngineEvent) -> EngineEventDto {
        match event {
            EngineEvent::State(state) => {
                let mut sent = self.sent.lock().unwrap();
                let queue = (*sent != Some(state.queue_rev)).then(|| state.queue.clone());
                *sent = Some(state.queue_rev);
                EngineEventDto::State(StateDto { state, queue })
            }
            other => to_dto(other),
        }
    }
}

/// Every state carries its queue (tests and one-off conversions).
pub fn to_dto(event: EngineEvent) -> EngineEventDto {
    match event {
        EngineEvent::State(s) => EngineEventDto::State(StateDto::full(s)),
        EngineEvent::NowPlaying(i) => EngineEventDto::NowPlaying { item: i },
        EngineEvent::Position { position_ms, duration_ms } => EngineEventDto::Position { position_ms, duration_ms },
        EngineEvent::Error { code, message } => EngineEventDto::Error { code, message },
    }
}

#[derive(Serialize)]
pub struct OutputDeviceDto {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn devices() -> Vec<OutputDeviceDto> {
    match output::enumerate_output_devices() {
        Ok(devices) => devices
            .into_iter()
            .map(|OutputDevice { id, name, is_default }| OutputDeviceDto { id, name, is_default })
            .collect(),
        Err(e) => {
            tracing::warn!("failed to enumerate output devices: {e}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn build_cache_selects_the_directory_and_cleanup_sweeps_stale_parts() {
        let dir =
            std::env::temp_dir().join(format!("oxitide-build-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("1-lossless-0000000000000001.fmp4.part");
        std::fs::write(&part, b"stale").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&part).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(2 * 3600)).unwrap();
        drop(file);

        let mut config = Config::default();
        config.cache.dir = dir.display().to_string();
        config.cache.max_size_mb = 1;

        let cache = build_cache(&config);
        assert_eq!(cache.dir(), dir.as_path());
        assert_eq!(cache.max_bytes(), 1024 * 1024);
        assert!(part.exists(), "build_cache must not scan the directory");
        cache.cleanup();
        assert!(!part.exists(), "cleanup must remove the stale .part");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn quality_dto_round_trips() {
        assert_eq!(QualityDto::Max.to_quality(), Quality::Max);
        assert_eq!(serde_json::to_value(QualityDto::Lossless).unwrap(), serde_json::json!("lossless"));
    }

    #[test]
    fn engine_event_dto_is_tagged() {
        let dto = to_dto(EngineEvent::Error { code: "auth".into(), message: "no".into() });
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["type"], "error");
        assert_eq!(value["code"], "auth");
    }

    fn state_with(ids: &[u64], volume: f32, queue_rev: u64) -> PlaybackState {
        use crate::remote::events::tests::{item, playback};
        let mut state = playback(ids.iter().map(|&id| item(id)).collect(), Some(0));
        state.volume = volume;
        state.queue_rev = queue_rev;
        state
    }

    #[test]
    fn the_gate_sends_a_queue_only_when_its_revision_moves() {
        let gate = QueueGate::default();
        let json = |event| serde_json::to_value(gate.dto(event)).unwrap();
        let first = json(EngineEvent::State(state_with(&[1, 2, 3], 1.0, 4)));
        assert_eq!(first["type"], "state");
        assert_eq!(first["queue"].as_array().map(Vec::len), Some(3), "first state must carry the queue");
        assert_eq!(first["queue_rev"], 4);
        let volume = json(EngineEvent::State(state_with(&[1, 2, 3], 0.5, 4)));
        assert!(volume.get("queue").is_none(), "an unchanged queue was re-sent: {volume}");
        assert_eq!(volume["volume"], 0.5);
        let edited = json(EngineEvent::State(state_with(&[1, 2], 0.5, 5)));
        assert_eq!(edited["queue"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn a_full_state_always_carries_the_queue() {
        let value = serde_json::to_value(StateDto::full(state_with(&[7], 1.0, 9))).unwrap();
        assert_eq!(value["queue"][0]["track_id"], 7);
        assert_eq!(value["queue_rev"], 9);
        assert_eq!(value["playing"], true, "the state fields are flattened");
    }

    /// Track 13 (EF-D1): what a volume tick costs on the wire with a 10k queue.
    #[test]
    fn a_volume_state_stays_small_with_a_large_queue() {
        let ids: Vec<u64> = (1..=10_000).collect();
        let gate = QueueGate::default();
        let _ = gate.dto(EngineEvent::State(state_with(&ids, 1.0, 1)));
        let tick = serde_json::to_vec(&gate.dto(EngineEvent::State(state_with(&ids, 0.4, 1)))).unwrap();
        assert!(tick.len() < 4096, "volume state is {} bytes", tick.len());
    }

    #[test]
    fn now_playing_none_serializes() {
        // Internally tagged enums cannot carry a bare `null` newtype payload,
        // which is why `NowPlaying` is a struct variant.
        let dto = to_dto(EngineEvent::NowPlaying(None));
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["type"], "now_playing");
        assert!(value["item"].is_null());
    }
}
