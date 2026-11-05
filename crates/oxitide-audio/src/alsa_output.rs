use crate::decode::{PcmFormat, StreamInfo};
use crate::error::{Error, Result};
use crate::output::{exclusive_formats, ExclusiveFallback, OutputDevice};

/// True when ALSA answered `-EPIPE`, an underrun. From `snd_pcm_start` it is a
/// start with no frames queued, which the first `write` recovers by starting
/// the stream itself; from `writei` the device ran dry since the last write.
#[cfg(target_os = "linux")]
pub fn is_underrun(error: &alsa::Error) -> bool {
    std::io::Error::from_raw_os_error(error.errno()).kind() == std::io::ErrorKind::BrokenPipe
}

/// What a lost device reports, in the words WASAPI's status uses.
pub const DEVICE_DISCONNECTED: &str = "output device disconnected";

/// True when ALSA says the device itself is gone (a USB DAC unplugged):
/// `ENODEV`, or `ESHUTDOWN` from some USB paths. Every later call on that PCM
/// fails the same way, so the engine's device-loss path takes over.
#[cfg(target_os = "linux")]
pub fn is_device_gone(error: &alsa::Error) -> bool {
    const ENODEV: i32 = 19;
    const ESHUTDOWN: i32 = 108;
    matches!(error.errno(), ENODEV | ESHUTDOWN)
}

/// Periods the ALSA open aims for.
pub const ALSA_PERIODS: u32 = 4;

/// Frames per period are kept a multiple of this, so a period is a multiple
/// of 128 bytes in every container OxiTide writes (2+ bytes per frame). HDA
/// controllers require that alignment of the period and the buffer.
const PERIOD_ALIGN_FRAMES: i64 = 64;

/// Frames the ALSA open asks for: about 150 ms, at least 1024, rounded down
/// to [`ALSA_PERIODS`] aligned periods. The buffer is the lag of a pause and
/// of a software volume change (ALSA has no ring of ours), so it stays short;
/// half a second was audibly late (2026-11-05 Linux run, L3). The rounding
/// matters: a raw size four periods cannot split (22050 frames at 44.1 kHz)
/// made the exact `set_periods(4)` answer EINVAL.
pub fn buffer_frames(sample_rate: u32) -> i64 {
    let step = ALSA_PERIODS as i64 * PERIOD_ALIGN_FRAMES;
    ((sample_rate as i64 * 3 / 20).max(1024) / step) * step
}

/// The `/proc/asound` entry of the card a PCM name is on: `Pro` for
/// `hw:CARD=Pro,DEV=0`, `plughw:CARD=Pro,DEV=0` or `sysdefault:CARD=Pro`,
/// `card3` for `hw:3,0`. `None` for names on no particular card (`default`,
/// `pipewire`, `null`), which a fallback never watches.
pub fn card_dir(device: &str) -> Option<String> {
    let (_, args) = device.split_once(':')?;
    let first = args.split(',').next()?;
    let card = first.strip_prefix("CARD=").unwrap_or(first);
    if card.is_empty() {
        return None;
    }
    if card.bytes().all(|b| b.is_ascii_digit()) {
        Some(format!("card{card}"))
    } else if card.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        Some(card.to_string())
    } else {
        None
    }
}

/// Whether the selected card, which a fallback replaced, has come back: it
/// must have been seen missing first, so a card that is present but busy
/// (another program holds it) never makes the engine reopen in a loop.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CardReturn {
    gone: bool,
    back: bool,
}

impl CardReturn {
    /// Start watching; `present` is whether the card exists right now.
    pub fn new(present: bool) -> Self {
        Self { gone: !present, back: false }
    }
    /// Record a check; returns whether the card is back.
    pub fn observe(&mut self, present: bool) -> bool {
        if !present {
            self.gone = true;
        } else if self.gone {
            self.back = true;
        }
        self.back
    }
    pub fn is_back(&self) -> bool {
        self.back
    }
}

/// The fill level at which a prepared PCM starts by itself: the whole buffer
/// but one period, so the first period plays with the rest already queued.
pub fn start_threshold(buffer: i64, period: i64) -> i64 {
    (buffer - period).max(period).max(1)
}

/// Frames `AlsaOutput` holds back from each write (10 ms), so a pause can end
/// the written audio with a fade instead of cutting it mid-waveform.
pub fn hold_frames(sample_rate: u32) -> usize {
    (sample_rate as usize / 100).max(1)
}

/// Fade `samples` (interleaved, `channels` wide) linearly to silence over
/// their length: the last frame ends at zero.
pub fn fade_to_silence(samples: &mut [i32], channels: usize) {
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    for (i, frame) in samples.chunks_exact_mut(channels).enumerate() {
        let gain = (frames - 1 - i) as f64 / frames as f64;
        for sample in frame {
            *sample = (*sample as f64 * gain) as i32;
        }
    }
}

/// The period to ask for once the device settled on `buffer` frames: a
/// quarter of it. Used with `set_period_size_near`, so a device that cannot
/// split its buffer that way picks the nearest period it supports instead of
/// failing the open.
pub fn period_frames(buffer: i64) -> i64 {
    (buffer / ALSA_PERIODS as i64).max(1)
}

/// The PCM a shared-mode open uses for `device`: a direct `hw:` device goes
/// through its `plughw:` twin, which converts format and rate. A raw `hw:`
/// open accepts only the device's native formats and rates, so a shared
/// session on it would fail for most tracks.
pub fn shared_pcm_name(device: &str) -> String {
    match device.strip_prefix("hw:") {
        Some(rest) => format!("plughw:{rest}"),
        None => device.to_string(),
    }
}

/// Notice shown when the selected device cannot be opened and playback moved
/// to the system default; matches WASAPI's wording.
pub const DEVICE_UNAVAILABLE_NOTICE: &str = "selected output device unavailable; using system default";

/// Error when an exclusive open fails because something holds the card
/// (another player, or the sound server that has not released it yet), so
/// it does not read as a device that cannot do exclusive mode.
pub const DEVICE_BUSY: &str =
    "output device is busy (another app or the sound server is using it)";

/// One playback PCM device on a sound card, from the card's control interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectPcm {
    /// The card's stable id (`Pro`, `Generic`), as in `hw:CARD=<id>`.
    pub card_id: String,
    /// The card's name (`MOONDROP Dawn Pro`).
    pub card_name: String,
    pub device: u32,
    /// The PCM's name (`USB Audio`, `ALC3246 Analog`, `HDMI 0`).
    pub pcm_name: String,
}

/// Picker entries for direct hardware PCMs. Current alsa-lib no longer lists
/// `hw:` names in its device hints, so they are built from the cards
/// themselves; only these ids can open exclusive (see [`is_direct_hw`]).
pub fn direct_devices(pcms: &[DirectPcm]) -> Vec<OutputDevice> {
    pcms.iter()
        .map(|pcm| {
            let card = pcm.card_name.trim();
            let card = if card.is_empty() { pcm.card_id.as_str() } else { card };
            let name = pcm.pcm_name.trim();
            OutputDevice {
                id: format!("hw:CARD={},DEV={}", pcm.card_id, pcm.device),
                name: if name.is_empty() {
                    format!("{card} (direct hardware)")
                } else {
                    format!("{card}, {name} (direct hardware)")
                },
                is_default: false,
            }
        })
        .collect()
}

