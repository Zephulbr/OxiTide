#![cfg(windows)]

//! ASIO output (Track 7, spec `2026-10-11-oxitide-track7-asio-output-design.md`).
//!
//! One owner thread per open loads the driver (COM STA, message pump) and
//! drives its lifecycle. The driver's buffer-switch callback has no user
//! pointer, so the render state is published through `ACTIVE`; `IN_CALLBACK`
//! lets teardown wait out a callback that already holds it.

use std::ffi::{c_long, c_void};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer, RingBuffer};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetDesktopWindow, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

use crate::asio_sys::{self, AsioBool, AsioBufferInfo, AsioCallbacks, AsioTime, Driver, IAsio};
use crate::decode::{PcmBuffer, PcmFormat};
use crate::error::{Error, Result};
use crate::output::{
    asio_device_key, AudioOutput, AudioOutputSpec, OutputDevice, OutputMode, OutputStatus,
    ASIO_PREFIX, SOFTWARE_VOLUME_NOTICE,
};
use crate::wasapi_output::push_word;

/// Floor for the ring so tiny formats still leave room for the driver buffer.
const MIN_RING_SAMPLES: usize = 4096;
const LOST_NOTICE: &str = "ASIO driver was reset or disconnected";

/// The installed ASIO drivers as output devices (registry only; no driver is
/// loaded here).
pub fn devices() -> Vec<OutputDevice> {
    asio_sys::registered_drivers()
        .into_iter()
        .map(|d| OutputDevice {
            id: format!("{ASIO_PREFIX}{}", d.key),
            name: format!("{} (ASIO)", d.description),
            is_default: false,
        })
        .collect()
}

/// A driver's native sample layout (little-endian types only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsioFormat {
    Int16,
    /// Packed 3-byte samples.
    Int24,
    /// A 32-bit container holding `bits` right-aligned bits (32 = full).
    Int32 { bits: u8 },
    Float32,
    Float64,
}

impl AsioFormat {
    pub fn from_type(sample_type: c_long) -> Option<Self> {
        Some(match sample_type {
            asio_sys::ST_INT16_LSB => Self::Int16,
            asio_sys::ST_INT24_LSB => Self::Int24,
            asio_sys::ST_INT32_LSB => Self::Int32 { bits: 32 },
            asio_sys::ST_INT32_LSB16 => Self::Int32 { bits: 16 },
            asio_sys::ST_INT32_LSB18 => Self::Int32 { bits: 18 },
            asio_sys::ST_INT32_LSB20 => Self::Int32 { bits: 20 },
            asio_sys::ST_INT32_LSB24 => Self::Int32 { bits: 24 },
            asio_sys::ST_FLOAT32_LSB => Self::Float32,
            asio_sys::ST_FLOAT64_LSB => Self::Float64,
            _ => return None,
        })
    }

    pub fn bytes(self) -> usize {
        match self {
            Self::Int16 => 2,
            Self::Int24 => 3,
            Self::Int32 { .. } | Self::Float32 => 4,
            Self::Float64 => 8,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::Float32 | Self::Float64)
    }

    /// Significant bits of an integer format; `None` for float.
    pub fn int_bits(self) -> Option<u16> {
        match self {
            Self::Int16 => Some(16),
            Self::Int24 => Some(24),
            Self::Int32 { bits } => Some(bits as u16),
            Self::Float32 | Self::Float64 => None,
        }
    }

    /// Encode one ring word into `out` (`self.bytes()` long). Integer words
    /// are left-aligned `i32`; float words are `f32` bits.
    pub fn encode(self, word: u32, out: &mut [u8]) {
        match self {
            Self::Int16 => out.copy_from_slice(&(((word as i32) >> 16) as i16).to_le_bytes()),
            Self::Int24 => out.copy_from_slice(&((word as i32) >> 8).to_le_bytes()[..3]),
            Self::Int32 { bits } => {
                out.copy_from_slice(&((word as i32) >> (32 - bits as u32)).to_le_bytes())
            }
            Self::Float32 => out.copy_from_slice(&word.to_le_bytes()),
            Self::Float64 => {
                out.copy_from_slice(&(f32::from_bits(word) as f64).to_le_bytes())
            }
        }
    }
}

