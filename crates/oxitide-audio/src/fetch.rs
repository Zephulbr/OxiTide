use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::cache::CacheWriter;
use crate::error::{Error, Result};
use crate::source::StreamPlan;

pub trait Fetcher: Send + Sync + 'static {
    fn get(&self, url: &str) -> Result<Vec<u8>>;
}

pub struct HttpFetcher {
    client: reqwest::blocking::Client,
}

/// How many times a single `get` is attempted before giving up.
const MAX_ATTEMPTS: u32 = 3;
/// Cap on any single retry wait (e.g. a large `Retry-After`).
const MAX_RETRY_WAIT: Duration = Duration::from_secs(5);
/// Cap on unconsumed buffered bytes; the downloader waits for the decoder to
/// catch up before fetching another segment.
pub const MAX_BUFFERED_BYTES: usize = 2 * 1024 * 1024;

impl HttpFetcher {
    pub fn new() -> Result<Self> {
        // Idle sockets are retired before routers/NATs silently drop them
        // (see `oxitide_tidal::http_client`); `get_with_retry` covers the rest.
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Fetch(describe(e)))?;
        Ok(Self { client })
    }

    /// One HTTP attempt. `Ok` carries the body; `Err((error, Some(wait)))` is a
    /// retryable failure with the wait before the next attempt, and
    /// `Err((error, None))` is fatal.
    fn get_once(
        &self,
        url: &str,
        attempt: u32,
    ) -> std::result::Result<Vec<u8>, (Error, Option<Duration>)> {
        let resp = self
            .client
            .get(url)
            .send()
            .map_err(|e| (Error::Fetch(describe(e)), None))?;
        let status = resp.status();
        if !status.is_success() {
            let message = format!("status {status} for {}", url.split('?').next().unwrap_or(url));
            let retryable = status.as_u16() == 429 || status.is_server_error();
            if !retryable {
                return Err((Error::FetchRejected(message), None));
            }
            let err = Error::Fetch(message);
            let wait = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(Duration::from_secs)
                .unwrap_or_else(|| Duration::from_millis(200) * attempt)
                .min(MAX_RETRY_WAIT);
            return Err((err, Some(wait)));
        }
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| (Error::Fetch(describe(e)), None))
    }
}

/// reqwest's message plus each distinct cause, without the URL: segment URLs
/// carry signed CDN tokens, and this text reaches logs and the UI.
fn describe(e: reqwest::Error) -> String {
    let e = e.without_url();
    let mut out = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(cause) = source {
        let text = cause.to_string();
        if !out.contains(&text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        source = cause.source();
    }
    out
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new().expect("build http client")
    }
}

impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        let mut last = None;
        for attempt in 1..=MAX_ATTEMPTS {
            match self.get_once(url, attempt) {
                Ok(bytes) => return Ok(bytes),
                Err((err, wait)) => {
                    last = Some(err);
                    match wait {
                        Some(wait) if attempt < MAX_ATTEMPTS => std::thread::sleep(wait),
                        _ => break,
                    }
                }
            }
        }
        Err(last.expect("at least one attempt"))
    }
}

/// A `Fetcher` over a fixed in-memory map. Used by tests and the fixture pipeline.
#[derive(Default, Clone)]
pub struct MapFetcher {
    pub files: HashMap<String, Vec<u8>>,
}

impl MapFetcher {
    pub fn with(mut self, url: &str, bytes: Vec<u8>) -> Self {
        self.files.insert(url.to_string(), bytes);
        self
    }
}

impl Fetcher for MapFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.files
            .get(url)
            .cloned()
            .ok_or_else(|| Error::Fetch(format!("no fixture for {url}")))
    }
}

struct State {
    data: Vec<u8>,
    /// Absolute offset of `data[0]`; grows as consumed bytes are released.
    start: usize,
    done: bool,
    error: Option<Failure>,
}

/// Why the download stopped, kept so the decoder's error can be reported as
/// the network failure it was rather than a generic demux error.
#[derive(Clone)]
struct Failure {
    message: String,
    rejected: bool,
}

impl Failure {
    fn from_error(e: &Error) -> Self {
        match e {
            Error::FetchRejected(message) => Self { message: message.clone(), rejected: true },
            Error::Fetch(message) => Self { message: message.clone(), rejected: false },
            other => Self { message: other.to_string(), rejected: false },
        }
    }