/// The ALSA sample formats OxiTide writes, best quality first. All are
/// little-endian and use a 32-, 24-, or 16-bit container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsaSampleFormat {
    S32Le,
    S24_3Le,
    S16Le,
}

impl AlsaSampleFormat {
    pub fn store_bits(self) -> u16 {
        match self {
            AlsaSampleFormat::S32Le => 32,
            AlsaSampleFormat::S24_3Le => 24,
            AlsaSampleFormat::S16Le => 16,
        }
    }
}

/// Formats to try, in order, for an exclusive open. Empty when the source is
/// not an integer 16/24/32-bit bit-perfect candidate. The 32-bit container is
/// preferred (symphonia left-aligns integer samples to 32 bits); packed 24-bit
/// and 16-bit containers are the step-down options for DACs without S32.
pub fn alsa_exclusive_formats(info: &StreamInfo) -> Vec<AlsaSampleFormat> {
    let PcmFormat::I32 { bits } = info.format else {
        return Vec::new();
    };
    // `exclusive_formats` gates the engine's 16/24-bit window; a 32-bit I32
    // source still has a valid S32_LE container for ALSA.
    if bits != 32 && exclusive_formats(info).is_empty() {
        return Vec::new();
    }
    let mut out = vec![AlsaSampleFormat::S32Le];
    if bits <= 24 {
        out.push(AlsaSampleFormat::S24_3Le);
    }
    if bits <= 16 {
        out.push(AlsaSampleFormat::S16Le);
    }
    out
}

/// Pack left-aligned i32 samples into interleaved little-endian bytes for
/// `format`. `out` is cleared first. S24_3Le keeps bytes 1..4 of the LE word
/// and S16Le bytes 2..4, which drops the low padding bits symphonia leaves in
/// the 32-bit container.
pub fn pack_i32(format: AlsaSampleFormat, samples: &[i32], out: &mut Vec<u8>) {
    out.clear();
    match format {
        AlsaSampleFormat::S32Le => {
            for &sample in samples {
                out.extend_from_slice(&sample.to_le_bytes());
            }
        }
        AlsaSampleFormat::S24_3Le => {
            for &sample in samples {
                let bytes = sample.to_le_bytes();
                out.extend_from_slice(&bytes[1..4]);
            }
        }
        AlsaSampleFormat::S16Le => {
            for &sample in samples {
                let bytes = sample.to_le_bytes();
                out.extend_from_slice(&bytes[2..4]);
            }
        }
    }
}

/// A parsed ALSA PCM hint (from `snd_device_name_hint`), before filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub name: String,
    pub desc: Option<String>,
    pub ioid: Option<String>,
}

/// True when the hint is playback-capable. An absent IOID means both directions.
pub fn hint_is_output(hint: &Hint) -> bool {
    !matches!(hint.ioid.as_deref(), Some(ioid) if ioid.eq_ignore_ascii_case("Input"))
}

/// The picker surfaces the system default, sysdefault, and direct hardware
/// names; plugin aliases (surround*, null, dmix, …) are hidden to keep the
/// list meaningful.
pub fn hint_is_pickable(name: &str) -> bool {
    name == "default"
        || name == "sysdefault"
        || name.starts_with("sysdefault:")
        || name.starts_with("hw:")
        || name.starts_with("plughw:")
}

fn hint_label(name: &str, desc: Option<&str>) -> String {
    let desc = desc.map(str::trim).filter(|desc| !desc.is_empty());
    if name.starts_with("sysdefault:") {
        // The card's ALSA default often runs through dmix at a fixed rate;
        // keep it apart from the "(direct hardware)" entry for the same card.
        let card = desc.and_then(|desc| desc.lines().next()).unwrap_or(name);
        return format!("{} (ALSA default, may resample)", card.trim());
    }
    desc.unwrap_or(name).to_string()
}

/// Collapse hints and direct PCMs into the picker list: one entry per ALSA
/// device name (a hint wins over a direct entry with the same id), the
/// default first, then the rest by id.
pub fn device_list(hints: &[Hint], direct: &[DirectPcm]) -> Vec<OutputDevice> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out: Vec<OutputDevice> = hints
        .iter()
        .filter(|hint| hint_is_output(hint) && hint_is_pickable(&hint.name))
        .map(|hint| OutputDevice {
            id: hint.name.clone(),
            name: hint_label(&hint.name, hint.desc.as_deref()),
            is_default: hint.name == "default",
        })
        .chain(direct_devices(direct))
        .filter(|device| seen.insert(device.id.clone()))
        .collect();
    out.sort_by(|a, b| (a.id != "default", &a.id).cmp(&(b.id != "default", &b.id)));
    out
}

/// Left-aligned i32 conversion for one f32 sample; the float is clamped to
/// -1.0..=1.0 and rounded, because `as i32` truncation would bias downward.
pub fn f32_to_i32_one(sample: f32) -> i32 {
    if sample.is_nan() {
        return 0;
    }
    let scaled = (sample as f64).clamp(-1.0, 1.0) * 2_147_483_648.0;
    let rounded = scaled.round();
    if rounded >= 2_147_483_647.0 {
        i32::MAX
    } else if rounded <= -2_147_483_648.0 {
        i32::MIN
    } else {
        rounded as i32
    }
}

/// Map a raw ALSA mixer volume into 0.0..=1.0.
pub fn raw_to_volume(raw: i64, min: i64, max: i64) -> f32 {
    if max <= min {
        return 1.0;
    }
    ((raw - min) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}

/// Map 0.0..=1.0 onto a raw ALSA mixer volume.
pub fn volume_to_raw(volume: f32, min: i64, max: i64) -> i64 {
    if max <= min {
        return max;
    }
    (min as f64 + (volume.clamp(0.0, 1.0) as f64) * (max - min) as f64).round() as i64
}

/// Map an ALSA PCM device name to the CTL name its mixer lives on:
/// `hw:CARD=X,DEV=0` -> `hw:CARD=X`, `hw:0,0` -> `hw:0`,
/// `plughw:`/`sysdefault:` -> `hw:`, `default` stays `default`.
pub fn ctl_name(pcm: &str) -> String {
    let base = if let Some(rest) = pcm.strip_prefix("plughw:") {
        format!("hw:{rest}")
    } else if let Some(rest) = pcm.strip_prefix("sysdefault:") {
        format!("hw:{rest}")
    } else {
        pcm.to_string()
    };
    match base.strip_prefix("hw:") {
        Some(rest) => format!("hw:{}", rest.split(',').next().unwrap_or(rest)),
        None => base,
    }
}

/// What a failed exclusive attempt does, mirroring WASAPI (`wasapi_output.rs:444`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsaOpen {
    Exclusive,
    SharedWithNotice,
    Fail,
}

pub fn exclusive_fallback_plan(fallback: ExclusiveFallback) -> AlsaOpen {
    match fallback {
        ExclusiveFallback::Shared => AlsaOpen::SharedWithNotice,
        ExclusiveFallback::WarnOnly => AlsaOpen::Fail,
    }
}

/// True when the PCM name addresses hardware directly (`hw:`). Every other
/// name goes through an alsa-lib plugin (dmix, plug, pulse, …) that may
/// resample or convert, so only `hw:` names can be exclusive/bit-perfect.
pub fn is_direct_hw(device: &str) -> bool {
    device.starts_with("hw:")
}

/// Reject device names with an interior NUL byte before they reach the alsa
/// crate, which builds a `CString` from them and panics.
pub fn validate_device_name(device: &str) -> Result<()> {
    if device.contains('\0') {
        return Err(Error::Output("device name contains a NUL byte".into()));
    }
    Ok(())
}