/// Convert one sample to its ring word for a float or integer driver, with
/// software volume applied when `volume` is not unity.
fn f32_word(sample: f32, float_target: bool, volume: Option<f32>) -> u32 {
    let sample = volume.map_or(sample, |v| sample * v);
    if float_target {
        sample.to_bits()
    } else {
        (sample.clamp(-1.0, 1.0) as f64 * i32::MAX as f64).round() as i32 as u32
    }
}

fn i32_word(sample: i32, float_target: bool, volume: Option<f32>) -> u32 {
    if float_target {
        let float = sample as f32 / 2_147_483_648.0;
        volume.map_or(float, |v| float * v).to_bits()
    } else {
        volume.map_or(sample, |v| (sample as f64 * v as f64) as i32) as u32
    }
}

/// Write channel `channel` of the interleaved whole frames in `words` into
/// one driver buffer, padding with silence (all-zero bytes in every format).
fn render_channel(words: &[u32], channels: usize, channel: usize, format: AsioFormat, out: &mut [u8]) {
    let bytes = format.bytes();
    let frames = words.len() / channels;
    for (frame, slot) in out.chunks_exact_mut(bytes).enumerate() {
        if frame < frames {
            format.encode(words[frame * channels + channel], slot);
        } else {
            slot.fill(0);
        }
    }
}

/// Sample-exact when integer input reaches an integer driver with at least
/// the source's bits at unity volume.
fn is_bit_perfect(input_bits: Option<u16>, format: AsioFormat, unity: bool) -> bool {
    match (input_bits, format.int_bits()) {
        (Some(source), Some(device)) => unity && device >= source,
        _ => false,
    }
}

/// The bits of integer input the engine will send, if any (the engine sends
/// left-aligned `i32` to an exclusive output for 16/24-bit sources).
fn integer_input_bits(spec: &AudioOutputSpec) -> Option<u16> {
    match spec.info.format {
        PcmFormat::I32 { bits }
            if spec.mode == OutputMode::Exclusive && spec.info.is_bit_perfect_candidate() =>
        {
            Some(bits)
        }
        _ => None,
    }
}

// ---- driver callbacks (process-global: ASIO passes no user pointer) ----

static ACTIVE: AtomicPtr<RenderState> = AtomicPtr::new(std::ptr::null_mut());
static IN_CALLBACK: AtomicUsize = AtomicUsize::new(0);
/// Claimed by the open driver; ASIO allows one driver per process.
static CLAIMED: AtomicBool = AtomicBool::new(false);
/// Set by the driver (reset request, buffer-size change); polled by the owner.
static DRIVER_RESET: AtomicBool = AtomicBool::new(false);
/// Last rate the driver reported through `sampleRateDidChange` (f64 bits).
static RATE_CHANGED: AtomicU64 = AtomicU64::new(0);

static CALLBACKS: AsioCallbacks = AsioCallbacks {
    buffer_switch,
    sample_rate_did_change,
    asio_message,
    buffer_switch_time_info,
};

extern "C" fn buffer_switch(index: c_long, _direct_process: AsioBool) {
    IN_CALLBACK.fetch_add(1, Ordering::SeqCst);
    let state = ACTIVE.load(Ordering::SeqCst);
    if !state.is_null() {
        // SAFETY: teardown clears `ACTIVE` and waits for `IN_CALLBACK` to
        // drain before freeing the state.
        unsafe { (*state).render((index & 1) as usize) };
    }
    IN_CALLBACK.fetch_sub(1, Ordering::SeqCst);
}

extern "C" fn buffer_switch_time_info(
    params: *mut AsioTime,
    index: c_long,
    direct_process: AsioBool,
) -> *mut AsioTime {
    buffer_switch(index, direct_process);
    params
}

extern "C" fn sample_rate_did_change(rate: f64) {
    RATE_CHANGED.store(rate.to_bits(), Ordering::SeqCst);
}

