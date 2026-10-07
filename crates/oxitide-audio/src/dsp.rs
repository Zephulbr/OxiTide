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
}

impl Default for CrossfadeSettings {
    fn default() -> Self {
        Self { duration_ms: 0, skip_same_album: true }
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
}