    fn to_error(&self) -> Error {
        if self.rejected {
            Error::FetchRejected(self.message.clone())
        } else {
            Error::Fetch(self.message.clone())
        }
    }
}

pub struct StreamBuffer {
    state: Mutex<State>,
    cond: Condvar,
    cancelled: AtomicBool,
    total_pushed: AtomicU64,
}

impl StreamBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State { data: Vec::new(), start: 0, done: false, error: None }),
            cond: Condvar::new(),
            cancelled: AtomicBool::new(false),
            total_pushed: AtomicU64::new(0),
        })
    }

    pub fn push(&self, bytes: &[u8]) {
        if self.cancelled.load(Ordering::SeqCst) {
            return;
        }
        let mut s = self.state.lock().unwrap();
        if s.error.is_some() {
            return;
        }
        s.data.extend_from_slice(bytes);
        self.total_pushed.fetch_add(bytes.len() as u64, Ordering::SeqCst);
        self.cond.notify_all();
    }

    /// Total bytes pushed into the buffer (not the currently buffered count).
    pub fn total_pushed(&self) -> u64 {
        self.total_pushed.load(Ordering::SeqCst)
    }

    pub fn finish(&self) {
        let mut s = self.state.lock().unwrap();
        s.done = true;
        self.cond.notify_all();
    }

    /// End the stream with a download failure. Bytes already buffered are
    /// still served; readers see the error only once they reach the end.
    pub fn fail(&self, error: &Error) {
        let mut s = self.state.lock().unwrap();
        if s.error.is_none() {
            s.error = Some(Failure::from_error(error));
        }
        s.done = true;
        self.cond.notify_all();
    }

    /// The download failure, if the stream ended with one (not a cancel).
    pub fn failure(&self) -> Option<Error> {
        if self.is_cancelled() {
            return None;
        }
        self.state.lock().unwrap().error.as_ref().map(Failure::to_error)
    }

    /// Cancel the stream: blocked readers/downloaders wake and exit. Used when
    /// a track is superseded (next/prev/seek/clear).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let mut s = self.state.lock().unwrap();
        s.done = true;
        self.cond.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Sleep up to `wait`, waking early on cancel; false when cancelled.
    pub fn sleep_unless_cancelled(&self, wait: Duration) -> bool {
        let deadline = std::time::Instant::now() + wait;
        let mut s = self.state.lock().unwrap();
        loop {
            if self.is_cancelled() {
                return false;
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return true;
            }
            s = self.cond.wait_timeout(s, deadline - now).unwrap().0;
        }
    }

    pub fn is_done(&self) -> bool {
        self.state.lock().unwrap().done
    }

    /// Number of unconsumed bytes currently held in memory.
    pub fn buffered_bytes(&self) -> usize {
        self.state.lock().unwrap().data.len()
    }

    /// Release everything before absolute offset `pos`. The demuxer calls this
    /// after every read so a long stream never grows without bound.
    pub fn consume_to(&self, pos: usize) {
        let mut s = self.state.lock().unwrap();
        if pos <= s.start {
            return;
        }
        let drop = (pos - s.start).min(s.data.len());
        s.data.drain(..drop);
        s.start += drop;
        self.cond.notify_all();
    }

    /// Block until fewer than `cap` unconsumed bytes are buffered, or the
    /// stream is done/cancelled. Downloader backpressure.
    pub fn wait_room(&self, cap: usize) {
        let mut s = self.state.lock().unwrap();
        while s.data.len() >= cap && !s.done {
            s = self.cond.wait(s).unwrap();
        }
    }

    /// Block until `pos` is buffered or the stream ends. Returns the absolute
    /// end offset of the buffered range. A cancel errors at once; a download
    /// failure errors only when `pos` is past the buffered audio, so what
    /// already arrived still plays.
    pub fn wait_len(&self, pos: usize) -> Result<usize> {
        let mut s = self.state.lock().unwrap();
        loop {
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(Error::Fetch("cancelled".into()));
            }
            let end = s.start + s.data.len();
            if pos < end {
                return Ok(end);
            }
            if let Some(failure) = &s.error {
                return Err(failure.to_error());
            }
            if s.done {
                return Ok(end);
            }
            s = self.cond.wait(s).unwrap();
        }
    }

    pub fn read_at(&self, pos: usize, out: &mut [u8]) -> Result<usize> {
        let end = self.wait_len(pos)?;
        if pos >= end {
            return Ok(0);
        }
        let s = self.state.lock().unwrap();
        if pos < s.start {
            return Err(Error::Fetch("requested bytes were already released".into()));
        }
        let n = out.len().min(s.start + s.data.len() - pos);
        out[..n].copy_from_slice(&s.data[pos - s.start..pos - s.start + n]);
        Ok(n)
    }
}