extern "C" fn asio_message(selector: c_long, value: c_long, _msg: *mut c_void, _opt: *mut f64) -> c_long {
    match selector {
        asio_sys::K_SELECTOR_SUPPORTED => matches!(
            value,
            asio_sys::K_ENGINE_VERSION
                | asio_sys::K_RESET_REQUEST
                | asio_sys::K_RESYNC_REQUEST
                | asio_sys::K_LATENCIES_CHANGED
        ) as c_long,
        asio_sys::K_ENGINE_VERSION => 2,
        asio_sys::K_RESET_REQUEST => {
            DRIVER_RESET.store(true, Ordering::SeqCst);
            1
        }
        // Unsupported: the driver follows up with a reset request.
        asio_sys::K_BUFFER_SIZE_CHANGE => 0,
        asio_sys::K_RESYNC_REQUEST | asio_sys::K_LATENCIES_CHANGED => 1,
        // We render from `bufferSwitch`; no time info needed.
        asio_sys::K_SUPPORTS_TIME_INFO => 0,
        _ => 0,
    }
}

/// Everything the render callback touches.
struct RenderState {
    /// The ring consumer and a frame-sized scratch buffer. Only the callback
    /// locks it (`try_lock`: a callback never blocks).
    ring: Mutex<(Consumer<u32>, Vec<u32>)>,
    /// Per channel, the driver's two half-buffers.
    buffers: Vec<[*mut c_void; 2]>,
    buffer_frames: usize,
    channels: usize,
    format: AsioFormat,
    driver: *mut IAsio,
    post_output: bool,
    flush_requested: AtomicU64,
    flush_completed: AtomicU64,
}

// SAFETY: the raw pointers are driver-owned buffers and the driver object,
// valid from `createBuffers` until teardown, which waits out every callback.
unsafe impl Send for RenderState {}
unsafe impl Sync for RenderState {}

impl RenderState {
    fn render(&self, half: usize) {
        let len = self.buffer_frames * self.format.bytes();
        let Ok(mut guard) = self.ring.try_lock() else {
            self.fill_silence(half, len);
            return;
        };
        let (consumer, scratch) = &mut *guard;
        let requested = self.flush_requested.load(Ordering::SeqCst);
        if self.flush_completed.load(Ordering::SeqCst) < requested {
            let pending = consumer.slots();
            if let Ok(chunk) = consumer.read_chunk(pending) {
                chunk.commit_all();
            }
            self.flush_completed.store(requested, Ordering::SeqCst);
        }
        // Whole frames only, so an underrun never shifts the channels.
        let frames = (consumer.slots() / self.channels).min(self.buffer_frames);
        let count = frames * self.channels;
        if let Ok(chunk) = consumer.read_chunk(count) {
            let (first, second) = chunk.as_slices();
            scratch[..first.len()].copy_from_slice(first);
            scratch[first.len()..count].copy_from_slice(second);
            chunk.commit_all();
        }
        for (channel, halves) in self.buffers.iter().enumerate() {
            // SAFETY: the driver allocated `buffer_frames` samples per half.
            let out = unsafe { std::slice::from_raw_parts_mut(halves[half].cast::<u8>(), len) };
            render_channel(&scratch[..count], self.channels, channel, self.format, out);
        }
        drop(guard);
        if self.post_output {
            // SAFETY: the driver is alive while buffers exist.
            unsafe { asio_sys::output_ready(self.driver) };
        }
    }

    fn fill_silence(&self, half: usize, len: usize) {
        for halves in &self.buffers {
            // SAFETY: as in `render`.
            unsafe { std::ptr::write_bytes(halves[half].cast::<u8>(), 0, len) };
        }
    }
}

/// What a successful open reports back to `AsioOutput`.
struct Opened {
    producer: Producer<u32>,
    format: AsioFormat,
    flush: FlushHandle,
}

/// The caller's side of the flush handshake with the callback.
#[derive(Clone, Copy)]
struct FlushHandle(*const RenderState);
// SAFETY: only dereferenced while the owner thread keeps the state alive
// (between a successful open and `shutdown_worker`).
unsafe impl Send for FlushHandle {}

pub struct AsioOutput {
    producer: Option<Producer<u32>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    flush: Option<FlushHandle>,
    format: Option<AsioFormat>,
    input_bits: Option<u16>,
    volume: f32,
    status: OutputStatus,
}

