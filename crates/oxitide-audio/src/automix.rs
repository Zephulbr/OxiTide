//! AutoMix analysis (Track 7b): silence bounds, beat grids, and transition
//! planning. Pure functions over in-memory audio — the engine supplies the
//! held-back tail of the outgoing track and a pre-read head of the next one.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

use crate::decode::PcmBuffer;
use crate::dsp::{extend_normalized, CrossfadeSettings, MixShape, Tail, TransitionStyle};

/// Short-term level analysis: block length and smoothing half-width (blocks).
const LEVEL_BLOCK_S: f64 = 0.05;
const LEVEL_SMOOTH: usize = 4;
/// The outgoing fade has started once the level stays this far under the body.
const FADE_DB: f64 = 4.0;
/// The outgoing track has died down once it stays this far under the body.
const QUIET_DB: f64 = 18.0;
/// An opening is "in" (or "cold" from its first 150 ms) this close to its body.
const ENTRY_DB: f64 = 6.0;
/// Quick overlaps never run longer than this.
const QUICK_MAX_S: f64 = 1.5;
/// Share of a quick overlap over which the incoming track comes up.
const QUICK_ATTACK: f64 = 0.35;
/// Most audible ending a blend cuts, and most intro (beyond silence) it skips.
const MAX_CUT_OUT_S: f64 = 12.0;
const MAX_CUT_IN_S: f64 = 8.0;
/// Width of the bass swap at the centre of a blend.
const SWAP_S: f64 = 0.25;
/// Outgoing tail held back beyond the crossfade, and incoming opening
/// pre-read, whenever AutoMix is on: enough for any style to be planned (and
/// replayed) from one transition.
const AUTOMIX_HOLD_MS: u32 = 14_000;
const AUTOMIX_HEAD_S: f64 = 20.0;
/// Audio kept around the overlap in a replay.
const REPLAY_CONTEXT_S: f64 = 6.0;

/// Peak level below which audio counts as silence (−54 dBFS).
const SILENCE_THRESHOLD: f64 = 0.001_995;
/// Longest leading silence trimmed from an incoming track, so quiet intros
/// (orchestral, shoegaze swells) are not eaten.
pub const MAX_LEAD_TRIM_S: f64 = 4.0;
/// Analysis runs at roughly this rate after box decimation.
const ANALYSIS_RATE: f64 = 11_025.0;
const FFT_SIZE: usize = 512;
const HOP: usize = 64;
const MIN_BPM: f64 = 60.0;
const MAX_BPM: f64 = 200.0;
const PRIOR_BPM: f64 = 120.0;
/// Log-Gaussian tempo prior width, in octaves.
const PRIOR_OCTAVES: f64 = 1.0;
/// Dynamic-programming penalty on deviating from the period.
const TIGHTNESS: f64 = 100.0;
/// Grids below this confidence never drive a beat-matched transition.
pub const CONFIDENCE_FLOOR: f64 = 0.35;
/// Largest relative period difference beat matching accepts without stretching.
const MAX_TEMPO_DIFF: f64 = 0.04;
/// Largest beat misalignment tolerated at either end of the overlap.
const MAX_DRIFT_S: f64 = 0.030;

/// Interleaved, normalized audio in memory.
#[derive(Debug, Clone, Copy)]
pub struct Clip<'a> {
    pub samples: &'a [f64],
    pub channels: usize,
    pub rate: u32,
}

impl<'a> Clip<'a> {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }

    /// The frames in `[start, end)`.
    fn slice(&self, start: usize, end: usize) -> Clip<'a> {
        let ch = self.channels.max(1);
        let end = end.min(self.frames());
        let start = start.min(end);
        Clip { samples: &self.samples[start * ch..end * ch], channels: ch, rate: self.rate }
    }
}

/// First audible frame and one past the last, or `None` for a silent clip.
pub fn audible_bounds(clip: &Clip) -> Option<(usize, usize)> {
    let ch = clip.channels.max(1);
    let loud = |frame: &[f64]| frame.iter().any(|s| s.abs() > SILENCE_THRESHOLD);
    let start = clip.samples.chunks(ch).position(loud)?;
    let end = clip.samples.chunks(ch).rposition(loud)? + 1;
    Some((start, end))
}

/// Short-term level in dB per `block` frames: `smooth` averages power over
/// ±200 ms, `raw` is the block alone.
struct Levels {
    smooth: Vec<f64>,
    raw: Vec<f64>,
    block: usize,
}

fn levels(clip: &Clip) -> Levels {
    let ch = clip.channels.max(1);
    let block = ((clip.rate as f64 * LEVEL_BLOCK_S) as usize).max(1);
    let power: Vec<f64> = clip
        .samples
        .chunks(block * ch)
        .map(|c| c.iter().map(|s| s * s).sum::<f64>() / c.len() as f64)
        .collect();
    let db = |p: f64| 10.0 * p.max(1e-12).log10();
    let smooth = (0..power.len())
        .map(|i| {
            let lo = i.saturating_sub(LEVEL_SMOOTH);
            let hi = (i + LEVEL_SMOOTH + 1).min(power.len());
            db(power[lo..hi].iter().sum::<f64>() / (hi - lo) as f64)
        })
        .collect();
    Levels { smooth, raw: power.iter().map(|&p| db(p)).collect(), block }
}

fn percentile(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// How an outgoing track ends, in frames of its (audible) tail.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ending {
    /// Where its level starts to stay `FADE_DB` under the body (a fade-out or
    /// the final chord's decay); the tail's end when it never drops.
    fade_start: usize,
    /// Where it stays `QUIET_DB` under the body: the track has died down.
    quiet: usize,
}

fn ending(clip: &Clip) -> Option<Ending> {
    let lv = levels(clip);
    let n = lv.smooth.len();
    let per_second = (1.0 / LEVEL_BLOCK_S) as usize;
    if n < 2 * per_second {
        return None;
    }
    // The body is the loud part of the tail away from its last 4 s.
    let body_end = n.saturating_sub(4 * per_second).max(n / 2);
    let body = percentile(&lv.smooth[..body_end], 0.75);
    let stays_under = |drop: f64| {
        let mut i = n;
        while i > 0 && lv.smooth[i - 1] < body - drop {
            i -= 1;
        }
        (i * lv.block).min(clip.frames())
    };
    Some(Ending { fade_start: stays_under(FADE_DB), quiet: stays_under(QUIET_DB) })
}

/// How an incoming track opens, in frames from its first audible sample.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Opening {
    /// First point within `ENTRY_DB` of its body level (end of a slow intro).
    entry: usize,
    /// Loud from its first 150 ms (a hit or a full band at once).
    cold: bool,
}

fn opening(clip: &Clip) -> Option<Opening> {
    let lv = levels(clip);
    if lv.smooth.len() < (1.0 / LEVEL_BLOCK_S) as usize {
        return None;
    }
    let body = percentile(&lv.smooth, 0.75);
    let entry = lv.smooth.iter().position(|&l| l >= body - ENTRY_DB)?;
    let cold = lv.raw.iter().take(3).any(|&l| l >= body - ENTRY_DB);
    Some(Opening { entry: entry * lv.block, cold })
}

