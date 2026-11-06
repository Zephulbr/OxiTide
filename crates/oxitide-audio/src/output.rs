use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::decode::{PcmBuffer, PcmFormat, StreamInfo};
use crate::error::Result;
#[cfg(windows)]
use crate::error::Error;

pub use crate::devices::{CapsSource, DeviceCaps, DeviceGroup, DeviceKind, Transport};

/// One output the picker lists (spec `2026-11-05-oxitide-output-device-picker-design.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    /// What the backends open: a WASAPI endpoint id, `asio:<key>`, or
    /// `hw:CARD=<id>,DEV=<n>`.
    pub id: String,
    /// Clean display name (`devices::wasapi_identity`, `alsa_identity`).
    pub name: String,
    /// The system's full name, for the tooltip and the nickname placeholder.
    pub system_name: String,
    pub group: DeviceGroup,
    pub kind: DeviceKind,
    pub transport: Transport,
    pub is_default: bool,
    /// An ASIO driver that wraps the Windows audio engine (ASIO4ALL, …).
    pub wrapper: bool,
    /// False for an output with nothing attached (an HDMI port without a monitor).
    pub present: bool,
}

impl OutputDevice {
    pub fn new(id: impl Into<String>, name: impl Into<String>, group: DeviceGroup) -> Self {
        let name = name.into();
        Self {
            id: id.into(),
            system_name: name.clone(),
            name,
            group,
            kind: DeviceKind::Unknown,
            transport: Transport::Unknown,
            is_default: false,
            wrapper: false,
            present: true,
        }
    }

    pub fn with_system_name(mut self, system_name: impl Into<String>) -> Self {
        self.system_name = system_name.into();
        self
    }
}

/// How the output was asked to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    #[default]
    Shared,
    Exclusive,
}

/// What to do when the exclusive format is not supported by the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusiveFallback {
    #[default]
    Shared,
    WarnOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioOutputSpec {
    pub device_id: Option<String>,
    pub info: StreamInfo,
    pub mode: OutputMode,
    pub exclusive_fallback: ExclusiveFallback,
}

/// What the output actually opened as, and whether the data path is still
/// bit-perfect.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputStatus {
    pub mode: OutputMode,
    pub bit_perfect: bool,
    pub notice: Option<String>,
}

/// Device ids of ASIO drivers carry this prefix before the registry key.
pub const ASIO_PREFIX: &str = "asio:";

/// Notice shown while an exclusive output scales samples in software.
pub const SOFTWARE_VOLUME_NOTICE: &str = "volume applied in software; bit-perfect compromised";

/// Exponent of the loudness-linear volume taper: `1 / (2·log10 2)`.
///
/// Perceived loudness doubles for every +10 dB (Stevens' power law: loudness
/// grows with amplitude^0.6). Making loudness proportional to the slider
/// position therefore means `dB = 10·log2(position)`, i.e. an amplitude of
/// `position^1.661`: 50% sounds half as loud as 100% (-10 dB), 25% half as
/// loud again (-20 dB), and every 5% step adds the same loudness anywhere on
/// the slider. This is the taper Windows' own `SetMasterVolumeLevelScalar`
/// gives on drivers that honour dB (about -10 dB at 50%, -35 dB at 10%), so
/// shared and exclusive output feel the same.
///
/// Rejected: linear amplitude (loudness piles into the bottom 20%: 5% -> 10%
/// is +6 dB, 85% -> 90% is +0.5 dB) and the cubic / 60 dB-exponential
/// curves of PulseAudio, ALSA and Dr. Lex's guide. Those suit a master
/// volume that spans the full range down to silence; on top of the system
/// volume they leave the bottom third inaudible (cubic is -31 dB at 30%).
pub const VOLUME_TAPER_EXPONENT: f32 = 1.661;

/// The amplitude gain for a volume-slider position (0.0..=1.0), on the
/// loudness-linear taper ([`VOLUME_TAPER_EXPONENT`]). The ends are exact
/// (0.0 is silence, 1.0 is unity, so full volume stays a no-op). Every
/// `volume` in the app is a slider position; only software scaling converts
/// it here. Endpoint volume needs no conversion:
/// `SetMasterVolumeLevelScalar` already applies the same taper.
pub fn volume_gain(position: f32) -> f32 {
    position.clamp(0.0, 1.0).powf(VOLUME_TAPER_EXPONENT)
}