/// Exact notice text used when software volume compromises bit-perfect output;
/// it matches WASAPI so the engine/UI behavior is identical on both platforms.
pub const SOFTWARE_VOLUME_NOTICE: &str = "volume applied in software; bit-perfect compromised";

#[cfg(target_os = "linux")]
mod linux {
    use std::time::{Duration, Instant};

    use alsa::device_name::HintIter;
    use alsa::pcm::{Access, Format, HwParams, PCM};
    use alsa::{Direction, Error as AlsaError, ValueOr};

    use std::sync::Arc;

    use super::*;
    use crate::decode::PcmBuffer;
    use crate::gain::{scale_i32, GainRamp};
    use crate::output::{
        volume_gain, AudioOutput, AudioOutputSpec, OutputDevice, OutputMode, OutputStatus,
        VolumeControl,
    };

    /// Maximum wall-clock time one `write` call may spend on the device before
    /// it is declared stalled.
    const WRITE_DEADLINE: Duration = Duration::from_secs(2);
    /// Longest single poll wait; each wake-up re-checks the deadline.
    const WAIT_SLICE: Duration = Duration::from_millis(100);
    /// Consecutive xrun/suspend recoveries tolerated within one `write` call.
    const MAX_CONSECUTIVE_RECOVERIES: u32 = 8;
    /// Longest a drain waits: the buffer (about 150 ms) and a sound server's, plus slack.
    const DRAIN_LIMIT: Duration = Duration::from_millis(1500);
    const DRAIN_STEP: Duration = Duration::from_millis(5);
    /// Longest a flush or stop waits for the device to play up to the faded
    /// end: the buffer (about 150 ms) plus slack.
    const SETTLE_LIMIT: Duration = Duration::from_millis(300);
    /// How often a fallback looks for the selected card (one `stat`).
    const CARD_CHECK: Duration = Duration::from_secs(1);
    /// How long an open after our own stream on `default` (a fallback, or
    /// the user's choice) waits for the card to stop being busy. PipeWire
    /// routes that stream to the card and keeps it until WirePlumber
    /// suspends the idle sink (5 s by default).
    const BUSY_WAIT: Duration = Duration::from_secs(7);
    const BUSY_STEP: Duration = Duration::from_millis(100);

    fn card_present(card: &str) -> bool {
        std::path::Path::new("/proc/asound").join(card).exists()
    }

    pub struct AlsaOutput {
        pcm: Option<PCM>,
        format: AlsaSampleFormat,
        device: String,
        channels: u16,
        /// The volume (the engine's, once attached); read on every write.
        control: Arc<VolumeControl>,
        software_volume: bool,
        /// Glides software volume changes (a step would click). ALSA has no
        /// ring of ours, so it runs as samples are written.
        ramp: GainRamp,
        status: OutputStatus,
        /// `writei` underruns (`EPIPE`) over this output's life.
        underruns: u64,
        /// The rate the device settled on, for [`Self::queued`].
        rate: u32,
        /// Audio written but not yet heard after the last `write`
        /// (`snd_pcm_delay`: the device buffer plus any sound server's).
        queued: Option<u32>,
        /// Scaled samples and packed bytes, kept between writes.
        scratch: Vec<i32>,
        bytes: Vec<u8>,
        /// The last [`hold_frames`] of audio, not yet written: the next write
        /// sends them first, a pause sends them faded ([`AudioOutput::fade_out`]),
        /// and a drain sends them as they are.
        held: Vec<i32>,
        /// After a fallback to `default`: the selected card's `/proc/asound`
        /// entry, and whether it has gone and come back
        /// ([`AudioOutput::needs_reopen`]).
        watch: Option<(String, std::cell::Cell<CardReturn>)>,
        next_check: std::cell::Cell<Instant>,
    }

    impl AlsaOutput {
        pub fn new() -> Self {
            Self {
                pcm: None,
                format: AlsaSampleFormat::S32Le,
                device: String::new(),
                channels: 2,
                control: Arc::new(VolumeControl::default()),
                software_volume: true,
                ramp: GainRamp::new(1.0, 48_000),
                status: OutputStatus {
                    mode: OutputMode::Shared,
                    bit_perfect: false,
                    notice: None,
                },
                underruns: 0,
                rate: 0,
                queued: None,
                scratch: Vec::new(),
                bytes: Vec::new(),
                held: Vec::new(),
                watch: None,
                next_check: std::cell::Cell::new(Instant::now()),
            }
        }

        /// Open and fully configure a PCM; on any failure the handle is
        /// dropped and the error returned so the caller can try the next
        /// candidate format.
        fn open_pcm(
            &mut self,
            name: &str,
            format: AlsaSampleFormat,
            spec: &AudioOutputSpec,
        ) -> Result<()> {
            validate_device_name(name)?;
            let pcm = PCM::new(name, Direction::Playback, false).map_err(to_error)?;
            {
                let hwp = HwParams::any(&pcm).map_err(to_error)?;
                hwp.set_channels(spec.info.channels as u32).map_err(to_error)?;
                hwp.set_rate(spec.info.sample_rate, ValueOr::Nearest).map_err(to_error)?;
                hwp.set_format(alsa_format(format)).map_err(to_error)?;
                hwp.set_access(Access::RWInterleaved).map_err(to_error)?;
                let buffer = hwp
                    .set_buffer_size_near(buffer_frames(spec.info.sample_rate))
                    .map_err(to_error)?;
                hwp.set_period_size_near(period_frames(buffer), ValueOr::Nearest)
                    .map_err(to_error)?;
                pcm.hw_params(&hwp).map_err(to_error)?;
                self.rate = hwp.get_rate().unwrap_or(spec.info.sample_rate);
                let settled = hwp.get_buffer_size().unwrap_or(buffer);
                let period = hwp.get_period_size().unwrap_or(period_frames(settled));
                // Start only once the buffer is primed. With alsa-lib's
                // default (one frame) the first write started the device
                // nearly empty and it ran dry before the second: an underrun,
                // and an audible tick, at every start, seek and resume.
                let swp = pcm.sw_params_current().map_err(to_error)?;
                swp.set_start_threshold(start_threshold(settled, period)).map_err(to_error)?;
                pcm.sw_params(&swp).map_err(to_error)?;
            }
            pcm.prepare().map_err(to_error)?;
            self.pcm = Some(pcm);
            self.format = format;
            self.device = name.to_string();
            self.channels = spec.info.channels;
            Ok(())
        }

        /// Open `device` for shared playback (see [`shared_pcm_name`]). When
        /// it cannot be opened (unplugged, held by another program) the
        /// system default is used instead and `Ok(false)` is returned, so the
        /// caller can say so, as WASAPI does for a missing device.
        fn open_shared(&mut self, device: &str, spec: &AudioOutputSpec) -> Result<bool> {
            let name = shared_pcm_name(device);
            match self.open_pcm(&name, AlsaSampleFormat::S32Le, spec) {
                Ok(()) => Ok(true),
                Err(error) if name != "default" => {
                    tracing::warn!(target: "alsa", "shared open of {name} failed ({error}); using the system default");
                    self.open_pcm("default", AlsaSampleFormat::S32Le, spec)?;
                    self.watch = card_dir(device).map(|card| {
                        let present = card_present(&card);
                        (card, std::cell::Cell::new(CardReturn::new(present)))
                    });
                    Ok(false)
                }
                Err(error) => Err(error),
            }
        }