/// A tempo estimate and beat positions for one clip.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatGrid {
    pub bpm: f64,
    pub period_s: f64,
    /// Beat times in seconds from the clip start, on the fitted grid.
    pub beats_s: Vec<f64>,
    /// Onset strength at each beat, for picking a phase when halving.
    pub strength: Vec<f64>,
    /// 0..1; see `confidence`.
    pub confidence: f64,
}

/// Onset strength envelope on a decimated mono downmix: log-compressed
/// spectral flux in two bands — below ~200 Hz (kick, bass) and full range —
/// each detrended and normalized on its own before summing, so a quiet kick is
/// not drowned by broadband hats and the grid locks to the downbeat side.
struct Envelope {
    values: Vec<f64>,
    /// Envelope samples per second.
    rate: f64,
}

/// Subtract a moving average (±0.25 s), half-wave rectify, and scale to unit
/// standard deviation; `None` when the band is flat.
fn condition(flux: &[f64], rate: f64) -> Option<Vec<f64>> {
    let half = (0.25 * rate).round() as usize;
    let mut prefix = vec![0.0; flux.len() + 1];
    for (i, v) in flux.iter().enumerate() {
        prefix[i + 1] = prefix[i] + v;
    }
    let detrended: Vec<f64> = (0..flux.len())
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(flux.len());
            let mean = (prefix[hi] - prefix[lo]) / (hi - lo) as f64;
            (flux[i] - mean).max(0.0)
        })
        .collect();
    let mean = detrended.iter().sum::<f64>() / detrended.len() as f64;
    let std = (detrended.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / detrended.len() as f64)
        .sqrt();
    (std > 1e-9).then(|| detrended.iter().map(|v| v / std).collect())
}

fn onset_envelope(clip: &Clip) -> Option<Envelope> {
    let ch = clip.channels.max(1);
    let factor = ((clip.rate as f64 / ANALYSIS_RATE).floor() as usize).max(1);
    let analysis_rate = clip.rate as f64 / factor as f64;
    let mono: Vec<f64> = clip
        .samples
        .chunks(ch * factor)
        .filter(|block| block.len() == ch * factor)
        .map(|block| block.iter().sum::<f64>() / block.len() as f64)
        .collect();
    if mono.len() < FFT_SIZE * 4 {
        return None;
    }
    let fft = FftPlanner::<f64>::new().plan_fft_forward(FFT_SIZE);
    let window: Vec<f64> = (0..FFT_SIZE)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / FFT_SIZE as f64).cos())
        .collect();
    let bins = FFT_SIZE / 2;
    let low_bins = ((200.0 * FFT_SIZE as f64 / analysis_rate).ceil() as usize).clamp(2, bins);
    let mut previous = vec![0.0f64; bins];
    let mut buffer = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    let frames = (mono.len() - FFT_SIZE) / HOP + 1;
    let mut low = Vec::with_capacity(frames);
    let mut full = Vec::with_capacity(frames);
    for (index, start) in (0..=mono.len() - FFT_SIZE).step_by(HOP).enumerate() {
        for (slot, (sample, w)) in buffer.iter_mut().zip(mono[start..start + FFT_SIZE].iter().zip(&window)) {
            *slot = Complex::new(sample * w, 0.0);
        }
        fft.process(&mut buffer);
        let (mut low_sum, mut full_sum) = (0.0, 0.0);
        for (k, (bin, prev)) in buffer[..bins].iter().zip(previous.iter_mut()).enumerate() {
            let magnitude = (1.0 + 1000.0 * bin.norm()).ln();
            let rise = if index == 0 { 0.0 } else { (magnitude - *prev).max(0.0) };
            *prev = magnitude;
            full_sum += rise;
            if k < low_bins {
                low_sum += rise;
            }
        }
        low.push(low_sum);
        full.push(full_sum);
    }
    let rate = analysis_rate / HOP as f64;
    let values: Vec<f64> = match (condition(&low, rate), condition(&full, rate)) {
        (Some(l), Some(f)) => l.iter().zip(&f).map(|(a, b)| a + b).collect(),
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => return None,
    };
    Some(Envelope { values, rate })
}

/// Best beat period (in envelope samples) from the prior-weighted
/// autocorrelation of the mean-removed envelope, refined to sub-sample
/// precision, plus its periodicity: the unweighted autocorrelation at that lag
/// relative to lag 0 (≈0 for noise, high for a steady beat).
fn estimate_period(env: &Envelope) -> Option<(f64, f64)> {
    let n = env.values.len();
    let min_lag = (env.rate * 60.0 / MAX_BPM).floor().max(2.0) as usize;
    let max_lag = ((env.rate * 60.0 / MIN_BPM).ceil() as usize).min(n / 2);
    if max_lag <= min_lag + 2 {
        return None;
    }
    let mean = env.values.iter().sum::<f64>() / n as f64;
    let centred: Vec<f64> = env.values.iter().map(|v| v - mean).collect();
    let acf = |lag: usize| {
        centred[..n - lag].iter().zip(&centred[lag..]).map(|(a, b)| a * b).sum::<f64>() / (n - lag) as f64
    };
    let energy = acf(0);
    if energy <= 0.0 {
        return None;
    }
    let raw: Vec<f64> = (min_lag - 1..=max_lag + 1).map(acf).collect();
    let weighted: Vec<f64> = raw
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let bpm = env.rate * 60.0 / (min_lag - 1 + i) as f64;
            let octaves = (bpm / PRIOR_BPM).log2() / PRIOR_OCTAVES;
            value * (-0.5 * octaves * octaves).exp()
        })
        .collect();
    // `weighted[i]` is lag `min_lag - 1 + i`; search the inner range so both
    // neighbours exist for the parabolic fit.
    let best = (1..weighted.len() - 1).max_by(|&a, &b| weighted[a].total_cmp(&weighted[b]))?;
    if weighted[best] <= 0.0 {
        return None;
    }
    let (a, b, c) = (weighted[best - 1], weighted[best], weighted[best + 1]);
    let denom = a - 2.0 * b + c;
    let offset = if denom.abs() > 1e-12 { (0.5 * (a - c) / denom).clamp(-0.5, 0.5) } else { 0.0 };
    Some(((min_lag - 1 + best) as f64 + offset, (raw[best] / energy).max(0.0)))
}

/// Ellis-style dynamic-programming beat tracking: beat positions (envelope
/// indices) that balance onset strength against a steady period.
fn track_beats(env: &[f64], period: f64) -> Vec<usize> {
    let n = env.len();
    let mut score = vec![0.0f64; n];
    let mut back = vec![usize::MAX; n];
    for t in 0..n {
        let lo = (t as f64 - 2.0 * period).round().max(0.0) as usize;
        let hi = t as f64 - (period / 2.0).round();
        let mut best = 0.0;
        if hi >= 0.0 {
            let hi = (hi as usize).min(t.saturating_sub(1));
            for (p, &previous) in score.iter().enumerate().take(hi + 1).skip(lo) {
                let gap = (t - p) as f64 / period;
                let candidate = previous - TIGHTNESS * gap.ln().powi(2);
                if back[t] == usize::MAX || candidate > best {
                    best = candidate;
                    back[t] = p;
                }
            }
        }
        score[t] = env[t] + if back[t] == usize::MAX { 0.0 } else { best };
    }
    let tail_start = n.saturating_sub(period.ceil() as usize);
    let Some(mut t) = (tail_start..n).max_by(|&a, &b| score[a].total_cmp(&score[b])) else {
        return Vec::new();
    };
    let mut beats = vec![t];
    while back[t] != usize::MAX {
        t = back[t];
        beats.push(t);
    }
    beats.reverse();
    beats
}

