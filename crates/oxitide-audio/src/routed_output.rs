//! Dispatches between the system backend (WASAPI) and ASIO by device id
//! (spec `2026-10-11-oxitide-track7-asio-output-design.md`, T7A-D3/D5).

use crate::decode::PcmBuffer;
use crate::error::{Error, Result};
use crate::output::{
    is_asio_device, AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode, OutputStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    System,
    Asio,
}

pub struct RoutedOutput {
    system: Box<dyn AudioOutput>,
    asio: Box<dyn AudioOutput>,
    active: Option<Backend>,
    /// Why an ASIO open fell back to shared system output.
    fallback_notice: Option<String>,
}

impl RoutedOutput {
    pub fn new(system: Box<dyn AudioOutput>, asio: Box<dyn AudioOutput>) -> Self {
        Self { system, asio, active: None, fallback_notice: None }
    }

    /// WASAPI plus ASIO.
    #[cfg(windows)]
    pub fn windows() -> Self {
        Self::new(
            Box::new(crate::wasapi_output::WasapiOutput::new()),
            Box::new(crate::asio_output::AsioOutput::new()),
        )
    }

    fn active_mut(&mut self) -> Result<&mut dyn AudioOutput> {
        match self.active {
            Some(Backend::System) => Ok(self.system.as_mut()),
            Some(Backend::Asio) => Ok(self.asio.as_mut()),
            None => Err(Error::Output("output not open".into())),
        }
    }

    fn release(&mut self, keep: Backend) {
        match (self.active, keep) {
            (Some(Backend::System), Backend::Asio) => {
                let _ = self.system.stop();
            }
            (Some(Backend::Asio), Backend::System) => {
                let _ = self.asio.stop();
            }
            _ => {}
        }
    }
}

impl AudioOutput for RoutedOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.fallback_notice = None;
        let wants_asio = spec.device_id.as_deref().is_some_and(is_asio_device);
        if !wants_asio {
            self.release(Backend::System);
            self.active = None;
            self.system.open(spec)?;
            self.active = Some(Backend::System);
            return Ok(());
        }
        self.release(Backend::Asio);
        self.active = None;
        let reason = match self.asio.open(spec) {
            Ok(()) => {
                self.active = Some(Backend::Asio);
                return Ok(());
            }
            Err(e) => e.to_string(),
        };
        match spec.exclusive_fallback {
            ExclusiveFallback::WarnOnly => Err(Error::Output(reason)),
            ExclusiveFallback::Shared => {
                tracing::warn!("asio open failed, falling back to shared output: {reason}");
                let shared = AudioOutputSpec {
                    device_id: None,
                    mode: OutputMode::Shared,
                    ..spec.clone()
                };
                self.system.open(&shared)?;
                self.active = Some(Backend::System);
                self.fallback_notice = Some(format!("ASIO unavailable ({reason}); using shared output"));
                Ok(())
            }
        }
    }

    fn start(&mut self) -> Result<()> {
        self.active_mut()?.start()
    }

    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        self.active_mut()?.write(buf)
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        // Both remember the volume so the next open starts at it.
        let other = match self.active {
            Some(Backend::Asio) => self.system.set_volume(volume),
            _ => self.asio.set_volume(volume),
        };
        match self.active_mut() {
            Ok(active) => active.set_volume(volume),
            Err(_) => other,
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self.active_mut() {
            Ok(active) => active.flush(),
            Err(_) => Ok(()),
        }
    }

    fn stop(&mut self) -> Result<()> {
        let result = match self.active_mut() {
            Ok(active) => active.stop(),
            Err(_) => Ok(()),
        };
        self.active = None;
        result
    }

    fn status(&self) -> OutputStatus {
        let mut status = match self.active {
            Some(Backend::Asio) => self.asio.status(),
            _ => self.system.status(),
        };
        if status.notice.is_none() {
            status.notice = self.fallback_notice.clone();
        }
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{PcmFormat, StreamInfo};
    use crate::output::NullOutput;

    struct Failing;
    impl AudioOutput for Failing {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            Err(Error::Output("ASIO driver does not support 96 kHz".into()))
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

    fn spec(device: &str, fallback: ExclusiveFallback) -> AudioOutputSpec {
        AudioOutputSpec {
            device_id: Some(device.into()),
            info: StreamInfo {
                sample_rate: 96000,
                channels: 2,
                bits_per_sample: 24,
                format: PcmFormat::I32 { bits: 24 },
            },
            mode: OutputMode::Exclusive,
            exclusive_fallback: fallback,
        }
    }

    #[test]
    fn routes_by_device_id_and_stops_the_other_backend() {
        let (system, asio) = (NullOutput::new(), NullOutput::new());
        let mut out = RoutedOutput::new(Box::new(system.clone()), Box::new(asio.clone()));
        out.open(&spec("asio:Topping", ExclusiveFallback::Shared)).unwrap();
        out.write(&PcmBuffer::I32(vec![7, 8])).unwrap();
        assert_eq!((system.opens(), asio.opens()), (0, 1));
        assert_eq!(asio.i32_samples(), vec![7, 8]);

        out.open(&spec("wasapi-endpoint", ExclusiveFallback::Shared)).unwrap();
        assert_eq!(asio.stops(), 1, "switching backends releases ASIO");
        assert_eq!(system.opens(), 1);
        assert_eq!(system.last_spec().unwrap().device_id.as_deref(), Some("wasapi-endpoint"));
    }

    #[test]
    fn asio_failure_falls_back_to_shared_default_with_a_notice() {
        let system = NullOutput::new();
        let mut out = RoutedOutput::new(Box::new(system.clone()), Box::new(Failing));
        out.open(&spec("asio:Topping", ExclusiveFallback::Shared)).unwrap();
        let opened = system.last_spec().unwrap();
        assert_eq!(opened.device_id, None);
        assert_eq!(opened.mode, OutputMode::Shared);
        let notice = out.status().notice.unwrap();
        assert!(notice.starts_with("ASIO unavailable (ASIO driver does not support 96 kHz)"), "{notice}");
    }

    #[test]
    fn asio_failure_with_warn_only_is_an_error() {
        let system = NullOutput::new();
        let mut out = RoutedOutput::new(Box::new(system.clone()), Box::new(Failing));
        let err = out.open(&spec("asio:Topping", ExclusiveFallback::WarnOnly)).unwrap_err();
        assert!(err.to_string().contains("96 kHz"));
        assert_eq!(system.opens(), 0);
        assert!(out.write(&PcmBuffer::F32(vec![0.0])).is_err());
    }
}