        fn close(&mut self) {
            self.queued = None;
            self.held.clear();
            if let Some(pcm) = self.pcm.take() {
                let _ = pcm.drop();
            }
        }
    }

    impl AlsaOutput {
        /// Pack `self.scratch` and write it all to the device, recovering
        /// xruns, then refresh [`Self::queued`].
        fn push_scratch(&mut self) -> Result<()> {
            let Some(pcm) = self.pcm.as_ref() else {
                return Err(Error::Output("output not open".into()));
            };
            pack_i32(self.format, &self.scratch, &mut self.bytes);
            let bytes = &self.bytes;
            let frame_bytes = self.format.store_bits() as usize / 8 * self.channels as usize;
            let io = pcm.io_bytes();
            let deadline = Instant::now() + WRITE_DEADLINE;
            let mut recoveries = 0u32;
            let mut offset = 0;
            while offset < bytes.len() {
                wait_writable(pcm, deadline)?;
                match io.writei(&bytes[offset..]) {
                    Ok(0) => return Err(Error::Output("alsa write made no progress".into())),
                    Ok(frames) => {
                        offset += frames * frame_bytes;
                        recoveries = 0;
                    }
                    Err(error) => {
                        recoveries += 1;
                        if recoveries > MAX_CONSECUTIVE_RECOVERIES {
                            return Err(Error::Output("audio device errored repeatedly".into()));
                        }
                        if is_underrun(&error) {
                            self.underruns += 1;
                        }
                        // xrun / suspend: recover and retry; unrecoverable
                        // errors are returned by `try_recover`.
                        pcm.try_recover(error, true).map_err(to_error)?;
                    }
                }
            }
            let held_frames = self.held.len() as u64 / u64::from(self.channels.max(1));
            self.queued = match pcm.delay() {
                Ok(frames) if self.rate > 0 => {
                    Some(((frames.max(0) as u64 + held_frames) * 1000 / self.rate as u64) as u32)
                }
                _ => None,
            };
            Ok(())
        }

        /// After our own stream on `default`, wait (bounded) until `device`
        /// can be opened: the card may still be held by the sound server that
        /// stream was routed through. Returns at once when the card is free
        /// or missing, and gives up quietly; the open then fails or falls
        /// back as usual.
        fn wait_until_free(device: &str) {
            let Some(card) = card_dir(device) else { return };
            if !card_present(&card) {
                return;
            }
            let give_up = Instant::now() + BUSY_WAIT;
            loop {
                match PCM::new(device, Direction::Playback, true) {
                    Ok(_) => return,
                    Err(error) if error.errno() == 16 && Instant::now() < give_up => {
                        std::thread::sleep(BUSY_STEP);
                    }
                    Err(error) => {
                        tracing::debug!(target: "alsa", "{device} still unavailable: {error}");
                        return;
                    }
                }
            }
        }

        /// The next write starts from silence and glides up to the volume,
        /// so a start, resume or seek does not step in mid-waveform.
        fn fade_in_next(&mut self) {
            self.ramp = GainRamp::new(0.0, self.rate.max(1));
        }

        /// Before the device buffer is discarded (seek, stop, reopen): end
        /// the audio on silence and wait (bounded) until the device has
        /// played up to it. Dropping a running PCM cuts mid-waveform, an
        /// audible click; alsa-lib's rewind is not reachable through the
        /// `alsa` crate, so the queued audio plays out instead (at most the
        /// buffer, about 150 ms). A PCM that is not running has nothing to
        /// cut.
        fn settle(&mut self) {
            let running = |pcm: &PCM| pcm.state() == alsa::pcm::State::Running;
            if !self.pcm.as_ref().is_some_and(running) {
                return;
            }
            if self.fade_out().is_err() {
                return;
            }
            let Some(pcm) = self.pcm.as_ref() else { return };
            // Inside the trailing silence `fade_out` wrote.
            let quiet = (hold_frames(self.rate.max(1)) / 2) as i64;
            let give_up = Instant::now() + SETTLE_LIMIT;
            while Instant::now() < give_up && running(pcm) {
                match pcm.delay() {
                    Ok(frames) if frames > quiet => std::thread::sleep(DRAIN_STEP),
                    _ => break,
                }
            }
        }
    }

    impl Default for AlsaOutput {
        fn default() -> Self {
            Self::new()
        }
    }

    fn alsa_format(format: AlsaSampleFormat) -> Format {
        match format {
            AlsaSampleFormat::S32Le => Format::S32LE,
            AlsaSampleFormat::S24_3Le => Format::S243LE,
            AlsaSampleFormat::S16Le => Format::S16LE,
        }
    }

    /// An ALSA failure as an output error. A lost device gets the plain
    /// [`DEVICE_DISCONNECTED`] (the UI shows this text), not alsa-lib's
    /// `snd_pcm_recover … No such device (19)`; the raw error goes to the log.
    fn to_error(error: AlsaError) -> Error {
        if is_device_gone(&error) {
            tracing::debug!(target: "alsa", "device gone: {error}");
            return Error::Output(DEVICE_DISCONNECTED.into());
        }
        Error::Output(error.to_string())
    }

