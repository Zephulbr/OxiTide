# Track 3 Phase B — Startup & Time-to-First-Sound Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every expensive service load, registry query, and cache scan off the pre-paint startup path, and replace the audio engine's 200 ms stream-info poll with an event-driven wait, then re-measure S1–S4.

**Architecture:** Startup keeps only what the first frame and window geometry need (`Config::load`, cheap service construction, window apply/show); disk-cache loads become lazy inside `ArtworkService`/`LyricsService`, scrobbler credentials load lazily inside `ScrobblerService`, and cache cleanup plus scheme registry work run on a named background thread after `window.show()`. The engine's `wait_for_info` blocks on the existing `wake` condvar and is woken by producer sends, command handling, generation bumps, and shutdown.

**Tech Stack:** Rust stable, Tauri 2, `std::sync::{OnceLock, Mutex, Condvar}`, Svelte 5 (no UI changes), Node/PowerShell perf harness (already shipped).

**Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track3-performance-design.md` (§6 Phase B; §4.2 budgets)
**Baseline:** `docs/superpowers/plans/notes/2026-09-30-track3-baseline.md` (S1–S4 rows and the Phase B target notes)

## Global Constraints

- **Behavior-preserving:** normal runs must keep the same observable behavior; only *when* initialization happens changes. No visible UI/animation change (T3-D5). No new runtime dependencies.
- **Budgets are absolute and unchanged** (`scripts/perf/budgets.json` is the standing gate): S1 first paint ≤1500 / UI ready ≤2500, S2 first paint ≤1000, S3-miss TTFS ≤2500 / mean ≤1200, S3-hit TTFS ≤300 + `cache_hits` min 1, S4 `advance_max_gap_ms` ≤100. Memory ceilings stay FAILs (Phase D).
- **Commands:** any new `#[tauri::command]` must be listed in `generate_handler![...]`; invoke args stay camelCase.
- **Tests:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`; `npm --prefix ui run check`; `npm --prefix ui run test`; `node --test scripts/perf/check.test.mjs`. HTTP tests use wiremock only; never log token or credential values.
- **Perf evidence:** raw runs live in `.superpowers/perf/runs/` (gitignored); phase summaries commit under `docs/superpowers/plans/notes/`. Quarantine, never delete, artifacts.
- Windows-first: background-thread code is plain `std::thread::Builder`, matching `oxitide-discord`/`oxitide-smtc`/`oxitide-perf-mem`.

## Review Focus

- **Normal-run behavior unchanged:** artwork/lyrics/scrobble commands and the engine subscription must work exactly as before once each service is first used; `state.*()` still returns `Some` from the very first call.
- **First-use failure:** a corrupt/absent cache file still yields the same empty-map/fallback behavior and still never fails a command (existing fallbacks).
- **Exit-before-background-finished:** quitting within milliseconds of show must not hang, panic, or restore a stale scheme registration; `scheme::restore_remembered` with no snapshot is a no-op.
- **Lost wakeups:** a stream event or generation bump that lands between a `try_recv` and the condvar wait must still be observed (the wait holds `wake_lock` across the check).
- **Producer disconnect:** a dropped decode sender still surfaces as "stream ended before any audio" without waiting for the 2 s safety timeout.
- **Playback unaffected:** all engine tests (seek, gapless, device loss, prefetch) stay green; the perf hook changes must not alter engine behavior.

---

### Task 1: Lazy artwork & lyrics disk-cache load

**Files:**
- Modify: `crates/oxitide-app/src/artwork.rs` (struct + `new` + `palette` + `save_disk`; tests)
- Modify: `crates/oxitide-app/src/lyrics.rs` (struct + `new` + `get` + `store` + `save_disk`; tests)

**Interfaces:**
- Consumes: nothing new.
- Produces: `ArtworkService::new` / `LyricsService::new` no longer read their JSON files; a private `memory()` accessor initializes the map from disk on first use via `OnceLock`. Public method signatures unchanged.

- [ ] **Step 1: Write the failing tests**

In `crates/oxitide-app/src/artwork.rs`'s `mod tests` add:

```rust
    fn palette_dto(primary: &str) -> PaletteDto {
        PaletteDto {
            primary: primary.into(),
            on_primary: "#000000".into(),
            secondary: "#111111".into(),
            tertiary: "#222222".into(),
            backdrop: "#333333".into(),
            ceiling_l: 0.5,
        }
    }

    fn write_palettes(path: &std::path::Path, uuid: &str, dto: &PaletteDto) {
        let mut palettes = std::collections::HashMap::new();
        palettes.insert(uuid.to_string(), dto.clone());
        let text = serde_json::to_string(&DiskCache { palettes }).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// The constructor must not parse palettes.json; the first use loads what
    /// is on disk *at that moment*.
    #[tokio::test]
    async fn disk_palettes_load_lazily_on_first_use() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let dir = temp_dir();
        let path = dir.join("palettes.json");
        write_palettes(&path, "ab-cd", &palette_dto("#aaaaaa"));
        let service = service(&server, &dir);
        write_palettes(&path, "ab-cd", &palette_dto("#bbbbbb"));
        let palette = service.palette("ab-cd").await;
        assert_eq!(palette.primary, "#bbbbbb");
        let _ = std::fs::remove_dir_all(dir);
    }
