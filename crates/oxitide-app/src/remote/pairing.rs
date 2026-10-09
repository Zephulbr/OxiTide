//! Pairing and the device store (spec §6.2, RM-D4). A pairing token is
//! single-use, short-lived, and held only in memory; pairing yields a device
//! token whose SHA-256 hash is the only thing persisted (`remote.json`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::oneshot;

/// How long a pairing QR stays valid.
pub const PAIRING_TTL: Duration = Duration::from_secs(5 * 60);
/// How long a phone waits for the desktop to approve it.
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);
/// `last_seen` is written at most this often per device.
const LAST_SEEN_RESOLUTION_MS: u64 = 60_000;
const FILE_VERSION: u32 = 1;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `bytes` of OS randomness, base64url without padding.
pub fn random_token(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|_| "the OS random source is unavailable".to_string())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in text.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

/// Compare without an early exit, so timing does not reveal a prefix match.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairError {
    /// Unknown, expired, or already used token (counts as a failed attempt).
    Invalid,
    /// Another phone is already waiting for approval with this token.
    Busy,
}

struct OutstandingToken {
    value: String,
    expires: Instant,
    reserved: bool,
}

/// The one outstanding pairing token.
#[derive(Default)]
pub struct Pairing {
    token: Mutex<Option<OutstandingToken>>,
}

impl Pairing {
    /// Issue a fresh token, replacing any outstanding one.
    pub fn begin(&self, now: Instant) -> Result<String, String> {
        let value = random_token(16)?;
        *self.token.lock().unwrap() = Some(OutstandingToken {
            value: value.clone(),
            expires: now + PAIRING_TTL,
            reserved: false,
        });
        Ok(value)
    }

    pub fn cancel(&self) {
        *self.token.lock().unwrap() = None;
    }

    /// Seconds left on the outstanding token, if any.
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        let guard = self.token.lock().unwrap();
        let token = guard.as_ref()?;
        (token.expires > now).then(|| token.expires - now)
    }

    fn matches(token: &OutstandingToken, candidate: &str, now: Instant) -> bool {
        now < token.expires && constant_time_eq(token.value.as_bytes(), candidate.as_bytes())
    }

    /// Hold the token for one pairing attempt (approval may take a while).
    pub fn reserve(&self, candidate: &str, now: Instant) -> Result<(), PairError> {
        let mut guard = self.token.lock().unwrap();
        match guard.as_mut() {
            Some(token) if Self::matches(token, candidate, now) => {
                if token.reserved {
                    Err(PairError::Busy)
                } else {
                    token.reserved = true;
                    Ok(())
                }
            }
            _ => Err(PairError::Invalid),
        }
    }

    /// The attempt ended without a decision (timeout): the token may be used
    /// again until it expires.
    pub fn release(&self, candidate: &str) {
        if let Some(token) = self.token.lock().unwrap().as_mut() {
            if constant_time_eq(token.value.as_bytes(), candidate.as_bytes()) {
                token.reserved = false;
            }
        }
    }

    /// Burn the token (approved or denied): single use.
    pub fn consume(&self, candidate: &str) {
        let mut guard = self.token.lock().unwrap();
        if guard
            .as_ref()
            .is_some_and(|t| constant_time_eq(t.value.as_bytes(), candidate.as_bytes()))
        {
            *guard = None;
        }
    }
}

/// A phone waiting for the desktop's decision.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PairRequestDto {
    pub id: String,
    pub name: String,
    pub ip: String,
}

/// Pending approvals, resolved by `resolve_remote_pair_request`.
#[derive(Default)]
pub struct Approvals {
    pending: Mutex<HashMap<String, (PairRequestDto, oneshot::Sender<bool>)>>,
}

impl Approvals {
    pub fn register(&self, name: String, ip: String) -> Result<(PairRequestDto, oneshot::Receiver<bool>), String> {
        let request = PairRequestDto { id: random_token(9)?, name, ip };
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(request.id.clone(), (request.clone(), tx));
        Ok((request, rx))
    }

    /// Deliver the decision; `false` when the request is gone (timed out).
    pub fn resolve(&self, id: &str, approve: bool) -> bool {
        match self.pending.lock().unwrap().remove(id) {
            Some((_, tx)) => tx.send(approve).is_ok(),
            None => false,
        }
    }

    pub fn remove(&self, id: &str) {
        self.pending.lock().unwrap().remove(id);
    }

    pub fn list(&self) -> Vec<PairRequestDto> {
        self.pending.lock().unwrap().values().map(|(r, _)| r.clone()).collect()
    }
}