/// The slider position for an amplitude gain; the inverse of
/// [`volume_gain`]. Converts volumes saved before the taper existed.
pub fn volume_position(gain: f32) -> f32 {
    gain.clamp(0.0, 1.0).powf(1.0 / VOLUME_TAPER_EXPONENT)
}

/// The volume (slider position) an output plays at, shared between the
/// engine and the thread that feeds the device. The engine writes it the
/// moment a volume command arrives; the device thread reads it every period
/// and glides to it (`gain::GainRamp`, or stepped endpoint volume), so a
/// change is heard within a device period instead of after the queued audio.
#[derive(Debug)]
pub struct VolumeControl {
    position: AtomicU32,
}

impl VolumeControl {
    pub fn new(position: f32) -> Self {
        Self { position: AtomicU32::new(position.clamp(0.0, 1.0).to_bits()) }
    }

    pub fn set(&self, position: f32) {
        self.position.store(position.clamp(0.0, 1.0).to_bits(), Ordering::SeqCst);
    }

    pub fn position(&self) -> f32 {
        f32::from_bits(self.position.load(Ordering::SeqCst))
    }
}

impl Default for VolumeControl {
    fn default() -> Self {
        Self::new(1.0)
    }
}

/// The ASIO driver key in an `asio:<key>` device id.
pub fn asio_device_key(device_id: &str) -> Option<&str> {
    device_id.strip_prefix(ASIO_PREFIX).filter(|key| !key.is_empty())
}

pub fn is_asio_device(device_id: &str) -> bool {
    asio_device_key(device_id).is_some()
}

/// The mode a session runs in: ASIO drivers are always exclusive, so the
/// engine sends them sample-exact integers whatever the bit-perfect toggle.
pub fn effective_mode(mode: OutputMode, device_id: Option<&str>) -> OutputMode {
    if device_id.is_some_and(is_asio_device) {
        OutputMode::Exclusive
    } else {
        mode
    }
}

/// A 32-bit container with `valid_bits` significant bits. `valid_bits >= info
/// bit depth` is safe because symphonia left-aligns integer samples to 32
/// bits: the same buffer is valid for any larger valid-bits count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExclusiveFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub store_bits: u16,
    pub valid_bits: u16,
}