/// Kurtosis of the onset envelope: sparse, spiky envelopes (drums, picked
/// attacks) score high; smooth modulation (vibrato, swells, tones beating
/// against each other) scores low even when it is periodic.
fn kurtosis(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    if var <= 0.0 {
        return 0.0;
    }
    values.iter().map(|v| (v - mean).powi(4)).sum::<f64>() / n / (var * var)
}

/// Grid confidence, 0..1: periodic onsets at the chosen tempo × impulsive
/// onsets (rhythm rather than modulation) × steady tracked beats.
fn confidence(periodicity: f64, impulsiveness: f64, beats: &[usize]) -> f64 {
    if beats.len() < 8 {
        return 0.0;
    }
    let intervals: Vec<f64> = beats.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    let mean_ibi = intervals.iter().sum::<f64>() / intervals.len() as f64;
    let cv = (intervals.iter().map(|i| (i - mean_ibi).powi(2)).sum::<f64>() / intervals.len() as f64)
        .sqrt()
        / mean_ibi;
    ((periodicity - 0.08) / 0.25).clamp(0.0, 1.0)
        * ((impulsiveness - 6.0) / 6.0).clamp(0.0, 1.0)
        * (1.0 - 4.0 * cv).clamp(0.0, 1.0)
}

/// Tempo, beat positions, and confidence for `clip`, or `None` when it is
/// too short or has no onsets at all.
pub fn analyze_beats(clip: &Clip) -> Option<BeatGrid> {
    let env = onset_envelope(clip)?;
    let (period, periodicity) = estimate_period(&env)?;
    let beats = track_beats(&env.values, period);
    if beats.len() < 4 {
        return None;
    }
    let confidence = confidence(periodicity, kurtosis(&env.values), &beats);
    // Least-squares line through the beat indices gives a precise period and
    // phase; the fitted grid is steadier than individual DP picks.
    let count = beats.len() as f64;
    let mean_i = (count - 1.0) / 2.0;
    let mean_t = beats.iter().sum::<usize>() as f64 / count;
    let (mut num, mut den) = (0.0, 0.0);
    for (i, &b) in beats.iter().enumerate() {
        num += (i as f64 - mean_i) * (b as f64 - mean_t);
        den += (i as f64 - mean_i).powi(2);
    }
    let slope = if den > 0.0 { num / den } else { period };
    let intercept = mean_t - slope * mean_i;
    // Envelope frame k covers analysis samples [k*HOP, k*HOP + FFT_SIZE); its
    // onset sits near the window centre.
    let to_seconds = |frame: f64| (frame * HOP as f64 + FFT_SIZE as f64 / 2.0) / (env.rate * HOP as f64);
    let period_s = slope / env.rate;
    let beats_s: Vec<f64> = (0..beats.len()).map(|i| to_seconds(intercept + slope * i as f64)).collect();
    let strength = beats
        .iter()
        .map(|&b| env.values[b.saturating_sub(1)..(b + 2).min(env.values.len())].iter().copied().fold(0.0, f64::max))
        .collect();
    Some(BeatGrid { bpm: 60.0 / period_s, period_s, beats_s, strength, confidence })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    /// Equal-power overlap of the (trimmed) tail and head.
    Crossfade,
    /// Overlap aligned beat-to-beat and sized in whole beats.
    BeatMatched,
    /// The outputs could not overlap; the tail faded out on its own.
    FadeOut,
}

/// What a transition did, for A/B listening notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionReport {
    pub kind: TransitionKind,
    pub overlap_ms: u32,
    pub trimmed_out_ms: u32,
    pub trimmed_in_ms: u32,
    pub resampled: bool,
    pub out_bpm: Option<f32>,
    pub in_bpm: Option<f32>,
    pub out_confidence: Option<f32>,
    pub in_confidence: Option<f32>,
    pub beats: Option<u32>,
    /// Why beat matching did not apply, when it was asked for (or another
    /// planning remark).
    pub note: Option<String>,
    pub style: TransitionStyle,
    /// Audible outgoing audio dropped after the overlap (its fade or ending).
    pub cut_out_ms: u32,
    /// Incoming intro skipped beyond its leading silence.
    pub cut_in_ms: u32,
}

/// How to cross from the held-back tail into the next track. Tail frames are
/// at the tail's rate; `incoming_skip_frames` at the head's.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionPlan {
    /// Tail frames played on their own before the overlap.
    pub preroll_frames: usize,
    /// Tail frames overlapped with the incoming track; the rest is dropped.
    pub mix_frames: usize,
    /// Incoming frames skipped before the overlap starts.
    pub incoming_skip_frames: usize,
    /// How the two tracks trade places across the overlap.
    pub shape: MixShape,
    pub report: TransitionReport,
}

#[derive(Debug, Clone, PartialEq)]
struct Alignment {
    preroll_s: f64,
    mix_s: f64,
    skip_s: f64,
    /// The outgoing beat the incoming centre beat lands on.
    centre_s: f64,
    beats: u32,
}

/// Every other beat, keeping the phase with the stronger onsets.
fn halve(grid: &BeatGrid) -> (Vec<f64>, f64) {
    let phase_strength = |phase: usize| grid.strength.iter().skip(phase).step_by(2).sum::<f64>();
    let phase = usize::from(phase_strength(1) > phase_strength(0));
    (grid.beats_s.iter().skip(phase).step_by(2).copied().collect(), grid.period_s * 2.0)
}

fn align(out: &BeatGrid, inn: &BeatGrid, target_s: f64, tail_end_s: f64) -> Result<Alignment, String> {
    if out.confidence < CONFIDENCE_FLOOR || inn.confidence < CONFIDENCE_FLOOR {
        return Err(format!(
            "beat confidence too low (out {:.2}, in {:.2})",
            out.confidence, inn.confidence
        ));
    }
    // Octave relations (half-time sections, double-time drums) match on the
    // slower grid.
    let ratio = inn.period_s / out.period_s;
    let ((ob, op), (ib, ip)) = if (ratio / 2.0 - 1.0).abs() <= MAX_TEMPO_DIFF {
        (halve(out), (inn.beats_s.clone(), inn.period_s))
    } else if (ratio * 2.0 - 1.0).abs() <= MAX_TEMPO_DIFF {
        ((out.beats_s.clone(), out.period_s), halve(inn))
    } else {
        ((out.beats_s.clone(), out.period_s), (inn.beats_s.clone(), inn.period_s))
    };
    let diff = (op - ip).abs();
    if diff / op > MAX_TEMPO_DIFF {
        return Err(format!("tempos too far apart ({:.0} vs {:.0} BPM)", out.bpm, inn.bpm));
    }
    for beats in [16usize, 8, 4] {
        let span = beats as f64 * op;
        if span > 1.25 * target_s.max(4.0 * op) || beats as f64 / 2.0 * diff > MAX_DRIFT_S {
            continue;
        }
        let half = beats / 2;
        if ib.len() <= half {
            continue;
        }
        // The latest outgoing window that ends before the audible end.
        let Some(k) = (0..ob.len())
            .rev()
            .find(|&k| k + half < ob.len() && ob[k] + span <= tail_end_s + 1e-9)
        else {
            continue;
        };
        let delta = ob[k + half] - ib[half];
        let preroll_s = ob[k].max(delta).max(0.0);
        let skip_s = preroll_s - delta;
        let mix_s = ob[k] + span - preroll_s;
        if mix_s < 2.0 * op {
            continue;
        }
        return Ok(Alignment { preroll_s, mix_s, skip_s, centre_s: ob[k + half], beats: beats as u32 });
    }
    Err("beats drift too far for even a 4-beat overlap".into())
}

