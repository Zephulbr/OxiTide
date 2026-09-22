#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Producer, RingBuffer};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

use crate::decode::PcmBuffer;
use crate::endpoint_volume;
use crate::error::{Error, Result};
use crate::output::{
    exclusive_formats, platform, AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode,
    OutputStatus,
};

/// Floor for the ring so tiny formats still leave room for the device period.
const MIN_RING_SAMPLES: usize = 4096;

pub struct WasapiOutput {
    device_id: Option<String>,
    resolved_device_id: Option<String>,
    producer: Option<Producer<u32>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    volume: f32,
    /// True when samples are scaled in software (shared mode, or an exclusive
    /// device whose endpoint volume is unavailable).
    software_volume: bool,
    device_lost: Arc<AtomicBool>,
    status: Arc<Mutex<OutputStatus>>,
}

impl WasapiOutput {
    pub fn new() -> Self {
        Self {
            device_id: None,
            resolved_device_id: None,
            producer: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            volume: 1.0,
            software_volume: true,
            device_lost: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(OutputStatus::default())),
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.producer = None;
    }
}

impl Default for WasapiOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for WasapiOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.shutdown_worker();
        platform::ensure_com()?;
        self.device_id = spec.device_id.clone();
        self.resolved_device_id = None;
        self.device_lost.store(false, Ordering::SeqCst);
        self.software_volume = true;

        let sample_rate = spec.info.sample_rate;
        let channels = spec.info.channels;
        let frames = (sample_rate as usize * channels as usize) / 5;
        let (producer, mut consumer) = RingBuffer::<u32>::new(frames.max(MIN_RING_SAMPLES));

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let spec_thread = spec.clone();
        let lost = self.device_lost.clone();

        let handle = std::thread::Builder::new()
            .name("oxitide-wasapi".into())
            .spawn(move || match setup_device(&spec_thread, &lost) {
                Ok((mut session, ready)) => {
                    let _ = ready_tx.send(Ok(ready));
                    if let Err(e) = session.pump(&mut consumer, &stop_thread) {
                        tracing::error!("wasapi output stopped: {e}");
                    }
                }
                Err(e) => {
                    // Setup failed before any audio was written.
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| Error::Output(e.to_string()))?;

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(ready)) => {
                self.producer = Some(producer);
                self.worker = Some(handle);
                self.stop = stop;
                self.resolved_device_id = ready.device_id;
                *self.status.lock().unwrap() = ready.status;
            }
            Ok(Err(message)) => {
                stop.store(true, Ordering::SeqCst);
                let _ = handle.join();
                return Err(Error::Output(message));
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                let _ = handle.join();
                return Err(Error::Output("audio device did not initialize in time".into()));
            }
        }

        let volume = self.volume;
        self.set_volume(volume)
    }

    fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        if self.device_lost.load(Ordering::SeqCst) {
            return Err(Error::Output("output device unavailable".into()));
        }
        let volume = self.volume;
        let software = self.software_volume && (volume - 1.0).abs() >= f32::EPSILON;
        // The requested mode can differ from the mode that actually opened
        // (exclusive -> shared fallback), so convert to what the device accepts.
        let actual_mode = self.status.lock().unwrap().mode;
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| Error::Output("output not open".into()))?;
        match buf {
            PcmBuffer::F32(v) => {
                for &sample in v {
                    let scaled = if software { sample * volume } else { sample };
                    push_word(producer, scaled.to_bits())?;
                }
            }
            PcmBuffer::I32(v) if actual_mode == OutputMode::Exclusive => {
                for &sample in v {
                    let scaled = if software {
                        (sample as f64 * volume as f64) as i32
                    } else {
                        sample
                    };
                    push_word(producer, scaled as u32)?;
                }
            }
            PcmBuffer::I32(v) => {
                // Shared output fell back from exclusive: the device wants
                // float samples. Left-aligned i32 -> f32 is a pure divide.
                for &sample in v {
                    let float = sample as f32 / 2_147_483_648.0;
                    let scaled = if software { float * volume } else { float };
                    push_word(producer, scaled.to_bits())?;
                }
            }
        }
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        self.volume = volume;
        let exclusive = self.status.lock().unwrap().mode == OutputMode::Exclusive;
        if exclusive {
            let target = self.resolved_device_id.as_deref().or(self.device_id.as_deref());
            if endpoint_volume::set_master_volume(target, volume).is_ok() {
                self.software_volume = false;
                return Ok(());
            }
            self.software_volume = true;
            let mut status = self.status.lock().unwrap();
            status.bit_perfect = false;
            status.notice = Some("volume applied in software; bit-perfect compromised".into());
        } else {
            self.software_volume = true;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.shutdown_worker();
        Ok(())
    }

    fn status(&self) -> OutputStatus {
        let mut status = self.status.lock().unwrap().clone();
        if self.device_lost.load(Ordering::SeqCst) {
            status.bit_perfect = false;
            status.notice = Some("output device disconnected".into());
        }
        status
    }
}

