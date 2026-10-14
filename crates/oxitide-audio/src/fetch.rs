use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
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
    download_urls(&buffer, fetcher.as_ref(), plan_urls(&plan), cache_writer);
}

fn plan_urls(plan: &StreamPlan) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(init) = &plan.init_url {
        urls.push(init.clone());
    }
    urls.extend(plan.segments.iter().map(|s| s.url.clone()));
    urls
}

/// `download`'s paced loop over an explicit URL list.
fn download_urls(
    buffer: &StreamBuffer,
    fetcher: &dyn Fetcher,
    urls: Vec<String>,
    cache_writer: Option<CacheWriter>,
) {
    let mut writer = cache_writer;
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
        match get_with_retry(fetcher, &url, buffer) {
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

/// Orders run-ahead downloads: only the first in line may download beyond
/// what its decoder needs, so the playing track never shares the connection
/// with a prefetch racing ahead. Priority tickets (the playing track,
/// including seeks and restarts) go before prefetches; ties go by age.
#[derive(Default)]
pub struct AheadGate {
    active: Mutex<std::collections::BTreeSet<(u8, u64)>>,
    next: AtomicU64,
}

impl AheadGate {
    pub fn register(self: &Arc<Self>, priority: bool) -> AheadTicket {
        let key = (u8::from(!priority), self.next.fetch_add(1, Ordering::SeqCst));
        self.active.lock().unwrap().insert(key);
        AheadTicket { gate: self.clone(), key }
    }
}

/// A download's place in the [`AheadGate`]; dropping it frees the place.
pub struct AheadTicket {
    gate: Arc<AheadGate>,
    key: (u8, u64),
}

impl AheadTicket {
    fn first_in_line(&self) -> bool {
        self.gate.active.lock().unwrap().first() == Some(&self.key)
    }
}

impl Drop for AheadTicket {
    fn drop(&mut self) {
        self.gate.active.lock().unwrap().remove(&self.key);
    }
}

/// A scratch file for a run-ahead download that is not cached (a seek
/// truncates the plan; the cache may refuse a second writer). Deleted on
/// drop; the `.part` suffix lets the cache's startup sweep remove leftovers.
pub struct ScratchFile {
    path: PathBuf,
    file: Option<File>,
}

impl ScratchFile {
    pub fn create(dir: &Path) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name =
            format!("spill-{}-{}.part", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst));
        let path = dir.join(name);
        let file = OpenOptions::new().write(true).create_new(true).open(&path)?;
        Ok(Self { path, file: Some(file) })
    }
}

