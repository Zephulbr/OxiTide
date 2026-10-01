# Track 3 Phase D — Memory, Cache/Eviction & Stability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bound the app's own caches, make normal playback scan-free, protect the playing/prefetched cache entries, fix the cached-seek defect, add the three deferred harness marks, then close Track 3 with the S1–S10 gate, an evidence-based memory renegotiation proposal, and the final note.

**Architecture:** Rust-only runtime changes plus perf-harness additions. Memory samples gain per-process attribution so the WebView2 floor is measurable before any budget decision (owner ruling: fix what's ours, then renegotiate in the final note with evidence). The palette cache gets the lyrics-style newest-N bound; `DiskCache` gains incremental on-disk/pending byte atomics so `finish_commit` stops rescanning the directory; `TrackStream` holds its `CacheKey` protection for the whole stream lifetime; the cached decode path learns where its source actually starts so seek discards match the network path. The harness gains a perf-only 10k queue-build command, a page-open timer, and a synthetic 500-track album, each gated in `budgets.json`.

**Tech Stack:** Rust stable, `std::sync::atomic`, Windows `windows` crate (already a dependency), Svelte 5 runes, Node 20 `node:test`, PowerShell runner.

**Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track3-performance-design.md` (§4.2 budgets, §6 Phase D, §7 DoD)
**Baseline:** `docs/superpowers/plans/notes/2026-09-30-track3-baseline.md` (memory 501–596 MB, deferred mark table, cached-seek defect)
**Phase B/C records:** `docs/superpowers/plans/notes/2026-10-01-track3-phase-b.md`, `docs/superpowers/plans/notes/2026-10-01-track3-phase-c.md`

## Global Constraints

- **Behavior-preserving (T3-D5):** no visible UI/animation change; no playback semantics change beyond fixing the cached-seek defect to match the network path. No IPC/event shape change.
- **Budgets are absolute except where the owner renegotiates in writing:** existing memory ceilings stay until the final note; the new rows added here (`queue_build_ms` ≤ 250, `page_open_ms` ≤ 500, `album_open_ms` ≤ 1000) are `required` (missing → FAIL). No existing budget is weakened.
- **Owner decisions already taken:** (1) fix the app-side memory bounds, then propose a written renegotiation in the final note with per-process evidence (no silent recalibration); (2) `S4.advance_max_gap_ms` stays at 100 with the WASAPI-ring finding documented, no metric-semantics change; (3) the default 2 GiB cache cap stays, current/next are protected, and a recommended cap is documented for the owner.
- **Commands:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`; `npm --prefix ui run check`; `npm --prefix ui test`; `node --test scripts/perf/check.test.mjs`. HTTP tests use wiremock only; never log token or credential values.
- **New `#[tauri::command]` fns must be added to `generate_handler![...]` in `main.rs`; invoke args stay camelCase.**
- **Perf-session-only code:** the queue-build command, synthetic album, and interaction timers only act while a perf session is active or the component is mounted during one; normal runs behave exactly as before.
- **Perf evidence:** raw runs live in `.superpowers/perf/runs/` (gitignored); phase summaries commit under `docs/superpowers/plans/notes/`. Quarantine, never delete, artifacts.
- Windows-first: background-thread code is plain `std::thread::Builder`; `keyring` uses `windows-native`.

## Review Focus

- **Eviction cannot starve a track it is about to play:** a prefetched cache-hit stream must stay protected from the moment its key is known until the stream is dropped; a commit for another track must not evict it. Pinned by Task 4's `a_cached_stream_protects_its_entry_for_its_lifetime`.
- **Bookkeeping must survive clear/abort interleavings:** a writer aborted after `clear()`, or a commit that races a removal, must leave `size_bytes()` at the true file total (no underflow, no drift). Pinned by Task 3's `sizes_stay_correct_across_clear_abort_and_remove`.
- **Cached seek must not overshoot duration:** a seek into a later segment of a fully cached multi-segment track starts at the target, not near zero, and cannot report positions past the item duration. Pinned by Task 5's `discard_is_relative_to_the_actual_source_start` plus the engine regression.
- **Palette pruning must keep what was used most recently:** after the cap, the highest `fetched_at` entries survive and old entries are dropped without breaking old-format files (`serde(default)`). Pinned by Task 2's `pruning_keeps_the_newest_palettes`.
- **The new gates cannot pass unmeasured:** missing `queue_build_ms` / `page_open_ms` / `album_open_ms` in their scenarios FAIL (required rows), and zero-count submissions are ignored. Pinned by Task 6/7's `check.test.mjs` additions.

---

## File Structure

- `crates/oxitide-app/src/perf.rs` — per-process memory rows (`memory_process` kind), aggregate kept.
- `crates/oxitide-app/src/artwork.rs` — `fetched_at` field, `PALETTE_MAX_ENTRIES`, prune-in-`save_disk`, borrowed serialization.
- `crates/oxitide-audio/src/cache.rs` — `on_disk_bytes` / `pending` atomics, `seeded` flag, incremental updates, `is_protected`.
- `crates/oxitide-audio/src/queue.rs` — no change (used by the new command).
- `crates/oxitide-audio/src/seek.rs` — `source_start_frames`, `discard_frames_for` helpers.
- `crates/oxitide-audio/src/engine.rs` — `TrackStream` key protection; `decode_source` source-start parameter; cached-seek fix; tests.
- `crates/oxitide-app/src/commands.rs` — `perf_queue_build` command; test.
- `crates/oxitide-app/src/main.rs` — handler registration.
- `ui/src/lib/perf.ts` — `InteractionLog` kinds `queue` / `page` / `album`.
- `ui/src/lib/perf.svelte.ts` — `recordInteraction` union.
- `ui/src/lib/perfFixture.ts` — `makeSyntheticAlbum`.
- `ui/src/lib/perfHooks.ts` — `buildSyntheticQueue`, `measureAlbumOpen` hooks.
- `ui/src/lib/api.ts` — `perfQueueBuild`.
- `ui/src/App.svelte` — bridge: `buildSyntheticQueue`, `gotoAlbum`, `albumHookReady`, `measureAlbumOpen`.
- `ui/src/views/CollectionTracks.svelte` — queue-build hook, page-open timer.
- `ui/src/views/Album.svelte` — synthetic album hook + timing.
- `scripts/perf/check.mjs` + `check.test.mjs` + `budgets.json` + `drive.mjs` + `run-scenario.ps1` — new metrics, gates, driver steps, S9 minutes flag.
- `docs/superpowers/plans/notes/<date>-track3-phase-d.md` and `-smoke.md` (new) — Task 9 records.
- `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md`, `AGENTS.md` — final status and perf commands.

---

### Task 1: Per-process memory attribution

**Files:**
- Modify: `crates/oxitide-app/src/perf.rs`

**Interfaces:**
- Consumes: the existing `Toolhelp32Snapshot` walk.
- Produces: `memory_process` JSONL lines `{kind, name, mb, self, t_ms}` alongside the unchanged aggregate `{kind:"memory", mb, t_ms}`; `process_tree_memory_mb()` stays and now sums the rows.

- [ ] **Step 1: Write the failing test**

In `perf.rs`'s `mod tests`, add:

```rust
    #[cfg(windows)]
    #[test]
    fn memory_samples_name_each_process_and_flag_the_app() {
        let dir = temp_dir("memproc");
        let session = PerfSession::create_in(&dir, "S1").unwrap();
        session.write_memory_sample();
        let text = std::fs::read_to_string(session.jsonl_path()).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("valid JSON"))
            .collect();
        let aggregate = lines.iter().find(|l| l["kind"] == "memory").expect("aggregate line");
        assert!(aggregate["mb"].as_u64().unwrap() > 0);
        let processes: Vec<_> =
            lines.iter().filter(|l| l["kind"] == "memory_process").collect();
        assert!(!processes.is_empty(), "at least the app process must be sampled");
        assert!(
            processes.iter().any(|l| l["self"].as_bool() == Some(true)
                && l["mb"].as_u64().unwrap() > 0
                && l["name"].as_str().is_some_and(|n| !n.is_empty())),
            "the app process must be named and flagged"
        );
        let sum: u64 = processes.iter().map(|l| l["mb"].as_u64().unwrap()).sum();
        assert_eq!(sum, aggregate["mb"].as_u64().unwrap(), "aggregate must be the row sum");
        let _ = std::fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app memory_samples_name_each_process_and_flag_the_app`
Expected: FAIL — no `memory_process` lines (and `write_memory_sample` writes only the aggregate).

- [ ] **Step 3: Split the walk into rows and write them**

Replace `process_tree_memory_mb` with a row-producing function and a summing wrapper:

```rust
/// Working set of this process plus all descendant processes (the WebView2
/// helpers), in MiB, one row per process: (name, mb, is_the_app_itself).
#[cfg(windows)]
fn process_memory_rows() -> Vec<(String, u64, bool)> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    let self_pid = std::process::id();
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut parents: Vec<(u32, u32, String)> = Vec::new();
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                parents.push((entry.th32ProcessID, entry.th32ParentProcessID, name));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    let is_descendant = |pid: u32| {
        let mut current = pid;
        for _ in 0..8 {
            if current == self_pid {
                return true;
            }
            match parents.iter().find(|(p, _, _)| *p == current) {
                Some((_, parent, _)) if *parent != 0 => current = *parent,
                _ => return false,
            }
        }
        false
    };
    let mut rows = Vec::new();
    for &(pid, _, ref name) in &parents {
        if !is_descendant(pid) {
            continue;
        }
        let handle: HANDLE =
            match unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) } {
                Ok(handle) => handle,
                Err(_) => continue,
            };
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let ok = unsafe {
            GetProcessMemoryInfo(
                handle,
                &mut counters,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
        };
        let _ = unsafe { CloseHandle(handle) };
        if ok.is_ok() {
            rows.push((name.clone(), counters.WorkingSetSize as u64 / (1024 * 1024), pid == self_pid));
        }
    }
    rows
}

#[cfg(not(windows))]
fn process_memory_rows() -> Vec<(String, u64, bool)> {
    Vec::new()
}

/// Working set of this process plus all descendant processes, in MiB.
fn process_tree_memory_mb() -> Option<u64> {
    let rows = process_memory_rows();
    if rows.is_empty() {
        return None;
    }
    Some(rows.iter().map(|(_, mb, _)| mb).sum())
}
```

Delete the old `#[cfg(windows)]`/`#[cfg(not(windows))]` pair for `process_tree_memory_mb` (the stub `None` is now the shared wrapper above). Replace `write_memory_sample`:

```rust
    pub fn write_memory_sample(&self) {
        let rows = process_memory_rows();
        if rows.is_empty() {
            return;
        }
        let total: u64 = rows.iter().map(|(_, mb, _)| mb).sum();
        let t_ms = self.elapsed_ms();
        self.write(serde_json::json!({
            "kind": "memory",
            "mb": total,
            "t_ms": t_ms,
        }));
        for (name, mb, is_self) in rows {
            self.write(serde_json::json!({
                "kind": "memory_process",
                "name": name,
                "mb": mb,
                "self": is_self,
                "t_ms": t_ms,
            }));
        }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app perf`
Expected: PASS — the new test and the existing `process_tree_memory_is_positive`.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p oxitide-app -- -D warnings`

```bash
git add crates/oxitide-app/src/perf.rs
git commit -m "feat(perf): attribute memory samples per process"
```

---

### Task 2: Bound the palette cache

**Files:**
- Modify: `crates/oxitide-app/src/artwork.rs`

**Interfaces:**
- Consumes: `PaletteDto` (serde), the lazy `memory()` map.
- Produces: `PALETTE_MAX_ENTRIES = 2_000`; `PaletteDto.fetched_at: u64` (serde default 0, so old `palettes.json` files still parse and their entries prune first); `save_disk` keeps the newest entries and serializes a borrowed map (no whole-map clone under the lock).

- [ ] **Step 1: Write the failing tests**

In `artwork.rs`'s `mod tests`, update the `palette_dto` helper to include the new field and add the pruning test:

```rust
    fn palette_dto(primary: &str) -> PaletteDto {
        PaletteDto {
            primary: primary.into(),
            on_primary: "#000000".into(),
            secondary: "#111111".into(),
            tertiary: "#222222".into(),
            backdrop: "#333333".into(),
            ceiling_l: 0.5,
            fetched_at: 0,
        }
    }

    #[tokio::test]
    async fn pruning_keeps_the_newest_palettes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let dir = temp_dir();
        let service = service(&server, &dir);
        {
            let mut memory = service.memory().lock().unwrap();
            for i in 0..(PALETTE_MAX_ENTRIES + 5) {
                let mut dto = palette_dto("#000000");
                dto.fetched_at = i as u64;
                memory.insert(format!("uuid-{i:05}"), dto);
            }
        }
        service.save_disk();

        let memory = service.memory().lock().unwrap();
        assert_eq!(memory.len(), PALETTE_MAX_ENTRIES);
        assert!(memory.contains_key("uuid-02004"), "the newest entry was pruned");
        assert!(!memory.contains_key("uuid-00000"), "the oldest entry survived");
        drop(memory);
        let text = std::fs::read_to_string(dir.join("palettes.json")).unwrap();
        let cache: DiskCache = serde_json::from_str(&text).unwrap();
        assert_eq!(cache.palettes.len(), PALETTE_MAX_ENTRIES);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn old_palette_files_without_fetched_at_still_parse() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let dir = temp_dir();
        let path = dir.join("palettes.json");
        std::fs::write(
            &path,
            r#"{"palettes":{"ab-cd":{"primary":"#aaaaaa","on_primary":"#000000","secondary":"#111111","tertiary":"#222222","backdrop":"#333333","ceiling_l":0.5}}}"#,
        )
        .unwrap();
        let palette = service(&server, &dir).palette("ab-cd").await;
        assert_eq!(palette.primary, "#aaaaaa");
        let _ = std::fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app pruning_keeps_the_newest_palettes old_palette_files`
Expected: FAIL — `fetched_at` and `PALETTE_MAX_ENTRIES` do not exist; the cap test panics on `memory.len()`.

- [ ] **Step 3: Add the field, cap, prune, and borrowed serialization**

At the top of `artwork.rs`:

```rust
const ANALYSIS_SIZE: u32 = 160;
const EXTRACT_SIDE: u32 = 48;
/// Newest-N bound, mirroring `lyrics::CACHE_MAX_ENTRIES`; a palette entry is
/// tiny, but the unbounded map also made every save an O(n) full-file rewrite.
const PALETTE_MAX_ENTRIES: usize = 2_000;

#[derive(Default, Serialize, Deserialize)]
struct DiskCache {
    palettes: HashMap<String, PaletteDto>,
}

/// Borrowed view for serialization, so `save_disk` never clones the map.
#[derive(Serialize)]
struct DiskCacheRef<'a> {
    palettes: &'a HashMap<String, PaletteDto>,
}
```

Add the field to `PaletteDto` and its `From<Palette>`:

```rust
    pub backdrop: String,
    pub ceiling_l: f32,
    /// Unix seconds of the last fetch; pre-Phase-D entries default to 0 and
    /// prune first. Not part of the API surface.
    #[serde(default)]
    pub fetched_at: u64,
