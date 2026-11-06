//! Local beat tracking for the CatJam easter egg (spec
//! `2026-10-21-oxitide-catjam-kuroneko-design.md` §5.1, CJ-D1).
//!
//! The playback worker offers every decoded buffer (track frames, before
//! DSP) to a [`BeatTap`]. While enabled, the tap keeps a decimated mono copy
//! of one analysis window at a time and hands it back when full; the engine
//! runs [`analyze`] on a helper thread. Tempo and beats come only from the
//! audio being played: nothing is fetched and nothing leaves the process.
//!
//! The first window of a track (or after a seek, or when the tap is turned
//! on) is built from the decoded audio already queued ahead of playback
//! ([`BeatTap::push_ahead`]), so the clock is ready a moment after the track
//! starts instead of `FIRST_WINDOW_S` later; those frames are then skipped as
//! they play. The second window follows the first directly; later ones are
//! `REFRESH_S` apart.
//!
//! The tap only reads samples. `push` is a no-op while disabled and never
//! blocks: if the lock is held, that buffer is skipped (and the window
//! restarts at the next one, as after a seek).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use crate::automix::{self, Clip, CONFIDENCE_FLOOR};
use crate::decode::PcmBuffer;

/// Audio in a track's first window: a quick first estimate.
pub const FIRST_WINDOW_S: u64 = 8;
/// Audio in each later window.
pub const WINDOW_S: u64 = 12;
/// Later windows start this long after the previous one did, so tempo drift
/// and section changes are followed.
pub const REFRESH_S: u64 = 30;
/// Analysis rate the decimation aims for (as in `automix`).
const TARGET_RATE: f64 = 11_025.0;

/// Tempo and phase of the playing track, in track time.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatClock {
    pub track_id: u64,
    /// `None` when the grid is below `automix::CONFIDENCE_FLOOR` (sway, do
    /// not claim a beat).
    pub bpm: Option<f32>,
    pub period_ms: f32,
    /// Track position of one beat (the last one in the analyzed window).
    pub anchor_ms: u64,
    pub confidence: f32,
}

/// One full analysis window: mono samples at `rate`, starting at `start_ms`
/// of the track.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub track_id: u64,
    pub start_ms: f64,
    pub rate: u32,
    pub samples: Vec<f64>,
}

/// The largest factor near `rate / 11025` that divides `rate`, so the
/// decimated rate stays an exact integer.
fn decimation(rate: u32) -> u32 {
    let mut factor = ((rate as f64 / TARGET_RATE).floor() as u32).max(1);
    while !rate.is_multiple_of(factor) {
        factor -= 1;
    }
    factor
}

#[derive(Default)]
struct TapState {
    track_id: Option<u64>,
    rate: u32,
    factor: u32,
    /// The track frame the next push should start at; anything else is a
    /// discontinuity (seek, a skipped buffer).
    next_frame: u64,
    /// No window is collected before this frame (the refresh gap).
    collect_from: u64,
    /// A window has completed for this track.
    first_done: bool,
    /// Track frame of `samples[0]`.
    window_start: u64,
    /// Decimated samples that make the current window full.
    target: usize,
    samples: Vec<f64>,
    acc: f64,
    acc_n: u32,
}

impl TapState {
    fn start_track(&mut self, track_id: u64, rate: u32) {
        self.track_id = Some(track_id);
        self.rate = rate;
        self.factor = decimation(rate);
        self.first_done = false;
    }

    fn restart(&mut self, at: u64, lengths: (u64, u64)) {
        self.samples.clear();
        self.acc = 0.0;
        self.acc_n = 0;
        self.collect_from = at;
        let ms = if self.first_done { lengths.1 } else { lengths.0 };
        self.target = (ms * (self.rate / self.factor) as u64 / 1000) as usize;
    }

