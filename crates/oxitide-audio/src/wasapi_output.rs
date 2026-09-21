#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Producer, RingBuffer};
use wasapi::{Direction, SampleType, StreamMode, WaveFormat};

use crate::decode::AudioFormat;
use crate::error::{Error, Result};
use crate::output::{platform, AudioOutput};

const RING_FRAMES: usize = 44100 * 2; // ~0.5s stereo at 44.1k; resized on open

pub struct WasapiOutput {
    device_id: Option<String>,
    format: Option<AudioFormat>,
    producer: Option<Producer<f32>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    started: bool,
}

impl WasapiOutput {
    pub fn new() -> Self {
        Self {
            device_id: None,
            format: None,
            producer: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            started: false,
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.producer = None;
        self.started = false;
    }
}

impl Default for WasapiOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for WasapiOutput {
    fn open(&mut self, device_id: Option<&str>, format: AudioFormat) -> Result<()> {
        self.shutdown_worker();
        platform::ensure_com()?;
        self.device_id = device_id.map(|s| s.to_string());
        self.format = Some(format);

        let frames = (format.sample_rate as usize * format.channels as usize).max(1024) * 2;
        let (producer, mut consumer) = RingBuffer::<f32>::new(frames.max(RING_FRAMES));

        let device_id = self.device_id.clone();
        let channels = format.channels;
        let sample_rate = format.sample_rate;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::Builder::new()
            .name("oxitide-wasapi".into())
            .spawn(move || {
                if let Err(e) =
                    run_device(device_id, sample_rate, channels, &mut consumer, &stop_thread)
                {
                    tracing::error!("wasapi output stopped: {e}");
                }
            })
            .map_err(|e| Error::Output(e.to_string()))?;

        self.producer = Some(producer);
        self.worker = Some(handle);
        self.stop = stop;
        self.started = false;
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        self.started = true;
        Ok(())
    }

    fn write(&mut self, samples: &[f32]) -> Result<()> {
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| Error::Output("output not open".into()))?;
        for &sample in samples {
            // If the ring is full, wait briefly rather than dropping audio — but
            // give up if the device has stalled, so playback cannot hang forever.
            let mut pending = sample;
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                match producer.push(pending) {
                    Ok(()) => break,
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
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.shutdown_worker();
        Ok(())
    }
}

impl Drop for WasapiOutput {
    fn drop(&mut self) {
        self.shutdown_worker();
    }
}

fn run_device(
    device_id: Option<String>,
    sample_rate: u32,
    channels: u16,
    consumer: &mut rtrb::Consumer<f32>,
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
    consumer: &mut rtrb::Consumer<f32>,
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
        let sample = consumer.pop().unwrap_or(0.0);
        chunk.copy_from_slice(&sample.to_le_bytes());
    }
    render
        .write_to_device(frames, &buf, None)
        .map_err(|e| Error::Output(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an audio device"]
    fn opens_default_device_and_plays_a_second_of_silence() {
        let mut out = WasapiOutput::new();
        out.open(
            None,
            AudioFormat {
                sample_rate: 44100,
                channels: 2,
            },
        )
        .unwrap();
        out.start().unwrap();
        let silence = vec![0.0f32; 44100 * 2];
        out.write(&silence).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        out.flush().unwrap();
        out.stop().unwrap();
    }
}
