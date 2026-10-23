//! Shared ring transfer for the output backends (WASAPI, ASIO): the playback
//! worker pushes sample words into an `rtrb` ring that a device thread or
//! callback drains. Track 13 (EF-D5).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer};

use crate::error::{Error, Result};

/// Counts the times the ring ran dry under a device that was being fed: a
/// device period that came up short after audio was written. One dry-out
/// counts once however many silent periods follow, until the next write
/// re-arms it. A ring that is empty because nothing was written yet (a fresh
/// open, after a flush) does not count; the engine discards dry-outs it
/// caused itself (pause, seek), see `engine::UnderrunTally`.
#[derive(Debug, Default)]
pub(crate) struct Starvation {
    armed: AtomicBool,
    count: AtomicU64,
}

impl Starvation {
    /// The writer pushed audio: the next short period is a dry-out.
    pub fn arm(&self) {
        self.armed.store(true, Ordering::Relaxed);
    }

    /// The ring was emptied on purpose (flush, open).
    pub fn disarm(&self) {
        self.armed.store(false, Ordering::Relaxed);
    }

    /// The device took `taken` of the `wanted` frames of one period.
    pub fn observe(&self, taken: usize, wanted: usize) {
        if taken < wanted && self.armed.swap(false, Ordering::Relaxed) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

/// A ring that has not drained for this long means the device stalled.
const STALL: Duration = Duration::from_secs(2);
/// Bounds for one wait on a full ring. The upper bound stays far below any
/// ring's length, so the device never runs dry while the worker sleeps.
const MIN_WAIT: Duration = Duration::from_millis(1);
const MAX_WAIT: Duration = Duration::from_millis(20);

/// Push `samples`, converted by `word`, into the ring in chunks. When the
/// ring is full, sleep about as long as the device needs to free a quarter of
/// it (`samples_per_sec` is rate × channels) instead of polling every couple
/// of milliseconds, and fail once it has not drained for [`STALL`].
pub(crate) fn push_samples<T: Copy>(
    producer: &mut Producer<u32>,
    samples: &[T],
    samples_per_sec: usize,
    word: impl Fn(T) -> u32,
) -> Result<()> {
    let quarter = (producer.buffer().capacity() / 4).max(1);
    let mut rest = samples;
    let mut stalled_since: Option<Instant> = None;
    while !rest.is_empty() {
        let free = producer.slots();
        if free == 0 {
            let since = *stalled_since.get_or_insert_with(Instant::now);
            if since.elapsed() > STALL {
                return Err(Error::Output("Audio device has been disconnected or stalled".into()));
            }
            std::thread::sleep(wait_for(rest.len().min(quarter), samples_per_sec));
            continue;
        }
        stalled_since = None;
        let n = free.min(rest.len());
        let chunk = producer
            .write_chunk_uninit(n)
            .map_err(|e| Error::Output(format!("output ring: {e}")))?;
        chunk.fill_from_iter(rest[..n].iter().map(|&s| word(s)));
        rest = &rest[n..];
    }
    Ok(())
}

/// How long the device takes to play `samples` words, clamped.
fn wait_for(samples: usize, samples_per_sec: usize) -> Duration {
    if samples_per_sec == 0 {
        return MIN_WAIT;
    }
    let micros = samples as u64 * 1_000_000 / samples_per_sec as u64;
    Duration::from_micros(micros).clamp(MIN_WAIT, MAX_WAIT)
}

/// How long the words queued in a ring take to play, in ms: `capacity`
/// minus the producer's `free_slots`, at `samples_per_sec` (rate × channels).
pub(crate) fn queued_ms(capacity: usize, free_slots: usize, samples_per_sec: usize) -> u32 {
    if samples_per_sec == 0 {
        return 0;
    }
    let queued = capacity.saturating_sub(free_slots) as u64;
    (queued * 1000 / samples_per_sec as u64).min(u32::MAX as u64) as u32
}

/// Fill `out` (little-endian words) from the ring: whole frames only, so an
/// underrun never shifts the channels, and silence (all-zero bytes, which is
/// 0.0 f32 and 0 i32) for whatever the ring cannot cover. `out` is reused by
/// the caller across device periods. Returns the frames taken from the ring.
pub(crate) fn pop_frames_le(consumer: &mut Consumer<u32>, channels: usize, out: &mut [u8]) -> usize {
    let channels = channels.max(1);
    let wanted = out.len() / 4;
    let available = consumer.slots().min(wanted);
    let count = available - available % channels;
    if count > 0 {
        if let Ok(chunk) = consumer.read_chunk(count) {
            let (first, second) = chunk.as_slices();
            for (slot, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(first.iter().chain(second)) {
                slot.copy_from_slice(&word.to_le_bytes());
            }
            chunk.commit_all();
        }
    }
    out[count * 4..].fill(0);
    count / channels
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    #[test]
    fn pushes_and_pops_in_order_with_silence_padding() {
        let (mut producer, mut consumer) = RingBuffer::<u32>::new(16);
        push_samples(&mut producer, &[1i32, 2, 3, 4], 44_100 * 2, |s| s as u32).unwrap();
        let mut out = vec![0xAAu8; 6 * 4];
        assert_eq!(pop_frames_le(&mut consumer, 2, &mut out), 2);
        let words: Vec<u32> = out.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
        assert_eq!(words, vec![1, 2, 3, 4, 0, 0]);
    }

    #[test]
    fn an_underrun_never_splits_a_frame() {
        let (mut producer, mut consumer) = RingBuffer::<u32>::new(16);
        push_samples(&mut producer, &[1u32, 2, 3], 1000, |s| s).unwrap();
        let mut out = vec![0xAAu8; 4 * 4];
        assert_eq!(pop_frames_le(&mut consumer, 2, &mut out), 1, "only one whole stereo frame");
        assert_eq!(&out[8..], &[0u8; 8]);
        // The odd word stays queued and starts the next period on the left.
        push_samples(&mut producer, &[4u32], 1000, |s| s).unwrap();
        pop_frames_le(&mut consumer, 2, &mut out);
        assert_eq!(u32::from_le_bytes(out[0..4].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(out[4..8].try_into().unwrap()), 4);
    }

    #[test]
    fn a_full_ring_waits_for_the_consumer() {
        let (mut producer, mut consumer) = RingBuffer::<u32>::new(8);
        let drain = std::thread::spawn(move || {
            let mut out = vec![0u8; 8 * 4];
            let mut got = 0;
            while got < 64 {
                got += pop_frames_le(&mut consumer, 1, &mut out);
                std::thread::sleep(Duration::from_millis(1));
            }
            got
        });
        let samples: Vec<u32> = (0..64).collect();
        push_samples(&mut producer, &samples, 8_000, |s| s).unwrap();
        assert_eq!(drain.join().unwrap(), 64);
    }

    #[test]
    fn queued_ms_counts_the_words_not_yet_played() {
        // A 200 ms stereo 48 kHz ring (19 200 words).
        assert_eq!(queued_ms(19_200, 0, 96_000), 200);
        assert_eq!(queued_ms(19_200, 19_200, 96_000), 0);
        assert_eq!(queued_ms(19_200, 4_800, 96_000), 150);
        assert_eq!(queued_ms(19_200, 0, 0), 0);
        let (mut producer, _consumer) = RingBuffer::<u32>::new(8_820);
        push_samples(&mut producer, &[0u32; 4_410], 88_200, |s| s).unwrap();
        assert_eq!(queued_ms(producer.buffer().capacity(), producer.slots(), 88_200), 50);
    }

    #[test]
    fn a_dry_out_counts_once_and_only_after_a_write() {
        let starvation = Starvation::default();
        // Nothing written yet: an empty ring is not an underrun.
        starvation.observe(0, 480);
        assert_eq!(starvation.count(), 0);
        starvation.arm();
        starvation.observe(480, 480);
        assert_eq!(starvation.count(), 0, "a full period is not a dry-out");
        starvation.observe(100, 480);
        starvation.observe(0, 480);
        starvation.observe(0, 480);
        assert_eq!(
            starvation.count(),
            1,
            "one dry-out, however many silent periods"
        );
        starvation.arm();
        starvation.observe(0, 480);
        assert_eq!(starvation.count(), 2);
        starvation.arm();
        starvation.disarm();
        starvation.observe(0, 480);
        assert_eq!(starvation.count(), 2, "a flush empties the ring on purpose");
    }

    #[test]
    fn waits_scale_with_the_needed_room_and_stay_bounded() {
        assert_eq!(wait_for(441, 44_100), Duration::from_millis(10));
        assert_eq!(wait_for(1, 44_100), MIN_WAIT);
        assert_eq!(wait_for(1_000_000, 44_100), MAX_WAIT);
        assert_eq!(wait_for(10, 0), MIN_WAIT);
    }
}