impl Drop for WasapiOutput {
    fn drop(&mut self) {
        self.shutdown_worker();
    }
}

struct DeviceReady {
    device_id: Option<String>,
    status: OutputStatus,
}

struct DeviceSession {
    client: wasapi::AudioClient,
    render: wasapi::AudioRenderClient,
    event: Option<wasapi::Handle>,
    channels: usize,
    // Kept alive so device-change notifications keep firing.
    _registration: Option<wasapi::DeviceEventRegistration>,
}

impl DeviceSession {
    fn pump(&mut self, consumer: &mut rtrb::Consumer<u32>, stop: &AtomicBool) -> Result<()> {
        // Prime the buffer before starting.
        write_frames(&self.client, &self.render, consumer, self.channels)?;
        self.client.start_stream().map_err(to_error)?;
        while !stop.load(Ordering::SeqCst) {
            match &self.event {
                Some(event) => {
                    if event.wait_for_event(200).is_err() {
                        continue; // timeout: re-check the stop flag
                    }
                }
                // Polling exclusive: no event handle, just poll.
                None => std::thread::sleep(Duration::from_millis(5)),
            }
            write_frames(&self.client, &self.render, consumer, self.channels)?;
        }
        let _ = self.client.stop_stream();
        Ok(())
    }
}

/// Resolve the requested device, falling back to the system default when the
/// id no longer exists. The bool is true when a fallback happened.
fn resolve_device(
    enumerator: &DeviceEnumerator,
    wanted: Option<&str>,
) -> Result<(wasapi::Device, bool)> {
    if let Some(id) = wanted {
        if let Ok(device) = enumerator.get_device(id) {
            return Ok((device, false));
        }
    }
    let device = enumerator.get_default_device(&Direction::Render).map_err(to_error)?;
    Ok((device, wanted.is_some()))
}

/// Try every negotiated exclusive format in order.
fn try_exclusive(
    device: &wasapi::Device,
    spec: &AudioOutputSpec,
    def_time: i64,
) -> Result<wasapi::AudioClient> {
    for format in exclusive_formats(&spec.info) {
        let wave = WaveFormat::new(
            32,
            format.valid_bits as usize,
            &SampleType::Int,
            format.sample_rate as usize,
            format.channels as usize,
            None,
        );
        let Ok(mut client) = device.get_iaudioclient() else {
            continue;
        };
        let Ok(accepted) = client.is_supported_exclusive_with_quirks(&wave) else {
            continue;
        };
        // Polling exclusive supports more devices and avoids the documented
        // USB-driver stutter of event-driven exclusive.
        let mode = StreamMode::PollingExclusive {
            buffer_duration_hns: def_time * 4,
            period_hns: def_time,
        };
        if client.initialize_client(&accepted, &Direction::Render, &mode).is_ok() {
            return Ok(client);
        }
    }
    Err(Error::Output("no supported exclusive format".into()))
}

fn open_shared(
    device: &wasapi::Device,
    spec: &AudioOutputSpec,
    def_time: i64,
) -> Result<(wasapi::AudioClient, Option<wasapi::Handle>)> {
    let mut client = device.get_iaudioclient().map_err(to_error)?;
    let wave = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        spec.info.sample_rate as usize,
        spec.info.channels as usize,
        None,
    );
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: def_time };
    client.initialize_client(&wave, &Direction::Render, &mode).map_err(to_error)?;
    let event = client.set_get_eventhandle().map_err(to_error)?;
    Ok((client, Some(event)))
}

