//! Render-time software volume: a short linear ramp between gains, so a
//! volume change never steps the waveform (the step is an audible click) and
//! is applied where samples leave for the device, not where they are queued.

/// How long a software gain change takes. Short enough to feel instant, long
/// enough that the change carries no click.
pub(crate) const RAMP_MS: u32 = 30;

/// A gain that glides to each new target over [`RAMP_MS`]. Owned by the
/// thread that hands samples to the device; it starts settled, so an open
/// never fades in.
#[derive(Debug, Clone)]
pub(crate) struct GainRamp {
    current: f32,
    target: f32,
    /// Change per frame while ramping.
    step: f32,
    /// Frames left in the ramp; 0 once settled.
    remaining: u32,
    ramp_frames: u32,
}

impl GainRamp {
    pub(crate) fn new(gain: f32, rate: u32) -> Self {
        let ramp_frames = (u64::from(rate) * u64::from(RAMP_MS) / 1000).max(1) as u32;
        Self { current: gain, target: gain, step: 0.0, remaining: 0, ramp_frames }
    }

    /// Glide from wherever the gain is now (mid-ramp included) to `target`.
    pub(crate) fn retarget(&mut self, target: f32) {
        if (target - self.target).abs() < f32::EPSILON {
            return;
        }
        self.target = target;
        self.remaining = self.ramp_frames;
        self.step = (target - self.current) / self.ramp_frames as f32;
    }

    /// Settled at unity: samples pass through untouched (bit-perfect).
    pub(crate) fn is_identity(&self) -> bool {
        self.remaining == 0 && self.current == 1.0
    }

    fn next_gain(&mut self) -> f32 {
        if self.remaining > 0 {
            self.remaining -= 1;
            // The last frame lands exactly on the target, so a ramp to unity
            // ends in a true pass-through.
            self.current = if self.remaining == 0 { self.target } else { self.current + self.step };
        }
        self.current
    }

    /// Scale interleaved frames of `frame_len` items each through `scale`,
    /// one gain per frame so every channel moves together. A no-op when
    /// settled at unity.
    pub(crate) fn apply_frames<T>(
        &mut self,
        samples: &mut [T],
        frame_len: usize,
        mut scale: impl FnMut(&mut [T], f32),
    ) {
        if self.is_identity() {
            return;
        }
        for frame in samples.chunks_mut(frame_len.max(1)) {
            let gain = self.next_gain();
            scale(frame, gain);
        }
    }
}

/// Scale left-aligned `i32` samples.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn scale_i32(frame: &mut [i32], gain: f32) {
    for sample in frame {
        *sample = (*sample as f64 * gain as f64) as i32;
    }
}

/// How a ring word encodes its sample.
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordKind {
    /// `f32` bits.
    F32,
    /// A left-aligned `i32`.
    I32,
}

#[cfg(windows)]
fn scale_word(word: u32, gain: f32, kind: WordKind) -> u32 {
    match kind {
        WordKind::F32 => (f32::from_bits(word) * gain).to_bits(),
        WordKind::I32 => ((word as i32) as f64 * gain as f64) as i32 as u32,
    }
}

/// Scale ring words.
#[cfg(windows)]
pub(crate) fn scale_words(frame: &mut [u32], gain: f32, kind: WordKind) {
    for word in frame {
        *word = scale_word(*word, gain, kind);
    }
}

/// Scale ring words already laid out as little-endian bytes for the device.
#[cfg(windows)]
pub(crate) fn scale_le_words(frame: &mut [u8], gain: f32, kind: WordKind) {
    for bytes in frame.as_chunks_mut::<4>().0 {
        *bytes = scale_word(u32::from_le_bytes(*bytes), gain, kind).to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ramp: &mut GainRamp, frames: usize) -> Vec<f32> {
        let mut samples = vec![1.0f32; frames];
        ramp.apply_frames(&mut samples, 1, |frame, gain| frame[0] *= gain);
        samples
    }

    #[test]
    fn a_new_ramp_is_settled() {
        let mut ramp = GainRamp::new(0.25, 1000);
        assert_eq!(run(&mut ramp, 3), vec![0.25; 3]);
        assert!(GainRamp::new(1.0, 1000).is_identity());
        assert!(!GainRamp::new(0.5, 1000).is_identity());
    }

    #[test]
    fn a_change_glides_over_the_ramp_and_lands_exactly() {
        // 1 kHz: a 30 ms ramp is 30 frames.
        let mut ramp = GainRamp::new(1.0, 1000);
        ramp.retarget(0.1);
        let out = run(&mut ramp, 40);
        let max_step = (1.0 - 0.1) / 30.0 + 1e-6;
        let mut last = 1.0;
        for (i, &gain) in out.iter().enumerate() {
            assert!(gain <= last, "frame {i} rose");
            assert!(last - gain <= max_step, "frame {i} jumped {}", last - gain);
            last = gain;
        }
        assert_eq!(out[29], 0.1, "the ramp ends on the target");
        assert!(out[30..].iter().all(|&g| g == 0.1));
    }

    #[test]
    fn a_ramp_back_to_unity_becomes_a_pass_through() {
        let mut ramp = GainRamp::new(0.5, 1000);
        ramp.retarget(1.0);
        assert!(!ramp.is_identity());
        run(&mut ramp, 30);
        assert!(ramp.is_identity());
        let mut words = vec![i32::MAX, i32::MIN, 12_345];
        ramp.apply_frames(&mut words, 1, scale_i32);
        assert_eq!(words, vec![i32::MAX, i32::MIN, 12_345], "unity must be sample-exact");
    }

    #[test]
    fn a_retarget_mid_ramp_starts_from_where_it_is() {
        let mut ramp = GainRamp::new(1.0, 1000);
        ramp.retarget(0.0);
        let first = run(&mut ramp, 15);
        let midway = *first.last().unwrap();
        assert!((midway - 0.5).abs() < 0.01, "{midway}");
        ramp.retarget(1.0);
        let second = run(&mut ramp, 30);
        assert!(second[0] > midway && second[0] - midway < 0.02, "no jump: {}", second[0]);
        assert_eq!(second[29], 1.0);
    }

    #[test]
    fn every_channel_of_a_frame_gets_the_same_gain() {
        let mut ramp = GainRamp::new(1.0, 1000);
        ramp.retarget(0.0);
        let mut stereo = vec![1.0f32; 8];
        ramp.apply_frames(&mut stereo, 2, |frame, gain| frame.iter_mut().for_each(|s| *s *= gain));
        for pair in stereo.chunks(2) {
            assert_eq!(pair[0], pair[1]);
        }
    }

    #[cfg(windows)]
    #[test]
    fn words_scale_in_their_own_encoding() {
        let mut floats = vec![0.5f32.to_bits(), (-1.0f32).to_bits()];
        scale_words(&mut floats, 0.5, WordKind::F32);
        assert_eq!(floats, vec![0.25f32.to_bits(), (-0.5f32).to_bits()]);
        let mut ints = vec![1000i32 as u32, -1000i32 as u32];
        scale_words(&mut ints, 0.5, WordKind::I32);
        assert_eq!(ints, vec![500i32 as u32, -500i32 as u32]);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0.5f32.to_bits().to_le_bytes());
        scale_le_words(&mut bytes, 0.5, WordKind::F32);
        assert_eq!(f32::from_bits(u32::from_le_bytes(bytes[..4].try_into().unwrap())), 0.25);
    }
}
