use std::sync::Arc;

use oxitide_audio::cache::DiskCache;
use oxitide_audio::engine::{EngineEvent, StreamResolver};
use oxitide_audio::output::{self, AudioOutput, OutputDevice};
use oxitide_audio::source;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::stream::Quality;
use serde::{Deserialize, Serialize};

use crate::config::{default_cache_dir, Config};

/// Build the disk cache from config: the configured directory when usable,
/// else the default, else a temp directory. `DiskCache::new` is infallible.
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

/// The real output: a WASAPI device on Windows, silent `NullOutput` elsewhere
/// (keeps the crate building on non-Windows dev machines).
pub fn real_output() -> Box<dyn AudioOutput> {
    #[cfg(windows)]
    {
        Box::new(oxitide_audio::wasapi_output::WasapiOutput::new())
    }
    #[cfg(not(windows))]
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
    State(oxitide_audio::engine::PlaybackState),
    NowPlaying { item: Option<oxitide_audio::queue::QueueItem> },
    Position { position_ms: u64, duration_ms: u64 },
    Error { code: String, message: String },
}

pub fn to_dto(event: EngineEvent) -> EngineEventDto {
    match event {
        EngineEvent::State(s) => EngineEventDto::State(s),
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
