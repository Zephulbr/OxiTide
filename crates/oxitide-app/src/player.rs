use std::sync::Arc;

use oxitide_audio::cache::DiskCache;
use oxitide_audio::devices::{DeviceCaps, DeviceGroup, DeviceKind, Transport};
use oxitide_audio::engine::{EngineEvent, PlaybackState, StreamResolver};
use oxitide_audio::output::{self, AudioOutput, OutputDevice, OutputMode};
use oxitide_audio::queue::QueueItem;
use oxitide_audio::source;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::stream::Quality;
use serde::{Deserialize, Serialize};

use crate::config::{default_cache_dir, AudioConfig, Config};

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
    /// `None` only during `drop`.
    runtime: Option<tokio::runtime::Runtime>,
}

impl TidalResolver {
    pub fn new(client: TidalClient) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        Ok(Self {
            client,
            runtime: Some(runtime),
        })
    }
}

impl Drop for TidalResolver {
    /// The engine's last reference can go inside an async command (a player
    /// rebuild), where a plain runtime drop panics; shut down without blocking.
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
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
            .as_ref()
            .expect("runtime lives until drop")
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
    NowPlaying {
        item: Option<oxitide_audio::queue::QueueItem>,
    },
    Position {
        position_ms: u64,
        duration_ms: u64,
        lead_ms: u32,
    },
    /// CatJam's beat clock (local analysis; `bpm` is null when not confident).
    Beat {
        track_id: u64,
        bpm: Option<f32>,
        period_ms: f32,
        anchor_ms: u64,
        confidence: f32,
    },
    Error {
        code: String,
        message: String,
    },
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
        EngineEvent::Position {
            position_ms,
            duration_ms,
            lead_ms,
        } => EngineEventDto::Position {
            position_ms,
            duration_ms,
            lead_ms,
        },
        EngineEvent::Beat(clock) => EngineEventDto::Beat {
            track_id: clock.track_id,
            bpm: clock.bpm,
            period_ms: clock.period_ms,
            anchor_ms: clock.anchor_ms,
            confidence: clock.confidence,
        },
        EngineEvent::Error { code, message } => EngineEventDto::Error { code, message },
    }
}