impl AsioOutput {
    pub fn new() -> Self {
        Self {
            producer: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            lost: Arc::new(AtomicBool::new(false)),
            flush: None,
            format: None,
            input_bits: None,
            volume: 1.0,
            status: OutputStatus::default(),
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.flush = None;
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.producer = None;
        self.format = None;
    }

    fn refresh_status(&mut self) {
        let unity = (self.volume - 1.0).abs() < f32::EPSILON;
        let bit_perfect = self
            .format
            .is_some_and(|f| is_bit_perfect(self.input_bits, f, unity));
        let software = !unity && self.input_bits.is_some();
        self.status = OutputStatus {
            mode: OutputMode::Exclusive,
            bit_perfect,
            notice: software.then(|| SOFTWARE_VOLUME_NOTICE.to_string()),
        };
    }
}

impl Default for AsioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for AsioOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.shutdown_worker();
        let key = spec
            .device_id
            .as_deref()
            .and_then(asio_device_key)
            .ok_or_else(|| Error::Output("not an ASIO device".into()))?
            .to_string();
        let driver = asio_sys::registered_drivers()
            .into_iter()
            .find(|d| d.key == key)
            .ok_or_else(|| Error::Output(format!("ASIO driver \"{key}\" is not installed")))?;
        let clsid = asio_sys::parse_clsid(&driver.clsid)
            .ok_or_else(|| Error::Output(format!("ASIO driver \"{key}\" has an invalid CLSID")))?;

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let lost = Arc::new(AtomicBool::new(false));
        let (stop_thread, lost_thread) = (stop.clone(), lost.clone());
        let rate = spec.info.sample_rate;
        let channels = spec.info.channels as usize;
        let handle = std::thread::Builder::new()
            .name("oxitide-asio".into())
            .spawn(move || owner_main(clsid, rate, channels, ready_tx, &stop_thread, &lost_thread))
            .map_err(|e| Error::Output(e.to_string()))?;

        // Driver loads can be slow (USB interfaces wake up, firmware syncs).
        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(opened)) => {
                self.producer = Some(opened.producer);
                self.format = Some(opened.format);
                self.flush = Some(opened.flush);
                self.worker = Some(handle);
                self.stop = stop;
                self.lost = lost;
            }
            Ok(Err(message)) => {
                let _ = handle.join();
                return Err(Error::Output(message));
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                // Do not join: a hung driver would hang the engine with it.
                return Err(Error::Output("ASIO driver did not initialize in time".into()));
            }
        }
        self.input_bits = integer_input_bits(spec);
        self.refresh_status();
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        if self.lost.load(Ordering::SeqCst) {
            return Err(Error::Output(LOST_NOTICE.into()));
        }
        let float_target = self.format.is_some_and(AsioFormat::is_float);
        let volume = ((self.volume - 1.0).abs() >= f32::EPSILON).then_some(self.volume);
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| Error::Output("output not open".into()))?;
        match buf {
            PcmBuffer::F32(v) => {
                for &sample in v {
                    push_word(producer, f32_word(sample, float_target, volume))?;
                }
            }
            PcmBuffer::I32(v) => {
                for &sample in v {
                    push_word(producer, i32_word(sample, float_target, volume))?;
                }
            }
        }
        Ok(())
    }

    /// ASIO has no endpoint volume: anything below 100% is software.
    fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.volume = volume.clamp(0.0, 1.0);
        self.refresh_status();
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        let Some(FlushHandle(state)) = self.flush else {
            return Ok(());
        };
        // SAFETY: see `FlushHandle`.
        let state = unsafe { &*state };
        let seq = state.flush_requested.fetch_add(1, Ordering::SeqCst) + 1;
        let deadline = Instant::now() + Duration::from_secs(1);
        while state.flush_completed.load(Ordering::SeqCst) < seq {
            if self.lost.load(Ordering::SeqCst) || Instant::now() > deadline {
                return Err(Error::Output("ASIO driver did not flush in time".into()));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.shutdown_worker();
        Ok(())
    }

    fn status(&self) -> OutputStatus {
        let mut status = self.status.clone();
        if self.lost.load(Ordering::SeqCst) {
            status.bit_perfect = false;
            status.notice = Some(LOST_NOTICE.into());
        }
        status
    }
}

impl Drop for AsioOutput {
    fn drop(&mut self) {
        self.shutdown_worker();
    }
}

type Ready = std::sync::mpsc::Sender<std::result::Result<Opened, String>>;

