use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::source::StreamPlan;

pub trait Fetcher: Send + Sync + 'static {
    fn get(&self, url: &str) -> Result<Vec<u8>>;
}

pub struct HttpFetcher {
    client: reqwest::blocking::Client,
}

impl HttpFetcher {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Fetch(e.to_string()))?;
        Ok(Self { client })
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new().expect("build http client")
    }
}

impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.client.get(url).send().map_err(|e| Error::Fetch(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Fetch(format!("status {} for {url}", resp.status())));
        }
        resp.bytes().map(|b| b.to_vec()).map_err(|e| Error::Fetch(e.to_string()))
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
    done: bool,
    error: Option<String>,
}

pub struct StreamBuffer {
    state: Mutex<State>,
    cond: Condvar,
}

impl StreamBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(State { data: Vec::new(), done: false, error: None }), cond: Condvar::new() })
    }

    pub fn push(&self, bytes: &[u8]) {
        let mut s = self.state.lock().unwrap();
        s.data.extend_from_slice(bytes);
        self.cond.notify_all();
    }

    pub fn finish(&self) {
        let mut s = self.state.lock().unwrap();
        s.done = true;
        self.cond.notify_all();
    }

    pub fn fail(&self, message: String) {
        let mut s = self.state.lock().unwrap();
        s.error = Some(message);
        s.done = true;
        self.cond.notify_all();
    }

    /// Block until `pos` is buffered or the stream ends. Returns the currently
    /// buffered length, or a typed error if the download failed.
    pub fn wait_len(&self, pos: usize) -> Result<usize> {
        let mut s = self.state.lock().unwrap();
        loop {
            if let Some(err) = &s.error {
                return Err(Error::Fetch(err.clone()));
            }
            if pos < s.data.len() || s.done {
                return Ok(s.data.len());
            }
            s = self.cond.wait(s).unwrap();
        }
    }

    pub fn read_at(&self, pos: usize, out: &mut [u8]) -> Result<usize> {
        let len = self.wait_len(pos)?;
        if pos >= len {
            return Ok(0);
        }
        let s = self.state.lock().unwrap();
        let end = (pos + out.len()).min(len);
        let n = end - pos;
        out[..n].copy_from_slice(&s.data[pos..end]);
        Ok(n)
    }
}

/// Download every part of `plan` in order into `buffer`. Runs on the calling
/// thread; intended to be spawned via `std::thread::spawn`.
pub fn download(buffer: Arc<StreamBuffer>, fetcher: Arc<dyn Fetcher>, plan: StreamPlan) {
    let mut urls = Vec::new();
    if let Some(init) = &plan.init_url {
        urls.push(init.clone());
    }
    urls.extend(plan.media_urls.iter().cloned());
    for url in urls {
        match fetcher.get(&url) {
            Ok(bytes) => buffer.push(&bytes),
            Err(e) => {
                buffer.fail(e.to_string());
                return;
            }
        }
    }
    buffer.finish();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_pushes_all_parts_in_order() {
        let fetcher = Arc::new(
            MapFetcher::default()
                .with("init", b"INIT".to_vec())
                .with("a", b"AAA".to_vec())
                .with("b", b"BB".to_vec()),
        );
        let plan = StreamPlan { init_url: Some("init".into()), media_urls: vec!["a".into(), "b".into()], codec: None, sample_rate: None, bit_depth: None };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, plan);
        let mut out = [0u8; 9];
        let n = buf.read_at(0, &mut out).unwrap();
        assert_eq!(&out[..n], b"INITAAABB");
    }

    #[test]
    fn fetch_failure_is_reported_to_readers() {
        let fetcher = Arc::new(MapFetcher::default());
        let plan = StreamPlan { init_url: None, media_urls: vec!["missing".into()], codec: None, sample_rate: None, bit_depth: None };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, plan);
        assert!(matches!(buf.wait_len(0), Err(Error::Fetch(_))));
    }
}