impl Drop for ScratchFile {
    fn drop(&mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Where a run-ahead download lands before the feeder reads it back.
pub enum AheadSink {
    /// The whole track, published to the cache as soon as it is complete.
    Cache(CacheWriter),
    Scratch(ScratchFile),
}

impl AheadSink {
    fn path(&self) -> &Path {
        match self {
            AheadSink::Cache(writer) => writer.part_path(),
            AheadSink::Scratch(scratch) => &scratch.path,
        }
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        match self {
            AheadSink::Cache(writer) => writer.write_all(data),
            AheadSink::Scratch(scratch) => scratch
                .file
                .as_mut()
                .ok_or_else(|| io::Error::other("scratch file closed"))?
                .write_all(data),
        }
    }

    /// The download completed: publish a cache entry; a scratch file stays
    /// until the feeder has read it (the reader's handle keeps it alive).
    fn finish(self) {
        if let AheadSink::Cache(writer) = self {
            writer.commit();
        }
    }
}

/// Bytes read back from the spill file per push into the stream buffer.
const FEED_CHUNK: usize = 256 * 1024;

/// How often the waiting network and feeder threads re-check the other side
/// and cancellation.
const AHEAD_POLL: Duration = Duration::from_millis(100);

#[derive(Clone)]
enum NetEnd {
    Done,
    Failed(Failure),
    /// The spill file failed; the network thread now feeds the buffer itself.
    Direct,
}

#[derive(Default)]
struct NetProgress {
    downloaded: u64,
    fed: u64,
    end: Option<NetEnd>,
}

#[derive(Default)]
struct Progress {
    state: Mutex<NetProgress>,
    cond: Condvar,
}

impl Progress {
    fn update(&self, f: impl FnOnce(&mut NetProgress)) {
        f(&mut self.state.lock().unwrap());
        self.cond.notify_all();
    }
}

/// Download `plan` to `sink` as fast as the connection allows (when first in
/// line at the [`AheadGate`]) while this thread feeds the bytes back into
/// `buffer` at the decoder's pace. Once a track is on disk a network drop
/// cannot interrupt it, and a cached track is published before it finishes
/// playing. Without a readable spill file it falls back to [`download`].
pub fn download_ahead(
    buffer: Arc<StreamBuffer>,
    fetcher: Arc<dyn Fetcher>,
    plan: StreamPlan,
    sink: AheadSink,
    ticket: AheadTicket,
) {
    let reader = match File::open(sink.path()) {
        Ok(reader) => reader,
        Err(e) => {
            tracing::warn!("download-ahead spill unreadable ({e}); streaming directly");
            let writer = match sink {
                AheadSink::Cache(writer) => Some(writer),
                AheadSink::Scratch(_) => None,
            };
            return download(buffer, fetcher, plan, writer);
        }
    };
    let progress = Arc::new(Progress::default());
    let urls = plan_urls(&plan);
    let network = {
        let buffer = buffer.clone();
        let progress = progress.clone();
        std::thread::Builder::new()
            .name("oxitide-fetch".into())
            .spawn(move || run_network(&buffer, fetcher.as_ref(), urls, sink, ticket, &progress))
    };
    if let Err(e) = network {
        buffer.fail(&Error::Fetch(format!("failed to start the download: {e}")));
        return;
    }
    feed(&buffer, reader, &progress);
}

fn run_network(
    buffer: &StreamBuffer,
    fetcher: &dyn Fetcher,
    urls: Vec<String>,
    mut sink: AheadSink,
    ticket: AheadTicket,
    progress: &Progress,
) {
    let mut urls = urls.into_iter();
    while let Some(url) = urls.next() {
        // Run ahead only when first in line; otherwise fetch as the decoder
        // needs it, like a plain stream.
        loop {
            if buffer.is_cancelled() {
                return; // dropping `sink` aborts a partial cache entry
            }
            let state = progress.state.lock().unwrap();
            let needed = state.fed == state.downloaded
                && buffer.buffered_bytes() < MAX_BUFFERED_BYTES;
            if needed || ticket.first_in_line() {
                break;
            }
            drop(progress.cond.wait_timeout(state, AHEAD_POLL).unwrap());
        }
        let bytes = match get_with_retry(fetcher, &url, buffer) {
            Ok(bytes) => bytes,
            Err(e) => {
                progress.update(|p| p.end = Some(NetEnd::Failed(Failure::from_error(&e))));
                return;
            }
        };
        if let Err(e) = sink.write_all(&bytes) {
            tracing::warn!("download-ahead write failed ({e}); streaming the rest directly");
            // Let the feeder drain the file, then take the buffer over.
            let mut state = progress.state.lock().unwrap();
            while state.fed < state.downloaded && !buffer.is_cancelled() {
                state = progress.cond.wait_timeout(state, AHEAD_POLL).unwrap().0;
            }
            state.end = Some(NetEnd::Direct);
            drop(state);
            progress.cond.notify_all();
            drop(sink);
            drop(ticket);
            buffer.push(&bytes);
            download_urls(buffer, fetcher, urls.collect(), None);
            return;
        }
        progress.update(|p| p.downloaded += bytes.len() as u64);
    }
    sink.finish();
    progress.update(|p| p.end = Some(NetEnd::Done));
}

fn feed(buffer: &StreamBuffer, mut reader: File, progress: &Progress) {
    let mut chunk = vec![0u8; FEED_CHUNK];
    loop {
        buffer.wait_room(MAX_BUFFERED_BYTES);
        if buffer.is_cancelled() {
            return;
        }
        let (available, end) = {
            let mut state = progress.state.lock().unwrap();
            while state.downloaded == state.fed && state.end.is_none() && !buffer.is_cancelled() {
                state = progress.cond.wait_timeout(state, AHEAD_POLL).unwrap().0;
            }
            (state.downloaded - state.fed, state.end.clone())
        };
        if buffer.is_cancelled() {
            return;
        }
        if available > 0 {
            let n = (available as usize).min(chunk.len());
            if let Err(e) = reader.read_exact(&mut chunk[..n]) {
                buffer.fail(&Error::Fetch(format!("download-ahead read failed: {e}")));
                return;
            }
            buffer.push(&chunk[..n]);
            progress.update(|p| p.fed += n as u64);
            continue;
        }
        match end {
            Some(NetEnd::Done) => return buffer.finish(),
            Some(NetEnd::Failed(failure)) => return buffer.fail(&failure.to_error()),
            Some(NetEnd::Direct) => return,
            None => {}
        }
    }
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

    const AHEAD_CHUNK: usize = 64 * 1024;

    fn chunk_plan(segments: usize) -> StreamPlan {
        StreamPlan {
            init_url: None,
            segments: (0..segments)
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
        }
    }

    fn wait_until(mut ready: impl FnMut() -> bool) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !ready() {
            if std::time::Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        true
    }

    /// Read the stream to its end like the demuxer; the byte count, or the
    /// error with the count read before it.
    fn drain(buf: &StreamBuffer) -> std::result::Result<usize, (Error, usize)> {
        let mut pos = 0;
        let mut out = vec![0u8; 32 * 1024];
        loop {
            match buf.read_at(pos, &mut out) {
                Ok(0) => return Ok(pos),
                Ok(n) => {
                    pos += n;
                    buf.consume_to(pos);
                }
                Err(e) => return Err((e, pos)),
            }
        }
    }

    fn spill_files(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
            .count()
    }

    #[test]
    fn download_ahead_fetches_the_whole_track_without_a_consumer() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 30);
        let key = cache_key();
        let writer = cache.begin_write(&key).unwrap();
        let gate = Arc::new(AheadGate::default());
        let fetcher = Arc::new(ChunkFetcher { chunk: vec![7u8; AHEAD_CHUNK], calls: Mutex::new(0) });
        let buf = StreamBuffer::new();
        let handle = {
            let (buf, fetcher, ticket) = (buf.clone(), fetcher.clone(), gate.register(true));
            std::thread::spawn(move || {
                download_ahead(buf, fetcher, chunk_plan(64), AheadSink::Cache(writer), ticket)
            })
        };

        // Nothing reads, yet all 4 MiB arrive and are published to the cache.
        assert!(wait_until(|| cache.lookup(&key).is_some()), "the entry was never published");
        assert_eq!(fetcher.calls(), 64);
        assert!(buf.buffered_bytes() <= MAX_BUFFERED_BYTES + FEED_CHUNK, "memory grew past the cap");

        assert_eq!(drain(&buf).ok(), Some(64 * AHEAD_CHUNK));
        handle.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_download_behind_the_playing_track_stays_paced_until_it_is_first() {
        let dir = temp_dir();
        let gate = Arc::new(AheadGate::default());
        let playing = gate.register(true);
        let fetcher = Arc::new(ChunkFetcher { chunk: vec![7u8; AHEAD_CHUNK], calls: Mutex::new(0) });
        let buf = StreamBuffer::new();
        let handle = {
            let (buf, fetcher, ticket) = (buf.clone(), fetcher.clone(), gate.register(false));
            let sink = AheadSink::Scratch(ScratchFile::create(&dir).unwrap());
            std::thread::spawn(move || download_ahead(buf, fetcher, chunk_plan(64), sink, ticket))
        };

        std::thread::sleep(Duration::from_millis(300));
        let paced = MAX_BUFFERED_BYTES / AHEAD_CHUNK + 2;
        assert!(fetcher.calls() <= paced, "a prefetch ran ahead: {} fetches", fetcher.calls());

        drop(playing);
        assert!(wait_until(|| fetcher.calls() == 64), "it never ran ahead once first in line");
        assert_eq!(drain(&buf).ok(), Some(64 * AHEAD_CHUNK));
        handle.join().unwrap();
        assert!(wait_until(|| spill_files(&dir) == 0), "the scratch file was left behind");
        let _ = std::fs::remove_dir_all(dir);
    }

    struct FailAfter {
        ok: usize,
        calls: Mutex<usize>,
    }

    impl Fetcher for FailAfter {
        fn get(&self, _url: &str) -> Result<Vec<u8>> {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            if *calls <= self.ok {
                Ok(vec![1u8; AHEAD_CHUNK])
            } else {
                Err(Error::Fetch("network is unreachable".into()))
            }
        }
    }

    #[test]
    fn downloaded_audio_plays_out_before_a_network_failure() {
        let dir = temp_dir();
        let gate = Arc::new(AheadGate::default());
        let fetcher = Arc::new(FailAfter { ok: 3, calls: Mutex::new(0) });
        let buf = StreamBuffer::new();
        let sink = AheadSink::Scratch(ScratchFile::create(&dir).unwrap());
        download_ahead(buf.clone(), fetcher, chunk_plan(6), sink, gate.register(true));
        match drain(&buf) {
            Err((Error::Fetch(_), read)) => assert_eq!(read, 3 * AHEAD_CHUNK),
            other => panic!("expected a fetch error after 3 segments, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    struct SlowFetcher;

    impl Fetcher for SlowFetcher {
        fn get(&self, _url: &str) -> Result<Vec<u8>> {
            std::thread::sleep(Duration::from_millis(5));
            Ok(vec![1u8; AHEAD_CHUNK])
        }
    }

    #[test]
    fn a_cancelled_download_ahead_aborts_its_cache_entry() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 30);
        let key = cache_key();
        let writer = cache.begin_write(&key).unwrap();
        let gate = Arc::new(AheadGate::default());
        let buf = StreamBuffer::new();
        let handle = {
            let (buf, ticket) = (buf.clone(), gate.register(true));
            std::thread::spawn(move || {
                download_ahead(buf, Arc::new(SlowFetcher), chunk_plan(500), AheadSink::Cache(writer), ticket)
            })
        };
        std::thread::sleep(Duration::from_millis(30));
        buf.cancel();
        handle.join().unwrap();
        assert!(wait_until(|| spill_files(&dir) == 0), "the partial entry was left behind");
        assert!(cache.lookup(&key).is_none(), "a partial download was published");
        let _ = std::fs::remove_dir_all(dir);
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