/// Owns the driver for one open: set up, report, pump until stopped, tear
/// down. Every driver call except `outputReady` happens on this thread.
fn owner_main(
    clsid: windows::core::GUID,
    rate: u32,
    channels: usize,
    ready: Ready,
    stop: &AtomicBool,
    lost: &AtomicBool,
) {
    if CLAIMED.swap(true, Ordering::SeqCst) {
        let _ = ready.send(Err("another ASIO driver is already open".into()));
        return;
    }
    // SAFETY: balanced by CoUninitialize below.
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    match Session::open(clsid, rate, channels) {
        Ok((session, opened)) => {
            let _ = ready.send(Ok(opened));
            session.run(stop, lost);
            session.close();
        }
        Err(message) => {
            let _ = ready.send(Err(message));
        }
    }
    if com.is_ok() {
        // SAFETY: matches the successful CoInitializeEx.
        unsafe { CoUninitialize() };
    }
    CLAIMED.store(false, Ordering::SeqCst);
}

struct Session {
    driver: Driver,
    state: *mut RenderState,
    rate: f64,
}

impl Session {
    fn open(
        clsid: windows::core::GUID,
        rate: u32,
        channels: usize,
    ) -> std::result::Result<(Self, Opened), String> {
        let driver = Driver::create(&clsid)?;
        // SAFETY: plain Win32 call.
        let hwnd = unsafe { GetDesktopWindow() };
        if !driver.init(hwnd) {
            let reason = driver.error_message();
            return Err(if reason.is_empty() {
                "ASIO driver failed to initialize".into()
            } else {
                format!("ASIO driver failed to initialize: {reason}")
            });
        }
        let (_, outputs) = driver.channels().map_err(asio_error("read channels"))?;
        if (outputs.max(0) as usize) < channels {
            return Err(format!("ASIO driver has {outputs} outputs; {channels} needed"));
        }
        let wanted = rate as f64;
        if !driver.can_sample_rate(wanted) {
            return Err(format!("ASIO driver does not support {}", format_rate(rate)));
        }
        let current = driver.sample_rate().unwrap_or(0.0);
        if (current - wanted).abs() > 0.5 {
            driver.set_sample_rate(wanted).map_err(asio_error("set the sample rate"))?;
        }
        let sample_type = driver.output_channel_type(0).map_err(asio_error("read the sample type"))?;
        let format = AsioFormat::from_type(sample_type)
            .ok_or_else(|| format!("ASIO sample type {sample_type} is not supported"))?;
        let (_, _, preferred, _) = driver.buffer_size().map_err(asio_error("read the buffer size"))?;
        if preferred <= 0 {
            return Err("ASIO driver reported no buffer size".into());
        }
        let buffer_frames = preferred as usize;

        let mut infos: Vec<AsioBufferInfo> = (0..channels)
            .map(|channel| AsioBufferInfo {
                is_input: 0,
                channel_num: channel as c_long,
                buffers: [std::ptr::null_mut(); 2],
            })
            .collect();
        DRIVER_RESET.store(false, Ordering::SeqCst);
        RATE_CHANGED.store(wanted.to_bits(), Ordering::SeqCst);
        // SAFETY: `CALLBACKS` is static.
        unsafe { driver.create_buffers(&mut infos, preferred, &CALLBACKS) }
            .map_err(asio_error("create buffers"))?;

        let ring_len = (rate as usize * channels / 5)
            .max(buffer_frames * channels * 4)
            .max(MIN_RING_SAMPLES);
        let (producer, consumer) = RingBuffer::<u32>::new(ring_len);
        let state = Box::into_raw(Box::new(RenderState {
            ring: Mutex::new((consumer, vec![0; buffer_frames * channels])),
            buffers: infos.iter().map(|i| i.buffers).collect(),
            buffer_frames,
            channels,
            format,
            driver: driver.raw(),
            post_output: driver.supports_output_ready(),
            flush_requested: AtomicU64::new(0),
            flush_completed: AtomicU64::new(0),
        }));
        // Start from silence in both halves.
        // SAFETY: freshly created state.
        unsafe {
            let len = buffer_frames * format.bytes();
            (*state).fill_silence(0, len);
            (*state).fill_silence(1, len);
        }
        ACTIVE.store(state, Ordering::SeqCst);
        let session = Session { driver, state, rate: wanted };
        if let Err(code) = session.driver.start() {
            session.close();
            return Err(format!("ASIO driver could not start ({})", asio_sys::error_text(code)));
        }
        tracing::info!(
            "asio output open: {} Hz, {channels} ch, {format:?}, {buffer_frames} frames",
            rate
        );
        Ok((session, Opened { producer, format, flush: FlushHandle(state) }))
    }

