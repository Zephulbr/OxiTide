use oxitide_audio::engine::{EngineEvent, OutputFactory, StreamResolver};
use oxitide_audio::output::{self, AudioOutput, OutputDevice};
use oxitide_audio::source;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::stream::Quality;
use serde::{Deserialize, Serialize};

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
        source::plan_from_info(&info)
    }
}

/// The real output factory: one `WasapiOutput` per track. Falling back to a
/// silent `NullOutput` on non-Windows keeps the crate building elsewhere.
pub struct RealOutputs;

impl OutputFactory for RealOutputs {
    fn create(&self) -> Box<dyn AudioOutput> {
        #[cfg(windows)]
        {
            Box::new(oxitide_audio::wasapi_output::WasapiOutput::new())
        }
        #[cfg(not(windows))]
        {
            Box::new(oxitide_audio::output::NullOutput::new())
        }
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
    output::enumerate_output_devices()
        .unwrap_or_default()
        .into_iter()
        .map(|OutputDevice { id, name, is_default }| OutputDeviceDto { id, name, is_default })
        .collect()
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