/// Waits between attempts at one segment: about 16 s in total, so a Wi-Fi
/// roam or a short outage is ridden out while buffered audio keeps playing.
/// Longer outages end the stream and the engine waits for the network.
#[cfg(not(test))]
const SEGMENT_RETRY_WAITS: [Duration; 6] = [
    Duration::from_millis(200),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
];
#[cfg(test)]
const SEGMENT_RETRY_WAITS: [Duration; 6] = [Duration::from_millis(1); 6];

/// Fetch `url`, retrying on the `SEGMENT_RETRY_WAITS` schedule. A rejected
/// URL (4xx) fails at once — only a fresh resolve can fix it — and a cancel
/// stops the waiting.
fn get_with_retry(fetcher: &dyn Fetcher, url: &str, buffer: &StreamBuffer) -> Result<Vec<u8>> {
    let mut waits = SEGMENT_RETRY_WAITS.iter();
    loop {
        match fetcher.get(url) {
            Ok(bytes) => return Ok(bytes),
            Err(e @ Error::FetchRejected(_)) => return Err(e),
            Err(e) => {
                let Some(&wait) = waits.next() else { return Err(e) };
                tracing::warn!("segment fetch failed ({e}); retrying in {wait:?}");
                if !buffer.sleep_unless_cancelled(wait) {
                    return Err(e);
                }
            }
        }
    }
}