```

```rust
impl From<Palette> for PaletteDto {
    fn from(p: Palette) -> Self {
        Self {
            primary: p.primary,
            on_primary: p.on_primary,
            secondary: p.secondary,
            tertiary: p.tertiary,
            backdrop: p.backdrop,
            ceiling_l: p.ceiling_l,
            fetched_at: 0,
        }
    }
}
```

In `palette()`, stamp the insert:

```rust
        let mut dto = PaletteDto::from(palette.clone());
        dto.fetched_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.memory().lock().unwrap().insert(uuid.to_string(), dto);
        self.save_disk();
        palette
```

Replace `save_disk`:

```rust
    fn save_disk(&self) {
        let mut memory = self.memory().lock().unwrap();
        if memory.len() > PALETTE_MAX_ENTRIES {
            let mut entries: Vec<(String, PaletteDto)> = memory.drain().collect();
            entries.sort_by_key(|(_, dto)| std::cmp::Reverse(dto.fetched_at));
            entries.truncate(PALETTE_MAX_ENTRIES);
            memory.extend(entries);
        }
        let Ok(text) = serde_json::to_string_pretty(&DiskCacheRef { palettes: &memory }) else {
            return;
        };
        let tmp = self.disk_path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &self.disk_path);
        }
    }
```

- [ ] **Step 4: Run the artwork tests to verify they pass**

Run: `cargo test -p oxitide-app artwork`
Expected: PASS — the two new tests and the five existing artwork tests.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p oxitide-app -- -D warnings`