/// One paired phone. The token itself is never stored.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub id: String,
    pub name: String,
    pub created_ms: u64,
    pub last_seen_ms: u64,
    token_hash: String,
}

impl std::fmt::Debug for DeviceRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceRecord")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("created_ms", &self.created_ms)
            .field("last_seen_ms", &self.last_seen_ms)
            .field("token_hash", &"<redacted>")
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
struct DeviceFile {
    version: u32,
    #[serde(default)]
    devices: Vec<DeviceRecord>,
}

/// `remote.json`: versioned, written atomically (tmp + rename). `path: None`
/// keeps the store in memory (tests, no writable data directory).
pub struct DeviceStore {
    path: Option<PathBuf>,
    devices: Mutex<Vec<DeviceRecord>>,
}

/// `%APPDATA%\OxiTide\config\remote.json`, next to `session.json`.
pub fn devices_path() -> Option<PathBuf> {
    crate::config::config_path().map(|path| path.with_file_name("remote.json"))
}

impl DeviceStore {
    /// A missing file is an empty store; a malformed or future-version file is
    /// renamed `.bak` so it can never block the feature.
    pub fn load(path: Option<PathBuf>) -> Self {
        let devices = path.as_deref().map(read_devices).unwrap_or_default();
        Self { path, devices: Mutex::new(devices) }
    }

    fn persist(&self, devices: &[DeviceRecord]) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let file = DeviceFile { version: FILE_VERSION, devices: devices.to_vec() };
        let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// Create a device; returns the record and its one-time clear token.
    pub fn create(&self, name: String, now_ms: u64) -> Result<(DeviceRecord, String), String> {
        let token = random_token(32)?;
        let record = DeviceRecord {
            id: random_token(9)?,
            name,
            created_ms: now_ms,
            last_seen_ms: now_ms,
            token_hash: to_hex(&hash_token(&token)),
        };
        let mut devices = self.devices.lock().unwrap();
        devices.push(record.clone());
        self.persist(&devices)?;
        Ok((record, token))
    }

    /// The device owning `token`. Every record is compared (no early exit),
    /// and `last_seen` is refreshed at most once a minute.
    pub fn authenticate(&self, token: &str, now_ms: u64) -> Option<DeviceRecord> {
        let candidate = hash_token(token);
        let mut devices = self.devices.lock().unwrap();
        let mut found = None;
        for (index, device) in devices.iter().enumerate() {
            let stored = from_hex(&device.token_hash).unwrap_or([0u8; 32]);
            if constant_time_eq(&stored, &candidate) {
                found = Some(index);
            }
        }
        let index = found?;
        if now_ms.saturating_sub(devices[index].last_seen_ms) >= LAST_SEEN_RESOLUTION_MS {
            devices[index].last_seen_ms = now_ms;
            if let Err(e) = self.persist(&devices) {
                tracing::warn!("remote: failed to save device last-seen: {e}");
            }
        }
        Some(devices[index].clone())
    }

    pub fn revoke(&self, id: &str) -> Result<bool, String> {
        let mut devices = self.devices.lock().unwrap();
        let before = devices.len();
        devices.retain(|d| d.id != id);
        if devices.len() == before {
            return Ok(false);
        }
        self.persist(&devices)?;
        Ok(true)
    }

    pub fn list(&self) -> Vec<DeviceRecord> {
        self.devices.lock().unwrap().clone()
    }
}