    #[allow(clippy::too_many_arguments)]
    fn feed<T: Copy>(
        &mut self,
        track_id: u64,
        start_frame: u64,
        samples: &[T],
        channels: usize,
        rate: u32,
        lengths: (u64, u64),
        to_f64: impl Fn(T) -> f64,
    ) -> Option<Window> {
        if self.track_id != Some(track_id) || self.rate != rate {
            self.start_track(track_id, rate);
            self.restart(start_frame, lengths);
        } else if start_frame != self.next_frame {
            self.restart(start_frame, lengths);
        }
        let frames = samples.len() / channels;
        self.next_frame = start_frame + frames as u64;
        if self.next_frame <= self.collect_from {
            return None;
        }
        let skip = self.collect_from.saturating_sub(start_frame) as usize;
        let mut done = None;
        for (i, frame) in samples.chunks_exact(channels).enumerate().skip(skip) {
            let at = start_frame + i as u64;
            // After a window completes here, the next may start later in
            // this same buffer (the second one follows the first directly).
            if at < self.collect_from {
                continue;
            }
            if self.samples.is_empty() && self.acc_n == 0 {
                self.window_start = at;
            }
            self.acc += frame.iter().map(|&s| to_f64(s)).sum::<f64>() / channels as f64;
            self.acc_n += 1;
            if self.acc_n == self.factor {
                self.samples.push(self.acc / self.factor as f64);
                self.acc = 0.0;
                self.acc_n = 0;
                // A window is far longer than a buffer: at most one ends here.
                if self.samples.len() >= self.target && done.is_none() {
                    done = Some(self.finish(track_id, lengths));
                }
            }
        }
        done
    }

    fn finish(&mut self, track_id: u64, lengths: (u64, u64)) -> Window {
        let window = Window {
            track_id,
            start_ms: self.window_start as f64 * 1000.0 / self.rate as f64,
            rate: self.rate / self.factor,
            samples: std::mem::take(&mut self.samples),
        };
        // The second window follows the first at once: a track whose opening
        // has no steady beat (an intro) gets its clock from the next 12 s, not
        // half a minute later. Later windows keep the refresh gap.
        let next = if self.first_done {
            self.window_start + REFRESH_S * self.rate as u64
        } else {
            self.window_start + (window.samples.len() as u64) * self.factor as u64
        };
        self.first_done = true;
        self.restart(next, lengths);
        window
    }
}

pub struct BeatTap {
    enabled: AtomicBool,
    state: Mutex<TapState>,
    /// Window lengths in ms (first, later); shortened only by tests.
    first_ms: AtomicU64,
    window_ms: AtomicU64,
}

impl Default for BeatTap {
    fn default() -> Self {
        Self::new()
    }
}

