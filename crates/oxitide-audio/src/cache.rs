//! Whole-track disk cache: blobs keyed by track, requested quality, and the
//! stream's manifest identity. Every operation is best-effort; a cache error
//! never fails playback.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use oxitide_tidal::stream::Quality;

/// `.part` files older than this are stale temporary downloads.
const STALE_PART_AGE: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub track_id: u64,
    pub quality: Quality,
    pub manifest_hash: u64,
}

impl CacheKey {
    /// `{track_id}-{quality}-{hash:016x}.fmp4`, e.g. `77646170-lossless-9f3c….fmp4`.
    pub fn file_name(&self) -> String {
        format!("{}-{}-{:016x}.fmp4", self.track_id, self.quality.label(), self.manifest_hash)
    }
}

struct Entry {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

pub struct DiskCache {
    dir: PathBuf,
    max_bytes: AtomicU64,
    protected: Mutex<HashSet<PathBuf>>,
    usable: bool,
}

impl DiskCache {
    /// Infallible: an unusable directory (cannot create/write) disables the
    /// cache instead of failing startup.
    pub fn new(dir: PathBuf, max_bytes: u64) -> Arc<Self> {
        let usable = fs::create_dir_all(&dir).is_ok();
        if !usable {
            tracing::warn!("cache directory {} is unusable; caching disabled", dir.display());
        }
        Arc::new(Self {
            dir,
            max_bytes: AtomicU64::new(max_bytes),
            protected: Mutex::new(HashSet::new()),
            usable,
        })
    }

    pub fn usable(&self) -> bool {
        self.usable
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes.load(Ordering::SeqCst)
    }

    fn path_for(&self, key: &CacheKey) -> PathBuf {
        self.dir.join(key.file_name())
    }

    fn part_for(&self, key: &CacheKey) -> PathBuf {
        self.dir.join(format!("{}.part", key.file_name()))
    }

    /// True when the entry exists; touches mtime for LRU.
    pub fn lookup(&self, key: &CacheKey) -> Option<PathBuf> {
        if !self.usable {
            return None;
        }
        let path = self.path_for(key);
        if !path.is_file() {
            return None;
        }
        if let Ok(file) = OpenOptions::new().write(true).open(&path) {
            let _ = file.set_modified(SystemTime::now());
        }
        Some(path)
    }

    /// Creates a `.part` temp file; `None` (and a log) when the cache is unusable.
    pub fn begin_write(self: &Arc<Self>, key: &CacheKey) -> Option<CacheWriter> {
        if !self.usable {
            return None;
        }
        let part = self.part_for(key);
        let _ = fs::remove_file(&part);
        match OpenOptions::new().write(true).create_new(true).open(&part) {
            Ok(file) => {
                Some(CacheWriter { cache: self.clone(), key: *key, part, file, bytes: 0 })
            }
            Err(e) => {
                tracing::warn!("cache write for {} failed: {e}", key.file_name());
                None
            }
        }
    }

    /// Atomic publish: fsync temp, rename to the final name, touch mtime, evict.
    pub fn commit(&self, writer: CacheWriter) {
        let CacheWriter { key, part, file, .. } = writer;
        if let Err(e) = file.sync_all() {
            tracing::warn!("cache fsync for {} failed: {e}", key.file_name());
        }
        drop(file);
        let final_path = self.path_for(&key);
        if final_path.exists() {
            let _ = fs::remove_file(&final_path);
        }
        if let Err(e) = fs::rename(&part, &final_path) {
            tracing::warn!("cache commit for {} failed: {e}", key.file_name());
            let _ = fs::remove_file(&part);
            return;
        }
        if let Ok(file) = OpenOptions::new().write(true).open(&final_path) {
            let _ = file.set_modified(SystemTime::now());
        }
        self.evict_to_fit();
    }

    /// Deletes a temp file after a cancelled/failed download.
    pub fn abort(&self, writer: CacheWriter) {
        let CacheWriter { part, .. } = writer;
        let _ = fs::remove_file(&part);
    }

    pub fn remove(&self, key: &CacheKey) {
        let _ = fs::remove_file(self.path_for(key));
    }

    /// Sum of every `.fmp4` entry.
    pub fn size_bytes(&self) -> u64 {
        self.entries().iter().map(|entry| entry.size).sum()
    }

    pub fn clear(&self) {
        if !self.usable {
            return;
        }
        let Ok(read) = fs::read_dir(&self.dir) else { return };
        for entry in read.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".fmp4") || name.ends_with(".part") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    pub fn set_max_bytes(&self, max_bytes: u64) {
        self.max_bytes.store(max_bytes, Ordering::SeqCst);
        self.evict_to_fit();
    }

    /// Keeps an in-use key from eviction while it plays or is being prefetched.
    pub fn protect(&self, key: &CacheKey) {
        self.protected.lock().unwrap().insert(self.path_for(key));
    }

    pub fn unprotect(&self, key: &CacheKey) {
        self.protected.lock().unwrap().remove(&self.path_for(key));
    }

    /// Startup: remove stale `.part` files, then evict to fit.
    pub fn cleanup(&self) {
        if !self.usable {
            return;
        }
        let now = SystemTime::now();
        let Ok(read) = fs::read_dir(&self.dir) else { return };
        for entry in read.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().ends_with(".part") {
                continue;
            }
            let stale = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .map(|modified| now.duration_since(modified).unwrap_or_default() > STALE_PART_AGE)
                .unwrap_or(true);
            if stale {
                let _ = fs::remove_file(entry.path());
            }
        }
        self.evict_to_fit();
    }