/// Download every part of `plan` in order into `buffer`, waiting for the
/// consumer when the buffer is full. When `cache_writer` is set, every fetched
/// part is also written to the temp file, which is committed after the last
/// part and aborted on cancel/failure. Runs on the calling thread; intended to
/// be spawned via `std::thread::spawn`.
pub fn download(
    buffer: Arc<StreamBuffer>,
    fetcher: Arc<dyn Fetcher>,
    plan: StreamPlan,
    cache_writer: Option<CacheWriter>,
) {
    let mut writer = cache_writer;
    let mut urls = Vec::new();
    if let Some(init) = &plan.init_url {
        urls.push(init.clone());
    }
    urls.extend(plan.segments.iter().map(|s| s.url.clone()));
    for url in urls {
        if buffer.is_cancelled() {
            if let Some(writer) = writer.take() {
                writer.abort();
            }
            return;
        }
        buffer.wait_room(MAX_BUFFERED_BYTES);
        if buffer.is_cancelled() {
            if let Some(writer) = writer.take() {
                writer.abort();
            }
            return;
        }
        match get_with_retry(fetcher.as_ref(), &url, &buffer) {
            Ok(bytes) => {
                let mut write_failed = false;
                if let Some(writer) = writer.as_mut() {
                    if let Err(e) = writer.write_all(&bytes) {
                        tracing::warn!("cache write failed: {e}");
                        write_failed = true;
                    }
                }
                if write_failed {
                    if let Some(writer) = writer.take() {
                        writer.abort();
                    }
                }
                buffer.push(&bytes);
            }
            Err(e) => {
                buffer.fail(&e);
                if let Some(writer) = writer.take() {
                    writer.abort();
                }
                return;
            }
        }
    }
    if let Some(writer) = writer.take() {
        writer.commit();
    }
    buffer.finish();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheKey, DiskCache};
    use crate::source::Segment;
    use oxitide_tidal::stream::Quality;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-fetch-cache-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cache_key() -> CacheKey {
        CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: 7 }
    }

    fn small_plan() -> StreamPlan {
        StreamPlan {
            init_url: Some("init".into()),
            segments: vec![
                Segment { url: "a".into(), start_ticks: 0, duration_ticks: 0 },
                Segment { url: "b".into(), start_ticks: 0, duration_ticks: 0 },
            ],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            quality: None,
            manifest_hash: Some(7),
            replay_gain: Default::default(),
        }
    }

    #[test]
    fn download_tees_parts_into_the_cache_writer() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = cache_key();
        let writer = cache.begin_write(&key).unwrap();
        let fetcher = Arc::new(
            MapFetcher::default().with("init", b"INIT".to_vec()).with("a", b"AAA".to_vec()).with("b", b"BB".to_vec()),
        );
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, small_plan(), Some(writer));
        assert_eq!(buf.total_pushed(), 9);
        let path = cache.lookup(&key).expect("committed entry");
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 9);
        assert_eq!(std::fs::read(&path).unwrap(), b"INITAAABB");
        assert_eq!(cache.size_bytes(), 9);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn download_aborts_the_writer_on_failure() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = cache_key();
        let writer = cache.begin_write(&key).unwrap();
        let fetcher = Arc::new(MapFetcher::default());
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, small_plan(), Some(writer));
        assert!(cache.lookup(&key).is_none());
        assert_eq!(cache.size_bytes(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn download_pushes_all_parts_in_order() {
        let fetcher = Arc::new(
            MapFetcher::default()
                .with("init", b"INIT".to_vec())
                .with("a", b"AAA".to_vec())
                .with("b", b"BB".to_vec()),
        );
        let plan = StreamPlan {
            init_url: Some("init".into()),
            segments: vec![
                Segment { url: "a".into(), start_ticks: 0, duration_ticks: 0 },
                Segment { url: "b".into(), start_ticks: 0, duration_ticks: 0 },
            ],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: Default::default(),
        };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, plan, None);
        let mut out = [0u8; 9];
        let n = buf.read_at(0, &mut out).unwrap();
        assert_eq!(&out[..n], b"INITAAABB");
    }

    #[test]
    fn fetch_failure_is_reported_to_readers() {
        let fetcher = Arc::new(MapFetcher::default());
        let plan = StreamPlan {
            init_url: None,
            segments: vec![Segment { url: "missing".into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: Default::default(),
        };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, plan, None);
        assert!(matches!(buf.wait_len(0), Err(Error::Fetch(_))));
    }

    struct FlakyFetcher {
        fail_first: usize,
        calls: Mutex<usize>,
    }

    impl FlakyFetcher {
        fn new(fail_first: usize) -> Self {
            Self { fail_first, calls: Mutex::new(0) }
        }
        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    impl Fetcher for FlakyFetcher {
        fn get(&self, _url: &str) -> Result<Vec<u8>> {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            if *calls <= self.fail_first {
                Err(Error::Fetch("transient".into()))
            } else {
                Ok(b"OK".to_vec())
            }
        }
    }

    #[test]
    fn download_retries_transient_fetch_failures() {
        let fetcher = Arc::new(FlakyFetcher::new(2));
        let plan = StreamPlan {
            init_url: None,
            segments: vec![Segment { url: "a".into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: Default::default(),
        };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher.clone(), plan, None);
        let mut out = [0u8; 2];
        let n = buf.read_at(0, &mut out).unwrap();
        assert_eq!(&out[..n], b"OK");
        assert_eq!(fetcher.calls(), 3);
    }

    #[test]
    fn download_fails_after_bounded_attempts() {
        let fetcher = Arc::new(FlakyFetcher::new(usize::MAX));
        let plan = StreamPlan {
            init_url: None,
            segments: vec![Segment { url: "a".into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: Default::default(),
        };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher.clone(), plan, None);
        assert!(matches!(buf.wait_len(0), Err(Error::Fetch(_))));
        assert_eq!(fetcher.calls(), SEGMENT_RETRY_WAITS.len() + 1);
    }

    struct RejectingFetcher {
        calls: Mutex<usize>,
    }

    impl Fetcher for RejectingFetcher {
        fn get(&self, _url: &str) -> Result<Vec<u8>> {
            *self.calls.lock().unwrap() += 1;
            Err(Error::FetchRejected("status 403 Forbidden for a".into()))
        }
    }

    #[test]
    fn a_rejected_url_is_not_retried() {
        let fetcher = Arc::new(RejectingFetcher { calls: Mutex::new(0) });
        let mut plan = small_plan();
        plan.init_url = None;
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher.clone(), plan, None);
        assert!(matches!(buf.wait_len(0), Err(Error::FetchRejected(_))));
        assert_eq!(*fetcher.calls.lock().unwrap(), 1);
        assert!(matches!(buf.failure(), Some(Error::FetchRejected(_))));
    }

    #[test]
    fn buffered_bytes_play_out_before_a_failure_is_reported() {
        let buf = StreamBuffer::new();
        buf.push(b"AAAA");
        buf.fail(&Error::Fetch("connection reset".into()));
        let mut out = [0u8; 4];
        assert_eq!(buf.read_at(0, &mut out).unwrap(), 4, "buffered audio was dropped");
        assert_eq!(&out, b"AAAA");
        assert!(matches!(buf.read_at(4, &mut out), Err(Error::Fetch(_))));
    }

    #[test]
    fn a_cancelled_stream_reports_no_failure() {
        let buf = StreamBuffer::new();
        buf.push(b"AAAA");
        buf.cancel();
        assert!(buf.failure().is_none());
        let mut out = [0u8; 4];
        assert!(buf.read_at(0, &mut out).is_err(), "a cancel must stop readers at once");
    }

    #[test]
    fn consume_releases_bytes_and_keeps_absolute_offsets() {
        let buf = StreamBuffer::new();
        buf.push(b"AAAA");
        buf.push(b"BBBB");
        assert_eq!(buf.buffered_bytes(), 8);
        buf.consume_to(4);
        assert_eq!(buf.buffered_bytes(), 4);
        let mut out = [0u8; 4];
        let n = buf.read_at(4, &mut out).unwrap();
        assert_eq!(&out[..n], b"BBBB");
        buf.consume_to(8);
        assert_eq!(buf.buffered_bytes(), 0);
        buf.finish();
        assert_eq!(buf.read_at(8, &mut out).unwrap(), 0);
    }

    #[test]
    fn cancel_wakes_waiting_readers() {
        let buf = StreamBuffer::new();
        let waiter = {
            let buf = buf.clone();
            std::thread::spawn(move || buf.wait_len(0))
        };
        std::thread::sleep(Duration::from_millis(50));
        buf.cancel();
        assert!(matches!(waiter.join().unwrap(), Err(Error::Fetch(_))));
        assert!(buf.is_cancelled());
    }

    struct ChunkFetcher {
        chunk: Vec<u8>,
        calls: Mutex<usize>,
    }

    impl ChunkFetcher {
        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    impl Fetcher for ChunkFetcher {
        fn get(&self, _url: &str) -> Result<Vec<u8>> {
            *self.calls.lock().unwrap() += 1;
            Ok(self.chunk.clone())
        }
    }

    /// Review Focus: a long stream must not grow the buffer without bound.
    #[test]
    fn download_is_backpressured_and_bounded() {
        const CHUNK: usize = 64 * 1024;
        const SEGMENTS: usize = 64; // 4 MiB total, twice the cap
        let fetcher = Arc::new(ChunkFetcher { chunk: vec![7u8; CHUNK], calls: Mutex::new(0) });
        let plan = StreamPlan {
            init_url: None,
            segments: (0..SEGMENTS)
                .map(|i| Segment { url: format!("s{i}"), start_ticks: 0, duration_ticks: 0 })
                .collect(),
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: Default::default(),
        };
        let buf = StreamBuffer::new();
        let handle = {
            let buf = buf.clone();
            let fetcher = fetcher.clone();
            std::thread::spawn(move || download(buf, fetcher, plan, None))
        };

        // With no consumer, the downloader must stop fetching once the cap is
        // reached (plus the in-flight segment).
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            buf.buffered_bytes() <= MAX_BUFFERED_BYTES + CHUNK,
            "buffered {} bytes",
            buf.buffered_bytes()
        );
        assert!(
            fetcher.calls() < SEGMENTS,
            "downloader never hit backpressure ({} calls)",
            fetcher.calls()
        );

        // Draining releases consumed bytes and lets the downloader finish.
        let mut pos = 0usize;
        let mut out = [0u8; CHUNK];
        loop {
            let n = buf.read_at(pos, &mut out).unwrap();
            if n == 0 && buf.is_done() {
                break;
            }
            pos += n;
            buf.consume_to(pos);
        }
        handle.join().unwrap();
        assert_eq!(pos, CHUNK * SEGMENTS);
    }
}
