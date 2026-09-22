#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Producer, RingBuffer};
use wasapi::{Direction, SampleType, StreamMode, WaveFormat};

use crate::decode::PcmBuffer;
use crate::error::{Error, Result};
use crate::output::{platform, AudioOutput, AudioOutputSpec, OutputMode, OutputStatus};

/// Floor for the ring so tiny formats still leave room for the device period.
const MIN_RING_SAMPLES: usize = 4096;

pub struct WasapiOutput {
    device_id: Option<String>,
    producer: Option<Producer<u32>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    volume: f32,
    status: Arc<Mutex<OutputStatus>>,
}

impl WasapiOutput {
    pub fn new() -> Self {
        Self {
            device_id: None,
            producer: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            volume: 1.0,
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
        let sample_rate = spec.info.sample_rate;
        let channels = spec.info.channels;

        // ~200ms of samples (u32 words: one 4-byte word per sample). A small
        // ring keeps pause/volume changes responsive.
        let frames = (sample_rate as usize * channels as usize) / 5;
        let (producer, mut consumer) = RingBuffer::<u32>::new(frames.max(MIN_RING_SAMPLES));

        let device_id = self.device_id.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::Builder::new()
            .name("oxitide-wasapi".into())
            .spawn(move || {
                if let Err(e) = run_device(device_id, sample_rate, channels, &mut consumer, &stop_thread)
                {
                    tracing::error!("wasapi output stopped: {e}");
                }
            })
            .map_err(|e| Error::Output(e.to_string()))?;

        self.producer = Some(producer);
        self.worker = Some(handle);
        self.stop = stop;
        // Task 4 opens exclusive; until then every spec opens shared.
        *self.status.lock().unwrap() =
            OutputStatus { mode: OutputMode::Shared, bit_perfect: false, notice: None };
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        let volume = self.volume;
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| Error::Output("output not open".into()))?;
        match buf {
            PcmBuffer::F32(v) => {
                for &sample in v {
                    let scaled =
                        if (volume - 1.0).abs() < f32::EPSILON { sample } else { sample * volume };
                    push_word(producer, scaled.to_bits())?;
                }
            }
            PcmBuffer::I32(v) => {
                for &sample in v {
                    let scaled = if (volume - 1.0).abs() < f32::EPSILON {
                        sample
                    } else {
                        (sample as f64 * volume as f64) as i32
                    };
                    push_word(producer, scaled as u32)?;
                }
            }
        }
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.volume = volume.clamp(0.0, 1.0);
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
        self.status.lock().unwrap().clone()
    }
}

impl Drop for WasapiOutput {
    fn drop(&mut self) {
        self.shutdown_worker();
    }
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

fn run_device(
    device_id: Option<String>,
    sample_rate: u32,
    channels: u16,
    consumer: &mut rtrb::Consumer<u32>,
    stop: &AtomicBool,
) -> Result<()> {
    platform::ensure_com()?;
    let enumerator = wasapi::DeviceEnumerator::new().map_err(|e| Error::Output(e.to_string()))?;
    let device = match device_id {
        Some(id) => enumerator
            .get_device(&id)
            .map_err(|e| Error::Output(e.to_string()))?,
        None => enumerator
            .get_default_device(&Direction::Render)
            .map_err(|e| Error::Output(e.to_string()))?,
    };
    let mut client = device
        .get_iaudioclient()
        .map_err(|e| Error::Output(e.to_string()))?;
    let desired = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        sample_rate as usize,
        channels as usize,
        None,
    );
    let (def_time, _min_time) = client
        .get_device_period()
        .map_err(|e| Error::Output(e.to_string()))?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: def_time,
    };
    client
        .initialize_client(&desired, &Direction::Render, &mode)
        .map_err(|e| Error::Output(e.to_string()))?;
    let event = client
        .set_get_eventhandle()
        .map_err(|e| Error::Output(e.to_string()))?;
    let render = client
        .get_audiorenderclient()
        .map_err(|e| Error::Output(e.to_string()))?;

    // Prime the buffer before starting.
    write_frames(&client, &render, consumer, channels as usize)?;
    client
        .start_stream()
        .map_err(|e| Error::Output(e.to_string()))?;

    while !stop.load(Ordering::SeqCst) {
        if event.wait_for_event(500).is_err() {
            continue; // timeout: re-check the stop flag
        }
        write_frames(&client, &render, consumer, channels as usize)?;
    }
    let _ = client.stop_stream();
    Ok(())
}

fn write_frames(
    client: &wasapi::AudioClient,
    render: &wasapi::AudioRenderClient,
    consumer: &mut rtrb::Consumer<u32>,
    channels: usize,
) -> Result<()> {
    let frames = client
        .get_available_space_in_frames()
        .map_err(|e| Error::Output(e.to_string()))? as usize;
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
        .map_err(|e| Error::Output(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{PcmFormat, StreamInfo};
    use crate::output::ExclusiveFallback;

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