    fn entries(&self) -> Vec<Entry> {
        if !self.usable {
            return Vec::new();
        }
        let Ok(read) = fs::read_dir(&self.dir) else { return Vec::new() };
        read.flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                if !name.to_string_lossy().ends_with(".fmp4") {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                Some(Entry {
                    path: entry.path(),
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                })
            })
            .collect()
    }

    fn evict_to_fit(&self) {
        if !self.usable {
            return;
        }
        let max = self.max_bytes.load(Ordering::SeqCst);
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|entry| entry.size).sum();
        if total <= max {
            return;
        }
        let protected = self.protected.lock().unwrap().clone();
        entries.sort_by_key(|entry| entry.modified);
        for entry in entries {
            if total <= max {
                break;
            }
            if protected.contains(&entry.path) {
                continue;
            }
            if fs::remove_file(&entry.path).is_ok() {
                total = total.saturating_sub(entry.size);
            }
        }
    }
}

pub struct CacheWriter {
    cache: Arc<DiskCache>,
    key: CacheKey,
    part: PathBuf,
    file: File,
    bytes: u64,
}

impl CacheWriter {
    /// Bytes written so far.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.file.write_all(data)?;
        self.bytes += data.len() as u64;
        Ok(())
    }

    /// Publish the temp file under its final name.
    pub fn commit(self) {
        let cache = self.cache.clone();
        cache.commit(self);
    }

    /// Drop a cancelled/failed download.
    pub fn abort(self) {
        let cache = self.cache.clone();
        cache.abort(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-cache-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn key(track_id: u64) -> CacheKey {
        CacheKey { track_id, quality: Quality::Lossless, manifest_hash: 0x9f3c }
    }

    fn write_entry(cache: &Arc<DiskCache>, key: &CacheKey, data: &[u8]) {
        let mut writer = cache.begin_write(key).unwrap();
        writer.write_all(data).unwrap();
        writer.commit();
    }

    fn set_mtime(path: &Path, time: SystemTime) {
        OpenOptions::new().write(true).open(path).unwrap().set_modified(time).unwrap();
    }

    #[test]
    fn file_name_is_deterministic_and_readable() {
        let key = CacheKey { track_id: 77646170, quality: Quality::Lossless, manifest_hash: 0x9f3c };
        assert_eq!(key.file_name(), "77646170-lossless-0000000000009f3c.fmp4");
        assert_eq!(key.file_name(), key.file_name());
    }

    #[test]
    fn begin_commit_produces_the_final_file_without_part() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = key(1);
        write_entry(&cache, &key, b"hello");
        assert!(cache.lookup(&key).is_some());
        assert!(!dir.join(format!("{}.part", key.file_name())).exists());
        assert_eq!(cache.size_bytes(), 5);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn abort_removes_the_part() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = key(1);
        let mut writer = cache.begin_write(&key).unwrap();
        writer.write_all(b"partial").unwrap();
        writer.abort();
        assert!(cache.lookup(&key).is_none());
        assert_eq!(cache.size_bytes(), 0);
        assert!(!dir.join(format!("{}.part", key.file_name())).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn lookup_touches_mtime() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = key(1);
        write_entry(&cache, &key, b"hello");
        let path = cache.path_for(&key);
        let old = SystemTime::now() - Duration::from_secs(3600);
        set_mtime(&path, old);
        let _ = cache.lookup(&key);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert!(modified > old, "lookup must touch the mtime for LRU");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn eviction_respects_the_cap_and_skips_protected() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let a = key(1);
        let b = key(2);
        let c = key(3);
        write_entry(&cache, &a, &[0u8; 10]);
        write_entry(&cache, &b, &[0u8; 10]);
        write_entry(&cache, &c, &[0u8; 10]);
        let now = SystemTime::now();
        set_mtime(&cache.path_for(&a), now - Duration::from_secs(3 * 3600));
        set_mtime(&cache.path_for(&b), now - Duration::from_secs(2 * 3600));
        set_mtime(&cache.path_for(&c), now - Duration::from_secs(3600));
        cache.protect(&a);
        cache.set_max_bytes(25);
        assert!(cache.lookup(&a).is_some(), "protected entry was evicted");
        assert!(cache.lookup(&b).is_none(), "the oldest unprotected entry survived");
        assert!(cache.lookup(&c).is_some());
        assert_eq!(cache.size_bytes(), 20);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_removes_entries_and_parts() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        write_entry(&cache, &key(1), b"data");
        let _writer = cache.begin_write(&key(2)).unwrap();
        cache.clear();
        assert_eq!(cache.size_bytes(), 0);
        assert!(!dir.join(format!("{}.part", key(2).file_name())).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cleanup_removes_stale_parts() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let stale = cache.begin_write(&key(1)).unwrap();
        let stale_path = dir.join(format!("{}.part", key(1).file_name()));
        drop(stale);
        let fresh = cache.begin_write(&key(2)).unwrap();
        set_mtime(&stale_path, SystemTime::now() - Duration::from_secs(2 * 3600));
        cache.cleanup();
        assert!(!stale_path.exists(), "stale part survived cleanup");
        assert!(dir.join(format!("{}.part", key(2).file_name())).exists());
        fresh.abort();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unusable_dir_disables_the_cache() {
        let dir = temp_dir();
        let file = dir.join("not-a-dir");
        fs::write(&file, b"x").unwrap();
        let cache = DiskCache::new(file, 1 << 20);
        assert!(!cache.usable());
        assert!(cache.lookup(&key(1)).is_none());
        assert!(cache.begin_write(&key(1)).is_none());
        assert_eq!(cache.size_bytes(), 0);
        cache.clear();
        cache.cleanup();
        cache.set_max_bytes(0);
        let _ = fs::remove_dir_all(dir);
    }
}