    fn run(&self, stop: &AtomicBool, lost: &AtomicBool) {
        let mut msg = MSG::default();
        while !stop.load(Ordering::SeqCst) {
            // Some drivers post to the thread that initialized them.
            // SAFETY: standard message pump on this thread.
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            let rate = f64::from_bits(RATE_CHANGED.load(Ordering::SeqCst));
            let reset =
                DRIVER_RESET.swap(false, Ordering::SeqCst) || (rate - self.rate).abs() > 0.5;
            if reset && !lost.swap(true, Ordering::SeqCst) {
                tracing::warn!("asio driver requested a reset");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn close(self) {
        let _ = self.driver.stop();
        ACTIVE.store(std::ptr::null_mut(), Ordering::SeqCst);
        while IN_CALLBACK.load(Ordering::SeqCst) > 0 {
            std::thread::yield_now();
        }
        let _ = self.driver.dispose_buffers();
        // SAFETY: unpublished above and no callback holds it.
        drop(unsafe { Box::from_raw(self.state) });
        // `driver` drops here: Release.
    }
}

fn asio_error(action: &'static str) -> impl Fn(c_long) -> String {
    move |code| format!("ASIO driver could not {action} ({})", asio_sys::error_text(code))
}

fn format_rate(rate: u32) -> String {
    if rate.is_multiple_of(1000) {
        format!("{} kHz", rate / 1000)
    } else {
        format!("{:.1} kHz", rate as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::StreamInfo;
    use crate::output::ExclusiveFallback;

    #[test]
    fn maps_little_endian_pcm_types_only() {
        assert_eq!(AsioFormat::from_type(18), Some(AsioFormat::Int32 { bits: 32 }));
        assert_eq!(AsioFormat::from_type(27), Some(AsioFormat::Int32 { bits: 24 }));
        assert_eq!(AsioFormat::from_type(19), Some(AsioFormat::Float32));
        assert_eq!(AsioFormat::from_type(2), None, "MSB types are rejected");
        assert_eq!(AsioFormat::from_type(32), None, "DSD types are rejected");
    }

    #[test]
    fn encodes_every_format() {
        let word = 0x1234_5678u32;
        let mut two = [0u8; 2];
        AsioFormat::Int16.encode(word, &mut two);
        assert_eq!(two, [0x34, 0x12]);
        let mut three = [0u8; 3];
        AsioFormat::Int24.encode(word, &mut three);
        assert_eq!(three, [0x56, 0x34, 0x12]);
        let mut four = [0u8; 4];
        AsioFormat::Int32 { bits: 32 }.encode(word, &mut four);
        assert_eq!(four, word.to_le_bytes());
        AsioFormat::Int32 { bits: 24 }.encode(word, &mut four);
        assert_eq!(four, 0x0012_3456u32.to_le_bytes());
        AsioFormat::Int32 { bits: 16 }.encode(0xFFFF_0000, &mut four);
        assert_eq!(i32::from_le_bytes(four), -1, "right-aligned keeps the sign");
        AsioFormat::Float32.encode(0.5f32.to_bits(), &mut four);
        assert_eq!(f32::from_le_bytes(four), 0.5);
        let mut eight = [0u8; 8];
        AsioFormat::Float64.encode((-0.25f32).to_bits(), &mut eight);
        assert_eq!(f64::from_le_bytes(eight), -0.25);
    }

    #[test]
    fn sixteen_bit_source_survives_every_integer_format() {
        let sample: i16 = -12345;
        let word = ((sample as i32) << 16) as u32;
        let mut two = [0u8; 2];
        AsioFormat::Int16.encode(word, &mut two);
        assert_eq!(i16::from_le_bytes(two), sample);
        let mut four = [0u8; 4];
        AsioFormat::Int32 { bits: 16 }.encode(word, &mut four);
        assert_eq!(i32::from_le_bytes(four), sample as i32);
    }

    #[test]
    fn render_deinterleaves_whole_frames_and_pads_silence() {
        // Two frames of stereo into a three-frame Int16 buffer.
        let words = [1u32 << 16, 2 << 16, 3 << 16, 4 << 16];
        let mut left = [0xAAu8; 6];
        let mut right = [0xAAu8; 6];
        render_channel(&words, 2, 0, AsioFormat::Int16, &mut left);
        render_channel(&words, 2, 1, AsioFormat::Int16, &mut right);
        assert_eq!(left, [1, 0, 3, 0, 0, 0]);
        assert_eq!(right, [2, 0, 4, 0, 0, 0]);
    }

    #[test]
    fn words_convert_between_float_and_integer_targets() {
        assert_eq!(i32_word(1000, false, None), 1000);
        assert_eq!(i32_word(1000, false, Some(0.5)) as i32, 500);
        assert_eq!(f32::from_bits(i32_word(i32::MIN, true, None)), -1.0);
        assert_eq!(f32_word(0.5, true, Some(0.5)), 0.25f32.to_bits());
        assert_eq!(f32_word(1.0, false, None) as i32, i32::MAX);
        assert_eq!(f32_word(-2.0, false, None) as i32, -i32::MAX, "clamped");
    }

    #[test]
    fn bit_perfect_needs_integer_input_enough_bits_and_unity() {
        let int24 = AsioFormat::Int24;
        assert!(is_bit_perfect(Some(16), int24, true));
        assert!(is_bit_perfect(Some(24), AsioFormat::Int32 { bits: 32 }, true));
        assert!(!is_bit_perfect(Some(24), AsioFormat::Int16, true));
        assert!(!is_bit_perfect(Some(16), int24, false), "software volume");
        assert!(!is_bit_perfect(None, int24, true), "float input");
        assert!(!is_bit_perfect(Some(16), AsioFormat::Float32, true), "float driver");
    }

    fn spec(mode: OutputMode, format: PcmFormat) -> AudioOutputSpec {
        AudioOutputSpec {
            device_id: Some("asio:Test".into()),
            info: StreamInfo { sample_rate: 44100, channels: 2, bits_per_sample: 16, format },
            mode,
            exclusive_fallback: ExclusiveFallback::Shared,
        }
    }

    #[test]
    fn integer_input_only_for_exclusive_integer_sources() {
        let int16 = PcmFormat::I32 { bits: 16 };
        assert_eq!(integer_input_bits(&spec(OutputMode::Exclusive, int16)), Some(16));
        assert_eq!(integer_input_bits(&spec(OutputMode::Shared, int16)), None);
        assert_eq!(integer_input_bits(&spec(OutputMode::Exclusive, PcmFormat::F32)), None);
    }

    #[test]
    fn open_rejects_unknown_drivers_without_loading_anything() {
        let mut out = AsioOutput::new();
        let mut wanted = spec(OutputMode::Exclusive, PcmFormat::I32 { bits: 16 });
        wanted.device_id = Some("asio:__no such driver__".into());
        let err = out.open(&wanted).unwrap_err();
        assert!(err.to_string().contains("not installed"), "{err}");
    }

    #[test]
    #[ignore = "requires an installed ASIO driver"]
    fn opens_an_installed_driver_and_plays_silence() {
        let drivers = devices();
        assert!(!drivers.is_empty(), "no ASIO driver installed");
        let mut opened = 0;
        for device in drivers {
            let mut out = AsioOutput::new();
            let mut wanted = spec(OutputMode::Exclusive, PcmFormat::I32 { bits: 16 });
            wanted.device_id = Some(device.id.clone());
            if let Err(e) = out.open(&wanted) {
                eprintln!("{}: {e}", device.name);
                continue;
            }
            out.write(&PcmBuffer::I32(vec![0; 44100 * 2])).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            out.flush().unwrap();
            let status = out.status();
            eprintln!("{}: opened {:?}, {status:?}", device.name, out.format);
            assert_eq!(status.mode, OutputMode::Exclusive);
            out.stop().unwrap();
            opened += 1;
        }
        assert!(opened > 0, "no driver opened");
    }
}
