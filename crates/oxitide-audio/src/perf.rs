//! Performance events emitted by the audio engine. The app installs a hook to
//! timestamp them; the audio crate stays independent of any metrics backend.

use std::sync::Arc;

/// A milestone the engine reports to an installed [`PerfHook`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioPerfEvent {
    /// The decode thread is about to ask the resolver for the stream plan.
    ResolveStart { track_id: u64 },
    /// The manifest resolved (playback info fetched and parsed).
    DecodeStart { track_id: u64 },
    /// Stream info is available (the fetch produced enough to decode).
    InfoReady { track_id: u64 },
    /// The output device accepted and started the stream.
    OutputOpened { track_id: u64 },
    /// A usable disk-cache entry was found for the track.
    CacheHit { track_id: u64 },
    /// No usable cache entry: no cache key, no entry, or a failed hit.
    CacheMiss { track_id: u64 },
    /// The first PCM buffer of a track was written to the output.
    FirstPcm { track_id: u64 },
    /// A different track started after one ended; `gap_ms` is the time since
    /// the previous track's last PCM write.
    Advance { track_id: u64, gap_ms: u64 },
}

/// Called on the worker/decode threads; must be cheap and non-blocking.
pub type PerfHook = Arc<dyn Fn(AudioPerfEvent) + Send + Sync>;