fn ms(frames: usize, rate: u32) -> u32 {
    (frames as f64 * 1000.0 / rate.max(1) as f64).round() as u32
}

/// Plan the crossing from `tail` into the track whose opening is `head`.
/// Classic without any AutoMix toggle is the plain Track 7 crossfade: the
/// last `duration_ms` of the tail overlap the start of the next track.
pub fn plan_transition(
    tail: &Clip,
    head: Option<&Clip>,
    settings: &CrossfadeSettings,
) -> TransitionPlan {
    match settings.style {
        TransitionStyle::Classic => plan_classic(tail, head, settings),
        TransitionStyle::Quick => plan_quick(tail, head, settings),
        TransitionStyle::Blend => plan_blend(tail, head, settings),
    }
}

/// The tail's audible end and the head's leading silence, when trimming.
fn edges(tail: &Clip, head: Option<&Clip>, trim: bool) -> (usize, usize) {
    if !trim {
        return (tail.frames(), 0);
    }
    let audible_end = audible_bounds(tail).map_or(0, |(_, end)| end);
    let lead = head.map_or(0, |head| {
        let cap = (MAX_LEAD_TRIM_S * head.rate as f64) as usize;
        audible_bounds(head).map_or(cap, |(start, _)| start).min(cap).min(head.frames())
    });
    (audible_end, lead)
}

fn frames_at(seconds: f64, rate: u32) -> usize {
    (seconds * rate as f64).round() as usize
}

/// A crossfade report before any overlap is chosen.
fn base_report(
    style: TransitionStyle,
    tail: &Clip,
    head: Option<&Clip>,
    audible_end: usize,
    lead: usize,
) -> TransitionReport {
    TransitionReport {
        kind: TransitionKind::Crossfade,
        overlap_ms: 0,
        trimmed_out_ms: ms(tail.frames() - audible_end, tail.rate),
        trimmed_in_ms: head.map_or(0, |h| ms(lead, h.rate)),
        resampled: false,
        out_bpm: None,
        in_bpm: None,
        out_confidence: None,
        in_confidence: None,
        beats: None,
        note: None,
        style,
        cut_out_ms: 0,
        cut_in_ms: 0,
    }
}

/// Beat grids of the outgoing window and the incoming opening, recorded in
/// the report; `None` (with a note) when either is missing.
fn grids(
    tail: &Clip,
    head: Option<&Clip>,
    tail_end: usize,
    head_start: usize,
    report: &mut TransitionReport,
) -> Option<(BeatGrid, BeatGrid)> {
    let Some(head) = head else {
        report.note = Some("next track was not decoded ahead in time".into());
        return None;
    };
    let out = analyze_beats(&tail.slice(0, tail_end));
    let inn = analyze_beats(&head.slice(head_start, head.frames()));
    report.out_bpm = out.as_ref().map(|g| g.bpm as f32);
    report.in_bpm = inn.as_ref().map(|g| g.bpm as f32);
    report.out_confidence = out.as_ref().map(|g| g.confidence as f32);
    report.in_confidence = inn.as_ref().map(|g| g.confidence as f32);
    match (out, inn) {
        (Some(out), Some(inn)) => Some((out, inn)),
        _ => {
            report.note = Some("no beat grid (too short or no onsets)".into());
            None
        }
    }
}

/// Track 7 (plus the 7b toggles): the last `duration_ms` of audible audio,
/// equal power.
fn plan_classic(tail: &Clip, head: Option<&Clip>, settings: &CrossfadeSettings) -> TransitionPlan {
    let target_frames = frames_at(settings.duration_ms as f64 / 1000.0, tail.rate);
    let (audible_end, lead) = edges(tail, head, settings.trim_silence);
    let mix = target_frames.min(audible_end);
    let mut report = base_report(TransitionStyle::Classic, tail, head, audible_end, lead);
    report.overlap_ms = ms(mix, tail.rate);
    let mut plan = TransitionPlan {
        preroll_frames: audible_end - mix,
        mix_frames: mix,
        incoming_skip_frames: lead,
        shape: MixShape::EqualPower,
        report,
    };
    if !settings.beat_match {
        return plan;
    }
    let Some((out, inn)) = grids(tail, head, audible_end, lead, &mut plan.report) else {
        return plan;
    };
    let head_rate = head.map_or(tail.rate, |h| h.rate);
    let target_s = settings.duration_ms as f64 / 1000.0;
    let tail_end_s = audible_end as f64 / tail.rate as f64;
    match align(&out, &inn, target_s, tail_end_s) {
        Ok(a) => {
            let preroll = frames_at(a.preroll_s, tail.rate).min(audible_end);
            let mix = frames_at(a.mix_s, tail.rate).min(audible_end - preroll);
            plan.preroll_frames = preroll;
            plan.mix_frames = mix;
            plan.incoming_skip_frames = lead + frames_at(a.skip_s, head_rate);
            plan.report.kind = TransitionKind::BeatMatched;
            plan.report.overlap_ms = ms(mix, tail.rate);
            plan.report.beats = Some(a.beats);
        }
        Err(reason) => plan.report.note = Some(reason),
    }
    plan
}

/// A short overlap once the outgoing track has died down (AM-D9): the
/// incoming track lands as the last one rings out.
fn plan_quick(tail: &Clip, head: Option<&Clip>, settings: &CrossfadeSettings) -> TransitionPlan {
    let rate = tail.rate;
    let (audible_end, lead) = edges(tail, head, true);
    let len = frames_at((settings.duration_ms as f64 / 1000.0).min(QUICK_MAX_S), rate);
    let quiet = ending(&tail.slice(0, audible_end)).map_or(audible_end, |e| e.quiet);
    let start = quiet.saturating_sub(len / 2).min(audible_end.saturating_sub(len));
    let mix = len.min(audible_end - start);
    let cold = head
        .and_then(|h| opening(&h.slice(lead, h.frames())))
        .is_some_and(|o| o.cold);
    let mut report = base_report(TransitionStyle::Quick, tail, head, audible_end, lead);
    report.overlap_ms = ms(mix, rate);
    report.cut_out_ms = ms(audible_end - start - mix, rate);
    if cold {
        report.note = Some("next track opens loud: no fade-in".into());
    }
    TransitionPlan {
        preroll_frames: start,
        mix_frames: mix,
        incoming_skip_frames: lead,
        shape: MixShape::Quick { attack: if cold { 0.0 } else { QUICK_ATTACK } },
        report,
    }
}

