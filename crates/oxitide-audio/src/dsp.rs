//! Opt-in sound processing: ReplayGain, a 10-band graphic EQ, and the
//! crossfade primitives. Every stage reports when it would leave samples
//! untouched so the engine can skip it and keep the bit-perfect path exact.

use oxitide_tidal::stream::ReplayGain;
use serde::{Deserialize, Serialize};

use crate::decode::{PcmBuffer, StreamInfo};

/// ISO octave band centres, in Hz.
pub const EQ_BANDS_HZ: [f64; 10] =
    [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
/// Band and EQ preamp range, in dB (symmetric).
pub const EQ_MAX_DB: f32 = 12.0;
/// ReplayGain preamp / untagged gain range, in dB (symmetric).
pub const REPLAY_GAIN_MAX_DB: f32 = 15.0;
/// Longest crossfade, in milliseconds.
pub const CROSSFADE_MAX_MS: u32 = 12_000;
/// One octave per band.
const EQ_Q: f64 = std::f64::consts::SQRT_2;
/// Bands at or above this fraction of the sample rate are skipped.
const EQ_NYQUIST_GUARD: f64 = 0.45;
/// Linear gains this close to 1.0 are treated as untouched.
const UNITY_EPSILON: f64 = 1e-6;
const I32_SCALE: f64 = 2_147_483_648.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EqSettings {
    pub enabled: bool,
    /// Use `-max(0, loudest band)` as the preamp so boosts cannot clip.
    pub auto_preamp: bool,
    pub preamp_db: f32,
    /// One gain per `EQ_BANDS_HZ` entry.
    pub bands_db: Vec<f32>,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_preamp: true,
            preamp_db: 0.0,
            bands_db: vec![0.0; EQ_BANDS_HZ.len()],
        }
    }
}

impl EqSettings {
    /// The preamp actually applied, in dB.
    pub fn effective_preamp_db(&self) -> f32 {
        if self.auto_preamp {
            let loudest = self.bands_db.iter().copied().fold(0.0f32, f32::max);
            -loudest
        } else {
            self.preamp_db
        }
    }