/// One picker row (spec `2026-11-05-oxitide-output-device-picker-design.md` §5.2).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutputDeviceDto {
    pub id: String,
    pub name: String,
    pub system_name: String,
    pub group: DeviceGroup,
    pub kind: DeviceKind,
    pub transport: Transport,
    pub is_default: bool,
    pub connected: bool,
    pub wrapper: bool,
    /// False for an output with nothing attached (an HDMI port without a monitor).
    pub present: bool,
    pub caps: Option<DeviceCaps>,
    pub prefs: DevicePrefsDto,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DevicePrefsDto {
    pub nickname: String,
    pub icon: String,
    pub pinned: bool,
    pub hidden: bool,
    pub mode: OutputMode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SystemDefaultDto {
    /// The device the system default is right now (Windows), if known.
    pub now: Option<String>,
    pub mode: OutputMode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutputDeviceListDto {
    pub system_default: SystemDefaultDto,
    pub devices: Vec<OutputDeviceDto>,
    pub pinned: Vec<String>,
    pub selected: String,
    /// `ui.device_icons`, so the quality menu needs no config read.
    pub icons: bool,
    /// `(old id, new id)`: a device whose id changed (a USB DAC on another
    /// port) that should take over the old id's settings (`move_device_prefs`).
    pub moved: Vec<(String, String)>,
}

/// The connected devices, or none (logged as a warning).
pub fn enumerate_devices() -> Vec<OutputDevice> {
    output::enumerate_output_devices().unwrap_or_else(|e| {
        tracing::warn!("failed to enumerate output devices: {e}");
        Vec::new()
    })
}

pub fn transport_name(transport: Transport) -> &'static str {
    match transport {
        Transport::Usb => "usb",
        Transport::Bluetooth => "bluetooth",
        Transport::Hdmi => "hdmi",
        Transport::Internal => "internal",
        Transport::Virtual => "virtual",
        Transport::Unknown => "",
    }
}

/// The picker group of a saved id whose device is not connected.
fn group_of(id: &str) -> DeviceGroup {
    if output::is_asio_device(id) {
        DeviceGroup::Asio
    } else if id.starts_with("hw:") || cfg!(target_os = "linux") {
        DeviceGroup::Alsa
    } else {
        DeviceGroup::Wasapi
    }
}

/// Merge the connected devices with the saved settings: prefs on each row,
/// "Not connected" rows for selected or pinned devices that are gone (DP-D11),
/// and id-drift candidates (§6.3).
pub fn device_list_dto(
    devices: Vec<OutputDevice>,
    audio: &AudioConfig,
    caps: impl Fn(&str) -> Option<DeviceCaps>,
) -> OutputDeviceListDto {
    let prefs_for = |id: &str| {
        let saved = audio.devices.get(id).cloned().unwrap_or_default();
        DevicePrefsDto {
            nickname: saved.nickname,
            icon: saved.icon,
            pinned: audio.pinned.iter().any(|pin| pin == id),
            hidden: saved.hidden,
            mode: audio.mode_for(id),
        }
    };
    let now = devices
        .iter()
        .find(|device| device.is_default && device.group != DeviceGroup::Asio)
        .map(|device| {
            audio
                .devices
                .get(&device.id)
                .map(|p| p.nickname.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| device.name.clone())
        });

    // Saved ids with no connected device that someone cares about.
    let mut wanted: Vec<String> = Vec::new();
    for id in std::iter::once(&audio.output_device)
        .chain(audio.pinned.iter())
        .chain(
            audio
                .devices
                .iter()
                .filter(|(_, p)| !p.is_default_choice())
                .map(|(id, _)| id),
        )
    {
        if !id.is_empty() && !devices.iter().any(|d| &d.id == id) && !wanted.contains(id) {
            wanted.push(id.clone());
        }
    }
    let mut moved = Vec::new();
    for old in &wanted {
        let Some(saved) = audio.devices.get(old) else {
            continue;
        };
        if saved.system_name.is_empty() {
            continue;
        }
        let candidate = devices.iter().find(|d| {
            d.group == group_of(old)
                && d.system_name == saved.system_name
                && transport_name(d.transport) == saved.transport
                && !audio.devices.contains_key(&d.id)
                && !audio.pinned.contains(&d.id)
                && !moved.iter().any(|(_, new): &(String, String)| new == &d.id)
        });
        if let Some(device) = candidate {
            moved.push((old.clone(), device.id.clone()));
        }
    }

    let mut rows: Vec<OutputDeviceDto> = devices
        .into_iter()
        .map(|device| OutputDeviceDto {
            caps: caps(&device.id),
            prefs: prefs_for(&device.id),
            id: device.id,
            name: device.name,
            system_name: device.system_name,
            group: device.group,
            kind: device.kind,
            transport: device.transport,
            is_default: device.is_default,
            connected: true,
            wrapper: device.wrapper,
            present: device.present,
        })
        .collect();
    for id in wanted {
        if moved.iter().any(|(old, _)| old == &id) {
            continue;
        }
        let saved = audio.devices.get(&id).cloned().unwrap_or_default();
        if !audio.pinned.contains(&id) && id != audio.output_device {
            continue;
        }
        let name = if saved.name.is_empty() {
            id.clone()
        } else {
            saved.name.clone()
        };
        rows.push(OutputDeviceDto {
            caps: caps(&id),
            prefs: prefs_for(&id),
            system_name: if saved.system_name.is_empty() {
                name.clone()
            } else {
                saved.system_name.clone()
            },
            name,
            group: group_of(&id),
            kind: DeviceKind::Unknown,
            transport: Transport::Unknown,
            is_default: false,
            connected: false,
            wrapper: false,
            present: true,
            id,
        });
    }
    OutputDeviceListDto {
        system_default: SystemDefaultDto {
            now,
            mode: audio.mode_for(""),
        },
        devices: rows,
        pinned: audio.pinned.clone(),
        selected: audio.output_device.clone(),
        icons: true,
        moved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DevicePrefs;
    use std::time::{Duration, SystemTime};

    fn wasapi(id: &str, name: &str, system_name: &str, transport: Transport) -> OutputDevice {
        let mut device =
            OutputDevice::new(id, name, DeviceGroup::Wasapi).with_system_name(system_name);
        device.transport = transport;
        device
    }

    #[test]
    fn device_list_merges_prefs_and_keeps_missing_selected_or_pinned_devices() {
        let mut audio = AudioConfig {
            output_device: "gone-dac".into(),
            pinned: vec!["speakers".into(), "gone-tv".into()],
            ..AudioConfig::default()
        };
        audio.devices.insert(
            "gone-dac".into(),
            DevicePrefs {
                name: "Living room DAC".into(),
                mode: OutputMode::Exclusive,
                ..DevicePrefs::default()
            },
        );
        audio.devices.insert(
            "speakers".into(),
            DevicePrefs {
                nickname: "Desk".into(),
                ..DevicePrefs::default()
            },
        );
        let mut speakers = wasapi(
            "speakers",
            "Speakers",
            "Speakers (Realtek(R) Audio)",
            Transport::Internal,
        );
        speakers.is_default = true;
        let list = device_list_dto(vec![speakers], &audio, |id| {
            (id == "speakers").then(|| DeviceCaps::from_formats(&[(192_000, 24)]))
        });
        assert_eq!(
            list.system_default.now.as_deref(),
            Some("Desk"),
            "the nickname names the default"
        );
        assert_eq!(list.selected, "gone-dac");
        let ids: Vec<(&str, bool)> = list
            .devices
            .iter()
            .map(|d| (d.id.as_str(), d.connected))
            .collect();
        assert_eq!(
            ids,
            vec![("speakers", true), ("gone-dac", false), ("gone-tv", false)]
        );
        let speakers = &list.devices[0];
        assert!(speakers.prefs.pinned);
        assert_eq!(speakers.prefs.nickname, "Desk");
        assert!(speakers.caps.as_ref().unwrap().exclusive);
        let dac = &list.devices[1];
        assert_eq!(dac.name, "Living room DAC");
        assert_eq!(dac.prefs.mode, OutputMode::Exclusive);
        assert_eq!(list.devices[2].name, "gone-tv", "no saved name: the id");
    }

    #[test]
    fn a_device_on_a_new_id_is_offered_the_old_settings() {
        let mut audio = AudioConfig {
            output_device: "old".into(),
            pinned: vec!["old".into()],
            ..AudioConfig::default()
        };
        audio.devices.insert(
            "old".into(),
            DevicePrefs {
                name: "DX5".into(),
                system_name: "DX5 (TOPPING USB DAC)".into(),
                transport: "usb".into(),
                nickname: "Desk DAC".into(),
                ..DevicePrefs::default()
            },
        );
        // In the group a plain id has on this platform (ALSA on Linux), as
        // the drift match only looks within the saved id's group.
        let devices: Vec<OutputDevice> = vec![
            wasapi("new", "DX5", "DX5 (TOPPING USB DAC)", Transport::Usb),
            wasapi("other", "DX5", "DX5 (TOPPING USB DAC)", Transport::Hdmi),
        ]
        .into_iter()
        .map(|mut d| {
            d.group = group_of(&d.id);
            d
        })
        .collect();
        let list = device_list_dto(devices, &audio, |_| None);
        assert_eq!(list.moved, vec![("old".to_string(), "new".to_string())]);
        assert!(
            list.devices.iter().all(|d| d.connected),
            "the moved id is not listed as missing"
        );
    }

    #[test]
    fn asio_rows_always_report_exclusive() {
        let audio = AudioConfig::default();
        let list = device_list_dto(
            vec![OutputDevice::new(
                "asio:Focusrite",
                "Focusrite USB ASIO",
                DeviceGroup::Asio,
            )],
            &audio,
            |_| None,
        );
        assert_eq!(list.devices[0].prefs.mode, OutputMode::Exclusive);
        assert_eq!(list.system_default.mode, OutputMode::Shared);
    }

    #[test]
    fn build_cache_selects_the_directory_and_cleanup_sweeps_stale_parts() {
        let dir =
            std::env::temp_dir().join(format!("oxitide-build-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("1-lossless-0000000000000001.fmp4.part");
        std::fs::write(&part, b"stale").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&part).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(2 * 3600))
            .unwrap();
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

    /// A rebuild (`auth_restore` on a page reload) can drop the old engine's
    /// last reference inside an async command; the resolver's runtime must
    /// not panic there ("Cannot drop a runtime in a context where blocking is
    /// not allowed"), which used to poison `AppState::player`.
    #[tokio::test]
    async fn resolver_drops_inside_an_async_context() {
        use oxitide_tidal::auth::SessionInfo;
        use oxitide_tidal::token_store::{MemoryStore, TokenSet};
        let client = TidalClient::new(
            reqwest::Client::new(),
            "http://127.0.0.1:9",
            "http://127.0.0.1:9/token",
            SessionInfo {
                session_id: "s".into(),
                country_code: "GR".into(),
                user_id: 1,
            },
            TokenSet {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                token_type: "Bearer".into(),
                expires_at: None,
            },
            Arc::new(MemoryStore::default()),
        );
        drop(TidalResolver::new(client).unwrap());
    }

    #[test]
    fn quality_dto_round_trips() {
        assert_eq!(QualityDto::Max.to_quality(), Quality::Max);
        assert_eq!(
            serde_json::to_value(QualityDto::Lossless).unwrap(),
            serde_json::json!("lossless")
        );
    }

    #[test]
    fn engine_event_dto_is_tagged() {
        let dto = to_dto(EngineEvent::Error {
            code: "auth".into(),
            message: "no".into(),
        });
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["type"], "error");
        assert_eq!(value["code"], "auth");
    }

    #[test]
    fn position_and_beat_events_carry_the_catjam_fields() {
        let position = to_dto(EngineEvent::Position {
            position_ms: 5000,
            duration_ms: 9000,
            lead_ms: 215,
        });
        let value = serde_json::to_value(&position).unwrap();
        assert_eq!(value["type"], "position");
        assert_eq!(value["lead_ms"], 215);

        let beat = to_dto(EngineEvent::Beat(oxitide_audio::beat::BeatClock {
            track_id: 7,
            bpm: Some(128.0),
            period_ms: 468.75,
            anchor_ms: 61_250,
            confidence: 0.8,
        }));
        let value = serde_json::to_value(&beat).unwrap();
        assert_eq!(value["type"], "beat");
        assert_eq!(value["track_id"], 7);
        assert_eq!(value["bpm"], 128.0);
        assert_eq!(value["period_ms"], 468.75);
        assert_eq!(value["anchor_ms"], 61_250);

        let unsure = to_dto(EngineEvent::Beat(oxitide_audio::beat::BeatClock {
            track_id: 7,
            bpm: None,
            period_ms: 500.0,
            anchor_ms: 0,
            confidence: 0.1,
        }));
        assert!(serde_json::to_value(&unsure).unwrap()["bpm"].is_null());
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
        assert_eq!(
            first["queue"].as_array().map(Vec::len),
            Some(3),
            "first state must carry the queue"
        );
        assert_eq!(first["queue_rev"], 4);
        let volume = json(EngineEvent::State(state_with(&[1, 2, 3], 0.5, 4)));
        assert!(
            volume.get("queue").is_none(),
            "an unchanged queue was re-sent: {volume}"
        );
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
        let tick =
            serde_json::to_vec(&gate.dto(EngineEvent::State(state_with(&ids, 0.4, 1)))).unwrap();
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