```

In `crates/oxitide-app/src/lyrics.rs`'s `mod tests` add:

```rust
    fn write_lyrics(path: &std::path::Path, track_id: u64, text: &str) {
        let entry = serde_json::json!({
            "tracks": {
                track_id.to_string(): {
                    "fetched_at": 1,
                    "lyrics": {
                        "track_id": track_id,
                        "source": "tidal",
                        "synced": true,
                        "rtl": false,
                        "lines": [{ "time_ms": 1, "text": text, "words": null }]
                    }
                }
            }
        });
        std::fs::write(path, entry.to_string()).unwrap();
    }

    /// The constructor must not parse lyrics.json; the first use loads what is
    /// on disk at that moment.
    #[tokio::test]
    async fn disk_entries_load_lazily_on_first_use() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let path = dir.join("lyrics.json");
        write_lyrics(&path, 7, "old");
        let service = LyricsService::new(reqwest::Client::new(), lrclib.uri(), path.clone());
        write_lyrics(&path, 7, "new");
        let got = service.get(&tidal_client(&tidal), &request(7)).await.unwrap();
        assert_eq!(got.lines[0].text, "new");
        let _ = std::fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app load_lazily_on_first_use`
Expected: both FAIL — the eager constructors read the first file, so `"#aaaaaa"`/`"old"` come back.

- [ ] **Step 3: Make `ArtworkService` lazy**

Replace the struct and constructor, add the accessor, and route all map access through it:

```rust
use std::sync::{Mutex, OnceLock};

pub struct ArtworkService {
    http: reqwest::Client,
    base_url: String,
    memory: OnceLock<Mutex<HashMap<String, PaletteDto>>>,
    disk_path: PathBuf,
}

impl ArtworkService {
    pub fn new(http: reqwest::Client, base_url: String, disk_path: PathBuf) -> Self {
        if let Some(parent) = disk_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            memory: OnceLock::new(),
            disk_path,
        }
    }

    /// The palette map, read from disk on first use so startup never parses it.
    fn memory(&self) -> &Mutex<HashMap<String, PaletteDto>> {
        self.memory.get_or_init(|| {
            let memory = std::fs::read_to_string(&self.disk_path)
                .ok()
                .and_then(|text| serde_json::from_str::<DiskCache>(&text).ok())
                .map(|cache| cache.palettes)
                .unwrap_or_default();
            Mutex::new(memory)
        })
    }
```

In `palette`, replace `self.memory.lock()` with `self.memory().lock()` (both the lookup and the insert). In `save_disk`, replace:

```rust
        let cache = DiskCache { palettes: self.memory().lock().unwrap().clone() };
```

- [ ] **Step 4: Make `LyricsService` lazy**

Same pattern in `crates/oxitide-app/src/lyrics.rs`:

```rust
use std::sync::{Mutex, OnceLock};

pub struct LyricsService {
    http: reqwest::Client,
    base_url: String,
    memory: OnceLock<Mutex<HashMap<u64, CacheEntry>>>,
    disk_path: PathBuf,
}

impl LyricsService {
    pub fn new(http: reqwest::Client, lrclib_url: impl Into<String>, disk_path: PathBuf) -> Self {
        if let Some(parent) = disk_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self {
            http,
            base_url: lrclib_url.into().trim_end_matches('/').to_string(),
            memory: OnceLock::new(),
            disk_path,
        }
    }

