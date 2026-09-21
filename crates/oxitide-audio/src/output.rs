use std::sync::{Arc, Mutex};

use crate::decode::AudioFormat;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub trait AudioOutput: Send {
    fn open(&mut self, device_id: Option<&str>, format: AudioFormat) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn write(&mut self, samples: &[f32]) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

/// Captures everything written, for tests and headless use.
#[derive(Clone, Default)]
pub struct NullOutput {
    samples: Arc<Mutex<Vec<f32>>>,
    format: Option<AudioFormat>,
}

impl NullOutput {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn samples(&self) -> Vec<f32> {
        self.samples.lock().unwrap().clone()
    }
}

impl AudioOutput for NullOutput {
    fn open(&mut self, _device_id: Option<&str>, format: AudioFormat) -> Result<()> {
        self.format = Some(format);
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        Ok(())
    }
    fn write(&mut self, samples: &[f32]) -> Result<()> {
        self.samples.lock().unwrap().extend_from_slice(samples);
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    platform::enumerate()
}

#[cfg(not(windows))]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    Ok(Vec::new())
}

#[cfg(windows)]
pub(crate) mod platform {
    use super::*;
    use wasapi::{DeviceEnumerator, Direction};

    pub fn ensure_com() -> Result<()> {
        if wasapi::initialize_mta().is_ok() {
            Ok(())
        } else {
            Err(Error::Output("failed to initialize COM (MTA)".into()))
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

    #[test]
    fn null_output_collects_samples() {
        let mut out = NullOutput::new();
        out.open(None, AudioFormat { sample_rate: 44100, channels: 2 }).unwrap();
        out.start().unwrap();
        out.write(&[0.0, 0.5, -0.5]).unwrap();
        out.flush().unwrap();
        out.stop().unwrap();
        assert_eq!(out.samples(), vec![0.0, 0.5, -0.5]);
    }
}