    /// Poll until the PCM can accept frames, bounded by `deadline`. A blocking
    /// `writei` would otherwise wait forever on a full buffer (wedged server).
    /// PCMs without poll support make `wait` fail; falling through to the
    /// write keeps those devices working, as before.
    pub(super) fn wait_writable(pcm: &PCM, deadline: Instant) -> Result<()> {
        loop {
            if Instant::now() >= deadline {
                return Err(Error::Output("audio device stalled".into()));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait_ms = remaining.min(WAIT_SLICE).as_millis().max(1) as u32;
            match pcm.wait(Some(wait_ms)) {
                Ok(true) => return Ok(()),
                Ok(false) => continue,
                Err(_) => return Ok(()),
            }
        }
    }

    /// Enumerate output PCMs from ALSA's device hints.
    pub fn enumerate() -> Result<Vec<OutputDevice>> {
        let mut hints: Vec<Hint> = HintIter::new_str(None, "pcm")
            .map_err(to_error)?
            .map(|hint| Hint {
                name: hint.name.unwrap_or_default(),
                desc: hint.desc,
                ioid: match hint.direction {
                    Some(Direction::Capture) => Some("Input".to_string()),
                    Some(Direction::Playback) => Some("Output".to_string()),
                    None => None,
                },
            })
            .collect();
        // namehint omits the built-in `pcm.default` on systems without a
        // resolved card (e.g. WSL); it is always defined, so keep the picker's
        // system-default entry stable.
        if !hints.iter().any(|hint| hint.name == "default") {
            hints.push(Hint { name: "default".into(), desc: None, ioid: None });
        }
        Ok(device_list(&hints, &direct_pcms()))
    }

    /// Every playback PCM on every card, read from the cards' control
    /// interfaces. Opening a CTL does not touch the PCM, so a device in use
    /// (PipeWire, another player) is still listed. Errors skip the card.
    fn direct_pcms() -> Vec<DirectPcm> {
        use alsa::card::Iter as CardIter;
        use alsa::ctl::{Ctl, DeviceIter};
        let mut out = Vec::new();
        for card in CardIter::new().filter_map(|card| card.ok()) {
            let Ok(ctl) = Ctl::from_card(&card, false) else { continue };
            let Ok(info) = ctl.card_info() else { continue };
            let (Ok(card_id), Ok(card_name)) = (info.get_id(), info.get_name()) else { continue };
            let (card_id, card_name) = (card_id.to_string(), card_name.to_string());
            for device in DeviceIter::new(&ctl) {
                let Ok(device) = u32::try_from(device) else { continue };
                let Ok(pcm) = ctl.pcm_info(device, 0, Direction::Playback) else { continue };
                out.push(DirectPcm {
                    card_id: card_id.clone(),
                    card_name: card_name.clone(),
                    device,
                    pcm_name: pcm.get_name().unwrap_or_default().to_string(),
                });
            }
        }
        out
    }

    /// Set the card's playback volume through ALSA's mixer, preferring
    /// "Master" then "PCM". Returns an error when no writable playback
    /// element exists, which sends the caller down the software path.
    pub fn set_hardware_volume(device: &str, volume: f32) -> Result<()> {
        use alsa::mixer::{Mixer, SelemId};
        validate_device_name(device)?;
        let mixer = Mixer::new(&ctl_name(device), false).map_err(to_error)?;
        for name in ["Master", "PCM"] {
            let id = SelemId::new(name, 0);
            let Some(selem) = mixer.find_selem(&id) else {
                continue;
            };
            if !selem.has_playback_volume() {
                continue;
            }
            let (min, max) = selem.get_playback_volume_range();
            selem
                .set_playback_volume_all(volume_to_raw(volume, min, max))
                .map_err(to_error)?;
            return Ok(());
        }
        Err(Error::Output("no writable playback volume control".into()))
    }

    impl AudioOutput for AlsaOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            // Our last stream went through the sound server, which may still
            // hold the card we are about to open directly.
            let after_server = self.device == "default";
            self.close();
            // Back from a fallback because the selected card returned (or was
            // re-selected after returning): wait out the sound server's hold.
            // A card that never left is left alone, so a busy DAC does not
            // stall every reopen.
            let rejoining = self.watch.take().is_some_and(|(card, state)| {
                let mut seen = state.get();
                seen.observe(card_present(&card))
            });
            self.software_volume = true;
            // Reset before any fallible step so a failed reopen cannot leave
            // the previous session's exclusive/bit-perfect status visible.
            self.status = OutputStatus {
                mode: OutputMode::Shared,
                bit_perfect: false,
                notice: None,
            };
            let requested = spec.mode;
            let device = match spec.device_id.as_deref() {
                Some(id) if !id.is_empty() => id.to_string(),
                _ if requested == OutputMode::Exclusive => "hw:0,0".to_string(),
                _ => "default".to_string(),
            };
            validate_device_name(&device)?;
            tracing::debug!(target: "alsa", "open requested={requested:?} device={device} rate={} bits={}",
                spec.info.sample_rate, spec.info.bits_per_sample);
            if rejoining {
                tracing::info!(target: "alsa", "selected device {device} is back; waiting for it to be free");
            }
            if rejoining || (after_server && device != "default") {
                Self::wait_until_free(&device);
            }
            let mut status = OutputStatus {
                mode: OutputMode::Shared,
                bit_perfect: false,
                notice: None,
            };

            if requested == OutputMode::Exclusive {
                // Only `hw:` PCMs bypass alsa-lib's plugins, so only they can
                // honor a bit-perfect exclusive open. Any other selected name
                // (default, plughw:, sysdefault:) goes straight to the
                // fallback plan.
                let opened = if is_direct_hw(&device) {
                    alsa_exclusive_formats(&spec.info)
                        .into_iter()
                        .find(|format| {
                            let result = self.open_pcm(&device, *format, spec);
                            if let Err(error) = &result {
                                tracing::debug!(target: "alsa", "exclusive {format:?} failed: {error}");
                            }
                            result.is_ok()
                        })
                } else {
                    None
                };
                match opened {
                    Some(format) => {
                        self.format = format;
                        status.mode = OutputMode::Exclusive;
                        status.bit_perfect = true;
                    }
                    None => match exclusive_fallback_plan(spec.exclusive_fallback) {
                        AlsaOpen::SharedWithNotice => {
                            // Stay on the selected device when it can share
                            // (a `hw:` device through `plughw:`, e.g. for a
                            // lossy source); a busy or missing one falls back
                            // to the system default.
                            self.open_shared(&device, spec)?;
                            status.notice =
                                Some("exclusive mode unavailable; using shared output".into());
                        }
                        AlsaOpen::Fail | AlsaOpen::Exclusive => {
                            let busy = is_direct_hw(&device)
                                && PCM::new(&device, Direction::Playback, true)
                                    .is_err_and(|error| error.errno() == 16);
                            return Err(Error::Output(if busy {
                                DEVICE_BUSY.into()
                            } else {
                                "exclusive mode is not supported by this output device".into()
                            }));
                        }
                    },
                }
            } else if !self.open_shared(&device, spec)? {
                status.notice = Some(DEVICE_UNAVAILABLE_NOTICE.into());
            }

            self.status = status;
            self.set_volume(self.control.position())?;
            // An open fades in from silence (a few ms of the 30 ms ramp), so
            // starting mid-track does not click. Gapless advances keep the
            // open device, so steady playback stays sample-exact.
            self.fade_in_next();
            Ok(())
        }