/// Formats to try, in order, for an exclusive open; empty when the source is
/// not an integer 16/24-bit stream.
pub fn exclusive_formats(info: &StreamInfo) -> Vec<ExclusiveFormat> {
    let PcmFormat::I32 { bits } = info.format else {
        return Vec::new();
    };
    if !info.is_bit_perfect_candidate() {
        return Vec::new();
    }
    let mut out: Vec<ExclusiveFormat> = Vec::new();
    for valid_bits in [bits, 24, 32] {
        if valid_bits < bits {
            continue;
        }
        let candidate = ExclusiveFormat {
            sample_rate: info.sample_rate,
            channels: info.channels,
            store_bits: 32,
            valid_bits,
        };
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

pub trait AudioOutput: Send {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn write(&mut self, buf: &PcmBuffer) -> Result<()>;
    /// Set the volume (a slider position). Backends store it in their
    /// [`VolumeControl`] and apply it on the device thread.
    fn set_volume(&mut self, volume: f32) -> Result<()>;
    /// Share the engine's volume control, so a volume command reaches the
    /// device thread directly instead of waiting for the playback worker's
    /// next write. Outputs that apply volume elsewhere ignore it.
    fn attach_volume(&mut self, _control: Arc<VolumeControl>) {}
    fn flush(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    /// The mode/bit-perfect state currently in effect.
    fn status(&self) -> OutputStatus;
    /// How much written audio has not been heard yet, in ms: the backend's
    /// ring plus the device buffer it negotiated. `None` when unknown. A cheap
    /// read of counters the writer side owns; callable after every `write`.
    fn queued_ms(&self) -> Option<u32> {
        None
    }
    /// Times the device ran out of written audio, cumulative over this
    /// output's life (opens included). It counts every dry-out, the ones a
    /// pause or seek causes too; the engine keeps only those during playback.
    /// `None` when the backend cannot tell.
    fn underruns(&self) -> Option<u64> {
        None
    }
    /// True when the open fell back from the device it was asked for and
    /// that device (or the default standing in for it) has since come back:
    /// the engine reopens at the current position, without pausing. A cheap
    /// flag read, polled while playing; a fresh `open` clears it.
    fn needs_reopen(&self) -> bool {
        false
    }
    /// Playback is pausing, or skipping on the open device: end what was
    /// written without a click, so the device plays out its buffer into
    /// silence (ALSA holds back its newest few ms and writes them faded).
    /// The next write after it fades in.
    /// The default does nothing (WASAPI and ASIO have not needed it yet).
    fn fade_out(&mut self) -> Result<()> {
        Ok(())
    }
    /// Wait until the audio written so far has been heard, so the `stop` that
    /// follows does not cut it off (the end of the queue, a reopen for the
    /// next track's format). Returns early once `cancel` says so, and is
    /// bounded, so a stalled device cannot hang the caller. The default (an
    /// output with no buffer of its own) returns at once.
    fn drain(&mut self, cancel: &dyn Fn() -> bool) -> Result<()> {
        let _ = cancel;
        Ok(())
    }
}

/// Written audio to observe after a reset (open, flush) before sampling
/// again: the output refills its ring meanwhile and reads low.
const LEAD_SETTLE_MS: u64 = 500;
/// Smoothing per observed write; averages the ring's fill swing (a full ring
/// drains a quarter before the writer tops it up).
const LEAD_ALPHA: f32 = 0.1;

/// Smoothed `queued_ms`: how far the written position runs ahead of what is
/// heard, for UI sync (spec `2026-10-21-oxitide-catjam-kuroneko-design.md`
/// §5.2). Fed once per written buffer by the playback worker.
#[derive(Debug, Default)]
pub(crate) struct LeadEstimate {
    value: Option<f32>,
    settle_frames: u64,
}

impl LeadEstimate {
    /// Ignore the next `LEAD_SETTLE_MS` of written audio (at `rate`). The
    /// previous value stays as the estimate meanwhile.
    pub fn reset(&mut self, rate: u32) {
        self.settle_frames = rate as u64 * LEAD_SETTLE_MS / 1000;
    }

    /// Record one write of `frames` after which the output reported `queued`.
    pub fn observe(&mut self, frames: u64, queued: Option<u32>) {
        if self.settle_frames > 0 {
            self.settle_frames = self.settle_frames.saturating_sub(frames);
            return;
        }
        let Some(queued) = queued else {
            self.value = None;
            return;
        };
        let sample = queued as f32;
        self.value = Some(match self.value {
            Some(value) => value + LEAD_ALPHA * (sample - value),
            None => sample,
        });
    }

    pub fn ms(&self) -> u32 {
        self.value.map_or(0, |v| v.round().max(0.0) as u32)
    }
}

/// Underruns during playback, from an output's cumulative
/// [`AudioOutput::underruns`]. The output also counts the dry-outs the engine
/// causes itself (a pause, a network stall, the moment before a seek's flush),
/// so the count is re-read at the first write after each of those
/// ([`UnderrunTally::reset`]) and whatever happened before that is dropped.
#[derive(Debug, Default)]
pub(crate) struct UnderrunTally {
    baseline: Option<u64>,
}

impl UnderrunTally {
    /// Feeding resumes after a pause, flush, open or network stall.
    pub fn reset(&mut self) {
        self.baseline = None;
    }

    /// After a write: the underruns since the previous write, 0 for the first
    /// write after a reset.
    pub fn observe(&mut self, count: Option<u64>) -> u64 {
        let Some(count) = count else {
            return 0;
        };
        let new = self
            .baseline
            .map_or(0, |baseline| count.saturating_sub(baseline));
        self.baseline = Some(count);
        new
    }
}

/// Captures everything written, for tests and headless use. Clones share all
/// recorded state.
#[derive(Clone)]
pub struct NullOutput {
    volume: f32,
    samples: Arc<Mutex<Vec<f32>>>,
    i32_samples: Arc<Mutex<Vec<i32>>>,
    opens: Arc<AtomicUsize>,
    stops: Arc<AtomicUsize>,
    flushes: Arc<AtomicUsize>,
    fade_outs: Arc<AtomicUsize>,
    last_spec: Arc<Mutex<Option<AudioOutputSpec>>>,
    volumes: Arc<Mutex<Vec<f32>>>,
    status: Arc<Mutex<OutputStatus>>,
    queued_ms: Arc<AtomicU32>,
    underruns: Arc<AtomicU64>,
    /// What `needs_reopen` reports; `open` clears it.
    reopen: Arc<AtomicBool>,
    /// `open` / `flush` / `drain` / `stop`, in call order.
    calls: Arc<Mutex<Vec<&'static str>>>,
    /// The engine's control, once attached. `set_volume` stores into it like
    /// the real backends; writes still scale by the last `set_volume`.
    control: Arc<Mutex<Option<Arc<VolumeControl>>>>,
}

impl Default for NullOutput {
    fn default() -> Self {
        Self {
            volume: 1.0,
            samples: Arc::new(Mutex::new(Vec::new())),
            i32_samples: Arc::new(Mutex::new(Vec::new())),
            opens: Arc::new(AtomicUsize::new(0)),
            stops: Arc::new(AtomicUsize::new(0)),
            flushes: Arc::new(AtomicUsize::new(0)),
            fade_outs: Arc::new(AtomicUsize::new(0)),
            last_spec: Arc::new(Mutex::new(None)),
            volumes: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(Mutex::new(OutputStatus::default())),
            queued_ms: Arc::new(AtomicU32::new(0)),
            underruns: Arc::new(AtomicU64::new(0)),
            reopen: Arc::new(AtomicBool::new(false)),
            calls: Arc::new(Mutex::new(Vec::new())),
            control: Arc::new(Mutex::new(None)),
        }
    }
}

impl NullOutput {
    pub fn new() -> Self {
        Self::default()
    }
    /// The volume control the engine attached, if any.
    pub fn volume_control(&self) -> Option<Arc<VolumeControl>> {
        self.control.lock().unwrap().clone()
    }
    pub fn samples(&self) -> Vec<f32> {
        self.samples.lock().unwrap().clone()
    }
    pub fn i32_samples(&self) -> Vec<i32> {
        self.i32_samples.lock().unwrap().clone()
    }
    pub fn opens(&self) -> usize {
        self.opens.load(Ordering::SeqCst)
    }
    pub fn stops(&self) -> usize {
        self.stops.load(Ordering::SeqCst)
    }
    pub fn flushes(&self) -> usize {
        self.flushes.load(Ordering::SeqCst)
    }
    pub fn fade_outs(&self) -> usize {
        self.fade_outs.load(Ordering::SeqCst)
    }
    pub fn last_spec(&self) -> Option<AudioOutputSpec> {
        self.last_spec.lock().unwrap().clone()
    }
    pub fn volumes(&self) -> Vec<f32> {
        self.volumes.lock().unwrap().clone()
    }
    /// What `queued_ms` reports (0 unless set), as if audio were buffered.
    pub fn set_queued_ms(&self, ms: u32) {
        self.queued_ms.store(ms, Ordering::SeqCst);
    }
    /// Count `n` more dry-outs in `underruns`, as if the device ran dry.
    pub fn add_underruns(&self, n: u64) {
        self.underruns.fetch_add(n, Ordering::SeqCst);
    }
    /// Report `needs_reopen` until the next `open`, as if the selected device
    /// came back while playing on a fallback.
    pub fn request_reopen(&self) {
        self.reopen.store(true, Ordering::SeqCst);
    }
    /// The lifecycle calls so far (`open`, `flush`, `drain`, `stop`), in order.
    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl AudioOutput for NullOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.calls.lock().unwrap().push("open");
        self.opens.fetch_add(1, Ordering::SeqCst);
        self.reopen.store(false, Ordering::SeqCst);
        *self.last_spec.lock().unwrap() = Some(spec.clone());
        *self.status.lock().unwrap() = OutputStatus {
            mode: spec.mode,
            bit_perfect: spec.mode == OutputMode::Exclusive,
            notice: None,
        };
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        Ok(())
    }
    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        match buf {
            PcmBuffer::F32(v) => {
                let mut out = self.samples.lock().unwrap();
                if (self.volume - 1.0).abs() < f32::EPSILON {
                    out.extend_from_slice(v);
                } else {
                    out.extend(v.iter().map(|s| s * self.volume));
                }
            }
            PcmBuffer::I32(v) => {
                let mut out = self.i32_samples.lock().unwrap();
                if (self.volume - 1.0).abs() < f32::EPSILON {
                    out.extend_from_slice(v);
                } else {
                    out.extend(v.iter().map(|&s| (s as f64 * self.volume as f64) as i32));
                }
            }
        }
        Ok(())
    }
    fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.volume = volume.clamp(0.0, 1.0);
        self.volumes.lock().unwrap().push(self.volume);
        // Like the real backends, which keep their volume in the control.
        if let Some(control) = self.control.lock().unwrap().as_ref() {
            control.set(self.volume);
        }
        Ok(())
    }
    fn attach_volume(&mut self, control: Arc<VolumeControl>) {
        *self.control.lock().unwrap() = Some(control);
    }
    fn flush(&mut self) -> Result<()> {
        self.calls.lock().unwrap().push("flush");
        self.flushes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn fade_out(&mut self) -> Result<()> {
        self.fade_outs.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.calls.lock().unwrap().push("stop");
        self.stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn drain(&mut self, _cancel: &dyn Fn() -> bool) -> Result<()> {
        self.calls.lock().unwrap().push("drain");
        Ok(())
    }
    fn status(&self) -> OutputStatus {
        self.status.lock().unwrap().clone()
    }
    fn queued_ms(&self) -> Option<u32> {
        Some(self.queued_ms.load(Ordering::SeqCst))
    }
    fn underruns(&self) -> Option<u64> {
        Some(self.underruns.load(Ordering::SeqCst))
    }
    fn needs_reopen(&self) -> bool {
        self.reopen.load(Ordering::SeqCst)
    }
}

#[cfg(windows)]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    platform::enumerate()
}

#[cfg(target_os = "linux")]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    crate::alsa_output::enumerate()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    Ok(Vec::new())
}

