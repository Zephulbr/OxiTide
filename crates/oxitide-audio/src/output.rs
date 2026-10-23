use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::decode::{PcmBuffer, PcmFormat, StreamInfo};
use crate::error::Result;
#[cfg(windows)]
use crate::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
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
    fn set_volume(&mut self, volume: f32) -> Result<()>;
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
    last_spec: Arc<Mutex<Option<AudioOutputSpec>>>,
    volumes: Arc<Mutex<Vec<f32>>>,
    status: Arc<Mutex<OutputStatus>>,
    queued_ms: Arc<AtomicU32>,
    underruns: Arc<AtomicU64>,
    /// `open` / `flush` / `drain` / `stop`, in call order.
    calls: Arc<Mutex<Vec<&'static str>>>,
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
            last_spec: Arc::new(Mutex::new(None)),
            volumes: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(Mutex::new(OutputStatus::default())),
            queued_ms: Arc::new(AtomicU32::new(0)),
            underruns: Arc::new(AtomicU64::new(0)),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl NullOutput {
    pub fn new() -> Self {
        Self::default()
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
    /// The lifecycle calls so far (`open`, `flush`, `drain`, `stop`), in order.
    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl AudioOutput for NullOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.calls.lock().unwrap().push("open");
        self.opens.fetch_add(1, Ordering::SeqCst);
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
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        self.calls.lock().unwrap().push("flush");
        self.flushes.fetch_add(1, Ordering::SeqCst);
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

    pub fn enumerate() -> Result<Vec<OutputDevice>> {
        ensure_com()?;
        let enumerator = DeviceEnumerator::new().map_err(|e| Error::Output(e.to_string()))?;
        let default_name = enumerator
            .get_default_device_for_role(&Direction::Render, &wasapi::Role::Console)
            .ok()
            .and_then(|d| d.get_friendlyname().ok());
        let mut out = Vec::new();
        let collection = enumerator
            .get_device_collection(&Direction::Render)
            .map_err(|e| Error::Output(e.to_string()))?;
        let count = collection
            .get_nbr_devices()
            .map_err(|e| Error::Output(e.to_string()))?;
        for index in 0..count {
            let device = collection
                .get_device_at_index(index)
                .map_err(|e| Error::Output(e.to_string()))?;
            let name = device.get_friendlyname().unwrap_or_default();
            let id = device.get_id().unwrap_or_default();
            let is_default = default_name.as_deref() == Some(name.as_str());
            out.push(OutputDevice { id, name, is_default });
        }
        out.extend(crate::asio_output::devices());
        Ok(out)
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
        let devices = enumerate_output_devices().unwrap();
        assert!(!devices.is_empty());
    }
}