        fn start(&mut self) -> Result<()> {
            let Some(pcm) = self.pcm.as_ref() else {
                return Ok(());
            };
            match pcm.start() {
                Ok(()) => Ok(()),
                // A start on an empty prepared PCM underruns at once on USB
                // (and some HDMI) devices: the driver begins streaming with
                // nothing queued and answers EPIPE. Nothing is lost, because
                // the first `write` starts the stream once the buffer reaches
                // the start threshold, so re-prepare and let it.
                Err(error) if is_underrun(&error) => {
                    tracing::debug!(target: "alsa", "start underran on {}; the first write starts it", self.device);
                    pcm.prepare().map_err(to_error)
                }
                Err(error) => Err(to_error(error)),
            }
        }

        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            if self.pcm.is_none() {
                return Err(Error::Output("output not open".into()));
            }
            let target = if self.software_volume {
                volume_gain(self.control.position())
            } else {
                1.0
            };
            self.ramp.retarget(target);
            let channels = self.channels.max(1) as usize;
            // The audio held back last time goes first; it was already scaled.
            self.scratch.clear();
            self.scratch.append(&mut self.held);
            let fresh = self.scratch.len();
            match buf {
                PcmBuffer::I32(v) => self.scratch.extend_from_slice(v),
                PcmBuffer::F32(v) => {
                    self.scratch.extend(v.iter().map(|&sample| f32_to_i32_one(sample)))
                }
            }
            if !self.ramp.is_identity() {
                self.ramp.apply_frames(&mut self.scratch[fresh..], channels, scale_i32);
            }
            // Hold back the newest 10 ms so a pause can fade them out.
            let hold = (hold_frames(self.rate.max(1)) * channels).min(self.scratch.len());
            let keep = (self.scratch.len() - hold) / channels * channels;
            self.held.extend_from_slice(&self.scratch[keep..]);
            self.scratch.truncate(keep);
            // Even with everything held, this refreshes the reported lead.
            self.push_scratch()
        }

        fn attach_volume(&mut self, control: Arc<VolumeControl>) {
            self.control = control;
        }

        fn set_volume(&mut self, volume: f32) -> Result<()> {
            self.control.set(volume);
            let volume = self.control.position();
            let exclusive = self.status.mode == OutputMode::Exclusive;
            if exclusive {
                if set_hardware_volume(&self.device, volume).is_ok() {
                    self.software_volume = false;
                    if self.status.notice.as_deref() == Some(SOFTWARE_VOLUME_NOTICE) {
                        self.status.notice = None;
                        self.status.bit_perfect = true;
                    }
                    return Ok(());
                }
                self.software_volume = true;
                self.status.bit_perfect = false;
                self.status.notice = Some(SOFTWARE_VOLUME_NOTICE.into());
            } else {
                self.software_volume = true;
            }
            Ok(())
        }

        fn flush(&mut self) -> Result<()> {
            // Discard the held and device buffers but keep the negotiated
            // handle, so a seek does not release and re-lock an exclusive DAC.
            self.settle();
            self.held.clear();
            if let Some(pcm) = self.pcm.as_ref() {
                pcm.drop().map_err(to_error)?;
                pcm.prepare().map_err(to_error)?;
            }
            self.fade_in_next();
            Ok(())
        }

        /// A pause: write the held-back audio faded to silence, then a little
        /// silence, so the device plays out its buffer and stops on a zero
        /// instead of mid-waveform. The resume fades in.
        fn fade_out(&mut self) -> Result<()> {
            if self.pcm.is_none() {
                return Ok(());
            }
            let channels = self.channels.max(1) as usize;
            self.scratch.clear();
            self.scratch.append(&mut self.held);
            fade_to_silence(&mut self.scratch, channels);
            let silence = hold_frames(self.rate.max(1)) * channels;
            self.scratch.resize(self.scratch.len() + silence, 0);
            self.fade_in_next();
            self.push_scratch()
        }

        fn stop(&mut self) -> Result<()> {
            self.settle();
            self.close();
            Ok(())
        }

        fn status(&self) -> OutputStatus {
            self.status.clone()
        }

        fn queued_ms(&self) -> Option<u32> {
            self.queued
        }

        fn underruns(&self) -> Option<u64> {
            Some(self.underruns)
        }

        fn needs_reopen(&self) -> bool {
            let Some((card, state)) = self.watch.as_ref() else {
                return false;
            };
            let now = Instant::now();
            if now >= self.next_check.get() {
                self.next_check.set(now + CARD_CHECK);
                let mut seen = state.get();
                seen.observe(card_present(card));
                state.set(seen);
            }
            state.get().is_back()
        }

        /// Poll the frames still queued in the device until they have played
        /// (`snd_pcm_drain` would block without a way to cancel).
        fn drain(&mut self, cancel: &dyn Fn() -> bool) -> Result<()> {
            if self.pcm.is_none() {
                return Ok(());
            }
            // The held-back end of the audio is part of what must be heard.
            if !self.held.is_empty() {
                self.scratch.clear();
                self.scratch.append(&mut self.held);
                self.push_scratch()?;
            }
            let Some(pcm) = self.pcm.as_ref() else {
                return Ok(());
            };
            // Less than the start threshold written (a short last track):
            // the device never started, so start it to play what is queued.
            if pcm.state() == alsa::pcm::State::Prepared && pcm.delay().is_ok_and(|frames| frames > 0) {
                pcm.start().map_err(to_error)?;
            }
            let give_up = Instant::now() + DRAIN_LIMIT;
            while !cancel() && Instant::now() < give_up && pcm.state() == alsa::pcm::State::Running {
                match pcm.delay() {
                    Ok(frames) if frames > 0 => std::thread::sleep(DRAIN_STEP),
                    _ => break,
                }
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{enumerate, AlsaOutput};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    use crate::decode::PcmBuffer;
    #[cfg(target_os = "linux")]
    use crate::output::{AudioOutput, AudioOutputSpec, OutputMode, OutputStatus};

    fn info(bits: u16, format: PcmFormat) -> StreamInfo {
        StreamInfo { sample_rate: 44_100, channels: 2, bits_per_sample: bits, format }
    }

    #[test]
    fn alsa_formats_step_down_for_narrow_sources() {
        assert_eq!(
            alsa_exclusive_formats(&info(16, PcmFormat::I32 { bits: 16 })),
            vec![AlsaSampleFormat::S32Le, AlsaSampleFormat::S24_3Le, AlsaSampleFormat::S16Le]
        );
        assert_eq!(
            alsa_exclusive_formats(&info(24, PcmFormat::I32 { bits: 24 })),
            vec![AlsaSampleFormat::S32Le, AlsaSampleFormat::S24_3Le]
        );
        assert_eq!(
            alsa_exclusive_formats(&info(32, PcmFormat::I32 { bits: 32 })),
            vec![AlsaSampleFormat::S32Le]
        );
    }

    #[test]
    fn alsa_formats_are_empty_for_non_candidates() {
        assert!(alsa_exclusive_formats(&info(16, PcmFormat::F32)).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn only_epipe_counts_as_a_start_underrun() {
        use alsa::Error as AlsaError;
        assert!(is_underrun(&AlsaError::new("snd_pcm_start", 32)));
        assert!(!is_underrun(&AlsaError::new("snd_pcm_start", 19)));
        assert!(!is_underrun(&AlsaError::new("snd_pcm_start", 0)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_unplugged_device_is_recognised_by_errno() {
        use alsa::Error as AlsaError;
        assert!(is_device_gone(&AlsaError::new("snd_pcm_recover", 19)));
        assert!(is_device_gone(&AlsaError::new("snd_pcm_writei", 108)));
        assert!(!is_device_gone(&AlsaError::new("snd_pcm_writei", 32)));
        assert!(!is_device_gone(&AlsaError::new("snd_pcm_open", 16)));
    }

    #[test]
    fn buffer_frames_divide_evenly_into_aligned_periods_at_every_rate() {
        // 44.1 kHz is the case that used to fail: a raw half second is 22050
        // frames, which four periods cannot split, and ALSA answered EINVAL.
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000, 8_000] {
            let frames = buffer_frames(rate);
            let period = period_frames(frames);
            assert_eq!(frames % ALSA_PERIODS as i64, 0, "{frames} frames at {rate}");
            assert_eq!(period * ALSA_PERIODS as i64, frames, "{frames} at {rate}");
            // HDA wants 128-byte periods; the narrowest container here is a
            // 16-bit mono frame (2 bytes).
            assert_eq!(period * 2 % 128, 0, "{period}-frame period at {rate}");
            assert!(frames >= 1024, "{frames} frames at {rate}");
            assert!(frames > rate as i64 * 3 / 20 - 256, "{frames} at {rate}");
            assert!(frames <= (rate as i64 * 3 / 20).max(1024), "{frames} at {rate}");
        }
        assert_eq!(buffer_frames(44_100), 6_400);
        assert_eq!(buffer_frames(48_000), 7_168);
    }

    #[test]
    fn sysdefault_entries_say_they_may_resample() {
        assert_eq!(
            hint_label("sysdefault:CARD=Pro", Some("MOONDROP Dawn Pro, USB Audio\nDefault Audio Device")),
            "MOONDROP Dawn Pro, USB Audio (ALSA default, may resample)"
        );
        assert_eq!(hint_label("sysdefault:CARD=X", None), "sysdefault:CARD=X (ALSA default, may resample)");
        assert_eq!(hint_label("default", Some("Default ALSA Output")), "Default ALSA Output");
    }

    #[test]
    fn a_prepared_pcm_starts_once_all_but_one_period_is_queued() {
        assert_eq!(start_threshold(6_400, 1_600), 4_800);
        assert_eq!(start_threshold(16_384, 1_024), 15_360);
        assert_eq!(start_threshold(1_024, 1_024), 1_024, "a one-period buffer still starts");
        assert_eq!(start_threshold(0, 0), 1);
    }

    #[test]
    fn card_dirs_come_from_card_ids_and_indexes() {
        assert_eq!(card_dir("hw:CARD=Pro,DEV=0").as_deref(), Some("Pro"));
        assert_eq!(card_dir("plughw:CARD=Pro,DEV=0").as_deref(), Some("Pro"));
        assert_eq!(card_dir("sysdefault:CARD=Generic_1").as_deref(), Some("Generic_1"));
        assert_eq!(card_dir("hw:3,0").as_deref(), Some("card3"));
        assert_eq!(card_dir("default"), None);
        assert_eq!(card_dir("pipewire"), None);
        assert_eq!(card_dir("hw:CARD=../x,DEV=0"), None, "never a path outside /proc/asound");
        assert_eq!(card_dir("hw:"), None);
    }

    #[test]
    fn a_card_is_back_only_after_it_was_gone() {
        // Present but busy at the fallback: never reported, no reopen loop.
        let mut busy = CardReturn::new(true);
        assert!(!busy.observe(true));
        assert!(!busy.observe(true));
        // Unplugged at the fallback, then replugged.
        let mut unplugged = CardReturn::new(false);
        assert!(!unplugged.observe(false));
        assert!(unplugged.observe(true));
        assert!(unplugged.is_back(), "stays back until the next open");
        // Present at the fallback, unplugged, replugged.
        let mut later = CardReturn::new(true);
        assert!(!later.observe(false));
        assert!(later.observe(true));
    }

    #[test]
    fn a_fade_to_silence_ends_on_zero_and_starts_near_full() {
        let mut stereo = vec![1_000_000i32; 8];
        fade_to_silence(&mut stereo, 2);
        assert_eq!(&stereo[6..], &[0, 0], "the last frame is silent");
        assert_eq!(stereo[0], stereo[1], "both channels share a gain");
        assert_eq!(stereo[0], 750_000, "4 frames: 3/4, 2/4, 1/4, 0");
        assert!(stereo.chunks(2).map(|f| f[0]).collect::<Vec<_>>().windows(2).all(|w| w[0] > w[1]));
        let mut empty: Vec<i32> = Vec::new();
        fade_to_silence(&mut empty, 2);
        assert_eq!(hold_frames(44_100), 441);
        assert_eq!(hold_frames(48_000), 480);
    }

    #[test]
    fn shared_opens_route_direct_devices_through_plughw() {
        assert_eq!(shared_pcm_name("hw:CARD=Pro,DEV=0"), "plughw:CARD=Pro,DEV=0");
        assert_eq!(shared_pcm_name("hw:0,0"), "plughw:0,0");
        assert_eq!(shared_pcm_name("plughw:CARD=Pro,DEV=0"), "plughw:CARD=Pro,DEV=0");
        assert_eq!(shared_pcm_name("sysdefault:CARD=Pro"), "sysdefault:CARD=Pro");
        assert_eq!(shared_pcm_name("default"), "default");
    }

    fn pcm(card_id: &str, card_name: &str, device: u32, pcm_name: &str) -> DirectPcm {
        DirectPcm {
            card_id: card_id.into(),
            card_name: card_name.into(),
            device,
            pcm_name: pcm_name.into(),
        }
    }

    #[test]
    fn direct_devices_are_named_after_the_card_and_pcm() {
        let devices = direct_devices(&[
            pcm("Pro", "MOONDROP Dawn Pro", 0, "USB Audio"),
            pcm("Generic", "HD-Audio Generic", 3, ""),
            pcm("X", " ", 1, "PCM"),
        ]);
        assert_eq!(devices[0], OutputDevice {
            id: "hw:CARD=Pro,DEV=0".into(),
            name: "MOONDROP Dawn Pro, USB Audio (direct hardware)".into(),
            is_default: false,
        });
        assert_eq!(devices[1].id, "hw:CARD=Generic,DEV=3");
        assert_eq!(devices[1].name, "HD-Audio Generic (direct hardware)");
        assert_eq!(devices[2].name, "X, PCM (direct hardware)");
        assert!(devices.iter().all(|device| is_direct_hw(&device.id)));
    }

    /// Issue #11 (2026-10-23 run): alsa-lib's hints carried no `hw:` names,
    /// so the picker could not offer an exclusive-capable device.
    #[test]
    fn device_list_adds_direct_pcms_the_hints_lack() {
        let list = device_list(
            &[
                hint("default", Some("Default"), None),
                hint("sysdefault:CARD=Pro", Some("MOONDROP Dawn Pro, USB Audio"), Some("Output")),
                hint("front:CARD=Pro,DEV=0", Some("Front"), Some("Output")),
            ],
            &[pcm("Pro", "MOONDROP Dawn Pro", 0, "USB Audio")],
        );
        let ids: Vec<&str> = list.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["default", "hw:CARD=Pro,DEV=0", "sysdefault:CARD=Pro"]);
    }

    #[test]
    fn device_list_prefers_a_hint_over_the_same_direct_pcm() {
        let list = device_list(
            &[hint("hw:CARD=Pro,DEV=0", Some("Dawn Pro direct"), Some("Output"))],
            &[pcm("Pro", "MOONDROP Dawn Pro", 0, "USB Audio")],
        );
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Dawn Pro direct");
    }

    #[test]
    fn store_bits_matches_the_container_or_packed_width() {
        assert_eq!(AlsaSampleFormat::S32Le.store_bits(), 32);
        assert_eq!(AlsaSampleFormat::S24_3Le.store_bits(), 24);
        assert_eq!(AlsaSampleFormat::S16Le.store_bits(), 16);
    }

    #[test]
    fn pack_s32le_preserves_left_aligned_words() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S32Le, &[0x7fff_ff00u32 as i32, -256], &mut out);
        assert_eq!(out, vec![0x00, 0xff, 0xff, 0x7f, 0x00, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn pack_s24_3le_keeps_the_top_three_bytes() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S24_3Le, &[0x7fff_ff00u32 as i32], &mut out);
        assert_eq!(out, vec![0xff, 0xff, 0x7f]);
    }

    #[test]
    fn pack_s16le_keeps_the_top_two_bytes() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S16Le, &[0x7fff_0000u32 as i32, 0], &mut out);
        assert_eq!(out, vec![0xff, 0x7f, 0x00, 0x00]);
    }

    #[test]
    fn pack_clears_previous_contents() {
        let mut out = vec![0xaa, 0xbb];
        pack_i32(AlsaSampleFormat::S32Le, &[], &mut out);
        assert!(out.is_empty());
    }

    use crate::output::{ExclusiveFallback, OutputDevice};

    fn hint(name: &str, desc: Option<&str>, ioid: Option<&str>) -> Hint {
        Hint { name: name.into(), desc: desc.map(str::to_string), ioid: ioid.map(str::to_string) }
    }

    #[test]
    fn device_list_filters_inputs_and_unpickable_names() {
        let list = device_list(&[
            hint("default", Some("Default output"), None),
            hint("sysdefault:CARD=X", Some("X"), Some("Output")),
            hint("hw:CARD=X,DEV=0", Some("X direct"), Some("Output")),
            hint("mic", Some("Microphone"), Some("Input")),
            hint("surround21:CARD=X,DEV=0", Some("Surround"), Some("Output")),
            hint("null", Some("Null"), Some("Output")),
        ], &[]);
        let ids: Vec<&str> = list.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["default", "hw:CARD=X,DEV=0", "sysdefault:CARD=X"]);
    }

    #[test]
    fn device_list_marks_default_and_dedupes() {
        let list = device_list(&[
            hint("hw:CARD=X,DEV=0", Some("Direct"), Some("Output")),
            hint("default", Some("Default"), None),
            hint("default", Some("Default again"), None),
        ], &[]);
        assert!(list[0].is_default);
        assert_eq!(list[0].name, "Default");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn device_list_falls_back_to_the_device_name_as_label() {
        let list = device_list(&[hint("plughw:CARD=Y,DEV=0", None, None)], &[]);
        assert_eq!(list[0].name, "plughw:CARD=Y,DEV=0");
        assert_eq!(list[0], OutputDevice {
            id: "plughw:CARD=Y,DEV=0".into(),
            name: "plughw:CARD=Y,DEV=0".into(),
            is_default: false,
        });
    }

    #[test]
    fn f32_conversion_saturates_at_the_rails() {
        assert_eq!(f32_to_i32_one(0.0), 0);
        assert_eq!(f32_to_i32_one(1.0), i32::MAX);
        assert_eq!(f32_to_i32_one(-1.0), i32::MIN);
        assert_eq!(f32_to_i32_one(2.0), i32::MAX);
        assert_eq!(f32_to_i32_one(f32::NAN), 0);
    }

    #[test]
    fn volume_math_maps_both_directions() {
        assert_eq!(raw_to_volume(0, 0, 100), 0.0);
        assert_eq!(raw_to_volume(50, 0, 100), 0.5);
        assert_eq!(raw_to_volume(100, 0, 100), 1.0);
        assert_eq!(volume_to_raw(0.5, 0, 100), 50);
        assert_eq!(volume_to_raw(1.0, 0, 100), 100);
        assert_eq!(raw_to_volume(10, 10, 10), 1.0);
    }

    #[test]
    fn ctl_name_maps_pcm_names_to_mixer_ctl_names() {
        assert_eq!(ctl_name("hw:0,0"), "hw:0");
        assert_eq!(ctl_name("hw:CARD=X,DEV=0"), "hw:CARD=X");
        assert_eq!(ctl_name("plughw:CARD=X,DEV=0"), "hw:CARD=X");
        assert_eq!(ctl_name("sysdefault:CARD=X"), "hw:CARD=X");
        assert_eq!(ctl_name("default"), "default");
    }

    #[test]
    fn exclusive_fallback_plan_matches_wasapi_semantics() {
        assert_eq!(
            exclusive_fallback_plan(ExclusiveFallback::Shared),
            AlsaOpen::SharedWithNotice
        );
        assert_eq!(exclusive_fallback_plan(ExclusiveFallback::WarnOnly), AlsaOpen::Fail);
    }

    #[test]
    fn is_direct_hw_only_accepts_hw_pcm_names() {
        assert!(is_direct_hw("hw:0,0"));
        assert!(is_direct_hw("hw:CARD=X,DEV=0"));
        assert!(!is_direct_hw("plughw:CARD=X,DEV=0"));
        assert!(!is_direct_hw("default"));
        assert!(!is_direct_hw("null"));
    }

    #[test]
    fn device_names_with_interior_nul_bytes_are_rejected() {
        assert!(validate_device_name("hw:0,0").is_ok());
        assert!(validate_device_name("default").is_ok());
        let err = validate_device_name("hw:0\0,0").unwrap_err();
        assert!(matches!(err, crate::error::Error::Output(_)), "got {err:?}");
        assert!(err.to_string().contains("NUL"), "got {err}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shared_output_lifecycle_on_the_null_device() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Shared,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.start().unwrap();
        out.write(&PcmBuffer::F32(vec![0.5, -0.5, 0.25, -0.25])).unwrap();
        assert!(out.queued_ms().is_some(), "a write reports the device lead");
        // More than the 10 ms hold-back: part goes to the device.
        out.write(&PcmBuffer::F32(vec![0.25; 2 * 4_410])).unwrap();
        out.fade_out().unwrap();
        out.write(&PcmBuffer::F32(vec![0.25; 2 * 4_410])).unwrap();
        out.drain(&|| false).unwrap();
        out.flush().unwrap();
        out.stop().unwrap();
        assert_eq!(out.queued_ms(), None, "a closed output has no lead");
        let status = out.status();
        assert_eq!(status.mode, OutputMode::Shared);
        assert!(!status.bit_perfect);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_on_a_non_direct_device_opens_shared_with_a_notice() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Exclusive,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.start().unwrap();
        out.write(&PcmBuffer::I32(vec![0x7fff_0000u32 as i32; 32])).unwrap();
        out.stop().unwrap();
        let status = out.status();
        assert_eq!(status.mode, OutputMode::Shared);
        assert_eq!(
            status.notice.as_deref(),
            Some("exclusive mode unavailable; using shared output")
        );
        assert!(!status.bit_perfect);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_warn_only_errors_on_a_missing_device() {
        let mut out = AlsaOutput::new();
        let err = out
            .open(&AudioOutputSpec {
                device_id: Some("hw:99,99".into()),
                info: info(16, PcmFormat::I32 { bits: 16 }),
                mode: OutputMode::Exclusive,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
            })
            .unwrap_err();
        assert!(matches!(err, crate::error::Error::Output(_)), "got {err:?}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_failed_reopen_clears_the_previous_status() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Exclusive,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        assert!(out.status().notice.is_some());
        let err = out
            .open(&AudioOutputSpec {
                device_id: Some("hw:99,99".into()),
                info: info(16, PcmFormat::I32 { bits: 16 }),
                mode: OutputMode::Exclusive,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
            })
            .unwrap_err();
        assert!(matches!(err, crate::error::Error::Output(_)), "got {err:?}");
        assert_eq!(out.status(), OutputStatus::default());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn enumerate_reports_at_least_one_device() {
        let devices = enumerate().unwrap();
        assert!(!devices.is_empty());
    }

    #[test]
    fn software_volume_notice_constant_matches_wasapi() {
        assert_eq!(
            SOFTWARE_VOLUME_NOTICE,
            "volume applied in software; bit-perfect compromised"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_fallback_survives_a_volume_change() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Exclusive,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.set_volume(0.5).unwrap();
        let status = out.status();
        assert_eq!(status.mode, OutputMode::Shared);
        assert!(!status.bit_perfect);
        // The fallback notice must not be replaced by the software-volume
        // notice, which only applies to a real exclusive session.
        assert_eq!(
            status.notice.as_deref(),
            Some("exclusive mode unavailable; using shared output")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_expired_write_deadline_reports_a_stall() {
        let pcm = alsa::pcm::PCM::new("null", alsa::Direction::Playback, false).unwrap();
        let err = super::linux::wait_writable(&pcm, std::time::Instant::now()).unwrap_err();
        assert!(matches!(err, crate::error::Error::Output(_)), "got {err:?}");
        assert_eq!(err.to_string(), "audio device stalled");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shared_volume_keeps_its_status_clean() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Shared,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.set_volume(0.5).unwrap();
        assert_eq!(out.status().notice, None);
        assert!(!out.status().bit_perfect);
    }
}