/// A bass swap of `SWAP_S` centred at `centre` (fraction of a `mix_s` overlap).
fn bass_swap(centre: f64, mix_s: f64) -> MixShape {
    let half = (SWAP_S / 2.0 / mix_s.max(1e-9)).min(0.5);
    MixShape::BassSwap {
        swap_start: (centre - half).clamp(0.0, 1.0),
        swap_end: (centre + half).clamp(0.0, 1.0),
    }
}

/// Apple-style (AM-D10): the overlap ends where the outgoing fade starts, so
/// that fade is cut; the incoming track skips a slow intro so it reaches its
/// body as the overlap ends; the lows swap at the centre.
fn plan_blend(tail: &Clip, head: Option<&Clip>, settings: &CrossfadeSettings) -> TransitionPlan {
    let rate = tail.rate;
    let (audible_end, lead) = edges(tail, head, true);
    let len = frames_at(settings.duration_ms as f64 / 1000.0, rate);
    let fade_start = ending(&tail.slice(0, audible_end)).map_or(audible_end, |e| e.fade_start);
    let end = fade_start
        .max(audible_end.saturating_sub(frames_at(MAX_CUT_OUT_S, rate)))
        .max(len.min(audible_end));
    let start = end.saturating_sub(len);
    let mix = end - start;
    let cut_in = head.map_or(0, |h| {
        let entry = opening(&h.slice(lead, h.frames())).map_or(0, |o| o.entry);
        let mix_in = frames_at(mix as f64 / rate as f64, h.rate);
        entry.saturating_sub(mix_in).min(frames_at(MAX_CUT_IN_S, h.rate))
    });
    let head_rate = head.map_or(rate, |h| h.rate);
    let mut report = base_report(TransitionStyle::Blend, tail, head, audible_end, lead);
    report.overlap_ms = ms(mix, rate);
    report.cut_out_ms = ms(audible_end - end, rate);
    report.cut_in_ms = ms(cut_in, head_rate);
    let mix_s = mix as f64 / rate as f64;
    let mut plan = TransitionPlan {
        preroll_frames: start,
        mix_frames: mix,
        incoming_skip_frames: lead + cut_in,
        shape: bass_swap(0.5, mix_s),
        report,
    };
    if !settings.beat_match {
        return plan;
    }
    let Some((out, inn)) = grids(tail, head, end, lead + cut_in, &mut plan.report) else {
        return plan;
    };
    let target_s = settings.duration_ms as f64 / 1000.0;
    match align(&out, &inn, target_s, end as f64 / rate as f64) {
        Ok(a) => {
            let preroll = frames_at(a.preroll_s, rate).min(end);
            let mix = frames_at(a.mix_s, rate).min(end - preroll);
            let skip = lead + cut_in + frames_at(a.skip_s, head_rate);
            plan.preroll_frames = preroll;
            plan.mix_frames = mix;
            plan.incoming_skip_frames = skip;
            plan.shape = bass_swap((a.centre_s - a.preroll_s) / a.mix_s.max(1e-9), a.mix_s);
            plan.report.kind = TransitionKind::BeatMatched;
            plan.report.overlap_ms = ms(mix, rate);
            plan.report.beats = Some(a.beats);
            plan.report.cut_out_ms = ms(audible_end - preroll - mix, rate);
            plan.report.cut_in_ms = ms(skip - lead, head_rate);
        }
        Err(reason) => plan.report.note = Some(reason),
    }
    plan
}

/// Whether any AutoMix behaviour is on; all off is the exact Track 7 path.
pub fn active(settings: &CrossfadeSettings) -> bool {
    settings.style != TransitionStyle::Classic || settings.trim_silence || settings.beat_match
}

/// Seconds of decoded audio worth pre-reading from the next track.
pub fn head_seconds(settings: &CrossfadeSettings) -> f64 {
    if active(settings) {
        AUTOMIX_HEAD_S
    } else {
        0.0
    }
}

/// Extra tail held back beyond the crossfade, so every style has room to
/// move the overlap earlier (AM-D11).
pub fn extra_hold_ms(settings: &CrossfadeSettings) -> u32 {
    if active(settings) {
        AUTOMIX_HOLD_MS
    } else {
        0
    }
}

/// A transition rendered offline for the replay lab (AM-D12): a few seconds
/// of the outgoing track, the overlap, and a few seconds of the incoming one,
/// interleaved in the incoming track's format.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub rate: u32,
    pub report: TransitionReport,
    pub overlap_start_ms: u32,
    pub overlap_end_ms: u32,
}