impl BeatTap {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            state: Mutex::new(TapState::default()),
            first_ms: AtomicU64::new(FIRST_WINDOW_S * 1000),
            window_ms: AtomicU64::new(WINDOW_S * 1000),
        }
    }

    /// Shorter windows, so engine tests can complete one on a short fixture.
    #[cfg(test)]
    pub(crate) fn set_window_ms(&self, first: u64, later: u64) {
        self.first_ms.store(first, Ordering::SeqCst);
        self.window_ms.store(later, Ordering::SeqCst);
    }

    /// Turning the tap on or off discards any partial window.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        if let Ok(mut state) = self.state.lock() {
            *state = TapState::default();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Track frames the first window of a `rate` track needs.
    pub fn first_window_frames(&self, rate: u32) -> u64 {
        self.first_ms.load(Ordering::Relaxed) * rate as u64 / 1000
    }

    /// Whether `track_id` still waits for its first window, so the engine
    /// should offer the decoded audio queued ahead of playback
    /// ([`push_ahead`](Self::push_ahead)). One atomic load while disabled.
    pub fn wants_ahead(&self, track_id: u64, rate: u32) -> bool {
        if !self.is_enabled() {
            return false;
        }
        match self.state.try_lock() {
            Ok(state) => !(state.track_id == Some(track_id) && state.rate == rate && state.first_done),
            Err(_) => false,
        }
    }

    /// Build the first window of `track_id` from decoded audio not yet played:
    /// `buffers` are consecutive, the first starting at track frame
    /// `start_frame`, which is also where the next [`push`](Self::push) will
    /// start. Without it the window fills only as fast as the audio plays
    /// (`FIRST_WINDOW_S` of silence from the cat at every track start, seek or
    /// mount). On success the tap continues as if those frames had been pushed:
    /// the pushes that play them are skipped, so no sample is analyzed twice.
    /// Returns `None`, changing nothing, when the buffers fall short.
    pub fn push_ahead<'a>(
        &self,
        track_id: u64,
        start_frame: u64,
        buffers: impl IntoIterator<Item = &'a PcmBuffer>,
        channels: u16,
        rate: u32,
    ) -> Option<Window> {
        if !self.is_enabled() || channels == 0 || rate == 0 {
            return None;
        }
        let mut state = self.state.try_lock().ok()?;
        if state.track_id == Some(track_id) && state.rate == rate && state.first_done {
            return None;
        }
        let ch = channels as usize;
        let lengths = (self.first_ms.load(Ordering::Relaxed), self.window_ms.load(Ordering::Relaxed));
        let mut ahead = TapState::default();
        let mut frame = start_frame;
        for buf in buffers {
            let window = match buf {
                PcmBuffer::F32(samples) => ahead.feed(track_id, frame, samples, ch, rate, lengths, |s| s as f64),
                PcmBuffer::I32(samples) => ahead.feed(track_id, frame, samples, ch, rate, lengths, |s| {
                    s as f64 / 2_147_483_648.0
                }),
            };
            if let Some(window) = window {
                // Playback resumes at `start_frame`; those pushes run up to the
                // next window's start without collecting anything, so the
                // next window begins empty (not with this buffer's rest).
                let next = ahead.collect_from;
                ahead.restart(next, lengths);
                ahead.next_frame = start_frame;
                *state = ahead;
                return Some(window);
            }
            frame += buf.frames(channels);
        }
        None
    }

    /// Offer one decoded buffer of `track_id` whose first frame is track
    /// frame `start_frame` (source rate, before DSP). Returns a window when
    /// this buffer completed one.
    pub fn push(
        &self,
        track_id: u64,
        start_frame: u64,
        buf: &PcmBuffer,
        channels: u16,
        rate: u32,
    ) -> Option<Window> {
        if !self.is_enabled() || channels == 0 || rate == 0 {
            return None;
        }
        let mut state = self.state.try_lock().ok()?;
        let channels = channels as usize;
        let lengths = (self.first_ms.load(Ordering::Relaxed), self.window_ms.load(Ordering::Relaxed));
        match buf {
            PcmBuffer::F32(samples) => {
                state.feed(track_id, start_frame, samples, channels, rate, lengths, |s| s as f64)
            }
            PcmBuffer::I32(samples) => {
                state.feed(track_id, start_frame, samples, channels, rate, lengths, |s| {
                    s as f64 / 2_147_483_648.0
                })
            }
        }
    }
}