```bash
git add crates/oxitide-app/src/artwork.rs
git commit -m "perf(app): bound and prune the palette cache"
```

---

### Task 3: Incremental cache size bookkeeping

**Files:**
- Modify: `crates/oxitide-audio/src/cache.rs`

**Interfaces:**
- Consumes: `CacheWriter`, `Entry`, `entries()`.
- Produces: O(1) `size_bytes()` / `pending_bytes()` backed by `on_disk_bytes` / `pending` atomics; `evict_to_fit` only scans when the tracked total exceeds the cap; `finish_commit` no longer scans on normal play; `is_protected(&CacheKey) -> bool` (new public accessor used by Task 4's engine test).

- [ ] **Step 1: Write the failing tests**

Append to `cache.rs`'s `mod tests`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio cache::`
Expected: FAIL — `is_protected` does not exist; the counter tests pass against the current scan-based code only because it recomputes each call, so the RED signal is `is_protected` plus the underflow path in `clear`/`abort` (the new assertions are the contract for the atomics).

- [ ] **Step 3: Add the counters**

In the `DiskCache` struct:

```rust
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
```

In `new`, initialize `on_disk_bytes: AtomicU64::new(0)`, `pending: AtomicU64::new(0)`, `seeded: AtomicBool::new(false)`.

Add the helpers and replace `size_bytes`/`pending_bytes`:

```rust
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
```

Update `remove`, `clear`, `evict_to_fit`, and the writer paths:

```rust
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
```

```rust
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
```

```rust
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
```

```rust
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
```

In `finish_commit`, after the rename succeeds the published bytes move from pending to committed and only an over-cap total triggers eviction:

```rust
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
        self.cache.seed_on_disk();
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
```

In `finish_abort`, subtract the written bytes before removing the part:

```rust
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
```

- [ ] **Step 4: Run the cache and engine suites to verify they pass**

Run: `cargo test -p oxitide-audio`
Expected: PASS — the three new tests plus all 14 existing cache tests and the engine suite (the engine relies on `evict_to_fit` semantics).

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p oxitide-audio -- -D warnings`

```bash
git add crates/oxitide-audio/src/cache.rs
git commit -m "perf(audio): track cache bytes incrementally and skip scans under the cap"
```

---

### Task 4: Protect the playing and prefetched cache entries

**Files:**
- Modify: `crates/oxitide-audio/src/engine.rs`
- Test: `crates/oxitide-audio/src/engine.rs` (mod tests)

**Interfaces:**
- Consumes: `DiskCache::protect/unprotect/is_protected`, `CacheKey`.
- Produces: `TrackStream` holds `key: Arc<Mutex<Option<CacheKey>>>`; its `Drop` unprotects; `decode_track` protects the key once known and holds it for the stream's lifetime instead of only around `decode_from_file`.

- [ ] **Step 1: Write the failing test**

Add to `engine.rs`'s `mod tests` (uses the existing `fixture_plan`, `cached_plan`, `item`, `test_cache`, `wait_for`, `NullOutput` helpers):

```rust
    /// Review Focus: a cached stream keeps its entry protected from the moment
    /// the key is known until the stream is dropped, so a commit for another
    /// track cannot evict the one being played or prefetched.
    #[test]
    fn a_cached_stream_protects_its_entry_for_its_lifetime() {
        const HASH: u64 = 0xabcd;
        let (tone_plan, files) = fixture_plan();
        let tone = files.get("mem://tone").cloned().expect("fixture bytes");
        let cache = test_cache();
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: HASH };
        let mut writer = cache.begin_write(&key).unwrap();
        writer.write_all(&tone).unwrap();
        writer.commit();

        let mut plan = cached_plan("mem://tone", HASH);
        plan.sample_rate = tone_plan.sample_rate;
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| cache.is_protected(&key));

        engine.handle(EngineCmd::Clear).unwrap();
        wait_for(|| !cache.is_protected(&key));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio a_cached_stream_protects`
Expected: FAIL — protection ends when `decode_from_file` returns, so the first `wait_for(cache.is_protected)` may or may not observe a transient true; the deterministic failure is the second phase only if decode is still running. To make the RED deterministic, run it and confirm it fails for a missing guarantee (the test is written to the post-fix contract; the pre-fix code unprotects as soon as the sink is built and the file is decoded into the bounded channel, so `wait_for` after `Clear` can hang). If it passes spuriously on this machine, tighten it to assert protection is still held after the first `wait_for` plus a 50 ms settle before `Clear`.

- [ ] **Step 3: Hold the key on the stream**

Change `TrackStream`:

```rust
/// A running resolve/fetch/decode pipeline for one track.
struct TrackStream {
    item: QueueItem,
    rx: Receiver<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
    /// The cache key this stream protects while it is alive, once resolved.
    cache_key: Arc<Mutex<Option<CacheKey>>>,
}