/// Plan the crossing from `tail` into `head` with `settings` and mix it the
/// way the engine would (same `Tail` mixer), without DSP.
pub fn render_transition(tail: &Clip, head: &Clip, settings: &CrossfadeSettings) -> Rendered {
    let mut plan = plan_transition(tail, Some(head), settings);
    let (rate, channels) = (head.rate, head.channels.max(1));
    let tail_ch = tail.channels.max(1);
    let same = tail.rate == head.rate && tail_ch == channels;
    plan.report.resampled = !same;
    let part = |from: usize, to: usize| {
        let part = Tail::from_normalized(tail.samples[from * tail_ch..to * tail_ch].to_vec(), tail_ch, false);
        if same {
            part
        } else {
            part.converted(tail.rate, rate, channels)
        }
    };
    let preroll = plan.preroll_frames.min(tail.frames());
    let mix_end = (preroll + plan.mix_frames).min(tail.frames());
    let context = frames_at(REPLAY_CONTEXT_S, tail.rate);
    let mut out: Vec<f64> = part(preroll.saturating_sub(context), preroll).remaining().to_vec();
    let overlap_start = out.len() / channels;
    let mut overlap = part(preroll, mix_end);
    overlap.set_shape(plan.shape, rate);
    let overlap_end = overlap_start + overlap.frames();
    let stop = overlap_end + frames_at(REPLAY_CONTEXT_S, rate);
    let mut pos = plan.incoming_skip_frames.min(head.frames());
    while pos < head.frames() && out.len() / channels < stop {
        let next = (pos + 4096).min(head.frames());
        let chunk = head.samples[pos * channels..next * channels].iter().map(|&s| s as f32).collect();
        extend_normalized(&mut out, &overlap.mix_into(PcmBuffer::F32(chunk)));
        pos = next;
    }
    if !overlap.is_done() {
        // The opening ran out mid-overlap: the tail fades out on its own.
        extend_normalized(&mut out, &overlap.fade_out_remaining());
    }
    out.truncate(stop.max(overlap_end) * channels);
    Rendered {
        samples: out.iter().map(|&s| s as f32).collect(),
        channels,
        rate,
        report: plan.report,
        overlap_start_ms: ms(overlap_start, rate),
        overlap_end_ms: ms(overlap_end, rate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngExt, SeedableRng};

    const RATE: u32 = 44_100;

    fn settings(duration_ms: u32) -> CrossfadeSettings {
        CrossfadeSettings { duration_ms, ..Default::default() }
    }

    /// A stereo drum loop: a decaying low kick on every beat, a noisy hat on
    /// the off-beats, and a quiet noise floor. `offset_s` delays the first
    /// beat.
    fn drums(bpm: f64, seconds: f64, offset_s: f64, seed: u64) -> Vec<f64> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let frames = (seconds * RATE as f64) as usize;
        let mut mono = vec![0.0f64; frames];
        for s in mono.iter_mut() {
            *s = (rng.random::<f64>() - 0.5) * 0.01;
        }
        let period = 60.0 / bpm;
        let mut add = |at: f64, f: &dyn Fn(f64) -> f64, len: f64| {
            let start = (at * RATE as f64) as usize;
            for i in 0..(len * RATE as f64) as usize {
                if let Some(slot) = mono.get_mut(start + i) {
                    *slot += f(i as f64 / RATE as f64);
                }
            }
        };
        let mut t = offset_s;
        let mut hat_rng = rand::rngs::StdRng::seed_from_u64(seed ^ 0xA5);
        let hats: Vec<f64> = (0..4096).map(|_| hat_rng.random::<f64>() - 0.5).collect();
        while t < seconds {
            add(t, &|x| 0.8 * (2.0 * std::f64::consts::PI * 55.0 * x).sin() * (-x * 25.0).exp(), 0.15);
            let hat = |x: f64| 0.25 * hats[((x * RATE as f64) as usize) % hats.len()] * (-x * 60.0).exp();
            add(t + period / 2.0, &hat, 0.05);
            t += period;
        }
        mono.iter().flat_map(|&s| [s, s]).collect()
    }

    /// A dense band mix: kick on 1 and 3, snare on 2 and 4, straight-eighth
    /// hats, and a loud clipped sawtooth power chord with pick noise on every
    /// eighth (rock/metal), under `guitar` gain.
    fn band(bpm: f64, seconds: f64, offset_s: f64, guitar: f64, seed: u64) -> Vec<f64> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let frames = (seconds * RATE as f64) as usize;
        let period = 60.0 / bpm;
        let noise: Vec<f64> = (0..8192).map(|_| rng.random::<f64>() - 0.5).collect();
        let mut mono = vec![0.0f64; frames];
        for (n, slot) in mono.iter_mut().enumerate() {
            let t = n as f64 / RATE as f64;
            // Power chord (E2 + B2), clipped like a distorted amp, re-picked on
            // every eighth with a softer attack than the drums.
            let saw = |f: f64| 2.0 * (t * f).fract() - 1.0;
            let since_pick = (t - offset_s).rem_euclid(period / 2.0);
            let pick = 0.7 + 0.3 * (-since_pick * 8.0).exp();
            let chord = ((saw(82.41) + saw(123.47)) * 2.5).tanh() * pick;
            *slot = guitar * (chord + 0.3 * noise[n % noise.len()]);
        }
        let mut add = |at: f64, f: &dyn Fn(f64) -> f64, len: f64| {
            let start = (at * RATE as f64) as usize;
            for i in 0..(len * RATE as f64) as usize {
                if let Some(slot) = mono.get_mut(start + i) {
                    *slot += f(i as f64 / RATE as f64);
                }
            }
        };
        let mut beat = 0usize;
        let mut t = offset_s;
        while t < seconds {
            if beat.is_multiple_of(2) {
                add(t, &|x| 0.9 * (2.0 * std::f64::consts::PI * 55.0 * x).sin() * (-x * 25.0).exp(), 0.15);
            } else {
                let snare = |x: f64| {
                    0.6 * (noise[((x * RATE as f64) as usize * 3) % noise.len()]
                        + 0.5 * (2.0 * std::f64::consts::PI * 190.0 * x).sin())
                        * (-x * 30.0).exp()
                };
                add(t, &snare, 0.12);
            }
            for half in [0.0, 0.5] {
                let hat = |x: f64| 0.15 * noise[((x * RATE as f64) as usize * 7) % noise.len()] * (-x * 80.0).exp();
                add(t + half * period, &hat, 0.04);
            }
            beat += 1;
            t += period;
        }
        mono.iter().map(|s| s.clamp(-1.0, 1.0)).flat_map(|s| [s, s]).collect()
    }

    #[test]
    fn dense_band_mixes_keep_a_usable_grid() {
        for (bpm, guitar) in [(100.0, 0.25), (150.0, 0.35), (190.0, 0.3)] {
            let audio = band(bpm, 16.0, 0.2, guitar, bpm as u64 + 40);
            let grid = analyze_beats(&clip(&audio)).expect("grid");
            assert!(octave_match(grid.bpm, bpm), "{bpm}: found {:.2}", grid.bpm);
            assert!(
                grid.confidence >= CONFIDENCE_FLOOR,
                "{bpm} with guitar {guitar}: confidence {:.2}",
                grid.confidence
            );
            let step = grid.period_s.min(60.0 / bpm);
            for &b in &grid.beats_s {
                let phase = ((b - 0.2) / step).rem_euclid(1.0);
                assert!(phase.min(1.0 - phase) * step < 0.03, "{bpm}: beat {b:.3}");
            }
        }
    }

    fn clip(samples: &[f64]) -> Clip<'_> {
        Clip { samples, channels: 2, rate: RATE }
    }

    fn octave_match(found: f64, expected: f64) -> bool {
        [0.5, 1.0, 2.0].iter().any(|m| (found / (expected * m) - 1.0).abs() < 0.01)
    }

    #[test]
    fn audible_bounds_skip_silence_on_both_ends() {
        let mut samples = vec![0.0; 2 * 1000];
        samples.extend(std::iter::repeat_n(0.5, 2 * 500));
        samples.extend(std::iter::repeat_n(0.0005, 2 * 300));
        assert_eq!(audible_bounds(&clip(&samples)), Some((1000, 1500)));
        assert_eq!(audible_bounds(&clip(&[0.0; 64])), None);
    }

    #[test]
    fn finds_tempo_and_phase_across_genres_tempi() {
        for (bpm, offset) in [(90.0, 0.31), (128.0, 0.12), (174.0, 0.05)] {
            let audio = drums(bpm, 16.0, offset, bpm as u64);
            let grid = analyze_beats(&clip(&audio)).expect("grid");
            assert!(octave_match(grid.bpm, bpm), "{bpm}: found {:.2}", grid.bpm);
            assert!(grid.confidence >= 0.5, "{bpm}: confidence {:.2}", grid.confidence);
            // Every detected beat sits on (a multiple of) the true grid.
            let period = 60.0 / bpm;
            let grid_period = grid.period_s.min(period);
            for &b in &grid.beats_s {
                let phase = ((b - offset) / grid_period).rem_euclid(1.0);
                let error = phase.min(1.0 - phase) * grid_period;
                // Onset detection reports attacks a constant ~15 ms early;
                // both sides of a transition share that bias, so alignment
                // (pinned to 15 ms below) is unaffected.
                assert!(error < 0.025, "{bpm}: beat {b:.3} off by {error:.4}s");
            }
        }
    }

    #[test]
    fn noise_and_sustained_chords_are_not_confident() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let noise: Vec<f64> = (0..RATE as usize * 12 * 2).map(|_| (rng.random::<f64>() - 0.5) * 0.4).collect();
        // A held chord as strings or a synth pad play it: gentle vibrato, a
        // slow swell, and a breathy noise layer.
        let mut breath = rand::rngs::StdRng::seed_from_u64(8);
        let pad: Vec<f64> = (0..RATE as usize * 12)
            .flat_map(|n| {
                let t = n as f64 / RATE as f64;
                let vibrato = 1.0 + 0.004 * (2.0 * std::f64::consts::PI * 5.3 * t).sin();
                let tone = [220.0, 277.18, 329.63]
                    .iter()
                    .enumerate()
                    .map(|(i, f)| (2.0 * std::f64::consts::PI * f * vibrato * t + i as f64).sin())
                    .sum::<f64>();
                let s = (0.2 * tone + 0.02 * (breath.random::<f64>() - 0.5)) * (0.6 + 0.4 * (0.3 * t).sin());
                [s, s]
            })
            .collect();
        for (name, audio) in [("noise", &noise), ("pad", &pad)] {
            let confidence = analyze_beats(&clip(audio)).map_or(0.0, |g| g.confidence);
            assert!(confidence < CONFIDENCE_FLOOR, "{name}: confidence {confidence:.2}");
        }
    }

    #[test]
    fn plain_settings_reproduce_the_track7_crossfade() {
        let tail = vec![0.3; 2 * RATE as usize * 8];
        let plan = plan_transition(&clip(&tail), None, &settings(6000));
        assert_eq!(plan.preroll_frames, 2 * RATE as usize);
        assert_eq!(plan.mix_frames, 6 * RATE as usize);
        assert_eq!(plan.incoming_skip_frames, 0);
        assert_eq!(plan.report.kind, TransitionKind::Crossfade);
        // A short tail (slow producer) overlaps all of it.
        let short = vec![0.3; 2 * RATE as usize * 3];
        let plan = plan_transition(&clip(&short), None, &settings(6000));
        assert_eq!((plan.preroll_frames, plan.mix_frames), (0, 3 * RATE as usize));
    }

    #[test]
    fn trim_moves_the_overlap_onto_audible_audio() {
        // 10 s of music then 2 s of silence; the head opens with 1 s of silence.
        let mut tail = vec![0.3; 2 * RATE as usize * 10];
        tail.extend(vec![0.0; 2 * RATE as usize * 2]);
        let mut head = vec![0.0; 2 * RATE as usize];
        head.extend(vec![0.3; 2 * RATE as usize * 3]);
        let mut s = settings(4000);
        s.trim_silence = true;
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &s);
        assert_eq!(plan.mix_frames, 4 * RATE as usize);
        assert_eq!(plan.preroll_frames, 6 * RATE as usize);
        assert_eq!(plan.incoming_skip_frames, RATE as usize);
        assert_eq!(plan.report.trimmed_out_ms, 2000);
        assert_eq!(plan.report.trimmed_in_ms, 1000);
    }

    #[test]
    fn leading_trim_is_capped_for_quiet_intros() {
        let tail = vec![0.3; 2 * RATE as usize * 8];
        let mut head = vec![0.0; 2 * RATE as usize * 6];
        head.extend(vec![0.3; 2 * RATE as usize]);
        let mut s = settings(4000);
        s.trim_silence = true;
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &s);
        assert_eq!(plan.incoming_skip_frames, (MAX_LEAD_TRIM_S * RATE as f64) as usize);
    }

    /// Beat positions of the incoming track, in tail time, after the plan.
    fn aligned_error(tail_bpm: f64, tail_offset: f64, head_bpm: f64, head_offset: f64, plan: &TransitionPlan) -> f64 {
        // The incoming frame at skip lands on tail frame preroll; its beats
        // follow at head positions offset + i * period.
        let skip_s = plan.incoming_skip_frames as f64 / RATE as f64;
        let preroll_s = plan.preroll_frames as f64 / RATE as f64;
        let mid_tail = preroll_s + plan.mix_frames as f64 / RATE as f64 / 2.0;
        let head_period = 60.0 / head_bpm;
        let tail_period = 60.0 / tail_bpm;
        // Nearest incoming beat to the overlap centre, in tail time.
        let head_time = mid_tail - preroll_s + skip_s;
        let i = ((head_time - head_offset) / head_period).round();
        let incoming_beat = head_offset + i * head_period - skip_s + preroll_s;
        let phase = ((incoming_beat - tail_offset) / tail_period).rem_euclid(1.0);
        phase.min(1.0 - phase) * tail_period
    }

    #[test]
    fn beat_match_aligns_near_tempos() {
        let tail = drums(128.0, 16.0, 0.12, 1);
        let head = drums(127.0, 12.0, 0.40, 2);
        let mut s = settings(6000);
        s.beat_match = true;
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &s);
        assert_eq!(plan.report.kind, TransitionKind::BeatMatched, "{:?}", plan.report);
        let beats = plan.report.beats.unwrap();
        assert!((4..=16).contains(&beats));
        assert!(plan.preroll_frames + plan.mix_frames <= tail.len() / 2);
        let error = aligned_error(128.0, 0.12, 127.0, 0.40, &plan);
        assert!(error < 0.015, "misaligned by {error:.4}s: {plan:?}");
    }

    #[test]
    fn beat_match_accepts_octave_related_tempos() {
        let tail = drums(140.0, 16.0, 0.05, 3);
        let head = drums(70.0, 12.0, 0.20, 4);
        let mut s = settings(6000);
        s.beat_match = true;
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &s);
        assert_eq!(plan.report.kind, TransitionKind::BeatMatched, "{:?}", plan.report);
    }

    #[test]
    fn beat_match_falls_back_with_a_reason() {
        let tail = drums(128.0, 16.0, 0.1, 5);
        let far = drums(100.0, 12.0, 0.1, 6);
        let mut s = settings(6000);
        s.beat_match = true;
        let plan = plan_transition(&clip(&tail), Some(&clip(&far)), &s);
        assert_eq!(plan.report.kind, TransitionKind::Crossfade);
        assert!(plan.report.note.as_deref().unwrap().contains("too far apart"), "{:?}", plan.report);
        assert_eq!(plan.mix_frames, 6 * RATE as usize, "fallback keeps the plain crossfade");

        let mut rng = rand::rngs::StdRng::seed_from_u64(9);
        let noise: Vec<f64> = (0..RATE as usize * 12 * 2).map(|_| (rng.random::<f64>() - 0.5) * 0.4).collect();
        let plan = plan_transition(&clip(&tail), Some(&clip(&noise)), &s);
        assert_eq!(plan.report.kind, TransitionKind::Crossfade);
        assert!(plan.report.note.is_some());

        let plan = plan_transition(&clip(&tail), None, &s);
        assert!(plan.report.note.as_deref().unwrap().contains("decoded ahead"));
    }

    #[test]
    fn hold_and_head_sizes_follow_the_toggles() {
        let mut s = settings(6000);
        assert_eq!((extra_hold_ms(&s), head_seconds(&s)), (0, 0.0));
        s.trim_silence = true;
        assert!(extra_hold_ms(&s) > 0 && head_seconds(&s) > MAX_LEAD_TRIM_S);
        s.trim_silence = false;
        s.style = TransitionStyle::Blend;
        // Room for the longest crossfade plus the largest cut.
        assert!(extra_hold_ms(&s) as f64 >= MAX_CUT_OUT_S * 1000.0);
        assert!(head_seconds(&s) >= MAX_CUT_IN_S + 12.0);
    }

    /// A stereo 220 Hz + 3 kHz tone whose level follows `gain_db(t)`.
    fn tone(seconds: f64, gain_db: impl Fn(f64) -> f64) -> Vec<f64> {
        (0..(seconds * RATE as f64) as usize)
            .flat_map(|n| {
                let t = n as f64 / RATE as f64;
                let g = 0.3 * 10f64.powf(gain_db(t) / 20.0);
                let s = g * (0.7 * (2.0 * std::f64::consts::PI * 220.0 * t).sin()
                    + 0.3 * (2.0 * std::f64::consts::PI * 3000.0 * t).sin());
                [s, s]
            })
            .collect()
    }

    fn style(style: TransitionStyle, duration_ms: u32) -> CrossfadeSettings {
        CrossfadeSettings { duration_ms, style, ..Default::default() }
    }

    fn secs(frames: usize) -> f64 {
        frames as f64 / RATE as f64
    }

    /// 10 s of body, then a 6 s fade of 40 dB, then 1 s of silence.
    fn fading_tail() -> Vec<f64> {
        let mut tail = tone(16.0, |t| if t < 10.0 { 0.0 } else { -40.0 * (t - 10.0) / 6.0 });
        tail.extend(vec![0.0; 2 * RATE as usize]);
        tail
    }

    /// `intro_s` at −20 dB, then the body.
    fn slow_intro(intro_s: f64, seconds: f64) -> Vec<f64> {
        tone(seconds, |t| if t < intro_s { -20.0 } else { 0.0 })
    }

    #[test]
    fn quick_lands_once_the_fade_has_died_down() {
        let tail = fading_tail();
        let head = slow_intro(1.0, 8.0);
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &style(TransitionStyle::Quick, 6000));
        // −18 dB is 2.7 s into the fade (12.7 s); the 1.5 s overlap is centred there.
        let start = secs(plan.preroll_frames);
        assert!((11.6..=12.3).contains(&start), "overlap starts at {start:.2} s");
        assert_eq!(plan.mix_frames, (1.5 * RATE as f64) as usize);
        assert_eq!(plan.shape, MixShape::Quick { attack: QUICK_ATTACK });
        assert!(plan.report.cut_out_ms > 1000, "the rest of the fade is dropped: {:?}", plan.report);
        assert_eq!(plan.report.style, TransitionStyle::Quick);
    }

    #[test]
    fn quick_does_not_fade_in_a_cold_opening() {
        let tail = fading_tail();
        let head = tone(8.0, |_| 0.0);
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &style(TransitionStyle::Quick, 6000));
        assert_eq!(plan.shape, MixShape::Quick { attack: 0.0 });
        assert!(plan.report.note.as_deref().unwrap().contains("opens loud"));
    }

    #[test]
    fn quick_on_a_hard_stop_overlaps_the_last_moment() {
        // Full level to the very end, then silence.
        let mut tail = tone(12.0, |_| 0.0);
        tail.extend(vec![0.0; RATE as usize]);
        let plan = plan_transition(&clip(&tail), None, &style(TransitionStyle::Quick, 4000));
        assert_eq!(secs(plan.preroll_frames + plan.mix_frames), 12.0);
        assert_eq!(secs(plan.mix_frames), 1.5);
        // Never longer than the crossfade setting.
        let plan = plan_transition(&clip(&tail), None, &style(TransitionStyle::Quick, 1000));
        assert_eq!(secs(plan.mix_frames), 1.0);
    }

    #[test]
    fn blend_cuts_the_fade_and_skips_a_slow_intro() {
        let tail = fading_tail();
        let head = slow_intro(10.0, 18.0);
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &style(TransitionStyle::Blend, 6000));
        // −4 dB is 0.6 s into the fade: the overlap ends there.
        let end = secs(plan.preroll_frames + plan.mix_frames);
        assert!((10.3..=11.0).contains(&end), "overlap ends at {end:.2} s");
        assert_eq!(secs(plan.mix_frames), 6.0);
        assert!(plan.report.cut_out_ms > 5_000, "{:?}", plan.report);
        // The intro reaches its body (10 s) as the 6 s overlap ends: 4 s skipped.
        let skip = secs(plan.incoming_skip_frames);
        assert!((3.6..=4.4).contains(&skip), "incoming skip {skip:.2} s");
        assert_eq!(plan.report.cut_in_ms, ms(plan.incoming_skip_frames, RATE));
        let MixShape::BassSwap { swap_start, swap_end } = plan.shape else {
            panic!("{:?}", plan.shape);
        };
        assert!(swap_start < 0.5 && swap_end > 0.5 && swap_end - swap_start < 0.1);
    }

    #[test]
    fn blend_caps_how_much_it_cuts() {
        // A 20 s fade starting 2 s in: the cut stops at 12 s of audible ending.
        let tail = tone(22.0, |t| if t < 2.0 { 0.0 } else { -40.0 * (t - 2.0) / 20.0 });
        let intro = slow_intro(30.0, 20.0);
        let plan = plan_transition(&clip(&tail), Some(&clip(&intro)), &style(TransitionStyle::Blend, 6000));
        assert!((secs(plan.preroll_frames + plan.mix_frames) - 10.0).abs() < 0.01, "{plan:?}");
        // An opening that never gets louder has nothing to skip.
        assert_eq!(plan.report.cut_in_ms, 0);
    }

    #[test]
    fn blend_beat_matches_and_swaps_on_the_centre_beat() {
        let tail = drums(128.0, 24.0, 0.12, 11);
        let head = drums(127.0, 16.0, 0.40, 12);
        let mut s = style(TransitionStyle::Blend, 6000);
        s.beat_match = true;
        let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &s);
        assert_eq!(plan.report.kind, TransitionKind::BeatMatched, "{:?}", plan.report);
        let MixShape::BassSwap { swap_start, swap_end } = plan.shape else {
            panic!("{:?}", plan.shape);
        };
        assert!(swap_start > 0.2 && swap_end < 0.8, "{:?}", plan.shape);
        assert!(aligned_error(128.0, 0.12, 127.0, 0.40, &plan) < 0.015);
    }

    #[test]
    fn render_places_the_overlap_between_context() {
        let tail = fading_tail();
        let head = slow_intro(1.0, 20.0);
        for style_ in [TransitionStyle::Classic, TransitionStyle::Quick, TransitionStyle::Blend] {
            let r = render_transition(&clip(&tail), &clip(&head), &style(style_, 6000));
            assert_eq!((r.rate, r.channels), (RATE, 2));
            assert!(!r.report.resampled);
            let overlap = r.overlap_end_ms - r.overlap_start_ms;
            assert_eq!(overlap, r.report.overlap_ms, "{style_:?}");
            // Up to 6 s of lead-in (less when the overlap starts early in the tail).
            let plan = plan_transition(&clip(&tail), Some(&clip(&head)), &style(style_, 6000));
            assert_eq!(r.overlap_start_ms, ms(plan.preroll_frames.min(6 * RATE as usize), RATE), "{style_:?}");
            let total_ms = ms(r.samples.len() / 2, RATE);
            assert_eq!(total_ms, r.overlap_end_ms + 6_000, "{style_:?}");
            assert!(r.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        }
        // A different incoming format renders in that format.
        let head48: Vec<f64> = slow_intro(1.0, 20.0);
        let head48 = Clip { samples: &head48, channels: 2, rate: 48_000 };
        let r = render_transition(&clip(&tail), &head48, &style(TransitionStyle::Blend, 6000));
        assert_eq!(r.rate, 48_000);
        assert!(r.report.resampled);
    }
}