    /// The entry map, read from disk on first use so startup never parses it.
    fn memory(&self) -> &Mutex<HashMap<u64, CacheEntry>> {
        self.memory.get_or_init(|| {
            let memory = std::fs::read_to_string(&self.disk_path)
                .ok()
                .and_then(|text| serde_json::from_str::<DiskCache>(&text).ok())
                .map(|cache| cache.tracks)
                .unwrap_or_default();
            Mutex::new(memory)
        })
    }
```

Route `get`, `store`, and `save_disk` through `self.memory()`. Update the existing `pruning_keeps_the_newest_entries` test's direct field access:

```rust
            let mut memory = service.memory().lock().unwrap();
            ...
        service.save_disk();
        ...
        let memory = reloaded.memory().lock().unwrap();
```

- [ ] **Step 5: Run the crate tests to verify**

Run: `cargo test -p oxitide-app`
Expected: all green, including the two new lazy tests, `the_disk_cache_survives_a_restart`, `disk_cache_survives_a_restart`, `negative_cache_survives_a_restart`, and `pruning_keeps_the_newest_entries`.

- [ ] **Step 6: Clippy and commit**

Run: `cargo clippy -p oxitide-app -- -D warnings`

```bash
git add crates/oxitide-app/src/artwork.rs crates/oxitide-app/src/lyrics.rs
git commit -m "perf(app): load artwork and lyrics caches lazily"
```

---

### Task 2: Lazy scrobbler credential load

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/mod.rs` (struct + `new` + `ensure_credentials_loaded` + call sites; tests)

**Interfaces:**
- Consumes: `CredentialStore::load` (unchanged).
- Produces: `ScrobblerService::new` no longer touches the keyring; `status()`, `handle_intent()`, and the `start()` worker trigger a one-time load. Public API unchanged.

- [ ] **Step 1: Write the failing test**

Add to `crates/oxitide-app/src/scrobble/mod.rs`'s `mod tests`:

```rust
    struct CountingStore {
        inner: MemoryCredentialStore,
        loads: std::sync::atomic::AtomicUsize,
    }

    impl CredentialStore for CountingStore {
        fn save(&self, service: &str, credential: &StoredCredential) -> Result<(), String> {
            self.inner.save(service, credential)
        }
        fn load(&self, service: &str) -> Result<Option<StoredCredential>, String> {
            self.loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.load(service)
        }
        fn clear(&self, service: &str) -> Result<(), String> {
            self.inner.clear(service)
        }
    }

    /// Construction must not read the Windows Credential Manager; the first
    /// status/worker use loads each service exactly once.
    #[test]
    fn credentials_load_lazily_on_first_use() {
        let store = Arc::new(CountingStore {
            inner: MemoryCredentialStore::default(),
            loads: std::sync::atomic::AtomicUsize::new(0),
        });
        store
            .inner
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            store.clone(),
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        assert_eq!(store.loads.load(std::sync::atomic::Ordering::SeqCst), 0);
        let status = service.status();
        assert!(status.lastfm.connected);
        assert_eq!(store.loads.load(std::sync::atomic::Ordering::SeqCst), 2);
        let _ = service.status();
        assert_eq!(store.loads.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app credentials_load_lazily_on_first_use`
Expected: FAIL at `assert_eq!(..., 0)` — `new` already loaded two credentials.

- [ ] **Step 3: Implement the lazy load**

Add a field to `ScrobblerService`:

```rust
    credentials_loaded: AtomicBool,
```

In `new`, delete the two `store.load(...)` lines and pass the untouched store; add the field:

```rust
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            lastfm,
            listenbrainz,
            store,
            lastfm_configured: !lastfm_api_key.is_empty() && !lastfm_api_secret.is_empty(),
            credentials_loaded: AtomicBool::new(false),
            tx,
            rx: Mutex::new(Some(rx)),
            inner: Mutex::new(Inner {
                lastfm_enabled,
                listenbrainz_enabled,
                ..Inner::default()
            }),
            on_status: Mutex::new(None),
            on_auth_error: Mutex::new(None),
        })
```

Add the method and call it at the top of `status()` and `handle_intent()`, and once inside the spawned worker before its loop:

```rust
    /// One-time keyring read, deferred to the first worker/status use so
    /// startup never waits on the Windows Credential Manager.
    fn ensure_credentials_loaded(&self) {
        if self.credentials_loaded.swap(true, Ordering::SeqCst) {
            return;
        }
        let lastfm = self.store.load(LASTFM_SERVICE).ok().flatten();
        let listenbrainz = self.store.load(LISTENBRAINZ_SERVICE).ok().flatten();
        let mut inner = self.inner.lock().unwrap();
        if inner.lastfm.is_none() {
            inner.lastfm = lastfm;
        }
        if inner.listenbrainz.is_none() {
            inner.listenbrainz = listenbrainz;
        }
    }
```

In `start()`'s spawned task, before the `loop`:

```rust
        tauri::async_runtime::spawn(async move {
            this.ensure_credentials_loaded();
            loop {
```

(`Ordering` and `AtomicBool` imports: use the same import path already used in the file; if absent, add `use std::sync::atomic::{AtomicBool, Ordering};`.)

- [ ] **Step 4: Run the scrobble tests**

Run: `cargo test -p oxitide-app scrobble`
Expected: all green, including the new test and the existing `retryable_failure_keeps_the_scrobble_queued` (which saves to the store before constructing and relies on `handle_intent` loading).

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p oxitide-app -- -D warnings`

```bash
git add crates/oxitide-app/src/scrobble/mod.rs
git commit -m "perf(app): load scrobbler credentials lazily"
```

---

### Task 3: Startup orchestration — window gate and background init

**Files:**
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `crates/oxitide-app/src/player.rs` (`build_cache`; test)
- Modify: `crates/oxitide-app/src/commands.rs` (`auth_restore`, `finish_login`)

**Interfaces:**
- Consumes: Tasks 1–2 (cheap constructors); `DiskCache::cleanup` (unchanged); `scheme::snapshot/remember` (unchanged).
- Produces: `perf` marks `logging_ready`, `cache_cleanup_done`, `startup_background_done`; `build_cache` no longer cleans up.

- [ ] **Step 1: Update the `build_cache` test to the new contract (red)**

Replace `crates/oxitide-app/src/player.rs`'s `build_cache_cleans_stale_parts_and_applies_the_cap` body after the temp-dir setup:

```rust
        let cache = build_cache(&config);
        assert_eq!(cache.dir(), dir.as_path());
        assert_eq!(cache.max_bytes(), 1024 * 1024);
        assert!(part.exists(), "build_cache must not scan the directory");
        cache.cleanup();
        assert!(!part.exists(), "cleanup must remove the stale .part");
        let _ = std::fs::remove_dir_all(dir);
```

Rename the test to `build_cache_selects_the_directory_and_cleanup_sweeps_stale_parts`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p oxitide-app build_cache_selects_the_directory_and_cleanup_sweeps_stale_parts`
Expected: FAIL at `assert!(part.exists(), ...)` — `build_cache` still calls `cleanup()`.

- [ ] **Step 3: Remove cleanup from `build_cache`**

In `crates/oxitide-app/src/player.rs`:

```rust
/// Build the disk cache from config: the configured directory when usable,
/// else the default, else a temp directory. `DiskCache::new` is infallible.
/// The stale-`.part` sweep and cap enforcement (`cleanup`) run on the startup
/// background thread after the window is shown; see `main.rs`.
pub fn build_cache(config: &Config) -> Arc<DiskCache> {
    let max_bytes = config.cache.max_size_mb.saturating_mul(1024 * 1024);
    let configured = config.cache.dir.trim();
    if !configured.is_empty() {
        let cache = DiskCache::new(std::path::PathBuf::from(configured), max_bytes);
        if cache.usable() {
            return cache;
        }
        tracing::warn!("configured cache dir is unusable; falling back to the default");
    }
    let dir = default_cache_dir().unwrap_or_else(|| std::env::temp_dir().join("oxitide-cache"));
    DiskCache::new(dir, max_bytes)
}
```

- [ ] **Step 4: Wire `main.rs`**

Add a mark after logging:

```rust
    init_logging();
    perf::mark("logging_ready");
```

In the `setup` closure, delete the two opening statements (`scheme::remember(scheme::snapshot());` and the `register_all` block). After `perf::mark("window_shown");` and before `Ok(())`, add:

```rust
            // Background startup: cache cleanup and scheme registration are
            // not needed for the first frame, so they run after the window is
            // shown. A quit before this finishes simply skips the scheme
            // restore (no snapshot remembered).
            let bg = app.handle().clone();
            let cache = app.state::<AppState>().cache();
            std::thread::Builder::new()
                .name("oxitide-startup".into())
                .spawn(move || {
                    if let Some(cache) = cache {
                        cache.cleanup();
                        perf::mark("cache_cleanup_done");
                    }
                    scheme::remember(scheme::snapshot());
                    #[cfg(any(windows, target_os = "linux"))]
                    if let Err(e) = bg.deep_link().register_all() {
                        tracing::warn!("deep link registration failed: {e}");
                    }
                    perf::mark("startup_background_done");
                })
                .ok();
```

The `on_open_url` handler, `get_current`, `set_handle`, scrobbler sinks/`start()`, geometry apply, mini handling, and `window.show()` all stay exactly where they are (they are cheap and needed for a correct first frame).

- [ ] **Step 5: Drop the duplicate config loads**

In `crates/oxitide-app/src/commands.rs`, `auth_restore` (around line 296) and `finish_login` (around line 169), replace:

```rust
            let config = Config::load();
            state.set_config(config.clone());
```

with:

```rust
            let config = state.config();
```

(The live config already reflects every in-app settings change; the reload only re-read the same file.)

- [ ] **Step 6: Verify compile, tests, clippy**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green. If `Config` becomes an unused import in `commands.rs`, remove it only if clippy says so.

- [ ] **Step 7: Commit**

```bash
git add crates/oxitide-app/src/main.rs crates/oxitide-app/src/player.rs crates/oxitide-app/src/commands.rs
git commit -m "perf(app): gate the window on geometry only and background the rest"
```

---

### Task 4: Event-driven stream-info wait in the engine

**Files:**
- Modify: `crates/oxitide-audio/src/engine.rs` (`wait_for_info`, new `wait_for_stream_event`, producer notify calls; tests)

**Interfaces:**
- Consumes: `Inner.wake_lock`/`wake`/`generation`/`shutdown`, `Inner::notify_worker`, `TrackStream.rx`.
- Produces: `fn wait_for_stream_event<T>(wake_lock: &Mutex<()>, wake: &Condvar, rx: &Receiver<T>) -> Result<Option<T>, ()>` (private, tested); producers notify after every send.

- [ ] **Step 1: Write the failing tests**

Add to `crates/oxitide-audio/src/engine.rs`'s `mod tests`:

```rust
    #[test]
    fn stream_wait_returns_queued_events_without_a_notify() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        tx.send(7).unwrap();
        let lock = Mutex::new(());
        let wake = Condvar::new();
        match wait_for_stream_event(&lock, &wake, &rx) {
            Ok(Some(7)) => {}
            Ok(other) => panic!("expected the queued event, got {other:?}"),
            Err(()) => panic!("unexpected disconnect"),
        }
    }

    #[test]
    fn stream_wait_wakes_on_notify_without_an_event() {
        let (_tx, rx) = std::sync::mpsc::channel::<u32>();
        let lock = std::sync::Arc::new(Mutex::new(()));
        let wake = std::sync::Arc::new(Condvar::new());
        let (locker, waker) = (lock.clone(), wake.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let _guard = locker.lock().unwrap();
            waker.notify_all();
        });
        let started = std::time::Instant::now();
        let result = wait_for_stream_event(&lock, &wake, &rx);
        assert!(matches!(result, Ok(None)), "expected a bare wake, got {result:?}");
        assert!(started.elapsed() < Duration::from_millis(1000));
    }

    #[test]
    fn stream_wait_reports_a_disconnected_producer() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        drop(tx);
        let lock = Mutex::new(());
        let wake = Condvar::new();
        assert!(matches!(wait_for_stream_event(&lock, &wake, &rx), Err(())));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio stream_wait`
Expected: FAIL — `wait_for_stream_event` not defined.

- [ ] **Step 3: Add the helper and rewrite `wait_for_info`**

Add near `wait_for_info`:

```rust
/// Wait until the producer queues a stream event, or the engine is woken by a
/// command / generation bump / shutdown. A bare wake returns `Ok(None)`; a
/// disconnected producer returns `Err(())`. Holding `wake_lock` across the
/// `try_recv` closes the lost-wakeup race: a send that lands after the check
/// blocks `notify_worker` until the condvar wait has started.
fn wait_for_stream_event<T>(
    wake_lock: &Mutex<()>,
    wake: &Condvar,
    rx: &std::sync::mpsc::Receiver<T>,
) -> Result<Option<T>, ()> {
    let guard = wake_lock.lock().unwrap();
    match rx.try_recv() {
        Ok(event) => Ok(Some(event)),
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            let _ = wake.wait_timeout(guard, Duration::from_secs(2));
            Ok(None)
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(()),
    }
}
```

Replace `wait_for_info`'s body:

```rust
fn wait_for_info(inner: &Arc<Inner>, stream: &mut TrackStream, gen: u64) -> Result<StreamStart> {
    loop {
        // A supersede while a track is still resolving/fetching (e.g. Next
        // pressed during a slow network resolve) must exit promptly; command
        // handling notifies the worker, which wakes the wait below.
        if inner.generation.load(Ordering::SeqCst) != gen {
            return Err(Error::Output("playback superseded".into()));
        }
        if inner.shutdown.load(Ordering::SeqCst) {
            return Err(Error::Output("engine shut down".into()));
        }
        match wait_for_stream_event(&inner.wake_lock, &inner.wake, &stream.rx) {
            Err(()) => return Err(Error::Decode("stream ended before any audio".into())),
            Ok(None) => continue,
            Ok(Some(Ok(StreamEvent::Info(start)))) => return Ok(start),
            Ok(Some(Ok(StreamEvent::Samples(_)))) => continue,
            Ok(Some(Err(e))) => return Err(e),
        }
    }
}
```

Remove the now-unused `RecvTimeoutError` arm imports only if clippy requires it (`RecvTimeoutError` is still used by the play loop).

- [ ] **Step 4: Notify on every producer send**

In `impl ChannelSink`, change `send` to wake the worker:

```rust
    fn send(&self, event: Result<StreamEvent>) -> Result<()> {
        let sent = self
            .tx
            .send(event)
            .map_err(|_| Error::Output("playback stream closed".into()));
        self.inner.notify_worker();
        sent
    }
```

In `decode_track`, after each early `let _ = tx.send(Err(e));` (resolve failure, fallback refusal, HE-AAC rejection) add `inner.notify_worker();`. In `decode_from_network`, after `let _ = tx.send(Err(e));` add `inner.notify_worker();`. In `start_track`, in the `Err(e)` branch where the decode thread failed to spawn, add `inner.notify_worker();` after the log.

- [ ] **Step 5: Run the engine suite**

Run: `cargo test -p oxitide-audio; if ($?) { cargo clippy -p oxitide-audio -- -D warnings }`
Expected: all green, including `next_while_playing_does_not_duplicate_or_skip`, seek, device-loss, and prefetch tests.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-audio/src/engine.rs
git commit -m "perf(audio): wake the worker on stream events instead of polling"
```

---

### Task 5: TTFS attribution — resolve / fetch / output-open

**Files:**
- Modify: `crates/oxitide-audio/src/perf.rs` (three new variants)
- Modify: `crates/oxitide-audio/src/engine.rs` (emit sites; extend the hook test)
- Modify: `crates/oxitide-app/src/perf.rs` (event kinds)
- Modify: `scripts/perf/check.mjs` + `scripts/perf/check.test.mjs` (derived metrics)

**Interfaces:**
- Produces: `AudioPerfEvent::{ResolveStart, InfoReady, OutputOpened}`; JSONL audio events `resolve_start` / `info_ready` / `output_opened`; check metrics `resolve_ms`, `fetch_ms`, `output_open_ms` (informational — **no budget rows**).

- [ ] **Step 1: Extend the engine hook test (red)**

In the existing `perf_hook_receives_decode_cache_and_first_pcm_events` test, after the current assertions add:

```rust
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::perf::AudioPerfEvent::ResolveStart { track_id: 1 })));
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::perf::AudioPerfEvent::InfoReady { track_id: 1 })));
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::perf::AudioPerfEvent::OutputOpened { track_id: 1 })));
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p oxitide-audio perf_hook_receives_decode_cache_and_first_pcm_events`
Expected: FAIL — the variants do not exist.

- [ ] **Step 3: Add the variants and emit them**

In `crates/oxitide-audio/src/perf.rs`:

```rust
    /// The decode thread is about to ask the resolver for the stream plan.
    ResolveStart { track_id: u64 },
    /// Stream info is available (the fetch produced enough to decode).
    InfoReady { track_id: u64 },
    /// The output device accepted and started the stream.
    OutputOpened { track_id: u64 },
