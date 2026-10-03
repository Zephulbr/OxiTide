use std::sync::atomic::{AtomicUsize, Ordering};
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
}

impl AudioOutput for NullOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
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
        self.flushes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn status(&self) -> OutputStatus {
        self.status.lock().unwrap().clone()
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

    #[cfg(target_os = "linux")]
    #[test]
    fn enumeration_is_wired_on_linux() {
        let devices = enumerate_output_devices().unwrap();
        assert!(!devices.is_empty());
    }
}