#[cfg(windows)]
pub(crate) mod platform {
    use super::*;
    use wasapi::{DeviceEnumerator, Direction};

    pub fn ensure_com() -> Result<()> {
        let hr = wasapi::initialize_mta();
        // `RPC_E_CHANGED_MODE` means COM is already initialized on this thread
        // with another apartment model (Tauri's main thread is STA). The device
        // enumerator works in either apartment, so this is not a failure.
        if hr.is_ok() || hr == windows::Win32::Foundation::RPC_E_CHANGED_MODE {
            Ok(())
        } else {
            Err(Error::Output(format!("failed to initialize COM (MTA): {hr:?}")))
        }
    }

    /// Active render endpoints (with their names, type and connection read
    /// from the property store), then the installed ASIO drivers.
    pub fn enumerate() -> Result<Vec<OutputDevice>> {
        use crate::devices::{dedupe_names, wasapi_identity, WasapiProps};
        use windows::Win32::Media::Audio::{
            eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
        };
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};

        ensure_com()?;
        let to_err = |e: windows::core::Error| Error::Output(e.to_string());
        let mut out = Vec::new();
        // SAFETY: COM is initialized on this thread; every returned string is
        // freed by `take_pwstr`.
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(to_err)?;
            let default_id = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .ok()
                .and_then(|device| device.GetId().ok())
                .map(|id| take_pwstr(id));
            let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).map_err(to_err)?;
            for index in 0..collection.GetCount().map_err(to_err)? {
                let Ok(device) = collection.Item(index) else { continue };
                let Ok(id) = device.GetId().map(|id| take_pwstr(id)) else { continue };
                let props = match device.OpenPropertyStore(STGM_READ) {
                    Ok(store) => WasapiProps {
                        endpoint: read_string(&store, &keys::DEVICE_DESC),
                        adapter: read_string(&store, &keys::INTERFACE_FRIENDLY_NAME),
                        friendly: read_string(&store, &keys::FRIENDLY_NAME),
                        enumerator: read_string(&store, &keys::ENUMERATOR_NAME),
                        form_factor: read_string(&store, &keys::FORM_FACTOR).trim().parse().ok(),
                    },
                    Err(_) => WasapiProps::default(),
                };
                let (name, kind, transport) = wasapi_identity(&props);
                let system_name = if props.friendly.is_empty() { name.clone() } else { props.friendly.clone() };
                let mut entry = OutputDevice::new(id.clone(), name, DeviceGroup::Wasapi).with_system_name(system_name);
                entry.kind = kind;
                entry.transport = transport;
                entry.is_default = default_id.as_deref() == Some(id.as_str());
                out.push(entry);
            }
        }
        out.extend(crate::asio_output::devices());
        dedupe_names(&mut out);
        Ok(out)
    }

    /// Endpoint property keys (`functiondiscoverykeys_devpkey.h`,
    /// `mmdeviceapi.h`), spelled out so no extra `windows` features are needed.
    mod keys {
        use windows::core::GUID;
        use windows::Win32::Foundation::PROPERTYKEY;

        const DEVICE: GUID = GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0);
        pub const DEVICE_DESC: PROPERTYKEY = PROPERTYKEY { fmtid: DEVICE, pid: 2 };
        pub const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: DEVICE, pid: 14 };
        pub const ENUMERATOR_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: DEVICE, pid: 24 };
        pub const INTERFACE_FRIENDLY_NAME: PROPERTYKEY =
            PROPERTYKEY { fmtid: GUID::from_u128(0x026e516e_b814_414b_83cd_856d6fef4822), pid: 2 };
        pub const FORM_FACTOR: PROPERTYKEY =
            PROPERTYKEY { fmtid: GUID::from_u128(0x1da5d803_d492_4edd_8c23_e0c0ffee7f0e), pid: 0 };
    }

    /// A property as text ("" when missing). Numbers come back as digits.
    unsafe fn read_string(
        store: &windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore,
        key: &windows::Win32::Foundation::PROPERTYKEY,
    ) -> String {
        use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
        let Ok(mut value) = store.GetValue(key) else {
            return String::new();
        };
        let text = PropVariantToStringAlloc(&value).map(|s| take_pwstr(s)).unwrap_or_default();
        let _ = PropVariantClear(&mut value);
        text
    }

    /// Copy and free a COM-allocated string.
    unsafe fn take_pwstr(text: windows::core::PWSTR) -> String {
        let out = text.to_string().unwrap_or_default();
        windows::Win32::System::Com::CoTaskMemFree(Some(text.0 as _));
        out
    }

    /// The exclusive formats (8 rates × 3 integer depths), the shared mix
    /// format and the hardware volume of one endpoint. About 20–80 ms.
    pub fn probe_caps(device_id: &str) -> Option<DeviceCaps> {
        use crate::devices::{PROBE_BITS, PROBE_RATES};
        use wasapi::{SampleType, WaveFormat};
        ensure_com().ok()?;
        let enumerator = DeviceEnumerator::new().ok()?;
        let device = enumerator.get_device(device_id).ok()?;
        let mut formats = Vec::new();
        for rate in PROBE_RATES {
            for bits in PROBE_BITS {
                let Ok(client) = device.get_iaudioclient() else { continue };
                let wave = WaveFormat::new(32, bits as usize, &SampleType::Int, rate as usize, 2, None);
                if let Ok(accepted) = client.is_supported_exclusive_with_quirks(&wave) {
                    if accepted.get_samplespersec() == rate {
                        formats.push((rate, accepted.get_validbitspersample().min(bits)));
                    }
                }
            }
        }
        let mut caps = DeviceCaps::from_formats(&formats);
        if let Ok(mix) = device.get_iaudioclient().and_then(|client| client.get_mixformat()) {
            caps.shared_rate = Some(mix.get_samplespersec());
            caps.shared_bits = Some(mix.get_validbitspersample());
        }
        caps.hw_volume = crate::endpoint_volume::EndpointVolume::open(Some(device_id))
            .ok()
            .map(|volume| volume.has_hardware_volume());
        Some(caps)
    }

    /// Register one device-change listener for the life of the process. The
    /// registration lives on its own thread (it must stay in one COM
    /// apartment); callbacks wake a debounce thread that calls `on_change` once
    /// per burst (a USB DAC arriving fires several notifications).
    pub fn watch(on_change: Box<dyn Fn() + Send + Sync>) {
        use std::sync::mpsc;
        use std::time::Duration;
        let (tx, rx) = mpsc::channel::<()>();
        let spawned = std::thread::Builder::new().name("oxitide-device-watch".into()).spawn(move || {
            if ensure_com().is_err() {
                return;
            }
            let Ok(enumerator) = DeviceEnumerator::new() else { return };
            let mut callbacks = wasapi::DeviceEventCallbacks::new();
            {
                let tx = tx.clone();
                callbacks.set_device_added_callback(move |_| {
                    let _ = tx.send(());
                });
            }
            {
                let tx = tx.clone();
                callbacks.set_device_removed_callback(move |_| {
                    let _ = tx.send(());
                });
            }
            {
                let tx = tx.clone();
                callbacks.set_device_state_callback(move |_, _| {
                    let _ = tx.send(());
                });
            }
            {
                let tx = tx.clone();
                callbacks.set_default_device_callback(move |direction, role, _| {
                    if direction == Direction::Render && role == wasapi::Role::Console {
                        let _ = tx.send(());
                    }
                });
            }
            let Ok(_registration) = enumerator.register_notification_callback(callbacks) else {
                tracing::warn!("device watch: registration failed; the picker refreshes when it opens");
                return;
            };
            drop(tx);
            loop {
                std::thread::park();
            }
        });
        if spawned.is_err() {
            return;
        }
        let _ = std::thread::Builder::new().name("oxitide-device-debounce".into()).spawn(move || {
            while rx.recv().is_ok() {
                while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
                on_change();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::PcmFormat;

    fn info(bits: u16, format: PcmFormat) -> StreamInfo {
        StreamInfo { sample_rate: 44100, channels: 2, bits_per_sample: bits, format }
    }

    fn spec(mode: OutputMode) -> AudioOutputSpec {
        AudioOutputSpec {
            device_id: None,
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode,
            exclusive_fallback: ExclusiveFallback::Shared,
        }
    }

    #[test]
    fn underrun_tally_counts_between_writes_and_drops_what_precedes_a_refill() {
        let mut tally = UnderrunTally::default();
        assert_eq!(
            tally.observe(Some(5)),
            0,
            "the first write only sets the baseline"
        );
        assert_eq!(tally.observe(Some(5)), 0);
        assert_eq!(tally.observe(Some(7)), 2);
        tally.reset();
        // The pause's dry-out lands before the first write after the refill.
        assert_eq!(tally.observe(Some(8)), 0);
        assert_eq!(tally.observe(Some(9)), 1);
        assert_eq!(
            tally.observe(None),
            0,
            "a backend that cannot tell counts nothing"
        );
    }

    #[test]
    fn lead_estimate_takes_its_first_sample_then_smooths() {
        let mut lead = LeadEstimate::default();
        assert_eq!(lead.ms(), 0);
        lead.observe(1024, Some(200));
        assert_eq!(lead.ms(), 200);
        lead.observe(1024, Some(150));
        assert_eq!(lead.ms(), 195, "one step moves a tenth of the way");
        for _ in 0..200 {
            lead.observe(1024, Some(175));
        }
        assert_eq!(lead.ms(), 175);
    }

    #[test]
    fn lead_estimate_skips_the_refill_after_a_reset_and_keeps_its_value() {
        let mut lead = LeadEstimate::default();
        lead.observe(1024, Some(210));
        lead.reset(44_100);
        // 0.5 s at 44.1 kHz = 22 050 frames of refill are not sampled.
        for _ in 0..21 {
            lead.observe(1024, Some(20));
        }
        assert_eq!(lead.ms(), 210, "refill samples ignored, prior value kept");
        lead.observe(1024, Some(20));
        assert_eq!(lead.ms(), 210, "the write that ends the settle is not sampled");
        lead.observe(1024, Some(230));
        assert_eq!(lead.ms(), 212);
    }

    #[test]
    fn lead_estimate_clears_when_the_output_cannot_report() {
        let mut lead = LeadEstimate::default();
        lead.observe(1024, Some(200));
        lead.observe(1024, None);
        assert_eq!(lead.ms(), 0);
    }

    #[test]
    fn null_output_reports_the_queued_time_it_was_given() {
        let out = NullOutput::new();
        assert_eq!(out.queued_ms(), Some(0));
        out.set_queued_ms(230);
        assert_eq!(out.clone().queued_ms(), Some(230));
    }

    #[test]
    fn null_output_records_lifecycle_and_volume() {
        let mut out = NullOutput::new();
        out.open(&spec(OutputMode::Shared)).unwrap();
        out.start().unwrap();
        out.set_volume(0.5).unwrap();
        out.write(&PcmBuffer::F32(vec![1.0, -1.0])).unwrap();
        out.flush().unwrap();
        out.stop().unwrap();

        assert_eq!(out.opens(), 1);
        assert_eq!(out.stops(), 1);
        assert_eq!(out.flushes(), 1);
        assert_eq!(out.last_spec(), Some(spec(OutputMode::Shared)));
        assert_eq!(out.volumes(), vec![0.5]);
        assert_eq!(out.samples(), vec![0.5, -0.5]);
        assert_eq!(out.status().mode, OutputMode::Shared);
        assert!(!out.status().bit_perfect);
    }

    #[test]
    fn volume_gain_is_an_exact_ended_loudness_taper() {
        assert_eq!(volume_gain(0.0), 0.0);
        assert_eq!(volume_gain(1.0), 1.0, "full volume must stay unity");
        assert_eq!(volume_gain(-0.5), 0.0);
        assert_eq!(volume_gain(2.0), 1.0);
        let db = |p: f32| 20.0 * volume_gain(p).log10();
        assert!((db(0.5) + 10.0).abs() < 0.05, "50% is {} dB", db(0.5));
        // Cubic was -31 dB here, which testers found inaudible.
        assert!(db(0.3) > -18.0, "30% is {} dB", db(0.3));
        assert!((db(0.1) + 33.2).abs() < 0.1, "10% is {} dB", db(0.1));
        // Linear gave 85% -> 90% just 0.5 dB.
        let top = db(0.9) - db(0.85);
        assert!(top > 0.8, "85% -> 90% is only {top} dB");
        let mut last = 0.0;
        for step in 1..=100 {
            let p = step as f32 / 100.0;
            let gain = volume_gain(p);
            assert!(gain > last, "the curve must rise at {step}%");
            last = gain;
            // Loudness (doubling per +10 dB) tracks the slider position.
            let loudness = 2f32.powf(db(p) / 10.0);
            assert!((loudness - p).abs() < 0.002, "{step}% sounds like {loudness}");
            assert!((volume_position(gain) - p).abs() < 1e-5, "inverse at {step}%");
        }
    }

    #[test]
    fn null_output_records_i32_with_software_volume() {
        let mut out = NullOutput::new();
        out.open(&spec(OutputMode::Exclusive)).unwrap();
        out.set_volume(0.5).unwrap();
        out.write(&PcmBuffer::I32(vec![1000, -1000])).unwrap();
        assert_eq!(out.i32_samples(), vec![500, -500]);
        assert_eq!(out.status().mode, OutputMode::Exclusive);
        assert!(out.status().bit_perfect);
    }

    #[test]
    fn exclusive_formats_chain_valid_bits() {
        let sixteen = exclusive_formats(&info(16, PcmFormat::I32 { bits: 16 }));
        assert_eq!(
            sixteen.iter().map(|f| f.valid_bits).collect::<Vec<_>>(),
            vec![16, 24, 32]
        );
        let twenty_four = exclusive_formats(&info(24, PcmFormat::I32 { bits: 24 }));
        assert_eq!(
            twenty_four.iter().map(|f| f.valid_bits).collect::<Vec<_>>(),
            vec![24, 32]
        );
        assert!(exclusive_formats(&info(16, PcmFormat::F32)).is_empty());
        assert!(sixteen.iter().all(|f| f.store_bits == 32 && f.sample_rate == 44100));
    }

    #[test]
    fn asio_ids_force_exclusive() {
        assert_eq!(asio_device_key("asio:Topping USB"), Some("Topping USB"));
        assert_eq!(asio_device_key("asio:"), None);
        assert_eq!(asio_device_key("{0.0.0.00000000}.{abc}"), None);
        assert_eq!(
            effective_mode(OutputMode::Shared, Some("asio:Realtek ASIO")),
            OutputMode::Exclusive
        );
        assert_eq!(effective_mode(OutputMode::Shared, Some("wasapi-id")), OutputMode::Shared);
        assert_eq!(effective_mode(OutputMode::Exclusive, None), OutputMode::Exclusive);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn enumeration_is_wired_on_linux() {
        // ALSA's card rows; none on a machine without cards (CI, WSL).
        let devices = enumerate_output_devices().unwrap();
        assert!(devices.iter().all(|d| d.id.starts_with("hw:CARD=")), "{devices:?}");
    }
}