fn setup_device(
    spec: &AudioOutputSpec,
    lost: &Arc<AtomicBool>,
) -> Result<(DeviceSession, DeviceReady)> {
    let enumerator = DeviceEnumerator::new().map_err(to_error)?;
    let (device, fell_back) = resolve_device(&enumerator, spec.device_id.as_deref())?;
    let resolved_id = device.get_id().ok();

    let (def_time, _min_time) = device
        .get_iaudioclient()
        .map_err(to_error)?
        .get_device_period()
        .map_err(to_error)?;

    let mut status = OutputStatus { mode: OutputMode::Shared, bit_perfect: false, notice: None };
    let (client, event) = match spec.mode {
        OutputMode::Exclusive => match try_exclusive(&device, spec, def_time) {
            Ok(client) => {
                status.mode = OutputMode::Exclusive;
                status.bit_perfect = true;
                (client, None)
            }
            Err(_) => match spec.exclusive_fallback {
                ExclusiveFallback::Shared => {
                    status.notice = Some("exclusive mode unavailable; using shared output".into());
                    open_shared(&device, spec, def_time)?
                }
                ExclusiveFallback::WarnOnly => {
                    return Err(Error::Output(
                        "exclusive mode is not supported by this output device".into(),
                    ));
                }
            },
        },
        OutputMode::Shared => open_shared(&device, spec, def_time)?,
    };
    if fell_back {
        status.notice = Some("selected output device unavailable; using system default".into());
    }

    let render = client.get_audiorenderclient().map_err(to_error)?;

    // Keep the device-change registration alive for the session.
    let mut registration = None;
    if let Ok(notify_enum) = DeviceEnumerator::new() {
        let mut callbacks = wasapi::DeviceEventCallbacks::new();
        {
            let lost = lost.clone();
            let our_id = resolved_id.clone();
            callbacks.set_device_removed_callback(move |id| {
                if our_id.as_deref() == Some(id.as_str()) {
                    lost.store(true, Ordering::SeqCst);
                }
            });
        }
        {
            let lost = lost.clone();
            let our_id = resolved_id.clone();
            let opened_default = spec.device_id.is_none();
            callbacks.set_default_device_callback(move |_direction, _role, id| {
                if opened_default && id.as_deref() != our_id.as_deref() {
                    lost.store(true, Ordering::SeqCst);
                }
            });
        }
        registration = notify_enum.register_notification_callback(callbacks).ok();
    }

    let session = DeviceSession {
        client,
        render,
        event,
        channels: spec.info.channels as usize,
        _registration: registration,
    };
    Ok((session, DeviceReady { device_id: resolved_id, status }))
}

/// Push one sample word, waiting briefly if the ring is full but giving up if
/// the device stalled so playback cannot hang forever.
fn push_word(producer: &mut Producer<u32>, word: u32) -> Result<()> {
    let mut pending = word;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match producer.push(pending) {
            Ok(()) => return Ok(()),
            Err(rtrb::PushError::Full(sample)) => {
                if std::time::Instant::now() > deadline {
                    return Err(Error::Output("audio device stalled".into()));
                }
                pending = sample;
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

fn write_frames(
    client: &wasapi::AudioClient,
    render: &wasapi::AudioRenderClient,
    consumer: &mut rtrb::Consumer<u32>,
    channels: usize,
) -> Result<()> {
    let frames = client
        .get_available_space_in_frames()
        .map_err(to_error)? as usize;
    if frames == 0 {
        return Ok(());
    }
    let mut buf = vec![0u8; frames * channels * 4];
    for chunk in buf.chunks_mut(4) {
        let word = consumer.pop().unwrap_or(0); // silence on underrun (0.0f32 / i32 0)
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    render
        .write_to_device(frames, &buf, None)
        .map_err(to_error)
}

fn to_error(e: wasapi::WasapiError) -> Error {
    Error::Output(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{PcmFormat, StreamInfo};

    #[test]
    #[ignore = "requires an audio device"]
    fn opens_default_device_and_plays_a_second_of_silence() {
        let mut out = WasapiOutput::new();
        out.open(&AudioOutputSpec {
            device_id: None,
            info: StreamInfo {
                sample_rate: 44100,
                channels: 2,
                bits_per_sample: 16,
                format: PcmFormat::I32 { bits: 16 },
            },
            mode: OutputMode::Shared,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.start().unwrap();
        out.write(&PcmBuffer::F32(vec![0.0f32; 44100 * 2])).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        out.flush().unwrap();
        out.stop().unwrap();
    }
}