/// The beat clock for one window, or `None` when it holds no usable onsets.
pub fn analyze(window: &Window) -> Option<BeatClock> {
    let clip = Clip { samples: &window.samples, channels: 1, rate: window.rate };
    let grid = automix::analyze_beats(&clip)?;
    let last = *grid.beats_s.last()?;
    let confident = grid.confidence >= CONFIDENCE_FLOOR;
    Some(BeatClock {
        track_id: window.track_id,
        bpm: confident.then_some(grid.bpm as f32),
        period_ms: (grid.period_s * 1000.0) as f32,
        anchor_ms: (window.start_ms + last * 1000.0).round().max(0.0) as u64,
        confidence: grid.confidence as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automix::tests::{band, drums, RATE};

    const CHUNK: usize = 4096;

    fn enabled() -> BeatTap {
        let tap = BeatTap::new();
        tap.set_enabled(true);
        tap
    }

    /// Push interleaved stereo `audio` in chunks starting at track frame
    /// `start`; the windows completed along the way.
    fn feed(tap: &BeatTap, track: u64, start: u64, audio: &[f64]) -> Vec<Window> {
        let mut windows = Vec::new();
        for (n, chunk) in audio.chunks(CHUNK * 2).enumerate() {
            let buf = PcmBuffer::F32(chunk.iter().map(|&s| s as f32).collect());
            if let Some(w) = tap.push(track, start + (n * CHUNK) as u64, &buf, 2, RATE) {
                windows.push(w);
            }
        }
        windows
    }

    fn octave_match(found: f64, expected: f64) -> bool {
        [0.5, 1.0, 2.0].iter().any(|m| (found / (expected * m) - 1.0).abs() < 0.01)
    }

    /// Distance (ms) from `anchor_ms` to the nearest beat of a grid with
    /// `first_ms` and `period_ms`.
    fn phase_error_ms(anchor_ms: u64, first_ms: f64, period_ms: f64) -> f64 {
        let phase = ((anchor_ms as f64 - first_ms) / period_ms).rem_euclid(1.0);
        phase.min(1.0 - phase) * period_ms
    }

    #[test]
    fn decimation_keeps_an_integer_rate_near_11_khz() {
        assert_eq!(decimation(44_100), 4);
        assert_eq!(decimation(48_000), 4);
        assert_eq!(decimation(96_000), 8);
        assert_eq!(decimation(192_000), 16);
        assert_eq!(decimation(22_050), 2);
        assert_eq!(decimation(8_000), 1);
    }

    #[test]
    fn the_first_window_finds_tempo_and_phase_across_tempi() {
        for (bpm, offset) in [(90.0, 0.31), (128.0, 0.12), (174.0, 0.05)] {
            let tap = enabled();
            let audio = drums(bpm, FIRST_WINDOW_S as f64 + 0.5, offset, bpm as u64);
            let windows = feed(&tap, 7, 0, &audio);
            assert_eq!(windows.len(), 1, "{bpm}: one window from the first {FIRST_WINDOW_S} s");
            let window = &windows[0];
            assert_eq!(window.rate, 11_025);
            assert_eq!(window.samples.len(), FIRST_WINDOW_S as usize * 11_025);
            let clock = analyze(window).expect("clock");
            assert_eq!(clock.track_id, 7);
            let found = clock.bpm.expect("confident") as f64;
            assert!(octave_match(found, bpm), "{bpm}: found {found:.2}");
            let period = (60_000.0 / bpm).min(clock.period_ms as f64);
            let error = phase_error_ms(clock.anchor_ms, offset * 1000.0, period);
            assert!(error < 30.0, "{bpm}: anchor {} off by {error:.1} ms", clock.anchor_ms);
        }
    }

    #[test]
    fn a_dense_band_mix_keeps_a_usable_clock() {
        let tap = enabled();
        let audio = band(150.0, FIRST_WINDOW_S as f64 + 0.5, 0.2, 0.35, 190);
        let window = feed(&tap, 1, 0, &audio).pop().expect("window");
        let clock = analyze(&window).expect("clock");
        assert!(octave_match(clock.bpm.expect("confident") as f64, 150.0));
        let period = 400.0f64.min(clock.period_ms as f64);
        assert!(phase_error_ms(clock.anchor_ms, 200.0, period) < 30.0);
    }

    #[test]
    fn anchors_are_in_track_time_when_the_tap_starts_mid_track() {
        let tap = enabled();
        let start_s = 30.0;
        let (bpm, offset) = (128.0, 0.12);
        let audio = drums(bpm, FIRST_WINDOW_S as f64 + 0.5, offset, 3);
        let window = feed(&tap, 2, (start_s * RATE as f64) as u64, &audio).pop().expect("window");
        assert!((window.start_ms - 30_000.0).abs() < 1.0, "window starts at {}", window.start_ms);
        let clock = analyze(&window).expect("clock");
        assert!(clock.anchor_ms >= 30_000, "anchor {} is track time", clock.anchor_ms);
        let period = (60_000.0 / bpm).min(clock.period_ms as f64);
        let error = phase_error_ms(clock.anchor_ms, (start_s + offset) * 1000.0, period);
        assert!(error < 30.0, "anchor {} off by {error:.1} ms", clock.anchor_ms);
    }

    #[test]
    fn a_seek_restarts_the_window_at_the_new_position() {
        let tap = enabled();
        let audio = drums(120.0, 4.0, 0.0, 4);
        assert!(feed(&tap, 3, 0, &audio).is_empty());
        // 4 s played, then a seek to 60 s: the earlier 4 s must not count.
        let seek = 60 * RATE as u64;
        let after = drums(120.0, 6.0, 0.0, 5);
        assert!(feed(&tap, 3, seek, &after).is_empty(), "pre-seek audio was counted");
        let more = drums(120.0, 2.5, 0.0, 6);
        let window = feed(&tap, 3, seek + 6 * RATE as u64, &more).pop().expect("window");
        assert!((window.start_ms - 60_000.0).abs() < 1.0, "window starts at {}", window.start_ms);
    }

    #[test]
    fn a_new_track_restarts_the_window() {
        let tap = enabled();
        assert!(feed(&tap, 1, 0, &drums(120.0, 6.0, 0.0, 1)).is_empty());
        assert!(feed(&tap, 2, 0, &drums(120.0, 6.0, 0.0, 2)).is_empty(), "track 1 audio was counted");
        let window = feed(&tap, 2, 6 * RATE as u64, &drums(120.0, 2.5, 0.0, 3)).pop().expect("window");
        assert_eq!(window.track_id, 2);
        assert_eq!(window.start_ms, 0.0);
    }

    #[test]
    fn the_second_window_follows_the_first_and_later_ones_the_refresh_gap() {
        let tap = enabled();
        let silence = vec![0.0f64; RATE as usize * 2 * 55];
        let mut completed = Vec::new();
        for (n, chunk) in silence.chunks(CHUNK * 2).enumerate() {
            let buf = PcmBuffer::F32(chunk.iter().map(|&s| s as f32).collect());
            let start = (n * CHUNK) as u64;
            if let Some(w) = tap.push(9, start, &buf, 2, RATE) {
                completed.push((start + (chunk.len() / 2) as u64, w));
            }
        }
        assert_eq!(completed.len(), 3);
        let (end_first, first) = &completed[0];
        let (end_second, second) = &completed[1];
        let (end_third, third) = &completed[2];
        assert_eq!(first.samples.len(), FIRST_WINDOW_S as usize * 11_025);
        assert_eq!(second.samples.len(), WINDOW_S as usize * 11_025);
        assert_eq!(third.samples.len(), WINDOW_S as usize * 11_025);
        let first_s = FIRST_WINDOW_S as f64;
        assert!((first.start_ms - 0.0).abs() < 1.0);
        assert!((second.start_ms - first_s * 1000.0).abs() < 1.0, "second starts at {}", second.start_ms);
        let third_s = first_s + REFRESH_S as f64;
        assert!((third.start_ms - third_s * 1000.0).abs() < 1.0, "third starts at {}", third.start_ms);
        let secs = |frame: &u64| *frame as f64 / RATE as f64;
        assert!((secs(end_first) - first_s).abs() < 0.1);
        assert!((secs(end_second) - (first_s + WINDOW_S as f64)).abs() < 0.1);
        assert!((secs(end_third) - (third_s + WINDOW_S as f64)).abs() < 0.1);
    }

    fn buffers(audio: &[f64]) -> Vec<PcmBuffer> {
        audio.chunks(CHUNK * 2).map(|c| PcmBuffer::F32(c.iter().map(|&s| s as f32).collect())).collect()
    }

    #[test]
    fn audio_queued_ahead_makes_the_first_window_without_waiting_for_playback() {
        let (bpm, offset) = (128.0, 0.12);
        let audio = drums(bpm, FIRST_WINDOW_S as f64 + 0.5, offset, 11);
        let played = feed(&enabled(), 6, 0, &audio).pop().expect("window from playback");

        let tap = enabled();
        assert!(tap.wants_ahead(6, RATE));
        let bufs = buffers(&audio);
        let ahead = tap.push_ahead(6, 0, &bufs, 2, RATE).expect("window from the queued audio");
        assert_eq!(ahead, played, "the same window as playback would have made");
        assert!(!tap.wants_ahead(6, RATE), "the first window is done");
        // Playing those frames analyzes nothing again.
        assert!(feed(&tap, 6, 0, &audio).is_empty());
    }

    #[test]
    fn after_a_window_from_ahead_the_next_starts_where_it_ended() {
        let tap = enabled();
        let silence = vec![0.0f64; RATE as usize * 2 * 22];
        let first = tap.push_ahead(3, 0, &buffers(&silence[..RATE as usize * 2 * 9]), 2, RATE).expect("first");
        assert_eq!(first.start_ms, 0.0);
        let windows = feed(&tap, 3, 0, &silence);
        assert_eq!(windows.len(), 1);
        assert!((windows[0].start_ms - FIRST_WINDOW_S as f64 * 1000.0).abs() < 1.0, "starts at {}", windows[0].start_ms);
    }

    #[test]
    fn too_little_audio_ahead_changes_nothing() {
        let tap = enabled();
        let audio = drums(120.0, 6.0, 0.0, 12);
        // Playback has collected 6 s of the first window.
        assert!(feed(&tap, 8, 0, &audio).is_empty());
        let short = drums(120.0, 1.0, 0.0, 13);
        assert!(tap.push_ahead(8, 6 * RATE as u64, &buffers(&short), 2, RATE).is_none());
        assert!(tap.wants_ahead(8, RATE));
        // The partial window is intact: 2.5 s more completes it.
        let window = feed(&tap, 8, 6 * RATE as u64, &drums(120.0, 2.5, 0.0, 14)).pop().expect("window");
        assert_eq!(window.start_ms, 0.0);
    }

    #[test]
    fn a_disabled_tap_wants_nothing_ahead() {
        let tap = BeatTap::new();
        assert!(!tap.wants_ahead(1, RATE));
        let bufs = buffers(&drums(120.0, 9.0, 0.0, 1));
        assert!(tap.push_ahead(1, 0, &bufs, 2, RATE).is_none());
    }

    #[test]
    fn a_disabled_tap_ignores_pushes() {
        let tap = BeatTap::new();
        assert!(feed(&tap, 1, 0, &drums(120.0, 9.0, 0.0, 1)).is_empty());
        tap.set_enabled(true);
        tap.set_enabled(false);
        assert!(feed(&tap, 1, 0, &drums(120.0, 9.0, 0.0, 1)).is_empty());
    }

    #[test]
    fn disabling_discards_a_partial_window() {
        let tap = enabled();
        assert!(feed(&tap, 1, 0, &drums(120.0, 6.0, 0.0, 1)).is_empty());
        tap.set_enabled(false);
        tap.set_enabled(true);
        assert!(feed(&tap, 1, 6 * RATE as u64, &drums(120.0, 6.0, 0.0, 2)).is_empty());
    }

    #[test]
    fn push_never_waits_for_a_held_lock() {
        let tap = enabled();
        let guard = tap.state.lock().unwrap();
        let buf = PcmBuffer::F32(vec![0.5; 2 * CHUNK]);
        assert!(tap.push(1, 0, &buf, 2, RATE).is_none());
        drop(guard);
    }

    #[test]
    fn integer_buffers_are_analyzed_like_float_ones() {
        let tap = enabled();
        let audio = drums(128.0, FIRST_WINDOW_S as f64 + 0.5, 0.12, 128);
        let mut window = None;
        for (n, chunk) in audio.chunks(CHUNK * 2).enumerate() {
            let buf = PcmBuffer::I32(chunk.iter().map(|&s| (s.clamp(-1.0, 1.0) * i32::MAX as f64) as i32).collect());
            window = window.or(tap.push(4, (n * CHUNK) as u64, &buf, 2, RATE));
        }
        let clock = analyze(&window.expect("window")).expect("clock");
        assert!(octave_match(clock.bpm.expect("confident") as f64, 128.0));
    }

    #[test]
    fn a_held_tone_claims_no_beat() {
        let tap = enabled();
        let frames = (FIRST_WINDOW_S as f64 + 0.5) * RATE as f64;
        let tone: Vec<f64> = (0..frames as usize)
            .flat_map(|n| {
                let s = 0.3 * (2.0 * std::f64::consts::PI * 220.0 * n as f64 / RATE as f64).sin();
                [s, s]
            })
            .collect();
        let window = feed(&tap, 5, 0, &tone).pop().expect("window");
        assert!(analyze(&window).is_none_or(|clock| clock.bpm.is_none()));
    }
}