    /// True when the EQ would leave every sample untouched.
    pub fn is_identity(&self) -> bool {
        !self.enabled
            || (self.bands_db.iter().all(|&g| g == 0.0) && self.effective_preamp_db() == 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayGainMode {
    #[default]
    Off,
    Track,
    /// Album gain, falling back to the track gain.
    Album,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplayGainSettings {
    pub mode: ReplayGainMode,
    /// Added to the tagged gain.
    pub preamp_db: f32,
    /// Applied to tracks without ReplayGain data.
    pub untagged_db: f32,
    /// Cap the gain so the track's peak cannot exceed full scale.
    pub prevent_clipping: bool,
}

impl Default for ReplayGainSettings {
    fn default() -> Self {
        Self {
            mode: ReplayGainMode::Off,
            preamp_db: 0.0,
            untagged_db: 0.0,
            prevent_clipping: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossfadeSettings {
    /// 0 disables crossfade.
    pub duration_ms: u32,
    /// Keep consecutive tracks of the same album gapless.
    pub skip_same_album: bool,
    /// AutoMix Phase 1: overlap audible audio, skipping silence at the
    /// outgoing end and the incoming start.
    pub trim_silence: bool,
    /// AutoMix Phase 1: overlap tracks of different sample rates or channel
    /// counts by resampling the held-back tail instead of fading it out.
    pub resample: bool,
    /// AutoMix Phase 2: align the overlap beat-to-beat when both tracks have a
    /// confident beat grid and compatible tempos.
    pub beat_match: bool,
}

impl Default for CrossfadeSettings {
    fn default() -> Self {
        Self {
            duration_ms: 0,
            skip_same_album: true,
            trim_silence: false,
            resample: false,
            beat_match: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DspSettings {
    pub eq: EqSettings,
    pub replay_gain: ReplayGainSettings,
    pub crossfade: CrossfadeSettings,
}

fn clamp_db(value: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-max, max)
    } else {
        0.0
    }
}

impl DspSettings {
    /// Clamp every value into its documented range; non-finite values become 0
    /// and the band list is padded/truncated to the band count.
    pub fn sanitized(mut self) -> Self {
        self.eq.preamp_db = clamp_db(self.eq.preamp_db, EQ_MAX_DB);
        self.eq.bands_db.resize(EQ_BANDS_HZ.len(), 0.0);
        for gain in &mut self.eq.bands_db {
            *gain = clamp_db(*gain, EQ_MAX_DB);
        }
        self.replay_gain.preamp_db = clamp_db(self.replay_gain.preamp_db, REPLAY_GAIN_MAX_DB);
        self.replay_gain.untagged_db = clamp_db(self.replay_gain.untagged_db, REPLAY_GAIN_MAX_DB);
        self.crossfade.duration_ms = self.crossfade.duration_ms.min(CROSSFADE_MAX_MS);
        self
    }
}

fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// The linear ReplayGain factor for a track under `settings`.
pub fn replay_gain_linear(settings: &ReplayGainSettings, data: &ReplayGain) -> f64 {
    let (gain, peak) = match settings.mode {
        ReplayGainMode::Off => return 1.0,
        ReplayGainMode::Track => (data.track_gain_db, data.track_peak),
        ReplayGainMode::Album => match data.album_gain_db {
            Some(gain) => (Some(gain), data.album_peak),
            None => (data.track_gain_db, data.track_peak),
        },
    };
    let mut linear = match gain {
        Some(gain) if gain.is_finite() => db_to_linear(gain as f64 + settings.preamp_db as f64),
        _ => return db_to_linear(settings.untagged_db as f64),
    };
    if settings.prevent_clipping {
        if let Some(peak) = peak.filter(|p| p.is_finite() && *p > 0.0) {
            linear = linear.min(1.0 / peak as f64);
        }
    }
    linear
}

/// RBJ peaking-EQ coefficients, normalized by `a0`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Coeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Coeffs {
    fn peaking(rate: u32, freq: f64, gain_db: f64, q: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * freq / rate as f64;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }
}

/// Transposed Direct Form II state for one channel of one band.
#[derive(Debug, Clone, Copy, Default)]
struct BiquadState {
    z1: f64,
    z2: f64,
}

impl BiquadState {
    #[inline]
    fn run(&mut self, c: &Coeffs, x: f64) -> f64 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }
}

/// A graphic EQ for one stream. Bands at 0 dB cost nothing; retuning keeps the
/// filter state so live changes do not click.
struct Equalizer {
    rate: u32,
    channels: usize,
    /// Active `(band, coefficients)` pairs.
    active: Vec<(usize, Coeffs)>,
    /// `state[band * channels + channel]`, kept for every band.
    state: Vec<BiquadState>,
}

impl Equalizer {
    fn new(rate: u32, channels: usize) -> Self {
        Self {
            rate,
            channels,
            active: Vec::new(),
            state: vec![BiquadState::default(); EQ_BANDS_HZ.len() * channels],
        }
    }

    fn retune(&mut self, bands_db: &[f32]) {
        let limit = self.rate as f64 * EQ_NYQUIST_GUARD;
        let previous: Vec<usize> = self.active.iter().map(|(band, _)| *band).collect();
        self.active = EQ_BANDS_HZ
            .iter()
            .zip(bands_db)
            .enumerate()
            .filter(|(_, (&freq, &gain))| gain != 0.0 && freq < limit)
            .map(|(band, (&freq, &gain))| (band, Coeffs::peaking(self.rate, freq, gain as f64, EQ_Q)))
            .collect();
        // A band that was off has stale state from an older run; start it clean.
        for (band, _) in &self.active {
            if !previous.contains(band) {
                for ch in 0..self.channels {
                    self.state[band * self.channels + ch] = BiquadState::default();
                }
            }
        }
    }

    fn is_identity(&self) -> bool {
        self.active.is_empty()
    }

    #[inline]
    fn run(&mut self, channel: usize, mut x: f64) -> f64 {
        for (band, coeffs) in &self.active {
            x = self.state[band * self.channels + channel].run(coeffs, x);
        }
        x
    }
}

/// Which stages alter samples, for the bit-perfect notice.
pub const STAGE_REPLAY_GAIN: &str = "ReplayGain";
pub const STAGE_EQ: &str = "EQ";
pub const STAGE_CROSSFADE: &str = "crossfade";

/// The per-track processing chain: ReplayGain, then EQ preamp, then EQ bands.
pub struct TrackDsp {
    channels: usize,
    replay_gain: ReplayGain,
    rg_linear: f64,
    rg_on: bool,
    eq_preamp: f64,
    eq: Equalizer,
}

impl TrackDsp {
    pub fn new(info: &StreamInfo, replay_gain: ReplayGain, settings: &DspSettings) -> Self {
        let channels = info.channels.max(1) as usize;
        let mut dsp = Self {
            channels,
            replay_gain,
            rg_linear: 1.0,
            rg_on: false,
            eq_preamp: 1.0,
            eq: Equalizer::new(info.sample_rate.max(1), channels),
        };
        dsp.update(settings);
        dsp
    }

    /// Apply new settings without resetting the filter state.
    pub fn update(&mut self, settings: &DspSettings) {
        self.rg_on = settings.replay_gain.mode != ReplayGainMode::Off;
        self.rg_linear = replay_gain_linear(&settings.replay_gain, &self.replay_gain);
        if settings.eq.enabled {
            self.eq_preamp = db_to_linear(settings.eq.effective_preamp_db() as f64);
            self.eq.retune(&settings.eq.bands_db);
        } else {
            self.eq_preamp = 1.0;
            self.eq.retune(&[]);
        }
    }

    fn gain(&self) -> f64 {
        self.rg_linear * self.eq_preamp
    }

    /// The ReplayGain actually applied, in dB, while ReplayGain is on.
    pub fn replay_gain_db(&self) -> Option<f32> {
        self.rg_on.then(|| (20.0 * self.rg_linear.log10()) as f32)
    }

    /// Stages that currently alter samples.
    pub fn active_stages(&self) -> Vec<&'static str> {
        let mut stages = Vec::new();
        if (self.rg_linear - 1.0).abs() > UNITY_EPSILON {
            stages.push(STAGE_REPLAY_GAIN);
        }
        if (self.eq_preamp - 1.0).abs() > UNITY_EPSILON || !self.eq.is_identity() {
            stages.push(STAGE_EQ);
        }
        stages
    }

    pub fn is_identity(&self) -> bool {
        (self.gain() - 1.0).abs() <= UNITY_EPSILON && self.eq.is_identity()
    }

    #[inline]
    fn sample(&mut self, channel: usize, x: f64, gain: f64) -> f64 {
        self.eq.run(channel, x * gain).clamp(-1.0, 1.0)
    }

    /// Process one interleaved buffer, keeping its representation. An identity
    /// chain returns the buffer untouched.
    pub fn process(&mut self, buf: PcmBuffer) -> PcmBuffer {
        if self.is_identity() {
            return buf;
        }
        let gain = self.gain();
        let channels = self.channels;
        match buf {
            PcmBuffer::F32(mut v) => {
                for (i, s) in v.iter_mut().enumerate() {
                    *s = self.sample(i % channels, *s as f64, gain) as f32;
                }
                PcmBuffer::F32(v)
            }
            PcmBuffer::I32(mut v) => {
                for (i, s) in v.iter_mut().enumerate() {
                    *s = f64_to_i32(self.sample(i % channels, *s as f64 / I32_SCALE, gain));
                }
                PcmBuffer::I32(v)
            }
        }
    }
}

/// A left-aligned 32-bit sample from a float in [-1, 1].
#[inline]
pub fn f64_to_i32(x: f64) -> i32 {
    (x * I32_SCALE).round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

/// Equal-power crossfade gains at `t` in [0, 1]: `(outgoing, incoming)`.
/// `out² + in² = 1` throughout, so uncorrelated tracks keep their loudness.
pub fn equal_power(t: f64) -> (f64, f64) {
    let angle = t.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}

#[inline]
fn sample_f64(buf: &PcmBuffer, index: usize) -> f64 {
    match buf {
        PcmBuffer::F32(v) => v[index] as f64,
        PcmBuffer::I32(v) => v[index] as f64 / I32_SCALE,
    }
}

/// The held-back end of a track, faded out across the start of the next one.
pub struct Tail {
    /// Interleaved, normalized samples.
    samples: Vec<f64>,
    channels: usize,
    /// Frames already mixed or written.
    cursor: usize,
    /// True when the source buffers were `i32` (exclusive output).
    int: bool,
}

impl Tail {
    pub fn new(buffers: &[PcmBuffer], channels: u16) -> Self {
        let mut samples = Vec::with_capacity(buffers.iter().map(PcmBuffer::len).sum());
        for buf in buffers {
            samples.extend((0..buf.len()).map(|i| sample_f64(buf, i)));
        }
        let int = matches!(buffers.first(), Some(PcmBuffer::I32(_)));
        Self { samples, channels: channels.max(1) as usize, cursor: 0, int }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }

    pub fn is_done(&self) -> bool {
        self.cursor >= self.frames()
    }

    fn gains(&self, frame: usize) -> (f64, f64) {
        equal_power(frame as f64 / self.frames().max(1) as f64)
    }

    /// Mix the tail into the start of `next` (the incoming track's samples),
    /// advancing through the fade. Frames past the tail's end are untouched.
    pub fn mix_into(&mut self, next: PcmBuffer) -> PcmBuffer {
        let channels = self.channels;
        let frames = self.frames();
        let mix = |tail: &Self, index: usize, value: f64| -> f64 {
            let frame = tail.cursor + index / channels;
            if frame >= frames {
                return value;
            }
            let (out_gain, in_gain) = tail.gains(frame);
            let old = tail.samples[frame * channels + index % channels];
            (value * in_gain + old * out_gain).clamp(-1.0, 1.0)
        };
        let mixed = match next {
            PcmBuffer::F32(mut v) => {
                for (i, s) in v.iter_mut().enumerate() {
                    *s = mix(self, i, *s as f64) as f32;
                }
                PcmBuffer::F32(v)
            }
            PcmBuffer::I32(mut v) => {
                for (i, s) in v.iter_mut().enumerate() {
                    if self.cursor + i / channels < frames {
                        *s = f64_to_i32(mix(self, i, *s as f64 / I32_SCALE));
                    }
                }
                PcmBuffer::I32(v)
            }
        };
        self.cursor = (self.cursor + mixed.len() / channels).min(frames);
        mixed
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Interleaved normalized samples not yet mixed or written.
    pub fn remaining(&self) -> &[f64] {
        &self.samples[self.cursor * self.channels..]
    }

    fn to_buffer(&self, values: impl Iterator<Item = f64>) -> PcmBuffer {
        // Exact for the source representation: i32/2^31 and f32 both survive
        // the f64 round trip, so an unmixed preroll stays bit-perfect.
        if self.int {
            PcmBuffer::I32(values.map(f64_to_i32).collect())
        } else {
            PcmBuffer::F32(values.map(|x| x as f32).collect())
        }
    }

    /// Remove and return the next `frames` frames unmixed, in the tail's own
    /// representation (the outgoing track's audio before the overlap).
    pub fn take_front(&mut self, frames: usize) -> PcmBuffer {
        let start = self.cursor * self.channels;
        let end = (start + frames * self.channels).min(self.samples.len());
        let out = self.to_buffer(self.samples[start..end].iter().copied());
        self.samples.drain(start..end);
        out
    }

    /// Keep at most `frames` remaining frames; the rest is dropped.
    pub fn truncate(&mut self, frames: usize) {
        let keep = (self.cursor + frames) * self.channels;
        self.samples.truncate(keep.min(self.samples.len()));
    }

    /// The remaining tail at another rate and channel count, so it can
    /// overlap a track whose output format differs. Channels map first (mono
    /// averages, extra channels repeat the last source channel), then an FFT
    /// resampler converts the rate.
    pub fn converted(&self, from_rate: u32, to_rate: u32, to_channels: usize) -> Tail {
        let src = self.remaining();
        let from_ch = self.channels;
        let to_ch = to_channels.max(1);
        let frames = src.len() / from_ch;
        let mut planar: Vec<Vec<f64>> = vec![Vec::with_capacity(frames); to_ch];
        for frame in src.chunks(from_ch) {
            for (c, plane) in planar.iter_mut().enumerate() {
                let value = if to_ch == 1 {
                    frame.iter().sum::<f64>() / from_ch as f64
                } else {
                    frame[c.min(from_ch - 1)]
                };
                plane.push(value);
            }
        }
        let planar = if from_rate == to_rate || frames == 0 {
            planar
        } else {
            resample_planar(&planar, from_rate, to_rate)
        };
        let out_frames = planar.first().map_or(0, Vec::len);
        let mut samples = Vec::with_capacity(out_frames * to_ch);
        for i in 0..out_frames {
            for plane in &planar {
                samples.push(plane[i].clamp(-1.0, 1.0));
            }
        }
        Tail { samples, channels: to_ch, cursor: 0, int: self.int }
    }

    /// Whatever is left of the tail, faded out on its own (no incoming track
    /// to overlap), in the representation it arrived in.
    pub fn fade_out_remaining(&mut self) -> PcmBuffer {
        let channels = self.channels;
        let start = self.cursor;
        let values = (start * channels..self.samples.len()).map(|i| {
            let (out_gain, _) = self.gains(i / channels);
            self.samples[i] * out_gain
        });
        let out = self.to_buffer(values);
        self.cursor = self.frames();
        out
    }
}

/// Offline rate conversion of whole planar channels (an FFT resampler fed in
/// fixed chunks, flushed, with its delay removed).
fn resample_planar(input: &[Vec<f64>], from_rate: u32, to_rate: u32) -> Vec<Vec<f64>> {
    use rubato::{FftFixedIn, Resampler};
    const CHUNK: usize = 1024;
    let channels = input.len();
    let frames = input[0].len();
    let expected = (frames as f64 * to_rate as f64 / from_rate as f64).round() as usize;
    let Ok(mut resampler) =
        FftFixedIn::<f64>::new(from_rate as usize, to_rate as usize, CHUNK, 2, channels)
    else {
        return input.to_vec();
    };
    let delay = resampler.output_delay();
    let mut out: Vec<Vec<f64>> = vec![Vec::with_capacity(expected + delay + CHUNK); channels];
    let append = |out: &mut Vec<Vec<f64>>, chunk: Vec<Vec<f64>>| {
        for (plane, part) in out.iter_mut().zip(chunk) {
            plane.extend(part);
        }
    };
    let mut pos = 0;
    while pos + resampler.input_frames_next() <= frames {
        let n = resampler.input_frames_next();
        let slices: Vec<&[f64]> = input.iter().map(|c| &c[pos..pos + n]).collect();
        match resampler.process(&slices, None) {
            Ok(chunk) => append(&mut out, chunk),
            Err(_) => return input.to_vec(),
        }
        pos += n;
    }
    if pos < frames {
        let slices: Vec<&[f64]> = input.iter().map(|c| &c[pos..]).collect();
        if let Ok(chunk) = resampler.process_partial(Some(&slices), None) {
            append(&mut out, chunk);
        }
    }
    // Flush the resampler's internal delay so the end of the tail survives.
    while out[0].len() < expected + delay {
        match resampler.process_partial::<&[f64]>(None, None) {
            Ok(chunk) if !chunk[0].is_empty() => append(&mut out, chunk),
            _ => break,
        }
    }
    out.into_iter()
        .map(|plane| plane.into_iter().skip(delay).take(expected).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::PcmFormat;

    fn info(rate: u32) -> StreamInfo {
        StreamInfo { sample_rate: rate, channels: 2, bits_per_sample: 16, format: PcmFormat::F32 }
    }

    fn sine(rate: u32, freq: f64, frames: usize, amp: f64) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * 2);
        for n in 0..frames {
            let s = (amp * (2.0 * std::f64::consts::PI * freq * n as f64 / rate as f64).sin()) as f32;
            out.push(s);
            out.push(s);
        }
        out
    }

    /// RMS of the left channel over the second half (filters have settled).
    fn rms_tail(v: &[f32]) -> f64 {
        let left: Vec<f64> = v.iter().step_by(2).map(|&s| s as f64).collect();
        let tail = &left[left.len() / 2..];
        (tail.iter().map(|s| s * s).sum::<f64>() / tail.len() as f64).sqrt()
    }

    fn eq_settings(band: usize, gain: f32) -> DspSettings {
        let mut settings = DspSettings::default();
        settings.eq.enabled = true;
        settings.eq.auto_preamp = false;
        settings.eq.bands_db[band] = gain;
        settings
    }

    #[test]
    fn defaults_are_identity_and_leave_buffers_untouched() {
        let settings = DspSettings::default();
        assert!(settings.eq.is_identity());
        let mut dsp = TrackDsp::new(&info(44100), ReplayGain::default(), &settings);
        assert!(dsp.is_identity());
        assert!(dsp.active_stages().is_empty());
        let buf = PcmBuffer::I32(vec![i32::MIN, -1, 0, 1, i32::MAX, 123_456_789]);
        assert_eq!(dsp.process(buf.clone()), buf);
    }

    #[test]
    fn enabled_flat_eq_is_identity() {
        let mut settings = DspSettings::default();
        settings.eq.enabled = true;
        let dsp = TrackDsp::new(&info(44100), ReplayGain::default(), &settings);
        assert!(dsp.is_identity());
    }

    #[test]
    fn peaking_band_boosts_its_centre_by_its_gain() {
        for (gain, rate) in [(6.0f32, 48000u32), (-6.0, 44100), (12.0, 192000)] {
            let mut dsp = TrackDsp::new(&info(rate), ReplayGain::default(), &eq_settings(5, gain));
            let input = sine(rate, 1000.0, rate as usize / 2, 0.1);
            let out = dsp.process(PcmBuffer::F32(input.clone()));
            let ratio_db = 20.0 * (rms_tail(out.as_f32().unwrap()) / rms_tail(&input)).log10();
            assert!((ratio_db - gain as f64).abs() < 0.3, "{gain} dB at {rate}: got {ratio_db}");
        }
    }

    #[test]
    fn peaking_band_leaves_distant_frequencies_nearly_flat() {
        let mut dsp = TrackDsp::new(&info(48000), ReplayGain::default(), &eq_settings(0, 12.0));
        let input = sine(48000, 8000.0, 24000, 0.1);
        let out = dsp.process(PcmBuffer::F32(input.clone()));
        let ratio_db = 20.0 * (rms_tail(out.as_f32().unwrap()) / rms_tail(&input)).log10();
        assert!(ratio_db.abs() < 0.1, "8 kHz moved by {ratio_db} dB");
    }

    #[test]
    fn bands_near_nyquist_are_skipped() {
        let dsp = TrackDsp::new(&info(22050), ReplayGain::default(), &eq_settings(9, 6.0));
        assert!(dsp.is_identity(), "16 kHz must be skipped at 22.05 kHz");
    }

    #[test]
    fn auto_preamp_cancels_the_loudest_boost() {
        let mut eq = EqSettings { enabled: true, ..Default::default() };
        eq.bands_db[2] = 4.0;
        eq.bands_db[7] = 9.0;
        eq.bands_db[8] = -3.0;
        assert_eq!(eq.effective_preamp_db(), -9.0);
        eq.bands_db = vec![-2.0; 10];
        assert_eq!(eq.effective_preamp_db(), 0.0);
        eq.auto_preamp = false;
        eq.preamp_db = 2.5;
        assert_eq!(eq.effective_preamp_db(), 2.5);
    }

    #[test]
    fn replay_gain_modes_pick_the_right_tag() {
        let data = ReplayGain {
            track_gain_db: Some(-6.0),
            track_peak: Some(0.5),
            album_gain_db: Some(-3.0),
            album_peak: Some(0.5),
        };
        let mut s = ReplayGainSettings::default();
        assert_eq!(replay_gain_linear(&s, &data), 1.0);
        s.mode = ReplayGainMode::Track;
        assert!((replay_gain_linear(&s, &data) - db_to_linear(-6.0)).abs() < 1e-9);
        s.mode = ReplayGainMode::Album;
        assert!((replay_gain_linear(&s, &data) - db_to_linear(-3.0)).abs() < 1e-9);
        let track_only = ReplayGain { album_gain_db: None, album_peak: None, ..data };
        assert!((replay_gain_linear(&s, &track_only) - db_to_linear(-6.0)).abs() < 1e-9);
    }

    #[test]
    fn replay_gain_preamp_untagged_and_clipping() {
        let mut s = ReplayGainSettings {
            mode: ReplayGainMode::Track,
            preamp_db: 3.0,
            untagged_db: -5.0,
            prevent_clipping: true,
        };
        let quiet = ReplayGain { track_gain_db: Some(9.0), track_peak: Some(0.5), ..Default::default() };
        // +12 dB would push a 0.5 peak to ~2.0; the cap holds it at full scale.
        assert!((replay_gain_linear(&s, &quiet) - 2.0).abs() < 1e-9);
        s.prevent_clipping = false;
        assert!((replay_gain_linear(&s, &quiet) - db_to_linear(12.0)).abs() < 1e-9);
        assert!((replay_gain_linear(&s, &ReplayGain::default()) - db_to_linear(-5.0)).abs() < 1e-9);
    }

    #[test]
    fn replay_gain_scales_i32_samples_and_reports_its_stage() {
        let mut settings = DspSettings::default();
        settings.replay_gain.mode = ReplayGainMode::Track;
        let data = ReplayGain { track_gain_db: Some(-6.020_6), ..Default::default() };
        let mut dsp = TrackDsp::new(&info(44100), data, &settings);
        assert_eq!(dsp.active_stages(), vec![STAGE_REPLAY_GAIN]);
        assert!((dsp.replay_gain_db().unwrap() + 6.0206).abs() < 1e-3);
        let out = dsp.process(PcmBuffer::I32(vec![1 << 30, -(1 << 30)]));
        let v = out.as_i32().unwrap();
        assert!((v[0] - (1 << 29)).abs() < 1 << 12, "{}", v[0]);
        assert!((v[1] + (1 << 29)).abs() < 1 << 12, "{}", v[1]);
    }

    #[test]
    fn zero_db_replay_gain_stays_identity_but_reports_its_value() {
        let mut settings = DspSettings::default();
        settings.replay_gain.mode = ReplayGainMode::Track;
        let data = ReplayGain { track_gain_db: Some(0.0), ..Default::default() };
        let dsp = TrackDsp::new(&info(44100), data, &settings);
        assert!(dsp.is_identity());
        assert_eq!(dsp.replay_gain_db(), Some(0.0));
    }

    #[test]
    fn output_is_clamped_to_full_scale() {
        let mut settings = DspSettings::default();
        settings.replay_gain.mode = ReplayGainMode::Track;
        settings.replay_gain.prevent_clipping = false;
        let data = ReplayGain { track_gain_db: Some(12.0), track_peak: Some(1.0), ..Default::default() };
        let mut dsp = TrackDsp::new(&info(44100), data, &settings);
        let out = dsp.process(PcmBuffer::F32(vec![0.9, -0.9]));
        assert_eq!(out.as_f32().unwrap(), &[1.0, -1.0]);
        let out = dsp.process(PcmBuffer::I32(vec![i32::MAX / 2, i32::MIN / 2]));
        assert_eq!(out.as_i32().unwrap(), &[i32::MAX, i32::MIN]);
    }

    #[test]
    fn update_applies_live_and_can_return_to_identity() {
        let mut dsp = TrackDsp::new(&info(48000), ReplayGain::default(), &DspSettings::default());
        assert!(dsp.is_identity());
        dsp.update(&eq_settings(3, 3.0));
        assert_eq!(dsp.active_stages(), vec![STAGE_EQ]);
        dsp.update(&DspSettings::default());
        assert!(dsp.is_identity());
    }

    #[test]
    fn sanitized_clamps_ranges_and_repairs_the_band_list() {
        let mut settings = DspSettings::default();
        settings.eq.bands_db = vec![40.0, f32::NAN, -40.0];
        settings.eq.preamp_db = -99.0;
        settings.replay_gain.preamp_db = 99.0;
        settings.replay_gain.untagged_db = f32::INFINITY;
        settings.crossfade.duration_ms = 60_000;
        let s = settings.sanitized();
        assert_eq!(s.eq.bands_db.len(), 10);
        assert_eq!(&s.eq.bands_db[..3], &[12.0, 0.0, -12.0]);
        assert_eq!(s.eq.preamp_db, -12.0);
        assert_eq!(s.replay_gain.preamp_db, 15.0);
        assert_eq!(s.replay_gain.untagged_db, 0.0);
        assert_eq!(s.crossfade.duration_ms, CROSSFADE_MAX_MS);
    }

    #[test]
    fn settings_deserialize_from_partial_toml_shapes() {
        let s: DspSettings =
            serde_json::from_value(serde_json::json!({ "replay_gain": { "mode": "album" } }))
                .unwrap();
        assert_eq!(s.replay_gain.mode, ReplayGainMode::Album);
        assert!(s.replay_gain.prevent_clipping);
        assert!(s.crossfade.skip_same_album);
        assert_eq!(s.eq.bands_db.len(), 10);
    }

    #[test]
    fn equal_power_has_unit_power_and_correct_endpoints() {
        assert_eq!(equal_power(0.0), (1.0, 0.0));
        let (out, inc) = equal_power(1.0);
        assert!(out.abs() < 1e-12 && (inc - 1.0).abs() < 1e-12);
        for step in 0..=20 {
            let (out, inc) = equal_power(step as f64 / 20.0);
            assert!((out * out + inc * inc - 1.0).abs() < 1e-12);
        }
        assert_eq!(equal_power(-1.0), (1.0, 0.0));
    }

    #[test]
    fn tail_mixes_across_buffer_boundaries_then_passes_through() {
        // A 4-frame stereo tail of 0.5 against an incoming signal of 0.25.
        let tail_buf = PcmBuffer::F32(vec![0.5; 8]);
        let mut tail = Tail::new(&[tail_buf], 2);
        assert_eq!(tail.frames(), 4);
        let first = tail.mix_into(PcmBuffer::F32(vec![0.25; 6]));
        let second = tail.mix_into(PcmBuffer::F32(vec![0.25; 6]));
        assert!(tail.is_done());
        let all: Vec<f32> = [first.as_f32().unwrap(), second.as_f32().unwrap()].concat();
        for frame in 0..4 {
            let (out, inc) = equal_power(frame as f64 / 4.0);
            let want = (0.25 * inc + 0.5 * out) as f32;
            assert!((all[frame * 2] - want).abs() < 1e-6, "frame {frame}");
            assert_eq!(all[frame * 2], all[frame * 2 + 1]);
        }
        assert_eq!(&all[8..], &[0.25; 4], "frames past the tail are untouched");
    }

    #[test]
    fn tail_mixes_i32_and_leaves_later_samples_bit_exact() {
        let mut tail = Tail::new(&[PcmBuffer::I32(vec![1 << 30, 1 << 30])], 2);
        let out = tail.mix_into(PcmBuffer::I32(vec![0, 0, 12_345, -12_345]));
        let v = out.as_i32().unwrap();
        assert_eq!(v[0], 1 << 30, "t = 0 is all outgoing");
        assert_eq!(&v[2..], &[12_345, -12_345]);
    }

    #[test]
    fn tail_fades_out_what_is_left() {
        let mut tail = Tail::new(&[PcmBuffer::F32(vec![1.0; 8])], 2);
        let _ = tail.mix_into(PcmBuffer::F32(vec![0.0; 2]));
        let rest = tail.fade_out_remaining();
        let v = rest.as_f32().unwrap();
        assert_eq!(v.len(), 6);
        assert!(v.windows(2).step_by(2).all(|w| w[0] == w[1]));
        assert!(v[0] > v[2] && v[2] > v[4] && v[4] > 0.0, "{v:?}");
        assert!(tail.is_done());
        let i32_rest = Tail::new(&[PcmBuffer::I32(vec![1 << 30; 4])], 2).fade_out_remaining();
        assert_eq!(i32_rest.as_i32().unwrap()[0], 1 << 30);
    }

    #[test]
    fn take_front_is_bit_exact_and_truncate_drops_the_rest() {
        let source = vec![i32::MIN, -7, 123_456_789, i32::MAX, 1 << 20, -(1 << 20)];
        let mut tail = Tail::new(&[PcmBuffer::I32(source.clone())], 2);
        assert_eq!(tail.take_front(2), PcmBuffer::I32(source[..4].to_vec()));
        assert_eq!(tail.frames(), 1);
        tail.truncate(0);
        assert!(tail.is_done());

        let floats = vec![0.1f32, -0.2, 0.3, -0.4];
        let mut tail = Tail::new(&[PcmBuffer::F32(floats.clone())], 2);
        assert_eq!(tail.take_front(1), PcmBuffer::F32(floats[..2].to_vec()));
    }

    #[test]
    fn converted_maps_channels() {
        let tail = Tail::new(&[PcmBuffer::F32(vec![0.2, 0.4, -0.2, -0.4])], 2);
        let mono = tail.converted(44_100, 44_100, 1);
        assert_eq!(mono.channels(), 1);
        assert!(mono.remaining().iter().zip([0.3, -0.3]).all(|(a, b)| (a - b).abs() < 1e-6));
        let stereo = Tail::new(&[PcmBuffer::F32(vec![0.5, -0.5])], 1).converted(48_000, 48_000, 2);
        assert_eq!(stereo.remaining(), &[0.5, 0.5, -0.5, -0.5]);
    }

    /// Zero crossings per second of the first channel.
    fn frequency(samples: &[f64], channels: usize, rate: u32) -> f64 {
        let mono: Vec<f64> = samples.iter().step_by(channels).copied().collect();
        let skip = mono.len() / 10;
        let body = &mono[skip..mono.len() - skip];
        let crossings = body.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        crossings as f64 / (body.len() as f64 / rate as f64)
    }

    #[test]
    fn converted_resamples_length_and_pitch() {
        let input: Vec<f32> = sine(44_100, 440.0, 44_100 * 2, 0.5);
        let tail = Tail::new(&[PcmBuffer::F32(input)], 2);
        for to in [48_000u32, 96_000, 32_000] {
            let out = tail.converted(44_100, to, 2);
            let expected = 2 * to as usize;
            assert!(out.frames().abs_diff(expected) <= 1, "{to}: {} frames", out.frames());
            let f = frequency(out.remaining(), 2, to);
            assert!((f - 440.0).abs() < 2.0, "{to}: {f:.1} Hz");
        }
    }
}
