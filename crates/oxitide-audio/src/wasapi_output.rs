#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Producer, RingBuffer};
use wasapi::{DeviceEnumerator, DeviceState, Direction, SampleType, StreamMode, WaveFormat};

use crate::decode::PcmBuffer;
use crate::endpoint_volume::EndpointVolume;
use crate::error::{Error, Result};
use crate::gain::{scale_le_words, GainRamp, WordKind};
use crate::output::{
    exclusive_formats, platform, volume_gain, AudioOutput, AudioOutputSpec, ExclusiveFallback,
    OutputMode, OutputStatus, VolumeControl, SOFTWARE_VOLUME_NOTICE,
};
use crate::ring::{self, pop_frames_le, push_samples, Starvation};

/// Floor for the ring so tiny formats still leave room for the device period.
const MIN_RING_SAMPLES: usize = 4096;

/// How long the DAC's own (endpoint) volume takes to reach a new level. It
/// moves in one step per device period, as many DACs click on a single jump.
const ENDPOINT_GLIDE: Duration = Duration::from_millis(40);

/// Where the endpoint volume should be on its glide to the latest target.
/// Pure (the caller sets the device), so the stepping is testable.
#[derive(Debug, Clone, Copy)]
struct EndpointGlide {
    applied: f32,
    from: f32,
    target: f32,
    started: Instant,
}

impl EndpointGlide {
    /// Starts settled at `position` (already set on the device).
    fn new(position: f32, now: Instant) -> Self {
        Self {
            applied: position,
            from: position,
            target: position,
            started: now,
        }
    }

    /// The value to set now for `target`, or `None` when the device already
    /// has it. A new target glides on from wherever the volume is.
    fn next(&mut self, target: f32, now: Instant) -> Option<f32> {
        if (target - self.target).abs() >= f32::EPSILON {
            self.from = self.applied;
            self.target = target;
            self.started = now;
        }
        if (self.applied - self.target).abs() < f32::EPSILON {
            return None;
        }
        let t = now.saturating_duration_since(self.started).as_secs_f32()
            / ENDPOINT_GLIDE.as_secs_f32();
        let value = if t >= 1.0 {
            self.target
        } else {
            self.from + (self.target - self.from) * t
        };
        self.applied = value;
        Some(value)
    }
}

/// The DAC's own volume, stepped by the device thread.
struct EndpointStepper {
    volume: EndpointVolume,
    glide: EndpointGlide,
}

/// Handshake with the pump thread for a mid-session flush (seek): the caller
/// bumps `requested` and waits until `completed` catches up. Keeps the device
/// open, so exclusive-mode DACs do not re-lock (audible click/hiss).
#[derive(Default)]
struct FlushSignal {
    requested: AtomicU64,
    completed: Mutex<u64>,
    cond: Condvar,
}