fn read_devices(path: &Path) -> Vec<DeviceRecord> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<DeviceFile>(&text) {
        Ok(file) if file.version == FILE_VERSION => file.devices,
        Ok(file) => {
            tracing::warn!("remote.json has unsupported version {}; set aside", file.version);
            let _ = fs::rename(path, path.with_extension("json.bak"));
            Vec::new()
        }
        Err(e) => {
            tracing::warn!("malformed remote.json ({e}); set aside");
            let _ = fs::rename(path, path.with_extension("json.bak"));
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-remote-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tokens_are_random_and_url_safe() {
        let a = random_token(16).unwrap();
        let b = random_token(16).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 22, "128 bits in base64url without padding");
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(random_token(32).unwrap().len(), 43);
    }

    #[test]
    fn constant_time_eq_compares_whole_values() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn pairing_token_expires() {
        let pairing = Pairing::default();
        let t0 = Instant::now();
        let token = pairing.begin(t0).unwrap();
        assert_eq!(pairing.remaining(t0), Some(PAIRING_TTL));
        assert_eq!(
            pairing.reserve(&token, t0 + PAIRING_TTL + Duration::from_secs(1)),
            Err(PairError::Invalid)
        );
        assert_eq!(pairing.remaining(t0 + PAIRING_TTL), None);
    }

    #[test]
    fn pairing_token_is_single_use() {
        let pairing = Pairing::default();
        let t0 = Instant::now();
        let token = pairing.begin(t0).unwrap();
        assert_eq!(pairing.reserve(&token, t0), Ok(()));
        assert_eq!(pairing.reserve(&token, t0), Err(PairError::Busy), "one attempt at a time");
        pairing.consume(&token);
        assert_eq!(pairing.reserve(&token, t0), Err(PairError::Invalid), "replay is rejected");
    }

    #[test]
    fn a_timed_out_attempt_releases_the_token() {
        let pairing = Pairing::default();
        let t0 = Instant::now();
        let token = pairing.begin(t0).unwrap();
        pairing.reserve(&token, t0).unwrap();
        pairing.release(&token);
        assert_eq!(pairing.reserve(&token, t0), Ok(()));
    }

    #[test]
    fn only_one_token_is_outstanding() {
        let pairing = Pairing::default();
        let t0 = Instant::now();
        let first = pairing.begin(t0).unwrap();
        let second = pairing.begin(t0).unwrap();
        assert_eq!(pairing.reserve(&first, t0), Err(PairError::Invalid));
        assert_eq!(pairing.reserve(&second, t0), Ok(()));
        pairing.cancel();
        assert_eq!(pairing.reserve(&second, t0), Err(PairError::Invalid));
    }

    #[tokio::test]
    async fn approvals_accept_deny_and_timeout() {
        let approvals = Approvals::default();
        let (accept, rx) = approvals.register("Pixel".into(), "192.168.1.9".into()).unwrap();
        assert_eq!(approvals.list(), vec![accept.clone()]);
        assert!(approvals.resolve(&accept.id, true));
        assert_eq!(rx.await, Ok(true));

        let (deny, rx) = approvals.register("iPhone".into(), "192.168.1.8".into()).unwrap();
        assert!(approvals.resolve(&deny.id, false));
        assert_eq!(rx.await, Ok(false));

        let (late, rx) = approvals.register("Tab".into(), "192.168.1.7".into()).unwrap();
        let waited = tokio::time::timeout(Duration::from_millis(20), rx).await;
        assert!(waited.is_err(), "no decision means the phone times out");
        approvals.remove(&late.id);
        assert!(!approvals.resolve(&late.id, true), "a timed-out request cannot be approved");
    }

    #[test]
    fn device_hash_round_trip_and_revoke() {
        let store = DeviceStore::load(None);
        let (device, token) = store.create("Pixel".into(), 1_000).unwrap();
        assert!(!format!("{device:?}").contains(&device.token_hash));
        assert_eq!(store.authenticate(&token, 2_000).unwrap().id, device.id);
        assert!(store.authenticate("wrong", 2_000).is_none());
        assert!(store.revoke(&device.id).unwrap());
        assert!(store.authenticate(&token, 3_000).is_none());
        assert!(!store.revoke(&device.id).unwrap());
    }

    #[test]
    fn last_seen_updates_at_most_once_a_minute() {
        let store = DeviceStore::load(None);
        let (_, token) = store.create("Pixel".into(), 1_000).unwrap();
        assert_eq!(store.authenticate(&token, 30_000).unwrap().last_seen_ms, 1_000);
        assert_eq!(store.authenticate(&token, 61_000).unwrap().last_seen_ms, 61_000);
    }

    #[test]
    fn store_persists_only_hashes_atomically() {
        let dir = temp_dir();
        let path = dir.join("remote.json");
        let store = DeviceStore::load(Some(path.clone()));
        let (device, token) = store.create("Pixel".into(), 1_000).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains(&token), "the clear token must never be written");
        assert!(text.contains("\"version\": 1"), "{text}");
        assert!(!dir.join("remote.json.tmp").exists());
        let reloaded = DeviceStore::load(Some(path));
        assert_eq!(reloaded.list(), vec![device.clone()]);
        assert_eq!(reloaded.authenticate(&token, 1_000).unwrap().id, device.id);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_or_future_files_are_set_aside() {
        let dir = temp_dir();
        let path = dir.join("remote.json");
        fs::write(&path, "{not json").unwrap();
        assert!(DeviceStore::load(Some(path.clone())).list().is_empty());
        assert!(dir.join("remote.json.bak").exists());
        fs::write(&path, r#"{"version": 99, "devices": []}"#).unwrap();
        assert!(DeviceStore::load(Some(path.clone())).list().is_empty());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
