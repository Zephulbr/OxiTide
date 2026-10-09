//! Read-only level and spectrum tap for the Hi-Fi Rack meters (Track 9.5,
//! RK-D7). The playback worker pushes every buffer it writes to the output;
//! a reader (the app's meter thread) periodically takes per-channel peak/RMS
//! since its last read plus a log-band spectrum of the most recent audio.
//!
//! The tap only reads samples. `push` is a no-op while disabled and never
//! blocks: if the reader holds the lock, that buffer is simply not measured.

use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::decode::PcmBuffer;

/// Samples in the spectrum window (power of two for the radix-2 FFT).
pub const FFT_SIZE: usize = 2048;
/// Spectrum bands, log-spaced between [`BAND_LO_HZ`] and [`BAND_HI_HZ`].
pub const BANDS: usize = 28;
pub const BAND_LO_HZ: f32 = 40.0;
pub const BAND_HI_HZ: f32 = 16_000.0;
/// Level reported for silent or empty bands, in dBFS.
pub const FLOOR_DB: f32 = -96.0;
/// Stereo scope: L/R sample pairs per reading, taken every `SCOPE_STRIDE`
/// frames from the newest audio (~10 ms at 44.1 kHz).
pub const SCOPE_POINTS: usize = 160;
pub const SCOPE_STRIDE: usize = 3;

/// One meter reading. Peaks and RMS are linear (1.0 = full scale); bands are
/// dBFS relative to a full-scale sine; `scope` holds recent L/R sample pairs
/// (oldest first) for the stereo audio monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelFrame {
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    pub bands: [f32; BANDS],
    pub scope: Vec<[f32; 2]>,
}

impl LevelFrame {
    pub fn silent() -> Self {
        Self { peak: [0.0; 2], rms: [0.0; 2], bands: [FLOOR_DB; BANDS], scope: Vec::new() }
    }
}

struct TapState {
    rate: u32,
    peak: [f32; 2],
    sum_sq: [f64; 2],
    count: u64,
    /// Mono mix of the most recent samples (ring).
    ring: Vec<f32>,
    /// The same frames as L/R pairs (ring, shares `pos`/`filled`).
    stereo: Vec<[f32; 2]>,
    pos: usize,
    filled: usize,
}

pub struct LevelTap {
    enabled: AtomicBool,
    state: Mutex<TapState>,
}

impl Default for LevelTap {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelTap {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            state: Mutex::new(TapState {
                rate: 44_100,
                peak: [0.0; 2],
                sum_sq: [0.0; 2],
                count: 0,
                ring: vec![0.0; FFT_SIZE],
                stereo: vec![[0.0; 2]; FFT_SIZE],
                pos: 0,
                filled: 0,
            }),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            if let Ok(mut state) = self.state.lock() {
                state.peak = [0.0; 2];
                state.sum_sq = [0.0; 2];
                state.count = 0;
                state.filled = 0;
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Measure a buffer that was just written to the output. Mono duplicates to
    /// both sides; more than two channels use the first two.
    pub fn push(&self, buf: &PcmBuffer, channels: u16, rate: u32) {
        if !self.is_enabled() || channels == 0 {
            return;
        }
        let Ok(mut state) = self.state.try_lock() else {
            return;
        };
        if rate > 0 {
            state.rate = rate;
        }
        match buf {
            PcmBuffer::F32(samples) => accumulate(&mut state, samples, channels, |s| s),
            PcmBuffer::I32(samples) => {
                accumulate(&mut state, samples, channels, |s| s as f32 / 2_147_483_648.0)
            }
        }
    }

    /// Levels since the previous read plus the current spectrum; resets the
    /// level accumulators. Nothing pushed since the last read reads as silence.
    pub fn read(&self) -> LevelFrame {
        let (peak, rms, window, rate, scope) = {
            let Ok(mut state) = self.state.lock() else {
                return LevelFrame::silent();
            };
            if state.count == 0 {
                return LevelFrame::silent();
            }
            let n = state.count as f64;
            let rms = [(state.sum_sq[0] / n).sqrt() as f32, (state.sum_sq[1] / n).sqrt() as f32];
            let peak = state.peak;
            // Oldest to newest; a partly filled ring is zero-padded at the front.
            let mut window = vec![0.0f32; FFT_SIZE];
            let filled = state.filled;
            for (i, slot) in window.iter_mut().skip(FFT_SIZE - filled).enumerate() {
                *slot = state.ring[(state.pos + FFT_SIZE - filled + i) % FFT_SIZE];
            }
            // The newest frames as decimated L/R pairs, oldest first.
            let span = (SCOPE_POINTS * SCOPE_STRIDE).min(filled);
            let start = (state.pos + FFT_SIZE - span) % FFT_SIZE;
            let scope: Vec<[f32; 2]> = (0..span)
                .step_by(SCOPE_STRIDE)
                .map(|i| state.stereo[(start + i) % FFT_SIZE])
                .collect();
            state.peak = [0.0; 2];
            state.sum_sq = [0.0; 2];
            state.count = 0;
            (peak, rms, window, state.rate, scope)
        };
        LevelFrame { peak, rms, bands: spectrum_bands(&window, rate), scope }
    }
}

fn accumulate<T: Copy>(state: &mut TapState, samples: &[T], channels: u16, to_f32: impl Fn(T) -> f32) {
    let channels = channels as usize;
    for frame in samples.chunks_exact(channels) {
        let l = to_f32(frame[0]);
        let r = if channels >= 2 { to_f32(frame[1]) } else { l };
        state.peak[0] = state.peak[0].max(l.abs());
        state.peak[1] = state.peak[1].max(r.abs());
        state.sum_sq[0] += (l as f64) * (l as f64);
        state.sum_sq[1] += (r as f64) * (r as f64);
        state.count += 1;
        let pos = state.pos;
        state.ring[pos] = 0.5 * (l + r);
        state.stereo[pos] = [l, r];
        state.pos = (pos + 1) % FFT_SIZE;
        state.filled = (state.filled + 1).min(FFT_SIZE);
    }
}

/// In-place iterative radix-2 FFT. `re.len()` must be a power of two.
pub fn fft_in_place(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        let (w_im, w_re) = angle.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cur_re, mut cur_im) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let t_re = re[b] * cur_re - im[b] * cur_im;
                let t_im = re[b] * cur_im + im[b] * cur_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next_re = cur_re * w_re - cur_im * w_im;
                cur_im = cur_re * w_im + cur_im * w_re;
                cur_re = next_re;
            }
        }
        len <<= 1;
    }
}