impl FlushSignal {
    fn request(&self) -> u64 {
        self.requested.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn pending(&self) -> bool {
        *self.completed.lock().unwrap() < self.requested.load(Ordering::SeqCst)
    }

    fn complete(&self) {
        let mut completed = self.completed.lock().unwrap();
        *completed = self.requested.load(Ordering::SeqCst);
        self.cond.notify_all();
    }

    /// Wait until `seq` has been completed. False on timeout.
    fn wait(&self, seq: u64) -> bool {
        let completed = self.completed.lock().unwrap();
        let (completed, timeout) = self
            .cond
            .wait_timeout_while(completed, Duration::from_secs(1), |done| *done < seq)
            .unwrap();
        let timed_out = timeout.timed_out();
        drop(completed);
        !timed_out
    }
}

pub struct WasapiOutput {
    device_id: Option<String>,
    resolved_device_id: Option<String>,
    producer: Option<Producer<u32>>,
    /// Rate × channels of the open stream; paces waits on a full ring.
    samples_per_sec: usize,
    /// The device buffer negotiated at open, in ms. The pump keeps it full,
    /// so it adds to the ring in `queued_ms`.
    device_ms: u32,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    flush: Option<Arc<FlushSignal>>,
    /// The volume the device thread applies (the engine's, once attached).
    control: Arc<VolumeControl>,
    /// Set by the device thread: true when samples are scaled in software
    /// (shared mode, or an exclusive device without hardware volume).
    software: Arc<AtomicBool>,
    device_lost: Arc<AtomicBool>,
    /// Set by device notifications when this open fell back from the selected
    /// device and that device (or the default) is back: see `needs_reopen`.
    rejoin: Arc<AtomicBool>,
    status: Arc<Mutex<OutputStatus>>,
    /// Shared with every pump thread this output starts, so the count spans
    /// reopens.
    starvation: Arc<Starvation>,
}

impl WasapiOutput {
    pub fn new() -> Self {
        Self {
            device_id: None,
            resolved_device_id: None,
            producer: None,
            samples_per_sec: 0,
            device_ms: 0,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            flush: None,
            control: Arc::new(VolumeControl::default()),
            software: Arc::new(AtomicBool::new(true)),
            device_lost: Arc::new(AtomicBool::new(false)),
            rejoin: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(OutputStatus::default())),
            starvation: Arc::new(Starvation::default()),
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.producer = None;
        self.flush = None;
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
        self.software.store(true, Ordering::SeqCst);
        // A fresh flag per open: a notification for the previous session's
        // device must not reopen this one.
        self.rejoin = Arc::new(AtomicBool::new(false));

        let sample_rate = spec.info.sample_rate;
        let channels = spec.info.channels;
        self.samples_per_sec = sample_rate as usize * channels as usize;
        let frames = (sample_rate as usize * channels as usize) / 5;
        let (producer, mut consumer) = RingBuffer::<u32>::new(frames.max(MIN_RING_SAMPLES));

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let spec_thread = spec.clone();
        let lost = self.device_lost.clone();
        let rejoin = self.rejoin.clone();
        let flush = Arc::new(FlushSignal::default());
        let flush_thread = flush.clone();
        self.starvation.disarm();
        let starvation = self.starvation.clone();
        let control = self.control.clone();
        let software = self.software.clone();

        let handle = std::thread::Builder::new()
            .name("oxitide-wasapi".into())
            .spawn(move || {
                match setup_device(&spec_thread, &lost, &rejoin, starvation, control, software) {
                    Ok((mut session, ready)) => {
                        let _ = ready_tx.send(Ok(ready));
                        if let Err(e) = session.pump(&mut consumer, &stop_thread, &flush_thread) {
                            tracing::error!("wasapi output stopped: {e}");
                            // Seen as AUDCLNT_E_DEVICE_INVALIDATED (0x88890004)
                            // when a USB DAC drops off: report the device lost now;
                            // the engine heard nothing for 2 s (the ring's stall
                            // timeout) before.
                            lost.store(true, Ordering::SeqCst);
                        }
                    }
                    Err(e) => {
                        // Setup failed before any audio was written.
                        let _ = ready_tx.send(Err(e.to_string()));
                    }
                }
            })
            .map_err(|e| Error::Output(e.to_string()))?;

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(ready)) => {
                self.producer = Some(producer);
                self.worker = Some(handle);
                self.stop = stop;
                self.flush = Some(flush);
                self.resolved_device_id = ready.device_id;
                self.device_ms = frames_to_ms(ready.buffer_frames, sample_rate);
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
                return Err(Error::Output(
                    "audio device did not initialize in time".into(),
                ));
            }
        }
        // The device thread applies the volume from `control` from its first
        // period on.
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        if self.device_lost.load(Ordering::SeqCst) {
            return Err(Error::Output("output device unavailable".into()));
        }
        // The ring holds unscaled samples: volume is applied as they leave
        // for the device (`DeviceSession::write_frames`), so a change is not
        // held back by the ~200 ms already queued. The requested mode can
        // differ from the mode that actually opened (exclusive -> shared
        // fallback), so convert to what the device accepts.
        let actual_mode = self.status.lock().unwrap().mode;
        let rate = self.samples_per_sec;
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| Error::Output("output not open".into()))?;
        let pushed = match buf {
            PcmBuffer::F32(v) => push_samples(producer, v, rate, |sample| sample.to_bits()),
            PcmBuffer::I32(v) if actual_mode == OutputMode::Exclusive => {
                push_samples(producer, v, rate, |sample| sample as u32)
            }
            // Shared output fell back from exclusive: the device wants float
            // samples. Left-aligned i32 -> f32 is a pure divide.
            PcmBuffer::I32(v) => push_samples(producer, v, rate, |sample| {
                (sample as f32 / 2_147_483_648.0).to_bits()
            }),
        };
        self.starvation.arm();
        pushed
    }

    /// Picked up by the device thread within one period: a glide of the
    /// samples (software) or of the DAC's own volume (exclusive endpoint).
    fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.control.set(volume);
        Ok(())
    }

    fn attach_volume(&mut self, control: Arc<VolumeControl>) {
        self.control = control;
    }

    fn flush(&mut self) -> Result<()> {
        let Some(signal) = self.flush.clone() else {
            return Ok(());
        };
        let seq = signal.request();
        if !signal.wait(seq) {
            return Err(Error::Output("audio device did not flush in time".into()));
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.shutdown_worker();
        Ok(())
    }

    fn status(&self) -> OutputStatus {
        let mut status = self.status.lock().unwrap().clone();
        // Exclusive without a usable endpoint volume scales in software;
        // unity is a true pass-through, so only a lower volume costs
        // bit-perfect.
        if status.mode == OutputMode::Exclusive
            && self.software.load(Ordering::SeqCst)
            && self.control.position() < 1.0
        {
            status.bit_perfect = false;
            status
                .notice
                .get_or_insert_with(|| SOFTWARE_VOLUME_NOTICE.into());
        }
        if self.device_lost.load(Ordering::SeqCst) {
            status.bit_perfect = false;
            status.notice = Some("output device disconnected".into());
        }
        status
    }

    fn queued_ms(&self) -> Option<u32> {
        let producer = self.producer.as_ref()?;
        let ring = ring::queued_ms(
            producer.buffer().capacity(),
            producer.slots(),
            self.samples_per_sec,
        );
        Some(ring + self.device_ms)
    }

    fn underruns(&self) -> Option<u64> {
        Some(self.starvation.count())
    }

    fn needs_reopen(&self) -> bool {
        self.producer.is_some() && self.rejoin.load(Ordering::SeqCst)
    }

    fn drain(&mut self, cancel: &dyn Fn() -> bool) -> Result<()> {
        if let Some(producer) = self.producer.as_ref() {
            let lost = &self.device_lost;
            ring::wait_drained(producer, self.samples_per_sec, self.device_ms, &|| {
                cancel() || lost.load(Ordering::SeqCst)
            });
        }
        Ok(())
    }
}

