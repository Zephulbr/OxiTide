use crate::source::StreamPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekTarget {
    pub segment_index: usize,
    pub discard_frames: u64,
    pub position_frames: u64,
}

/// Frames for a millisecond position at `sample_rate`.
pub fn target_frames(position_ms: u64, sample_rate: u32) -> u64 {
    position_ms.saturating_mul(sample_rate as u64) / 1000
}

/// Pick the segment covering `position_ms` and how much of it to discard.
/// Positions beyond the last segment clamp to that segment.
pub fn target_segment(plan: &StreamPlan, position_ms: u64, sample_rate: u32) -> SeekTarget {
    let frames = target_frames(position_ms, sample_rate);
    if plan.segments.is_empty() || plan.timescale == 0 {
        // A single-file source or unknown timing: decode from the start and
        // discard everything before the target.
        return SeekTarget { segment_index: 0, discard_frames: frames, position_frames: frames };
    }
    let mut index = 0;
    let mut start = 0u64;
    for (i, segment) in plan.segments.iter().enumerate() {
        let segment_start =
            segment.start_ticks.saturating_mul(sample_rate as u64) / plan.timescale;
        if segment_start <= frames {
            index = i;
            start = segment_start;
        } else {
            break;
        }
    }
    SeekTarget { segment_index: index, discard_frames: frames - start, position_frames: frames }
}

/// The track frame where a source begins decoding. A cached file contains the
/// track from its first segment; a network stream truncated by a seek begins
/// at the target segment.
pub fn source_start_frames(plan: &StreamPlan) -> u64 {
    if plan.timescale == 0 {
        return 0;
    }
    plan.sample_rate
        .and_then(|rate| {
            plan.segments
                .first()
                .map(|s| s.start_ticks.saturating_mul(rate as u64) / plan.timescale)
        })
        .unwrap_or(0)
}

/// How much of the decoded stream to drop before playback: everything between
/// where the source actually starts and the seek target.
pub fn discard_frames_for(target: &SeekTarget, source_start_frames: u64) -> u64 {
    target.position_frames.saturating_sub(source_start_frames)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Discarder {
    remaining_frames: u64,
}

impl Discarder {
    pub fn new(frames: u64) -> Self {
        Self { remaining_frames: frames }
    }

    /// Returns `(offset, len)` for the part of an interleaved buffer that
    /// should be used after dropping the pending frames. Operates on sample
    /// counts so it does not care about the sample type.
    pub fn apply(&mut self, total_samples: usize, channels: u16) -> (usize, usize) {
        let channels = channels.max(1) as usize;
        let available_frames = (total_samples / channels) as u64;
        if self.remaining_frames >= available_frames {
            self.remaining_frames -= available_frames;
            return (total_samples, 0);
        }
        let skip = self.remaining_frames as usize * channels;
        self.remaining_frames = 0;
        (skip, total_samples - skip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Segment;

    fn plan_with_segments(timescale: u64, segments: &[(u64, u64)]) -> StreamPlan {
        StreamPlan {
            init_url: Some("init".into()),
            segments: segments
                .iter()
                .enumerate()
                .map(|(i, &(start_ticks, duration_ticks))| Segment {
                    url: format!("seg-{i}"),
                    start_ticks,
                    duration_ticks,
                })
                .collect(),
            timescale,
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
        }
    }

    #[test]
    fn target_frames_rounds_down() {
        assert_eq!(target_frames(0, 44100), 0);
        assert_eq!(target_frames(1000, 44100), 44_100);
        assert_eq!(target_frames(1500, 44100), 66_150);
    }

    /// Duration-based MPD: timescale 1, 4-second segments; sample rate 1000 so
    /// frames == milliseconds.
    #[test]
    fn duration_based_target_uses_segment_math() {
        let plan = plan_with_segments(1, &[(0, 4), (4, 4), (8, 4)]);
        let target = target_segment(&plan, 5000, 1000);
        assert_eq!(target.segment_index, 1);
        assert_eq!(target.discard_frames, 1000);
        assert_eq!(target.position_frames, 5000);

        let start = target_segment(&plan, 4000, 1000);
        assert_eq!(start.segment_index, 1);
        assert_eq!(start.discard_frames, 0);

        let first = target_segment(&plan, 0, 1000);
        assert_eq!(first.segment_index, 0);
        assert_eq!(first.discard_frames, 0);
    }

    /// Timeline MPD: timescale 44100, 176400-tick segments.
    #[test]
    fn timeline_target_uses_timescale() {
        let plan = plan_with_segments(44100, &[(0, 176400), (176400, 176400), (352800, 176400)]);
        let target = target_segment(&plan, 2000, 44100);
        assert_eq!(target.segment_index, 0);
        assert_eq!(target.discard_frames, 88_200);
        assert_eq!(target.position_frames, 88_200);

        let second = target_segment(&plan, 5000, 44100);
        assert_eq!(second.segment_index, 1);
        assert_eq!(second.discard_frames, 44_100);
    }

    /// Review Focus: seeking beyond the track clamps to the last segment.
    #[test]
    fn target_segment_clamps_beyond_the_end() {
        let plan = plan_with_segments(1, &[(0, 4), (4, 4)]);
        let target = target_segment(&plan, 999_999, 1000);
        assert_eq!(target.segment_index, 1);
        assert_eq!(target.discard_frames, 999_999 - 4000);
        assert_eq!(target.position_frames, 999_999);
    }

    #[test]
    fn single_file_source_discards_from_the_start() {
        let plan = StreamPlan::single("file".into());
        let target = target_segment(&plan, 1000, 44100);
        assert_eq!(target.segment_index, 0);
        assert_eq!(target.discard_frames, 44_100);
    }

    #[test]
    fn discarder_drops_partial_buffers_across_calls() {
        let mut discarder = Discarder::new(5);
        // 8 samples = 4 stereo frames: all dropped, one frame still pending.
        assert_eq!(discarder.apply(8, 2), (8, 0));
        // Next buffer: 1 frame dropped, the remaining 3 frames are used.
        assert_eq!(discarder.apply(8, 2), (2, 6));
        // Once drained, buffers pass through untouched.
        assert_eq!(discarder.apply(8, 2), (0, 8));
    }

    #[test]
    fn source_start_frames_is_the_first_segment_start() {
        let timed = plan_with_segments(44100, &[(88_200, 88_200), (176_400, 88_200)]);
        assert_eq!(source_start_frames(&timed), 88_200);
        let untimed = plan_with_segments(0, &[(0, 0)]);
        assert_eq!(source_start_frames(&untimed), 0);
        let empty = plan_with_segments(1, &[]);
        assert_eq!(source_start_frames(&empty), 0);
    }

    #[test]
    fn discard_is_relative_to_the_actual_source_start() {
        let plan = plan_with_segments(1000, &[(0, 2000), (2000, 2000)]);
        let target = target_segment(&plan, 2500, 1000);
        assert_eq!(target.segment_index, 1);
        assert_eq!(target.discard_frames, 500);
        assert_eq!(target.position_frames, 2500);
        // Network path: the source begins at the target segment.
        assert_eq!(discard_frames_for(&target, 2000), 500);
        // Cached path: the source begins at the track's first frame.
        assert_eq!(discard_frames_for(&target, source_start_frames(&plan)), 2500);
    }
}