/// `bands + 1` log-spaced edges in Hz; the top edge never passes Nyquist.
pub fn band_edges(rate: u32, bands: usize, lo: f32, hi: f32) -> Vec<f32> {
    let hi = hi.min(rate as f32 / 2.0).max(lo * 1.01);
    (0..=bands).map(|i| lo * (hi / lo).powf(i as f32 / bands as f32)).collect()
}

/// Hann-windowed spectrum of `window` summed into [`BANDS`] log bands, in
/// dBFS (a full-scale sine reads ~0 dB in its band).
pub fn spectrum_bands(window: &[f32], rate: u32) -> [f32; BANDS] {
    let n = window.len();
    let mut re: Vec<f32> = window
        .iter()
        .enumerate()
        .map(|(i, s)| s * 0.5 * (1.0 - (2.0 * PI * i as f32 / (n as f32 - 1.0)).cos()))
        .collect();
    let mut im = vec![0.0f32; n];
    fft_in_place(&mut re, &mut im);
    // A Hann window halves the coherent gain; scale so a sine of amplitude A
    // reads A at its bin.
    let scale = 4.0 / n as f32;
    let bin_hz = rate as f32 / n as f32;
    let edges = band_edges(rate, BANDS, BAND_LO_HZ, BAND_HI_HZ);
    let mut out = [FLOOR_DB; BANDS];
    for (band, slot) in out.iter_mut().enumerate() {
        let lo_bin = (edges[band] / bin_hz).floor() as usize;
        let hi_bin = ((edges[band + 1] / bin_hz).ceil() as usize).max(lo_bin + 1).min(n / 2);
        let mut energy = 0.0f32;
        for k in lo_bin.max(1)..hi_bin {
            let mag = (re[k] * re[k] + im[k] * im[k]).sqrt() * scale;
            energy += mag * mag;
        }
        let amp = energy.sqrt();
        *slot = if amp > 0.0 { (20.0 * amp.log10()).max(FLOOR_DB) } else { FLOOR_DB };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: u32, frames: usize, amp: f32, channels: u16) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * channels as usize);
        for i in 0..frames {
            let s = amp * (2.0 * PI * freq * i as f32 / rate as f32).sin();
            for _ in 0..channels {
                out.push(s);
            }
        }
        out
    }

    fn enabled_tap() -> LevelTap {
        let tap = LevelTap::new();
        tap.set_enabled(true);
        tap
    }

    #[test]
    fn full_scale_sine_reads_unit_peak_and_root_half_rms() {
        let tap = enabled_tap();
        tap.push(&PcmBuffer::F32(sine(1000.0, 48_000, 4800, 1.0, 2)), 2, 48_000);
        let frame = tap.read();
        for ch in 0..2 {
            assert!((frame.peak[ch] - 1.0).abs() < 1e-3, "peak {}", frame.peak[ch]);
            assert!((frame.rms[ch] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-2, "rms {}", frame.rms[ch]);
        }
    }

    #[test]
    fn i32_full_scale_maps_to_unit_peak() {
        let tap = enabled_tap();
        tap.push(&PcmBuffer::I32(vec![i32::MAX, i32::MIN, 0, 0]), 2, 44_100);
        let frame = tap.read();
        assert!((frame.peak[0] - 1.0).abs() < 1e-6);
        assert!((frame.peak[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scope_returns_the_newest_pairs_decimated_oldest_first() {
        let tap = enabled_tap();
        let frames = SCOPE_POINTS * SCOPE_STRIDE + 30;
        let samples: Vec<f32> = (0..frames).flat_map(|i| [i as f32, -(i as f32)]).collect();
        tap.push(&PcmBuffer::F32(samples), 2, 44_100);
        let scope = tap.read().scope;
        assert_eq!(scope.len(), SCOPE_POINTS);
        assert_eq!(scope[0], [30.0, -30.0]);
        assert_eq!(scope[1], [(30 + SCOPE_STRIDE) as f32, -((30 + SCOPE_STRIDE) as f32)]);
        assert!(tap.read().scope.is_empty(), "no new audio reads as an empty scope");
    }

    #[test]
    fn mono_duplicates_to_both_sides() {
        let tap = enabled_tap();
        tap.push(&PcmBuffer::F32(vec![0.5, -0.25]), 1, 44_100);
        let frame = tap.read();
        assert_eq!(frame.peak, [0.5, 0.5]);
    }

    #[test]
    fn extra_channels_use_the_first_two() {
        let tap = enabled_tap();
        tap.push(&PcmBuffer::F32(vec![0.1, 0.2, 0.9, 0.9]), 4, 44_100);
        let frame = tap.read();
        assert_eq!(frame.peak, [0.1, 0.2]);
    }

    #[test]
    fn read_resets_and_empty_reads_are_silent() {
        let tap = enabled_tap();
        tap.push(&PcmBuffer::F32(vec![0.5, 0.5]), 2, 44_100);
        assert!(tap.read().peak[0] > 0.0);
        assert_eq!(tap.read(), LevelFrame::silent());
    }

    #[test]
    fn disabled_tap_ignores_pushes() {
        let tap = LevelTap::new();
        tap.push(&PcmBuffer::F32(vec![0.5, 0.5]), 2, 44_100);
        assert_eq!(tap.read(), LevelFrame::silent());
    }

    #[test]
    fn push_never_waits_for_a_held_lock() {
        let tap = enabled_tap();
        let guard = tap.state.lock().unwrap();
        tap.push(&PcmBuffer::F32(vec![0.5, 0.5]), 2, 44_100);
        drop(guard);
        assert_eq!(tap.read(), LevelFrame::silent());
    }

    #[test]
    fn sine_lands_in_its_band() {
        let rate = 48_000;
        let tap = enabled_tap();
        tap.push(&PcmBuffer::F32(sine(1000.0, rate, FFT_SIZE, 0.5, 2)), 2, rate);
        let frame = tap.read();
        let edges = band_edges(rate, BANDS, BAND_LO_HZ, BAND_HI_HZ);
        let band = edges.windows(2).position(|e| e[0] <= 1000.0 && 1000.0 < e[1]).unwrap();
        let loudest = frame
            .bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(loudest, band);
        // Amplitude 0.5 ≈ -6 dBFS (window leakage keeps it within a couple dB).
        assert!((frame.bands[band] + 6.0).abs() < 2.5, "band level {}", frame.bands[band]);
    }

    #[test]
    fn band_edges_are_log_spaced_and_capped_at_nyquist() {
        let edges = band_edges(22_050, 4, 40.0, 16_000.0);
        assert_eq!(edges.len(), 5);
        assert!((edges[0] - 40.0).abs() < 1e-3);
        assert!((edges[4] - 11_025.0).abs() < 1e-1);
        let r1 = edges[1] / edges[0];
        let r2 = edges[2] / edges[1];
        assert!((r1 - r2).abs() < 1e-3);
    }

    #[test]
    fn fft_of_an_impulse_is_flat() {
        let mut re = vec![0.0f32; 8];
        let mut im = vec![0.0f32; 8];
        re[0] = 1.0;
        fft_in_place(&mut re, &mut im);
        for k in 0..8 {
            assert!((re[k] - 1.0).abs() < 1e-6 && im[k].abs() < 1e-6);
        }
    }
}