fn frames_to_ms(frames: u32, rate: u32) -> u32 {
    if rate == 0 {
        0
    } else {
        (frames as u64 * 1000 / rate as u64) as u32
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
    /// The endpoint buffer, in frames of the stream format.
    buffer_frames: u32,
}

struct DeviceSession {
    client: wasapi::AudioClient,
    render: wasapi::AudioRenderClient,
    event: Option<wasapi::Handle>,
    channels: usize,
    /// One device period of bytes, reused (was one allocation per period).
    scratch: Vec<u8>,
    starvation: Arc<Starvation>,
    control: Arc<VolumeControl>,
    /// Shared with `WasapiOutput` for its status.
    software: Arc<AtomicBool>,
    /// The software gain, applied to each period as it leaves for the device.
    ramp: GainRamp,
    /// How the ring words encode samples (the device's format).
    kind: WordKind,
    /// The DAC's own volume, when an exclusive stream has hardware volume.
    endpoint: Option<EndpointStepper>,
    // Kept alive so device-change notifications keep firing.
    _registration: Option<wasapi::DeviceEventRegistration>,
}

impl DeviceSession {
    fn pump(
        &mut self,
        consumer: &mut rtrb::Consumer<u32>,
        stop: &AtomicBool,
        flush: &FlushSignal,
    ) -> Result<()> {
        // Prime the buffer before starting.
        self.write_frames(consumer)?;
        self.client.start_stream().map_err(to_error)?;
        while !stop.load(Ordering::SeqCst) {
            if flush.pending() {
                let result = self.flush_stream(consumer);
                // Always ack so a failed flush cannot hang the engine.
                flush.complete();
                result?;
                continue;
            }
            match &self.event {
                Some(event) => {
                    if event.wait_for_event(200).is_err() {
                        continue; // timeout: re-check the stop flag
                    }
                }
                // Polling exclusive: no event handle, just poll.
                None => std::thread::sleep(Duration::from_millis(5)),
            }
            self.step_endpoint();
            self.write_frames(consumer)?;
        }
        let _ = self.client.stop_stream();
        Ok(())
    }

    /// Discard everything buffered (ring and device) and restart the stream on
    /// the already-negotiated client. Used by seeks so the endpoint is not
    /// released and exclusive-mode DACs do not re-lock.
    fn flush_stream(&mut self, consumer: &mut rtrb::Consumer<u32>) -> Result<()> {
        self.client.stop_stream().map_err(to_error)?;
        let queued = consumer.slots();
        if let Ok(chunk) = consumer.read_chunk(queued) {
            chunk.commit_all();
        }
        self.starvation.disarm();
        // Shared mode only; in exclusive mode Stop already discards the buffer.
        let _ = self.client.reset_stream();
        self.write_frames(consumer)?;
        self.client.start_stream().map_err(to_error)
    }

    /// Hand the device everything it has room for, from the ring; silence for
    /// what the ring cannot cover (an underrun pads whole frames, see
    /// `ring::pop_frames_le`). `scratch` is reused across periods.
    fn write_frames(&mut self, consumer: &mut rtrb::Consumer<u32>) -> Result<()> {
        let frames = self
            .client
            .get_available_space_in_frames()
            .map_err(to_error)? as usize;
        if frames == 0 {
            return Ok(());
        }
        self.scratch.resize(frames * self.channels * 4, 0);
        let taken = pop_frames_le(consumer, self.channels, &mut self.scratch);
        self.starvation.observe(taken, frames);
        let target = if self.endpoint.is_some() {
            1.0
        } else {
            volume_gain(self.control.position())
        };
        self.ramp.retarget(target);
        let kind = self.kind;
        self.ramp
            .apply_frames(&mut self.scratch, self.channels * 4, |frame, gain| {
                scale_le_words(frame, gain, kind)
            });
        self.render
            .write_to_device(frames, &self.scratch, None)
            .map_err(to_error)
    }

    /// Move the DAC's own volume one step toward the control's position. A
    /// device that stops taking it hands the volume to software from here on
    /// (gliding down from unity, so the switch is silent too).
    fn step_endpoint(&mut self) {
        let Some(stepper) = self.endpoint.as_mut() else {
            return;
        };
        let Some(value) = stepper.glide.next(self.control.position(), Instant::now()) else {
            return;
        };
        if let Err(e) = stepper.volume.set(value) {
            tracing::warn!("endpoint volume failed, using software volume: {e}");
            self.endpoint = None;
            self.software.store(true, Ordering::SeqCst);
        }
    }
}

/// Resolve the requested device, falling back to the system default when the
/// id no longer exists or is not active (a USB DAC that is off, still waking
/// or unplugged keeps its endpoint id, but cannot be opened). The bool is true
/// when a fallback happened.
fn resolve_device(
    enumerator: &DeviceEnumerator,
    wanted: Option<&str>,
) -> Result<(wasapi::Device, bool)> {
    if let Some(id) = wanted {
        if let Ok(device) = enumerator.get_device(id) {
            // An unreadable state is not proof the device is gone: try it.
            match device.get_state() {
                Ok(DeviceState::Active) | Err(_) => return Ok((device, false)),
                Ok(state) => tracing::debug!("output device {id} is {state}"),
            }
        }
    }
    let device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(to_error)?;
    Ok((device, wanted.is_some()))
}

/// A device notification, as a session that fell back sees it.
#[derive(Debug, Clone, Copy)]
enum DeviceNotice<'a> {
    Added(&'a str),
    State(&'a str, DeviceState),
    Default {
        direction: Direction,
        role: wasapi::Role,
        id: Option<&'a str>,
    },
}

/// True when a session playing on the default in place of `wanted` should
/// reopen: `wanted` arrived or became active, or the render/console default it
/// stands in for moved. The reopen resolves `wanted` again, so a default move
/// while `wanted` is still away just follows the new default.
fn fallback_rejoins(wanted: &str, our_id: Option<&str>, notice: DeviceNotice<'_>) -> bool {
    match notice {
        DeviceNotice::Added(id) => id == wanted,
        DeviceNotice::State(id, state) => id == wanted && state == DeviceState::Active,
        DeviceNotice::Default {
            direction,
            role,
            id,
        } => direction == Direction::Render && role == wasapi::Role::Console && id != our_id,
    }
}

/// True when a default-device change means the device this session opened is
/// no longer the one we are playing to. Only the render/console default
/// matters: wasapi reports changes for every flow and role.
fn default_change_invalidates(
    opened_default: bool,
    direction: Direction,
    role: wasapi::Role,
    our_id: Option<&str>,
    new_id: Option<&str>,
) -> bool {
    opened_default
        && direction == Direction::Render
        && role == wasapi::Role::Console
        && new_id != our_id
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
        if client
            .initialize_client(&accepted, &Direction::Render, &mode)
            .is_ok()
        {
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
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: def_time,
    };
    client
        .initialize_client(&wave, &Direction::Render, &mode)
        .map_err(to_error)?;
    let event = client.set_get_eventhandle().map_err(to_error)?;
    Ok((client, Some(event)))
}

/// The DAC's own volume for an exclusive stream, set to `position` at once
/// (an open never glides). `None` when the device has no hardware volume:
/// exclusive streams bypass the audio engine, so its endpoint volume would
/// do nothing and the volume must be applied in software.
fn open_endpoint(device_id: Option<&str>, position: f32) -> Option<EndpointStepper> {
    let volume = match EndpointVolume::open(device_id) {
        Ok(volume) => volume,
        Err(e) => {
            tracing::debug!("no endpoint volume ({e}); using software volume");
            return None;
        }
    };
    if !volume.has_hardware_volume() {
        tracing::debug!("device has no hardware volume; using software volume");
        return None;
    }
    if let Err(e) = volume.set(position) {
        tracing::debug!("endpoint volume failed ({e}); using software volume");
        return None;
    }
    Some(EndpointStepper {
        volume,
        glide: EndpointGlide::new(position, Instant::now()),
    })
}

fn setup_device(
    spec: &AudioOutputSpec,
    lost: &Arc<AtomicBool>,
    rejoin: &Arc<AtomicBool>,
    starvation: Arc<Starvation>,
    control: Arc<VolumeControl>,
    software: Arc<AtomicBool>,
) -> Result<(DeviceSession, DeviceReady)> {
    platform::ensure_com()?;
    let enumerator = DeviceEnumerator::new().map_err(to_error)?;
    let (device, fell_back) = resolve_device(&enumerator, spec.device_id.as_deref())?;
    let resolved_id = device.get_id().ok();

    let (def_time, _min_time) = device
        .get_iaudioclient()
        .map_err(to_error)?
        .get_device_period()
        .map_err(to_error)?;

    let mut status = OutputStatus {
        mode: OutputMode::Shared,
        bit_perfect: false,
        notice: None,
    };
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
        tracing::warn!(
            "output device {} unavailable; playing on the system default {} until it returns",
            spec.device_id.as_deref().unwrap_or_default(),
            resolved_id.as_deref().unwrap_or("?"),
        );
        status.notice = Some("selected output device unavailable; using system default".into());
    }

    let render = client.get_audiorenderclient().map_err(to_error)?;
    // IAudioClient has no stream latency in this binding; the buffer the pump
    // keeps full is the device-side part of the lead.
    let buffer_frames = client.get_buffer_size().unwrap_or(0);

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
        // A fallback stands in for the selected device: watch for it (or a
        // new default) so playback moves back without waiting for a format
        // change. Not device loss: the engine reopens in place, no pause.
        let wanted = spec.device_id.clone().filter(|_| fell_back);
        if let Some(wanted) = wanted.clone() {
            let rejoin = rejoin.clone();
            callbacks.set_device_added_callback(move |id| {
                if fallback_rejoins(&wanted, None, DeviceNotice::Added(&id)) {
                    rejoin.store(true, Ordering::SeqCst);
                }
            });
        }
        if let Some(wanted) = wanted.clone() {
            let rejoin = rejoin.clone();
            callbacks.set_device_state_callback(move |id, state| {
                if fallback_rejoins(&wanted, None, DeviceNotice::State(&id, state)) {
                    rejoin.store(true, Ordering::SeqCst);
                }
            });
        }
        {
            let lost = lost.clone();
            let rejoin = rejoin.clone();
            let our_id = resolved_id.clone();
            let opened_default = spec.device_id.is_none();
            callbacks.set_default_device_callback(move |direction, role, id| {
                if default_change_invalidates(
                    opened_default,
                    direction,
                    role,
                    our_id.as_deref(),
                    id.as_deref(),
                ) {
                    lost.store(true, Ordering::SeqCst);
                }
                if let Some(wanted) = wanted.as_deref() {
                    let notice = DeviceNotice::Default {
                        direction,
                        role,
                        id: id.as_deref(),
                    };
                    if fallback_rejoins(wanted, our_id.as_deref(), notice) {
                        rejoin.store(true, Ordering::SeqCst);
                    }
                }
            });
        }
        registration = notify_enum.register_notification_callback(callbacks).ok();
    }
    // The selected device may have come up between the resolve and the
    // registration, with no notification left to tell us.
    if fell_back {
        let back = spec
            .device_id
            .as_deref()
            .and_then(|id| enumerator.get_device(id).ok());
        if back.is_some_and(|device| matches!(device.get_state(), Ok(DeviceState::Active))) {
            rejoin.store(true, Ordering::SeqCst);
        }
    }

    let exclusive = status.mode == OutputMode::Exclusive;
    let position = control.position();
    let endpoint = if exclusive {
        open_endpoint(resolved_id.as_deref(), position)
    } else {
        None
    };
    software.store(endpoint.is_none(), Ordering::SeqCst);
    let start_gain = if endpoint.is_some() {
        1.0
    } else {
        volume_gain(position)
    };
    let session = DeviceSession {
        client,
        render,
        event,
        channels: spec.info.channels as usize,
        scratch: Vec::new(),
        starvation,
        control,
        software,
        ramp: GainRamp::new(start_gain, spec.info.sample_rate),
        // Exclusive streams take the engine's left-aligned integers; shared
        // ones take floats.
        kind: if exclusive {
            WordKind::I32
        } else {
            WordKind::F32
        },
        endpoint,
        _registration: registration,
    };
    Ok((
        session,
        DeviceReady {
            device_id: resolved_id,
            status,
            buffer_frames,
        },
    ))
}

