use crate::decode::{PcmFormat, StreamInfo};
use crate::error::{Error, Result};
use crate::output::{exclusive_formats, ExclusiveFallback, OutputDevice};

/// True when `snd_pcm_start` answered `-EPIPE`: a start with no frames queued,
/// which the first `write` recovers by starting the stream itself.
#[cfg(target_os = "linux")]
pub fn is_start_underrun(error: &alsa::Error) -> bool {
    std::io::Error::from_raw_os_error(error.errno()).kind() == std::io::ErrorKind::BrokenPipe
}

/// Periods the ALSA open asks for.
pub const ALSA_PERIODS: u32 = 4;

/// Frames the ALSA open asks for: half a second, at least 1024, rounded down
/// to a multiple of [`ALSA_PERIODS`]. `snd_pcm_hw_params_set_periods` returns
/// EINVAL when the period count does not divide the buffer, so a raw half
/// second (22050 frames at 44.1 kHz) made every 44.1 kHz open fail and fall
/// back to shared output.
pub fn buffer_frames(sample_rate: u32) -> i64 {
    ((sample_rate as i64 / 2).max(1024) / ALSA_PERIODS as i64) * ALSA_PERIODS as i64
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
    desc.map(str::trim)
        .filter(|desc| !desc.is_empty())
        .unwrap_or(name)
        .to_string()
}

