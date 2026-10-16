//! Shared ring transfer for the output backends (WASAPI, ASIO): the playback
//! worker pushes sample words into an `rtrb` ring that a device thread or
//! callback drains. Track 13 (EF-D5).

use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer};

use crate::error::{Error, Result};

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
    fn waits_scale_with_the_needed_room_and_stay_bounded() {
        assert_eq!(wait_for(441, 44_100), Duration::from_millis(10));
        assert_eq!(wait_for(1, 44_100), MIN_WAIT);
        assert_eq!(wait_for(1_000_000, 44_100), MAX_WAIT);
        assert_eq!(wait_for(10, 0), MIN_WAIT);
    }
}