fn to_error(e: wasapi::WasapiError) -> Error {
    Error::Output(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{PcmFormat, StreamInfo};

    #[test]
    fn default_change_only_invalidates_render_console_defaults() {
        assert!(default_change_invalidates(
            true,
            Direction::Render,
            wasapi::Role::Console,
            Some("old"),
            Some("new")
        ));
        assert!(!default_change_invalidates(
            true,
            Direction::Render,
            wasapi::Role::Console,
            Some("same"),
            Some("same")
        ));
        assert!(!default_change_invalidates(
            true,
            Direction::Capture,
            wasapi::Role::Console,
            Some("old"),
            Some("new")
        ));
        assert!(!default_change_invalidates(
            true,
            Direction::Render,
            wasapi::Role::Communications,
            Some("old"),
            Some("new")
        ));
        assert!(!default_change_invalidates(
            false,
            Direction::Render,
            wasapi::Role::Console,
            Some("old"),
            Some("new")
        ));
    }

    #[test]
    fn a_fallback_rejoins_when_the_selected_device_comes_back() {
        let dac = "{dac}";
        let ours = Some("{monitor}");
        assert!(fallback_rejoins(dac, ours, DeviceNotice::Added(dac)));
        assert!(!fallback_rejoins(dac, ours, DeviceNotice::Added("{other}")));
        assert!(fallback_rejoins(
            dac,
            ours,
            DeviceNotice::State(dac, DeviceState::Active)
        ));
        assert!(!fallback_rejoins(
            dac,
            ours,
            DeviceNotice::State(dac, DeviceState::Unplugged)
        ));
        assert!(!fallback_rejoins(
            dac,
            ours,
            DeviceNotice::State("{other}", DeviceState::Active)
        ));
    }

    #[test]
    fn a_fallback_follows_a_new_render_console_default() {
        let dac = "{dac}";
        let ours = Some("{monitor}");
        let default = |direction, role, id| DeviceNotice::Default {
            direction,
            role,
            id,
        };
        assert!(fallback_rejoins(
            dac,
            ours,
            default(Direction::Render, wasapi::Role::Console, Some(dac))
        ));
        assert!(fallback_rejoins(
            dac,
            ours,
            default(Direction::Render, wasapi::Role::Console, Some("{tv}"))
        ));
        assert!(!fallback_rejoins(
            dac,
            ours,
            default(Direction::Render, wasapi::Role::Console, ours)
        ));
        assert!(!fallback_rejoins(
            dac,
            ours,
            default(Direction::Capture, wasapi::Role::Console, Some(dac))
        ));
        assert!(!fallback_rejoins(
            dac,
            ours,
            default(Direction::Render, wasapi::Role::Communications, Some(dac))
        ));
    }

    #[test]
    fn the_endpoint_volume_glides_in_steps_instead_of_jumping() {
        let start = Instant::now();
        let mut glide = EndpointGlide::new(0.9, start);
        assert_eq!(glide.next(0.9, start), None, "nothing to do at the target");
        // A click from 90% to 10%, stepped every 5 ms like the polling pump.
        let mut steps = Vec::new();
        for ms in (0..=60).step_by(5) {
            if let Some(value) = glide.next(0.1, start + Duration::from_millis(ms)) {
                steps.push(value);
            }
        }
        assert!(steps.len() >= 8, "too few steps: {steps:?}");
        assert!(
            steps.windows(2).all(|w| w[1] <= w[0]),
            "not monotonic: {steps:?}"
        );
        assert!(
            steps.windows(2).all(|w| w[0] - w[1] <= 0.11),
            "a step jumped: {steps:?}"
        );
        assert_eq!(*steps.last().unwrap(), 0.1, "the glide ends on the target");
        assert_eq!(glide.next(0.1, start + Duration::from_millis(100)), None);
    }

    #[test]
    fn a_new_endpoint_target_glides_on_from_where_the_volume_is() {
        let start = Instant::now();
        let mut glide = EndpointGlide::new(1.0, start);
        // The glide's clock starts when the pump first sees the target.
        assert_eq!(glide.next(0.0, start), Some(1.0));
        let halfway = glide.next(0.0, start + Duration::from_millis(20)).unwrap();
        assert!((halfway - 0.5).abs() < 0.01, "{halfway}");
        let resumed = glide.next(1.0, start + Duration::from_millis(20)).unwrap();
        assert!(
            (resumed - halfway).abs() < 1e-6,
            "jumped from {halfway} to {resumed}"
        );
        let end = glide.next(1.0, start + Duration::from_millis(60)).unwrap();
        assert_eq!(end, 1.0);
    }

    #[test]
    fn frames_to_ms_converts_at_the_stream_rate() {
        assert_eq!(frames_to_ms(480, 48_000), 10);
        assert_eq!(frames_to_ms(1_764, 44_100), 40);
        assert_eq!(frames_to_ms(480, 0), 0);
    }

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

    /// The end of the queue: `write` returns with the ring full, so a stop
    /// right after it would drop that audio. `drain` must wait for the ring
    /// and the device buffer to play out, then the ring reads empty.
    #[test]
    #[ignore = "requires an audio device"]
    fn drain_waits_until_the_written_audio_has_played() {
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
        // One second: more than the ring holds, so `write` blocks until all
        // but the ring's worth has played.
        out.write(&PcmBuffer::F32(vec![0.0f32; 44100 * 2])).unwrap();
        let queued = out.queued_ms().unwrap();
        let started = std::time::Instant::now();
        out.drain(&|| false).unwrap();
        let took = started.elapsed();
        eprintln!(
            "queued after write: {queued} ms; drain took {took:?} (device {} ms)",
            out.device_ms
        );
        let ring_left = out.queued_ms().unwrap().saturating_sub(out.device_ms);
        out.stop().unwrap();
        assert!(
            queued > 100,
            "the ring should be nearly full after the write: {queued} ms"
        );
        assert!(
            took >= Duration::from_millis(100),
            "drain returned before the audio played: {took:?}"
        );
        assert!(took < Duration::from_secs(1), "drain overran: {took:?}");
        assert_eq!(ring_left, 0, "the ring still held audio after the drain");
    }
}