/// Collapse hints into the picker list: one entry per ALSA device name, the
/// default first, then the rest by id.
pub fn device_list(hints: &[Hint]) -> Vec<OutputDevice> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out: Vec<OutputDevice> = hints
        .iter()
        .filter(|hint| hint_is_output(hint) && hint_is_pickable(&hint.name))
        .filter(|hint| seen.insert(hint.name.clone()))
        .map(|hint| OutputDevice {
            id: hint.name.clone(),
            name: hint_label(&hint.name, hint.desc.as_deref()),
            is_default: hint.name == "default",
        })
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

    use super::*;
    use crate::decode::PcmBuffer;
    use crate::output::{AudioOutput, AudioOutputSpec, OutputDevice, OutputMode, OutputStatus};

    /// Maximum wall-clock time one `write` call may spend on the device before
    /// it is declared stalled.
    const WRITE_DEADLINE: Duration = Duration::from_secs(2);
    /// Longest single poll wait; each wake-up re-checks the deadline.
    const WAIT_SLICE: Duration = Duration::from_millis(100);
    /// Consecutive xrun/suspend recoveries tolerated within one `write` call.
    const MAX_CONSECUTIVE_RECOVERIES: u32 = 8;

    pub struct AlsaOutput {
        pcm: Option<PCM>,
        format: AlsaSampleFormat,
        device: String,
        channels: u16,
        volume: f32,
        software_volume: bool,
        status: OutputStatus,
    }

    impl AlsaOutput {
        pub fn new() -> Self {
            Self {
                pcm: None,
                format: AlsaSampleFormat::S32Le,
                device: String::new(),
                channels: 2,
                volume: 1.0,
                software_volume: true,
                status: OutputStatus {
                    mode: OutputMode::Shared,
                    bit_perfect: false,
                    notice: None,
                },
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
                hwp.set_buffer_size_near(buffer_frames(spec.info.sample_rate))
                    .map_err(to_error)?;
                hwp.set_periods(ALSA_PERIODS, ValueOr::Nearest).map_err(to_error)?;
                pcm.hw_params(&hwp).map_err(to_error)?;
            }
            pcm.prepare().map_err(to_error)?;
            self.pcm = Some(pcm);
            self.format = format;
            self.device = name.to_string();
            self.channels = spec.info.channels;
            Ok(())
        }

        fn close(&mut self) {
            if let Some(pcm) = self.pcm.take() {
                let _ = pcm.drop();
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

    fn to_error(error: AlsaError) -> Error {
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
        Ok(device_list(&hints))
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
            self.close();
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
                            // Keep the plugin device the user selected; a
                            // failed direct `hw:` open falls back to the
                            // system default (which can be shared).
                            let fallback =
                                if is_direct_hw(&device) { "default" } else { device.as_str() };
                            self.open_pcm(fallback, AlsaSampleFormat::S32Le, spec)?;
                            status.notice =
                                Some("exclusive mode unavailable; using shared output".into());
                        }
                        AlsaOpen::Fail | AlsaOpen::Exclusive => {
                            return Err(Error::Output(
                                "exclusive mode is not supported by this output device".into(),
                            ));
                        }
                    },
                }
            } else {
                self.open_pcm(&device, AlsaSampleFormat::S32Le, spec)?;
            }

            self.status = status;
            self.set_volume(self.volume)?;
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
                Err(error) if is_start_underrun(&error) => {
                    tracing::debug!(target: "alsa", "start underran on {}; the first write starts it", self.device);
                    pcm.prepare().map_err(to_error)
                }
                Err(error) => Err(to_error(error)),
            }
        }

        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            let Some(pcm) = self.pcm.as_ref() else {
                return Err(Error::Output("output not open".into()));
            };
            let software = self.software_volume && (self.volume - 1.0).abs() >= f32::EPSILON;
            let samples: Vec<i32> = match buf {
                PcmBuffer::I32(v) => {
                    if software {
                        v.iter()
                            .map(|&sample| (sample as f64 * self.volume as f64) as i32)
                            .collect()
                    } else {
                        v.clone()
                    }
                }
                PcmBuffer::F32(v) => v
                    .iter()
                    .map(|&sample| {
                        let scaled = if software { sample * self.volume } else { sample };
                        f32_to_i32_one(scaled)
                    })
                    .collect(),
            };

            let mut bytes = Vec::with_capacity(samples.len() * self.format.store_bits() as usize / 8);
            pack_i32(self.format, &samples, &mut bytes);
            if bytes.is_empty() {
                return Ok(());
            }
            let frame_bytes =
                self.format.store_bits() as usize / 8 * self.channels as usize;
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
                        // xrun / suspend: recover and retry; unrecoverable
                        // errors are returned by `try_recover`.
                        pcm.try_recover(error, true).map_err(to_error)?;
                    }
                }
            }
            Ok(())
        }

        fn set_volume(&mut self, volume: f32) -> Result<()> {
            self.volume = volume.clamp(0.0, 1.0);
            let exclusive = self.status.mode == OutputMode::Exclusive;
            if exclusive {
                if set_hardware_volume(&self.device, self.volume).is_ok() {
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
            // Discard ring and device buffers but keep the negotiated handle,
            // so a seek does not release and re-lock an exclusive DAC.
            if let Some(pcm) = self.pcm.as_ref() {
                pcm.drop().map_err(to_error)?;
                pcm.prepare().map_err(to_error)?;
            }
            Ok(())
        }

        fn stop(&mut self) -> Result<()> {
            self.close();
            Ok(())
        }

        fn status(&self) -> OutputStatus {
            self.status.clone()
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
        assert!(is_start_underrun(&AlsaError::new("snd_pcm_start", 32)));
        assert!(!is_start_underrun(&AlsaError::new("snd_pcm_start", 19)));
        assert!(!is_start_underrun(&AlsaError::new("snd_pcm_start", 0)));
    }

    #[test]
    fn buffer_frames_divide_evenly_into_periods_at_every_rate() {
        // 44.1 kHz is the case that used to fail: a raw half second is 22050
        // frames, which four periods cannot split, and ALSA answered EINVAL.
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 8_000] {
            let frames = buffer_frames(rate);
            assert_eq!(frames % ALSA_PERIODS as i64, 0, "{frames} frames at {rate}");
            assert!(frames >= 1024, "{frames} frames at {rate}");
            assert!(frames >= rate as i64 / 2 - ALSA_PERIODS as i64, "{frames} at {rate}");
        }
        assert_eq!(buffer_frames(44_100), 22_048);
        assert_eq!(buffer_frames(48_000), 24_000);
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
        ]);
        let ids: Vec<&str> = list.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["default", "hw:CARD=X,DEV=0", "sysdefault:CARD=X"]);
    }

    #[test]
    fn device_list_marks_default_and_dedupes() {
        let list = device_list(&[
            hint("hw:CARD=X,DEV=0", Some("Direct"), Some("Output")),
            hint("default", Some("Default"), None),
            hint("default", Some("Default again"), None),
        ]);
        assert!(list[0].is_default);
        assert_eq!(list[0].name, "Default");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn device_list_falls_back_to_the_device_name_as_label() {
        let list = device_list(&[hint("sysdefault:CARD=Y", None, None)]);
        assert_eq!(list[0].name, "sysdefault:CARD=Y");
        assert_eq!(list[0], OutputDevice {
            id: "sysdefault:CARD=Y".into(),
            name: "sysdefault:CARD=Y".into(),
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
        out.flush().unwrap();
        out.stop().unwrap();
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
