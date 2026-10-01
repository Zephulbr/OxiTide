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
    /// Committed `.fmp4` bytes; seeded once by a scan, then maintained
    /// incrementally so normal play never rescans the directory.
    on_disk_bytes: AtomicU64,
    /// Bytes in in-flight `.part` files, maintained by writers.
    pending: AtomicU64,
    seeded: std::sync::atomic::AtomicBool,
    protected: Mutex<HashSet<PathBuf>>,
    /// Keys with a writer in flight; a second writer for the same key is refused.
    in_flight: Mutex<HashSet<CacheKey>>,
    usable: bool,
}

impl DiskCache {
    /// Infallible: an unusable directory (cannot create/write) disables the
    /// cache instead of failing startup.
    pub fn new(dir: PathBuf, max_bytes: u64) -> Arc<Self> {
        let usable = fs::create_dir_all(&dir).is_ok() && probe_writable(&dir);
        if !usable {
            tracing::warn!("cache directory {} is unusable; caching disabled", dir.display());
        }
        Arc::new(Self {
            dir,
            max_bytes: AtomicU64::new(max_bytes),
            on_disk_bytes: AtomicU64::new(0),
            pending: AtomicU64::new(0),
            seeded: std::sync::atomic::AtomicBool::new(false),
            protected: Mutex::new(HashSet::new()),
            in_flight: Mutex::new(HashSet::new()),
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

    /// Creates a `.part` temp file; `None` (and a log) when the cache is
    /// unusable or another writer is already in flight for the same key.
    pub fn begin_write(self: &Arc<Self>, key: &CacheKey) -> Option<CacheWriter> {
        if !self.usable {
            return None;
        }
        if !self.in_flight.lock().unwrap().insert(*key) {
            tracing::debug!("cache entry {} is already being written", key.file_name());
            return None;
        }
        let part = self.part_for(key);
        let _ = fs::remove_file(&part);
        match OpenOptions::new().write(true).create_new(true).open(&part) {
            Ok(file) => Some(CacheWriter {
                cache: self.clone(),
                key: *key,
                part,
                file: Some(file),
                bytes: 0,
                finished: false,
            }),
            Err(e) => {
                tracing::warn!("cache write for {} failed: {e}", key.file_name());
                self.in_flight.lock().unwrap().remove(key);
                None
            }
        }
    }

    /// Atomic publish: fsync temp, rename to the final name, touch mtime, evict.
    pub fn commit(&self, mut writer: CacheWriter) {
        writer.finish_commit();
    }

    /// Deletes a temp file after a cancelled/failed download.
    pub fn abort(&self, mut writer: CacheWriter) {
        writer.finish_abort();
    }

    fn add_saturating(value: &AtomicU64, delta: u64) {
        value.fetch_add(delta, Ordering::SeqCst);
    }

    fn sub_saturating(value: &AtomicU64, delta: u64) {
        let mut current = value.load(Ordering::SeqCst);
        loop {
            let next = current.saturating_sub(delta);
            match value.compare_exchange_weak(current, next, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }

    /// One directory scan to seed the committed total; later calls are O(1).
    fn seed_on_disk(&self) {
        if self.seeded.swap(true, Ordering::SeqCst) {
            return;
        }
        let total: u64 = self.entries().iter().map(|entry| entry.size).sum();
        self.on_disk_bytes.store(total, Ordering::SeqCst);
    }

    pub fn remove(&self, key: &CacheKey) {
        let path = self.path_for(key);
        if let Ok(meta) = fs::metadata(&path) {
            let size = meta.len();
            if fs::remove_file(&path).is_ok() {
                self.seed_on_disk();
                Self::sub_saturating(&self.on_disk_bytes, size);
            }
        }
    }

    /// Sum of every `.fmp4` entry; O(1) after the first seed.
    pub fn size_bytes(&self) -> u64 {
        self.seed_on_disk();
        self.on_disk_bytes.load(Ordering::SeqCst)
    }

    /// Sum of every in-flight `.part` file (a writer currently owns it).
    /// In-progress downloads are not counted against the cap.
    pub fn pending_bytes(&self) -> u64 {
        self.pending.load(Ordering::SeqCst)
    }

    pub fn is_protected(&self, key: &CacheKey) -> bool {
        self.protected.lock().unwrap().contains(&self.path_for(key))
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
        self.seeded.store(true, Ordering::SeqCst);
        self.on_disk_bytes.store(0, Ordering::SeqCst);
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
        self.entries_with_suffix(".fmp4")
    }

    fn entries_with_suffix(&self, suffix: &str) -> Vec<Entry> {
        if !self.usable {
            return Vec::new();
        }
        let Ok(read) = fs::read_dir(&self.dir) else { return Vec::new() };
        read.flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                if !name.to_string_lossy().ends_with(suffix) {
                    return None;
                }
                // Windows directory entries report a stale size while the file
                // is still open for writing, so query the path instead.
                let metadata = fs::metadata(entry.path()).ok()?;
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
        self.seed_on_disk();
        if self.on_disk_bytes.load(Ordering::SeqCst) <= max {
            return;
        }
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|entry| entry.size).sum();
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
        // A scan is authoritative; store what actually remains so the fast
        // path re-arms even if files were added or removed behind our back.
        self.on_disk_bytes.store(total, Ordering::SeqCst);
    }
}

/// True when a file can be created in `dir` (and cleaned up): an existing
/// read-only directory must not report itself usable.
fn probe_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".oxitide-probe-{}", std::process::id()));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub struct CacheWriter {
    cache: Arc<DiskCache>,
    key: CacheKey,
    part: PathBuf,
    file: Option<File>,
    bytes: u64,
    finished: bool,
}

impl CacheWriter {
    /// Bytes written so far.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("cache writer already finished"))?;
        file.write_all(data)?;
        self.bytes += data.len() as u64;
        DiskCache::add_saturating(&self.cache.pending, data.len() as u64);
        Ok(())
    }

    /// Publish the temp file under its final name.
    pub fn commit(mut self) {
        self.finish_commit();
    }

    /// Drop a cancelled/failed download.
    pub fn abort(mut self) {
        self.finish_abort();
    }

    fn finish_commit(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        let published = self.bytes;
        if let Some(file) = self.file.take() {
            if let Err(e) = file.sync_all() {
                tracing::warn!("cache fsync for {} failed: {e}", self.key.file_name());
            }
            drop(file);
        }
        // Seed before the rename: a not-yet-seeded scan must not see this file
        // as committed and then get `published` added on top of it.
        self.cache.seed_on_disk();
        let final_path = self.cache.path_for(&self.key);
        if final_path.exists() {
            let _ = fs::remove_file(&final_path);
        }
        let renamed = fs::rename(&self.part, &final_path);
        // Hold the single-flight key until the publish has finished, so a
        // concurrent writer cannot delete the temp file mid-rename.
        self.cache.in_flight.lock().unwrap().remove(&self.key);
        DiskCache::sub_saturating(&self.cache.pending, published);
        if let Err(e) = renamed {
            tracing::warn!("cache commit for {} failed: {e}", self.key.file_name());
            let _ = fs::remove_file(&self.part);
            return;
        }
        DiskCache::add_saturating(&self.cache.on_disk_bytes, published);
        if let Ok(file) = OpenOptions::new().write(true).open(&final_path) {
            let _ = file.set_modified(SystemTime::now());
        }
        if self.cache.on_disk_bytes.load(Ordering::SeqCst)
            > self.cache.max_bytes.load(Ordering::SeqCst)
        {
            self.cache.evict_to_fit();
        }
    }

    fn finish_abort(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.cache.in_flight.lock().unwrap().remove(&self.key);
        DiskCache::sub_saturating(&self.cache.pending, self.bytes);
        self.file.take();
        let _ = fs::remove_file(&self.part);
    }
}

impl Drop for CacheWriter {
    fn drop(&mut self) {
        // A writer that was neither committed nor explicitly aborted (e.g. a
        // dropped download) must not leave its temp file or in-flight key.
        self.finish_abort();
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
    fn pending_bytes_tracks_in_flight_parts() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = key(1);
        let mut writer = cache.begin_write(&key).unwrap();
        writer.write_all(b"12345").unwrap();
        assert_eq!(cache.pending_bytes(), 5);
        assert_eq!(cache.size_bytes(), 0, "a .part must not count as a committed entry");
        writer.commit();
        assert_eq!(cache.pending_bytes(), 0);
        assert_eq!(cache.size_bytes(), 5);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pending_bytes_ignores_orphan_parts() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let orphan = cache.part_for(&key(1));
        fs::write(&orphan, b"orphan").unwrap();
        assert_eq!(cache.pending_bytes(), 0, "a part without a writer must not count");
        assert_eq!(cache.size_bytes(), 0);
        fs::remove_file(&orphan).unwrap();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn commit_releases_the_in_flight_key() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = key(1);
        write_entry(&cache, &key, b"data");
        let second = cache.begin_write(&key).expect("the key was not released after commit");
        second.abort();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn begin_write_is_single_flight_per_key() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = key(1);
        let first = cache.begin_write(&key).unwrap();
        assert!(cache.begin_write(&key).is_none(), "a second writer was allowed");
        first.abort();
        assert!(cache.begin_write(&key).is_some(), "the key stayed in flight after abort");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dropping_a_writer_aborts_it() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let key = key(1);
        {
            let _writer = cache.begin_write(&key).unwrap();
        }
        assert!(
            !dir.join(format!("{}.part", key.file_name())).exists(),
            "the part survived the drop"
        );
        assert!(cache.begin_write(&key).is_some(), "the key stayed in flight after drop");
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
        let stale_path = dir.join(format!("{}.part", key(1).file_name()));
        fs::write(&stale_path, b"stale").unwrap();
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
        assert_eq!(cache.pending_bytes(), 0);
        cache.clear();
        cache.cleanup();
        cache.set_max_bytes(0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sizes_stay_correct_across_clear_abort_and_remove() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let a = key(1);
        let b = key(2);
        write_entry(&cache, &a, b"12345");
        assert_eq!(cache.size_bytes(), 5);
        let mut partial = cache.begin_write(&b).unwrap();
        partial.write_all(b"1234567").unwrap();
        assert_eq!(cache.pending_bytes(), 7);
        assert_eq!(cache.size_bytes(), 5);
        partial.abort();
        assert_eq!(cache.pending_bytes(), 0);
        cache.remove(&a);
        assert_eq!(cache.size_bytes(), 0);

        write_entry(&cache, &a, b"12345");
        let mut writer = cache.begin_write(&b).unwrap();
        writer.write_all(b"1234567").unwrap();
        cache.clear();
        assert_eq!(cache.size_bytes(), 0, "clear must reset the committed total");
        writer.abort();
        assert_eq!(cache.pending_bytes(), 0, "abort after clear must not underflow");
        assert_eq!(cache.size_bytes(), 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_commit_updates_both_counters_without_a_rescan() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let a = key(1);
        let mut writer = cache.begin_write(&a).unwrap();
        writer.write_all(b"1234567890").unwrap();
        assert_eq!(cache.pending_bytes(), 10);
        assert_eq!(cache.size_bytes(), 0);
        writer.commit();
        assert_eq!(cache.pending_bytes(), 0);
        assert_eq!(cache.size_bytes(), 10);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn is_protected_reports_the_protected_set() {
        let dir = temp_dir();
        let cache = DiskCache::new(dir.clone(), 1 << 20);
        let a = key(1);
        assert!(!cache.is_protected(&a));
        cache.protect(&a);
        assert!(cache.is_protected(&a));
        cache.unprotect(&a);
        assert!(!cache.is_protected(&a));
        let _ = fs::remove_dir_all(dir);
    }
}