impl Drop for TrackStream {
    fn drop(&mut self) {
        // Stop the producer's fetch/decode promptly when the stream is
        // abandoned (next/prev/seek/clear or track change), and release the
        // cache protection that kept the playing/prefetched file on disk.
        self.buffer.cancel();
        if let Some(key) = self.cache_key.lock().unwrap().take() {
            self.inner_cache_unprotect(&key);
        }
    }
}
```

`TrackStream` has no `inner`, so hold the cache handle directly:

```rust
struct TrackStream {
    item: QueueItem,
    rx: Receiver<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
    cache: Arc<DiskCache>,
    /// The cache key this stream protects while it is alive, once resolved.
    cache_key: Arc<Mutex<Option<CacheKey>>>,
}

impl Drop for TrackStream {
    fn drop(&mut self) {
        self.buffer.cancel();
        if let Some(key) = self.cache_key.lock().unwrap().take() {
            self.cache.unprotect(&key);
        }
    }
}
```

In `start_track`, create the slot and pass it through:

```rust
fn start_track(
    inner: &Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
) -> TrackStream {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<StreamEvent>>(PREFETCH_BUFFERS);
    let buffer = StreamBuffer::new();
    let cache_key = Arc::new(Mutex::new(None));
    let stream = TrackStream {
        item: item.clone(),
        rx,
        buffer: buffer.clone(),
        cache: inner.cache.clone(),
        cache_key: cache_key.clone(),
    };
    let decode_inner = inner.clone();
    let item = item.clone();
    let spawned = std::thread::Builder::new()
        .name("oxitide-decode".into())
        .spawn(move || {
            decode_track(decode_inner, item, mode, generation, seek_ms, tx, buffer, cache_key)
        });
    if let Err(e) = spawned {
        tracing::error!("failed to spawn decode thread: {e}");
        inner.notify_worker();
    }
    stream
}
```

Check whether `Inner.cache` is `Arc<DiskCache>` (it is, used as `inner.cache.lookup`); if the field is named differently, use the actual name. In `decode_track`, take the slot and protect once the key exists:

```rust
fn decode_track(
    inner: Arc<Inner>,
    item: QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: SyncSender<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
    cache_key_slot: Arc<Mutex<Option<CacheKey>>>,
) {
```

After computing `cache_key`:

```rust
    let cache_key = plan.manifest_hash.map(|manifest_hash| CacheKey {
        track_id: item.track_id,
        quality,
        manifest_hash,
    });
    if let Some(key) = cache_key {
        *cache_key_slot.lock().unwrap() = Some(key);
        inner.cache.protect(&key);
    }
```

Remove the now-redundant protect/unprotect pair around `decode_from_file`:

```rust
            Some(path) => {
                fire_perf(&inner, AudioPerfEvent::CacheHit { track_id: item.track_id });
                let result =
                    decode_from_file(&inner, &item, mode, generation, seek_ms, &tx, &path, &plan);
```

(The `Err` fallback still calls `inner.cache.remove(&key)`; the stream slot keeps the path protected until drop, which is harmless for a removed file.)

- [ ] **Step 4: Run the engine suite to verify it passes**

Run: `cargo test -p oxitide-audio engine::`
Expected: PASS — the new test plus the cache/prefetch/advance/device-loss suites.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p oxitide-audio -- -D warnings`

```bash
git add crates/oxitide-audio/src/engine.rs
git commit -m "perf(audio): protect playing and prefetched cache entries for the stream lifetime"
```

---

### Task 5: Fix the cached-seek defect

**Files:**
- Modify: `crates/oxitide-audio/src/seek.rs`
- Modify: `crates/oxitide-audio/src/engine.rs` (`decode_source`, `ChannelSink`, both callers; test)

**Interfaces:**
- Consumes: `SeekTarget`, `StreamPlan`.
- Produces: `seek::source_start_frames(plan) -> u64` (first segment start in frames; 0 for empty/timescale-0 plans) and `seek::discard_frames_for(target, source_start_frames) -> u64`; `decode_source` and `ChannelSink` take `source_start_frames: u64` and discard `target.position_frames - source_start_frames`, so a cached multi-segment file (which starts at track frame 0) discards the full offset while the network path (which starts at the target segment) keeps its current behavior.

- [ ] **Step 1: Write the failing tests**

In `seek.rs`'s `mod tests`:

```rust
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
```

In `engine.rs`'s `mod tests`, add the end-to-end regression (a cached multi-segment track seeked into segment 1 must start near the target; the pre-fix code discards only the segment-relative offset and plays from near zero):

```rust
    /// Review Focus: seeking a fully cached multi-segment track must discard
    /// relative to the file's actual start, not the target segment's start.
    #[test]
    fn cached_seek_starts_at_the_target() {
        const HASH: u64 = 0x5eed;
        let (tone_plan, files) = fixture_plan();
        let tone = files.get("mem://tone").cloned().expect("fixture bytes");
        let cache = test_cache();
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: HASH };
        let mut writer = cache.begin_write(&key).unwrap();
        writer.write_all(&tone).unwrap();
        writer.commit();

        // Two one-second segments at 44100 Hz; seek to 1500 ms lands in the
        // second segment, which starts at frame 44100.
        let mut plan = cached_plan("mem://tone", HASH);
        plan.init_url = None;
        plan.timescale = 44100;
        plan.segments = vec![
            Segment { url: "mem://tone".into(), start_ticks: 0, duration_ticks: 44100 },
            Segment { url: "mem://tone".into(), start_ticks: 44100, duration_ticks: 44100 },
        ];
        plan.sample_rate = tone_plan.sample_rate;

        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !output.frames_written().is_empty());
        engine.handle(EngineCmd::Seek(1500)).unwrap();

        // Post-fix, only the ~0.5 s after the target is written; pre-fix, ~1.5 s.
        wait_for(|| output.frames_written().last().is_some_and(|total| total < 44_100));
    }
```

Adapt the assertion to the actual `NullOutput` API used by the neighbouring tests (the test module's `worker_writes_samples_and_advances` shows how written samples are inspected; use that pattern if the method name differs).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio discard_is_relative source_start_frames`
Expected: FAIL — the helpers do not exist.

- [ ] **Step 3: Add the helpers and thread the origin through**

Append to `seek.rs`:

```rust
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
```

In `engine.rs`, add a field to `ChannelSink`:

```rust
    plan: StreamPlan,
    seek_ms: Option<u64>,
    /// The track frame the decoded stream actually begins at (0 for a cached
    /// file, the target segment's start for a seeked network stream).
    source_start_frames: u64,
    discarder: Discarder,
```

In `decode_source`, take the parameter and pass it through:

```rust
fn decode_source(
    inner: Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    source_start_frames: u64,
    tx: &SyncSender<Result<StreamEvent>>,
    source: impl symphonia::core::io::MediaSource + 'static,
    plan: StreamPlan,
    total_bytes: Arc<dyn Fn() -> u64 + Send + Sync>,
) -> Result<()> {
    let mut sink = ChannelSink {
        inner,
        generation,
        tx: tx.clone(),
        mode,
        out_format: PcmFormat::F32,
        channels: 2,
        plan,
        seek_ms,
        source_start_frames,
        discarder: Discarder::default(),
        start_frames: 0,
        duration_secs: item.duration_secs,
        total_bytes,
    };
    crate::decode::run(source, "mp4", &mut sink)
}
```

In `on_stream_info`, use the helper:

```rust
        if let Some(ms) = self.seek_ms {
            let target = seek::target_segment(&self.plan, ms, info.sample_rate);
            self.discarder =
                Discarder::new(seek::discard_frames_for(&target, self.source_start_frames));
            self.start_frames = target.position_frames;
        }
```

In `decode_from_network`, compute the target once and pass the origin:

```rust
    // Seek by segment index: media URLs are an ordered list, so restarting
    // at the segment covering the target needs no HTTP Range.
    let target = match seek_ms {
        Some(ms) if plan.sample_rate.is_some() => {
            Some(seek::target_segment(&plan, ms, plan.sample_rate.unwrap()))
        }
        _ => None,
    };
    let download_plan = match &target {
        Some(target) => plan.from_segment(target.segment_index),
        None => plan.clone(),
    };
    let source_start =
        target.as_ref().map(|t| t.position_frames - t.discard_frames).unwrap_or(0);
```

and pass `source_start` to `decode_source` (before `&tx`).

In `decode_from_file`, pass the file's origin:

```rust
    decode_source(
        inner.clone(),
        item,
        mode,
        generation,
        seek_ms,
        seek::source_start_frames(plan),
        tx,
        source,
        plan.clone(),
        total_bytes,
    )
```

- [ ] **Step 4: Run the audio suite to verify it passes**

Run: `cargo test -p oxitide-audio`
Expected: PASS — the new seek tests, the engine seek tests (network path unchanged: `discard_frames_for(target, target.position_frames - target.discard_frames) == target.discard_frames`), and the pipeline integration test.

- [ ] **Step 5: Clippy, Plan 2/3 smoke prep, and commit**

Run: `cargo clippy -p oxitide-audio -- -D warnings`

```bash
git add crates/oxitide-audio/src/seek.rs crates/oxitide-audio/src/engine.rs
git commit -m "fix(audio): discard cached seeks relative to the source start"
```

The owner smoke checklist in Task 9 covers seeking cached tracks; no other behavior change.

---

### Task 6: Gate the 10k play-all queue build

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs`, `crates/oxitide-app/src/main.rs`, `ui/src/lib/api.ts`, `ui/src/lib/perfHooks.ts`, `ui/src/lib/perf.ts`, `ui/src/lib/perf.test.ts`, `ui/src/lib/perf.svelte.ts`, `ui/src/App.svelte`, `ui/src/views/CollectionTracks.svelte`
- Modify: `scripts/perf/check.mjs`, `check.test.mjs`, `budgets.json`, `drive.mjs`

**Interfaces:**
- Consumes: `Queue::play_context` (pure build), `makeSyntheticTracks`, the perf session gate.
- Produces: `perf_queue_build(items) -> Result<(), String>` (perf-session-only; builds a throwaway `Queue`); bridge `buildSyntheticQueue(count): Promise<boolean>`; `InteractionLog` kind `"queue"` with `queue_count` / `queue_max_ms`; `check.mjs` metric `queue_build_ms`; budget S7 `queue_build_ms ≤ 250, required`.

- [ ] **Step 1: Write the failing tests**

In `commands.rs`'s `mod tests`:

```rust
    #[test]
    fn perf_queue_build_requires_a_session() {
        let items = vec![oxitide_audio::queue::QueueItem {
            track_id: 1,
            title: "T".into(),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 1,
            cover: None,
        }];
        let result = perf_queue_build_items(items, 0);
        assert!(result.is_err(), "the perf command must not run outside a session");
        assert_eq!(result.unwrap_err(), "no perf session");
    }
```

Refactor the command body into a testable helper:

```rust
fn perf_queue_build_items(
    items: Vec<oxitide_audio::queue::QueueItem>,
    start: usize,
) -> Result<(), String> {
    if crate::perf::session().is_none() {
        return Err("no perf session".into());
    }
    let mut queue = oxitide_audio::queue::Queue::default();
    queue.play_context(items, start);
    Ok(())
}
```

In `ui/src/lib/perf.test.ts`, extend the `InteractionLog` test with a queue sample and assert the new fields:

```ts
    log.record("queue", 42);
    expect(log.summary()).toEqual({
      filter_count: 2,
      filter_max_ms: 27,
      sort_count: 1,
      sort_max_ms: 40,
      queue_count: 1,
      queue_max_ms: 42,
    });
```

(and the reset expectation gains `queue_count: 0, queue_max_ms: 0`).

In `scripts/perf/check.test.mjs`:

```js
test("queue build samples aggregate into queue_build_ms", () => {
  const file = runFile([
    { kind: "session", run_id: "S7-1", scenario: "S7" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: { interactions: { queue_count: 1, queue_max_ms: 123, filter_count: 0, filter_max_ms: 0, sort_count: 0, sort_max_ms: 0 } },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.queue_build_ms, 123);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app perf_queue_build; if ($?) { npm --prefix ui test -- src/lib/perf.test.ts }; node --test scripts/perf/check.test.mjs`
Expected: FAIL — no `perf_queue_build_items`, no queue fields/metric.

- [ ] **Step 3: Implement the command and hook**

`commands.rs`:

```rust
/// Perf-session-only: builds a 10k-track play context so the S7 gate can
/// measure queue construction without touching playback. No-op error outside
/// a session, so normal runs cannot reach it.
#[tauri::command]
pub fn perf_queue_build(
    items: Vec<oxitide_audio::queue::QueueItem>,
    start: usize,
) -> Result<(), String> {
    perf_queue_build_items(items, start)
}
```

`main.rs`: add `commands::perf_queue_build,` next to the other perf commands in `generate_handler![...]`.

`ui/src/lib/api.ts`:

```ts
  perfQueueBuild: (items: QueueItem[]) => invoke<void>("perf_queue_build", { items, start: 0 }),
```

`ui/src/lib/perf.ts`: widen `InteractionLog`:

```ts
export interface InteractionSummary {
  filter_count: number;
  filter_max_ms: number;
  sort_count: number;
  sort_max_ms: number;
  queue_count: number;
  queue_max_ms: number;
}
```

with `queueCount`/`queueMax` fields and a `record(kind: "filter" | "sort" | "queue", ms)` branch. `ui/src/lib/perf.svelte.ts`'s `recordInteraction` takes the same union.

`ui/src/lib/perfHooks.ts`:

```ts
export interface PerfHooks {
  loadSyntheticTracks?: (count: number) => void;
  toggleViewSort?: (key: "title" | "duration") => void;
  buildSyntheticQueue?: (count: number) => Promise<void>;
}
```

`ui/src/views/CollectionTracks.svelte`, inside the existing `if (!perf.active) return;` effect:

```ts
    perfHooks.buildSyntheticQueue = async (count: number) => {
      const items = makeSyntheticTracks(count).map(trackToQueueItem);
      const started = performance.now();
      await api.perfQueueBuild(items);
      perf.recordInteraction("queue", performance.now() - started);
    };
```

(cleanup deletes it). `ui/src/App.svelte` bridge:

```ts
      buildSyntheticQueue: async (count: number) => {
        if (!perfHooks.buildSyntheticQueue) return false;
        await perfHooks.buildSyntheticQueue(count);
        return true;
      },
```

- [ ] **Step 4: Gate it in the harness**

`scripts/perf/check.mjs` `ui_metrics` branch:

```js
        if (typeof interactions.queue_max_ms === "number" && interactions.queue_count > 0) {
          metrics.queue_build_ms = Math.max(metrics.queue_build_ms ?? 0, interactions.queue_max_ms);
        }
```

`scripts/perf/budgets.json` S7:

```json
  "S7": [
    { "metric": "memory_max_mb", "max": 450 },
    { "metric": "queue_build_ms", "max": 250, "required": true }
  ],
```

`scripts/perf/drive.mjs` S7 branch, after the existing play and delay:

```js
      const built = await evaluate("window.__perf.buildSyntheticQueue(10000)");
      if (!built) throw new Error("S7: synthetic queue build unavailable");
      await delay(1000);
```

- [ ] **Step 5: Run all the affected suites**

Run: `cargo test -p oxitide-app; if ($?) { npm --prefix ui test; if ($?) { npm --prefix ui run check; if ($?) { node --test scripts/perf/check.test.mjs } } }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs ui/src/lib/api.ts ui/src/lib/perfHooks.ts ui/src/lib/perf.ts ui/src/lib/perf.test.ts ui/src/lib/perf.svelte.ts ui/src/App.svelte ui/src/views/CollectionTracks.svelte scripts/perf/check.mjs scripts/perf/check.test.mjs scripts/perf/budgets.json scripts/perf/drive.mjs
git commit -m "feat(perf): gate the 10k play-all queue build"
```

---

### Task 7: Gate page-open and the 500-track album

**Files:**
- Modify: `ui/src/lib/perfFixture.ts`, `perfFixture.test.ts`, `perf.ts`, `perf.test.ts`, `perf.svelte.ts`, `perfHooks.ts`, `App.svelte`, `views/CollectionTracks.svelte`, `views/Album.svelte`
- Modify: `scripts/perf/check.mjs`, `check.test.mjs`, `budgets.json`, `drive.mjs`

**Interfaces:**
- Consumes: `InteractionLog` (queue kind from Task 6), `Track`/`Album` DTOs.
- Produces: `makeSyntheticAlbum(count): { album: Album; tracks: Track[] }`; `InteractionLog` kinds `"page"` / `"album"` with `page_count`/`page_max_ms` and `album_count`/`album_max_ms`; bridge `gotoAlbum(id)`, `albumHookReady()`, `measureAlbumOpen(count)`; `check.mjs` metrics `page_open_ms`, `album_open_ms`; budget rows S5-instrument/S5-atmosphere `page_open_ms ≤ 500 required`, S6 `album_open_ms ≤ 1000 required`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/perfFixture.test.ts`:

```ts
  it("builds a deterministic 500-track album", () => {
    const { album, tracks } = makeSyntheticAlbum(500);
    expect(album.id).toBe(-1);
    expect(album.title).toContain("500");
    expect(album.numberOfTracks).toBe(500);
    expect(tracks).toHaveLength(500);
    expect(tracks[0].album?.id).toBe(album.id);
    const again = makeSyntheticAlbum(500);
    expect(again.tracks[42].title).toBe(tracks[42].title);
  });
```

`ui/src/lib/perf.test.ts`, in the `InteractionLog` test:

```ts
    log.record("page", 80);
    log.record("album", 240);
    expect(log.summary()).toMatchObject({
      page_count: 1,
      page_max_ms: 80,
      album_count: 1,
      album_max_ms: 240,
    });
```

`scripts/perf/check.test.mjs`:

```js
test("page and album interactions aggregate into their metrics", () => {
  const file = runFile([
    { kind: "session", run_id: "S5-instrument-1", scenario: "S5-instrument" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: { mode: "instrument", interactions: { page_count: 1, page_max_ms: 210, album_count: 1, album_max_ms: 640, filter_count: 0, filter_max_ms: 0, sort_count: 0, sort_max_ms: 0, queue_count: 0, queue_max_ms: 0 } },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.page_open_ms, 210);
  assert.equal(metrics.album_open_ms, 640);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/perfFixture.test.ts src/lib/perf.test.ts; node --test scripts/perf/check.test.mjs`
Expected: FAIL — `makeSyntheticAlbum` and the new fields/metrics do not exist.

- [ ] **Step 3: Implement the fixture, hooks, and timers**

`ui/src/lib/perfFixture.ts`:

```ts
import type { Album, Track } from "./api";

/// Deterministic synthetic album for the 500-track open gate; reuses the track
/// generator so list rendering and filtering see the same shapes as S6.
export function makeSyntheticAlbum(count: number): { album: Album; tracks: Track[] } {
  const tracks = makeSyntheticTracks(count).map((track) => ({
    ...track,
    album: { id: -1, title: `Synthetic Album (${count})`, cover: null },
  }));
  return {
    album: {
      id: -1,
      title: `Synthetic Album (${count})`,
      artists: [{ id: -1, name: "OxiTide Perf" }],
      cover: null,
      numberOfTracks: count,
      duration: tracks.reduce((sum, t) => sum + t.duration, 0),
      audioQuality: "LOSSLESS",
    },
    tracks,
  };
}
```

`ui/src/lib/perf.ts`: extend `InteractionSummary` with `page_count/page_max_ms/album_count/album_max_ms` and the record union to `"filter" | "sort" | "queue" | "page" | "album"` (same pattern as Task 6). `perf.svelte.ts`'s `recordInteraction` union matches.

`ui/src/views/CollectionTracks.svelte` — page-open timer, once per run, real page only:

```ts
  let pageTimed = false;
  $effect(() => {
    if (pageTimed || synthetic !== null || rows.length === 0 || !perf.active) return;
    pageTimed = true;
    const started = performance.now();
    requestAnimationFrame(() =>
      perf.recordInteraction("page", performance.now() - started),
    );
  });
```

`ui/src/views/Album.svelte` — register the synthetic hook and time it from data-set to first frame:

```ts
  import { perf } from "../lib/perf.svelte";
  import { makeSyntheticAlbum } from "../lib/perfFixture";
  import { perfHooks } from "../lib/perfHooks";

  $effect(() => {
    if (!perf.active) return;
    perfHooks.measureAlbumOpen = (count: number) => {
      const started = performance.now();
      const data = makeSyntheticAlbum(count);
      album = data.album;
      tracks = data.tracks;
      error = "";
      loading = false;
      requestAnimationFrame(() =>
        perf.recordInteraction("album", performance.now() - started),
      );
    };
    return () => {
      delete perfHooks.measureAlbumOpen;
    };
  });
```

`ui/src/lib/perfHooks.ts`:

```ts
  measureAlbumOpen?: (count: number) => void;
```

`ui/src/App.svelte` bridge:

```ts
      gotoAlbum: (id: number) => {
        nav.goto({ name: "album", id });
        return true;
      },
      albumHookReady: () => typeof perfHooks.measureAlbumOpen === "function",
      measureAlbumOpen: (count: number) => {
        if (!perfHooks.measureAlbumOpen) return false;
        perfHooks.measureAlbumOpen(count);
        return true;
      },
```

- [ ] **Step 4: Gate it in the harness**

`scripts/perf/check.mjs` (same `ui_metrics` branch):

```js
        if (typeof interactions.page_max_ms === "number" && interactions.page_count > 0) {
          metrics.page_open_ms = Math.max(metrics.page_open_ms ?? 0, interactions.page_max_ms);
        }
        if (typeof interactions.album_max_ms === "number" && interactions.album_count > 0) {
          metrics.album_open_ms = Math.max(metrics.album_open_ms ?? 0, interactions.album_max_ms);
        }
```

`budgets.json`: add `{ "metric": "page_open_ms", "max": 500, "required": true }` to S5-instrument and S5-atmosphere; add `{ "metric": "album_open_ms", "max": 1000, "required": true }` to S6.

`drive.mjs` S6 branch, after the sort loop and before the final delay:

```js
      await evaluate("window.__perf.gotoAlbum(0)");
      await until("window.__perf.albumHookReady()", "album hook");
      const albumMeasured = await evaluate("window.__perf.measureAlbumOpen(500)");
      if (!albumMeasured) throw new Error("S6: album measurement unavailable");
      await delay(2000);
```

The S5 branches need no driver change: `goto("tracks")` already loads the first page the timer observes.

- [ ] **Step 5: Run all the affected suites**

Run: `npm --prefix ui test; if ($?) { npm --prefix ui run check; if ($?) { node --test scripts/perf/check.test.mjs } }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/perfFixture.ts ui/src/lib/perfFixture.test.ts ui/src/lib/perf.ts ui/src/lib/perf.test.ts ui/src/lib/perf.svelte.ts ui/src/lib/perfHooks.ts ui/src/App.svelte ui/src/views/CollectionTracks.svelte ui/src/views/Album.svelte scripts/perf/check.mjs scripts/perf/check.test.mjs scripts/perf/budgets.json scripts/perf/drive.mjs
git commit -m "feat(perf): gate page-open and the 500-track album"
```

---

### Task 8: S9 clean soak, S10 checklist, and the S9 minutes flag

**Files:**
- Modify: `scripts/perf/run-scenario.ps1`
- Create: `docs/superpowers/plans/notes/<execution-date>-track3-phase-d-smoke.md` (S10 section lives here; the S9 result goes into the Task 9 note)

**Interfaces:**
- Consumes: `drive.mjs --minutes` (already implemented).
- Produces: `run-scenario.ps1 -Minutes <n>` forwarded to S9; a supervised soak result; the S10 checklist.

- [ ] **Step 1: Forward `-Minutes`**

In `run-scenario.ps1`'s param block add `[int]$Minutes = 0`; in the driver invocation:

```powershell
            $driverArgs = @("$root\scripts\perf\drive.mjs", "--scenario", $Scenario, "--port", $Port)
            if ($Minutes -gt 0) { $driverArgs += @("--minutes", "$Minutes") }
            node @driverArgs
```

- [ ] **Step 2: Verify the runner still parses and the driver flag is honest**

Run: `node --check scripts/perf/drive.mjs; powershell -NoProfile -Command "Get-Command -Syntax (Join-Path $PWD 'scripts/perf/run-scenario.ps1')"` — expect the `-Minutes` parameter in the syntax output.

- [ ] **Step 3: Run the supervised 2 h soak**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S9 -Runs 1 -Minutes 120 -SkipCheck
```

Keep the terminal open and the machine foreground for the whole run; do not close or replace it (the baseline's failure mode was an orphaned app at 60 minutes). After it exits, confirm the driver's `finally` restored quality/repeat and the runner killed the tree.

- [ ] **Step 4: Verify the corpus and gate S9**

Confirm ~2 s memory cadence across the full run, no `error` lines, and churn events throughout; then:

```powershell
node scripts/perf/check.mjs --scenario S9
```

Expected: `memory_drift_mb` and `memory_max_mb` rows measured (PASS/FAIL per the final numbers; the below-450 row may still fail until the Task 9 renegotiation).

- [ ] **Step 5: Write the S10 checklist**

Create the smoke file with the Task 9 note or ahead of it, covering (owner-run, real account/audio):
- Device hot-plug: play, yank/re-enable the output device; playback pauses with a notice and resumes after Resume, queue intact.
- Cache-clear-on-exit: enable Settings → cache clear-on-exit, play a track, quit; the cache directory is empty on the next launch and no `.part`/corrupt entries remain.
- Cached seek: play a track to completion once (cache it), replay and seek mid-track; audio jumps to the target and the position never exceeds the duration.
- S1/S2: cold/warm start still feel unchanged; palette color appears after login.

- [ ] **Step 6: Commit the runner change**

```bash
git add scripts/perf/run-scenario.ps1
git commit -m "feat(perf): pass the S9 soak duration through the runner"
```

---

### Task 9: Final S1–S10 run, memory renegotiation, and phase close

**Files:**
- Create: `docs/superpowers/plans/notes/<execution-date>-track3-phase-d.md`
- Modify: `docs/superpowers/plans/notes/<execution-date>-track3-phase-d-smoke.md` (created in Task 8 if not already)
- Modify: `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md`, `AGENTS.md`
- Modify: `docs/superpowers/specs/2026-09-30-oxitide-track3-performance-design.md` only if a budget is renegotiated (old value recorded in the note; the spec status moves to Done)

**Interfaces:**
- Consumes: the runner, `check.mjs`, the per-process attribution rows from Task 1, every phase note.
- Produces: the final Track 3 record; the memory renegotiation proposal with owner sign-off; roadmap and AGENTS.md updates; the official-client sanity placeholder filled by the owner.

- [ ] **Step 1: Quarantine the current corpus and run the full set**

```powershell
New-Item -ItemType Directory -Force .superpowers/perf/runs-quarantine/phase-d-pre | Out-Null
Move-Item -Path .superpowers/perf/runs/*.jsonl -Destination .superpowers/perf/runs-quarantine/phase-d-pre/ -ErrorAction SilentlyContinue
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-miss -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-hit -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S4 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-instrument -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S6 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S7 -Runs 5
```

S8 (10 tracks ×2 passes, ~50 min) and S9 (2 h) are separate supervised runs; S10 is the Task 8 checklist. Re-use the Task 8 S9 run as the final S9 corpus.

- [ ] **Step 2: Read the results and make the memory call (bounded rule)**

Run `node scripts/perf/check.mjs`. For each failing memory row:
- Sum the `memory_process` rows for the app process itself vs the WebView2 children at the peak sample. If the app-owned share is above ~150 MB idle / ~250 MB playback, stop and investigate before renegotiating.
- If the app-owned share is small and the WebView2 floor alone exceeds the ceiling, record a renegotiation proposal in the note: old value, measured p95 (all-process and app-only), proposed value with ~15% headroom over the WebView2-inclusive p95, and the reason. The owner signs it in the final note; without sign-off the rows stay documented FAILs. This is the wave DoD's "explicitly renegotiated in the final note" escape hatch, not a T3-D9 recalibration.

- [ ] **Step 3: Write the final note**

`docs/superpowers/plans/notes/<date>-track3-phase-d.md`:
- Header (Date, Build = `track3-memory-cache` worktree debug + Vite dev, Machine, Conditions, run inventory).
- Final budget table S1–S10 with PASS/FAIL/UNMEASURED and before/after vs the baseline.
- Memory attribution: the app-vs-WebView2 split at idle and during playback; the renegotiation proposal (or the signed values).
- Cache/eviction: S8 second-pass rate, palette cap behavior, scan-free commit evidence (the code change and the S8 run).
- Cached-seek: before/after observation from the owner smoke.
- The S4 `advance_max_gap_ms` finding (WASAPI ring backpressure; re-measured value).
- The official-client sanity paragraph (owner's qualitative note).
- What each Phase D commit changed.

- [ ] **Step 4: Update the roadmap and AGENTS.md**

Roadmap: add a `Revised:` line, move the Track 3 row to `Done — Phases A–D complete <date>`, append the Phase D paragraph, and reference the final note. AGENTS.md: add a `## Performance harness` section documenting `scripts/perf/run-scenario.ps1` (debug exe + Vite dependency, `-Runs`, `-Manual`, `-SkipCheck`, `-Minutes`), `check.mjs` / `npm --prefix ui run perf:check`, the gitignored `.superpowers/perf/runs/` output, the S1–S10 scenario table with automation types, and the human-in-the-loop caveat.

- [ ] **Step 5: Run the full local suite and commit**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; npm --prefix ui test; npm --prefix ui run check; node --test scripts/perf/check.test.mjs`

```bash
git add docs/superpowers/plans/notes docs/superpowers/specs AGENTS.md
git commit -m "test: record the Track 3 final results"
```

---

## Self-Review Output

**Spec coverage:** §6 Phase D offenders: unbounded palette map → Task 2; scan/eviction path → Task 3; S8 cap protection (owner: keep 2 GiB) → Task 4; cached-seek defect (baseline carryover) → Task 5; S9/S10 stability → Task 8; final S1–S10 + note + roadmap + AGENTS.md → Task 9; deferred marks queue build/page open/500-track album → Tasks 6–7; memory ceilings + per-process attribution and the owner-approved renegotiation procedure → Tasks 1 and 9; `S4.advance_max_gap_ms` documented finding → Task 9 (no code, per owner ruling). Budgets unchanged unless the final note records owner-signed renegotiation → Global Constraints.

**Placeholder scan:** no TBD/TODO; `<execution-date>` resolves when the notes are written; the Task 5 engine test notes the fallback assertion pattern if `NullOutput`'s accessor differs.

**Type consistency:** `process_memory_rows`/`memory_process`/`process_tree_memory_mb` agree across Task 1 and `write_memory_sample`; `PALETTE_MAX_ENTRIES`/`fetched_at`/`DiskCacheRef` agree across Task 2 and its tests; `on_disk_bytes`/`pending`/`seeded`/`is_protected` agree across Task 3 and Task 4's engine test; `source_start_frames`/`discard_frames_for` agree across `seek.rs`, `decode_source`, `ChannelSink`, and both callers; `queue_count`/`queue_max_ms`/`queue_build_ms`, `page_*`/`page_open_ms`, `album_*`/`album_open_ms` agree across `perf.ts`, `check.mjs`, `budgets.json`, and the tests; `buildSyntheticQueue`/`measureAlbumOpen`/`albumHookReady` agree across `perfHooks.ts`, `App.svelte`, and `drive.mjs`.

**Review Focus tests:** eviction starvation → Task 4 `a_cached_stream_protects_its_entry_for_its_lifetime`; clear/abort/remove bookkeeping → Task 3 `sizes_stay_correct_across_clear_abort_and_remove`; cached-seek overshoot → Task 5 `discard_is_relative_to_the_actual_source_start` + `cached_seek_starts_at_the_target`; palette recency → Task 2 `pruning_keeps_the_newest_palettes` (+ old-format parse); unmeasured gates → Task 6/7 required-row tests.