```

In `engine.rs`:
- At the top of `decode_track`, before `inner.resolver.resolve(...)`:

```rust
    fire_perf(inner, AudioPerfEvent::ResolveStart { track_id: item.track_id });
```

- In `run_session`, immediately after `let StreamStart { info, start_frames, meta } = start;`:

```rust
        fire_perf(inner, AudioPerfEvent::InfoReady { track_id: stream.item.track_id });
```

- Inside the `if open_spec.as_ref() != Some(&want)` block, after `output.start()` succeeds and before/after `set_volume`, add once:

```rust
            fire_perf(inner, AudioPerfEvent::OutputOpened { track_id: stream.item.track_id });
```

- [ ] **Step 4: Forward the new kinds in the app session**

In `crates/oxitide-app/src/perf.rs`'s `audio_event` match, add arms to the `(kind, track_id, gap_ms)` tuple:

```rust
            AudioPerfEvent::ResolveStart { track_id } => ("resolve_start", *track_id, None),
            AudioPerfEvent::InfoReady { track_id } => ("info_ready", *track_id, None),
            AudioPerfEvent::OutputOpened { track_id } => ("output_opened", *track_id, None),
```

- [ ] **Step 5: Derive the metrics in `check.mjs` (test-first)**

Add to `scripts/perf/check.test.mjs`:

```js
test("ttfs attribution derives resolve, fetch, and open stages", () => {
  const file = runFile([
    { kind: "session", run_id: "S3-miss-1", scenario: "S3-miss" },
    { kind: "audio", event: "resolve_start", track_id: 1, t_ms: 100 },
    { kind: "audio", event: "decode_start", track_id: 1, t_ms: 300 },
    { kind: "audio", event: "info_ready", track_id: 1, t_ms: 900 },
    { kind: "audio", event: "output_opened", track_id: 1, t_ms: 1000 },
    { kind: "audio", event: "first_pcm", track_id: 1, t_ms: 1040 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.resolve_ms, 200);
  assert.equal(metrics.fetch_ms, 600);
  assert.equal(metrics.output_open_ms, 100);
});
```

Run: `node --test scripts/perf/check.test.mjs` → FAIL (`resolve_ms` undefined).

Implement in `metricsFor`, inside the `line.kind === "audio"` branch:

```js
      const first = (key) => {
        const metric = `${key}_ms`;
        if (metrics[metric] === undefined) metrics[metric] = line.t_ms;
      };
      if (line.event === "resolve_start") first("resolve_start");
      if (line.event === "decode_start") first("decode_start");
      if (line.event === "info_ready") first("info_ready");
      if (line.event === "output_opened") first("output_opened");
```

and after the loop (before the cache-rate computation):

```js
  if (metrics.resolve_start_ms !== undefined && metrics.decode_start_ms !== undefined) {
    metrics.resolve_ms = metrics.decode_start_ms - metrics.resolve_start_ms;
  }
  if (metrics.decode_start_ms !== undefined && metrics.info_ready_ms !== undefined) {
    metrics.fetch_ms = metrics.info_ready_ms - metrics.decode_start_ms;
  }
  if (metrics.info_ready_ms !== undefined && metrics.output_opened_ms !== undefined) {
    metrics.output_open_ms = metrics.output_opened_ms - metrics.info_ready_ms;
  }
```

Do **not** add budget rows for these metrics.

- [ ] **Step 6: Verify**

Run: `cargo test -p oxitide-audio perf_hook; if ($?) { cargo test -p oxitide-app perf:: }; if ($?) { node --test scripts/perf/check.test.mjs }`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/oxitide-audio/src/perf.rs crates/oxitide-audio/src/engine.rs crates/oxitide-app/src/perf.rs scripts/perf/check.mjs scripts/perf/check.test.mjs
git commit -m "feat(perf): attribute time-to-first-sound by stage"
```

---

### Task 6: Re-measure S1–S4, decide on a TTFS fix, and close Phase B

**Files:**
- Create: `docs/superpowers/plans/notes/<execution-date>-track3-phase-b.md`
- Create: `docs/superpowers/plans/notes/<execution-date>-track3-phase-b-smoke.md`
- Modify: `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` (status line)

**Interfaces:**
- Consumes: the runner (`scripts/perf/run-scenario.ps1`), `check.mjs`, and the baseline note.
- Produces: the Phase B before/after record; a bounded decision on TTFS.

- [ ] **Step 1: Re-run the Phase B scenarios (5 runs each)**

From the worktree:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-miss -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-hit -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S4 -Runs 5
```

Quarantine the old `S1-*`/`S2-*`/`S3-*`/`S4-*` artifacts (move to `.superpowers/perf/runs-quarantine/`) so each scenario's measured corpus is uniform.

- [ ] **Step 2: Read the attribution and decide (bounded)**

Extract per run: `resolve_ms`, `fetch_ms`, `output_open_ms`, `ttfs_ms` (from the JSONL via `check.mjs` or a one-off Node script — do not add tooling). Record the p95 of each stage.

Decision rule:
- If one stage dominates (> 60% of `ttfs_ms`) **and** a specific, safe, behavior-preserving change follows directly from the data (e.g. output-open dominates and pre-opening is provably safe), implement **exactly one** fix, re-run S3-miss/S3-hit ×5, and record before/after.
- Otherwise, document the breakdown and stop; TTFS stays within budget or becomes a Phase D note.

- [ ] **Step 3: Write the Phase B note**

`docs/superpowers/plans/notes/<date>-track3-phase-b.md`:
- Header (Date, Build = `track3-harness` debug + Vite dev, Machine, Conditions).
- Before/after table per scenario (baseline note row → new p95), with PASS/FAIL.
- The startup breakdown: `logging_ready`, `config_loaded`, `cache_ready`, `services_ready`, `window_shown` deltas — confirming no pre-paint step remains > 100 ms (or listing the offender).
- The TTFS attribution table and the Task 6 Step 2 decision (fix applied or documented stop).
- Memory rows still FAIL (Phase D), unchanged budgets.

- [ ] **Step 4: Create the smoke checklist and hand it to the owner**

`docs/superpowers/plans/notes/<date>-track3-phase-b-smoke.md` with the audio-path items affected by Task 4 (owner-run, real account/audio):
- Play/pause, next/prev during a slow network load (supersede responsiveness), seek, gapless album advance, device hot-plug pause/recover, quality switch, logout while playing.
- S1/S2: cold and warm start feel; no stale login on deep-link cold start; palette/lyrics/cover color still appear after login.
- Scrobbler connects and now-playing still works; Settings → Connections status is correct immediately after launch.
Mark as pending until the owner closes it.

- [ ] **Step 5: Update the roadmap and commit**

Run the full local suite once before committing: `cargo test --workspace` + `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check` + `npm --prefix ui run test`, `node --test scripts/perf/check.test.mjs`.

Track 3 status: `Active — Phase A (harness + baseline) and Phase B (startup + TTFS) complete <date>; Phase C (UI + large library) next`.

```bash
git add docs/superpowers/plans/notes docs/superpowers/specs
git commit -m "test: record the Track 3 Phase B results"
```

---

## Self-Review Output

**Spec coverage:** §6 Phase B offenders: pre-paint cache scan → Task 3 Step 3/4; synchronous palettes/lyrics loads → Task 1; keyring reads → Task 2; `reg query` + `register_all` → Task 3 Step 4; duplicate `Config::load` → Task 3 Step 5. Target shape (gate show on geometry; background cache/artwork/lyrics/keyring/scheme; lazy or parallel on first use) → Tasks 1–3. TTFS event-driven wait → Task 4; measured review for warm-up/pipelining → Task 5 + Task 6 Step 2. Re-run S1–S4 + Plan 2/3 smoke → Task 6. Budgets unchanged → Global Constraints.

**Placeholder scan:** no TBD/TODO; `<execution-date>` is an intentional file-name pattern resolved when the notes are written.

**Type consistency:** `ResolveStart/InfoReady/OutputOpened` names match across the enum, emit sites, app match arms, and JSONL `resolve_start/info_ready/output_opened`; derived metrics `resolve_ms/fetch_ms/output_open_ms` match the test and implementation; `memory()` accessor name is the same in both services; `credentials_loaded`/`ensure_credentials_loaded` names are consistent; `wait_for_stream_event` signature matches all three tests.

**Review Focus tests:** normal-run behavior → Tasks 1–2 existing service tests plus new lazy tests; first-use failure → existing fallback tests stay green; exit-before-background → no snapshot is a no-op (existing `decides_what_to_restore_on_exit`); lost wakeups → `stream_wait_returns_queued_events_without_a_notify`; disconnect → `stream_wait_reports_a_disconnected_producer`; playback unaffected → full engine suite in Task 4 Step 5.
