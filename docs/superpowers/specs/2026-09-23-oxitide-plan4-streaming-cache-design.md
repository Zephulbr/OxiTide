# OxiTide — Plan 4 (Streaming, Cache, Quality & Full Settings) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-23
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md`
- **Predecessor:** `2026-09-21-oxitide-plan3-bitperfect-gapless-design.md`
- **Phase:** Phase 1 (MVP) — the fourth implementation **plan** within Phase 1 and covers milestone
  **M7** (aggressive prefetch, disk cache + LRU, quality fallback, AAC, `streaming.allow_fallback`,
  `cache.*`, full Settings) plus the shuffle/repeat UI parked from Plan 2.

---

## 1. Purpose & Outcome

Plan 4 makes streaming **fast, resilient, and fully visible**. It delivers a whole-track disk cache
with LRU eviction, next-track prefetch, a best-available quality policy with the actual served
quality reported end-to-end, AAC (Low/High) playback, shuffle/repeat controls, and the full Settings
surface for streaming, display, and cache — plus a player-bar quality display that can show either a
compact label or the exact format of what is playing.

Success = an album the owner has already played replays with **zero segment fetches** (cache hits),
the cache stays under its configured cap, every track reports its real quality/bit depth/sample
rate/bitrate in the player bar, Low/High AAC tracks play, shuffle/repeat work from the player bar,
and all settings persist in `config.toml`.

## 2. Scope

### In scope (Plan 4)
- **Disk cache subsystem** (`cache.rs`): whole-track blobs keyed by `trackId + quality +
  manifestHash`, atomic writes, LRU eviction with a size cap, clear action, clear-on-exit.
- **Cache-aware playback**: cache hit decodes from a file (no network); cache miss streams as today
  while teeing fetched bytes into the cache; only full-track plans are cached.
- **Next-track prefetch**: while a track plays, the next queue item is resolved and downloaded into
  the cache (bytes only, no decode); best-effort, single in-flight, non-fatal.
- **Quality fallback + truth**: the API's actual `audioQuality` is parsed (derived from the manifest
  when absent); `streaming.allow_fallback` decides whether a lower-than-requested quality plays or
  errors; `PlaybackState` reports the actual quality, codec, sample rate, bit depth, channels, and
  bitrate.
- **AAC decode** for Low/High via `symphonia`'s `aac` feature. No ffmpeg.
- **Shuffle-once and repeat UI**: `Queue::set_shuffle` performs a one-time shuffle of the tail;
  player-bar shuffle/repeat buttons.
- **Full Settings**: streaming (quality, allow fallback), display (compact/detailed quality display),
  cache (usage, cap, clear-on-exit, clear cache, read-only directory).
- **Player-bar quality display**: compact (`MAX`/`LOSSLESS`/`AAC`) or detailed
  (`24-bit / 96 kHz / 2304 kbps`), persisted.

### Explicitly deferred
- Offline playback / downloads and cache-while-logged-out (Phase 3 per the parent roadmap). The
  cache reduces network use; it is not offline mode — `playbackinfo` still requires auth + network.
- Explicit quality retry chain, per-segment cache, ffmpeg sidecar, cache folder picker UI,
  cache encryption, cache statistics/history.
- DSP/EQ, casting/TIDAL Connect, Dolby Atmos, lyrics, Last.fm, Discord RPC, SMTC (later phases).

## 3. Tech Stack Additions

| Concern | Crate | Notes |
|---|---|---|
| Shuffle randomness | `rand = "0.10"` | one-time Fisher–Yates over the queue tail; tests seed `StdRng` |
| AAC decode | `symphonia` `aac` feature | adds `symphonia-codec-aac`; AAC-LC in fMP4/ADTS |

Existing stack (`symphonia`, `rtrb`, `wasapi 0.24`, `windows 0.62`, `tokio`, `reqwest`, `serde`,
`toml`, `directories`) is reused. No new Tauri plugins.

## 4. Repository Layout Changes

```
crates/oxitide-audio/src/
  cache.rs            # NEW: CacheKey, DiskCache, CacheWriter, LRU eviction, clear, size
  lib.rs              # + pub mod cache
  source.rs           # StreamPlan: + manifest_hash, quality (actual)
  fetch.rs            # download() tees parts into an optional CacheWriter
  demux.rs            # NEW FileSource (Read+Seek over std::fs::File) for cache hits
  engine.rs           # cache-first decode, protect key, prefetch thread, allow_fallback,
                      # cache/stream metadata in PlaybackState, shuffle wiring
crates/oxitide-tidal/src/
  stream.rs           # PlaybackInfo.actual_quality from audioQuality (optional)
crates/oxitide-app/src/
  config.rs           # [streaming] allow_fallback, [ui] quality_display,
                      # [cache] dir/max_size_mb/clear_on_exit
  commands.rs         # get_cache_status, clear_cache, set_allow_fallback,
                      # set_quality_display, set_cache_max_size_mb, set_cache_clear_on_exit
  main.rs             # register commands; clear_on_exit on RunEvent::Exit
  state.rs/player.rs  # cache handles + engine construction
ui/src/
  lib/quality.ts          # NEW: pure compact/detailed formatter
  lib/quality.test.ts     # NEW
  lib/api.ts              # new types/commands/PlaybackState fields
  lib/player.svelte.ts    # mirror new state fields
  lib/components/PlayerBar.svelte   # shuffle/repeat buttons + quality display modes
  views/Settings.svelte             # streaming/display/cache sections
crates/oxitide-audio/tests/fixtures/
  tone_aac.mp4        # NEW: ffmpeg-generated AAC-LC fixture
```

## 5. Architecture

### 5.1 Disk cache (`cache`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub track_id: u64,
    pub quality: Quality,
    pub manifest_hash: u64, // FNV-1a 64 over the raw manifest bytes
}

impl CacheKey {
    /// `{track_id}-{quality}-{hash:016x}.fmp4`, e.g. `77646170-lossless-9f3c…fmp4`.
    pub fn file_name(&self) -> String;
}

pub struct DiskCache { /* dir, max_bytes: AtomicU64, state: Mutex<CacheState> */ }

impl DiskCache {
    /// Infallible: an unusable directory (cannot create/write) marks the cache
    /// disabled instead of failing startup.
    pub fn new(dir: PathBuf, max_bytes: u64) -> Arc<Self>;
    pub fn usable(&self) -> bool;
    /// True when the entry exists; touches mtime for LRU.
    pub fn lookup(&self, key: &CacheKey) -> Option<PathBuf>;
    /// Creates a `.part` temp file; None (and a log) when the cache is unusable.
    pub fn begin_write(&self, key: &CacheKey) -> Option<CacheWriter>;
    /// Atomic publish: fsync temp, rename to the final name, touch mtime, evict.
    pub fn commit(&self, writer: CacheWriter);
    /// Deletes a temp file after a cancelled/failed download.
    pub fn abort(&self, writer: CacheWriter);
    pub fn remove(&self, key: &CacheKey);
    pub fn size_bytes(&self) -> u64;
    pub fn clear(&self);
    pub fn set_max_bytes(&self, max_bytes: u64);
    /// Keeps an in-use key from eviction while it plays or is being prefetched.
    pub fn protect(&self, key: &CacheKey);
    pub fn unprotect(&self, key: &CacheKey);
    /// Startup: remove stale `.part` files, then evict to fit.
    pub fn cleanup(&self);
}
```

- `manifest_hash: Option<u64>` is computed in `source::plan_from_info` (FNV-1a over the raw manifest
  bytes) and stored on `StreamPlan`; plans without a manifest (tests, malformed manifests) have
  `None` and bypass the cache entirely.
- Eviction: `.fmp4` entries sorted by mtime (oldest first); delete until `size_bytes <= max_bytes`,
  never deleting a protected key; `.part` files older than one hour are removed at startup.
- All cache operations are best-effort: any error is logged and the caller streams normally. Cache
  errors never fail playback.
- The cache is constructed in `oxitide-app` (`DiskCache::new(config.cache.dir_or_default(),
  max_size_mb * 1024 * 1024)`) and passed to `Engine::new`; the audio crate never reads config.

### 5.2 Playback integration

`Engine::new(resolver, make_fetcher, output, quality, cache: Arc<DiskCache>, allow_fallback: bool)`;
the cache is an injected dependency and `allow_fallback` is stored in `Inner` behind an atomic
(setter for later changes).

- **Hit.** `decode_track` computes the key from `(item.track_id, quality, plan.manifest_hash)`,
  calls `cache.lookup`, protects the key, and decodes from a new `demux::FileSource` (open file,
  `byte_len` from metadata) instead of the network pipeline. No fetcher calls occur. The key is
  unprotected when the stream ends.
- **Miss (full plan).** The existing `StreamBuffer` pipeline runs unchanged, and `fetch::download`
  receives `Option<CacheWriter>`: every successfully fetched part is written to the temp file in
  order while also being pushed to the buffer. `download` commits the writer when the last part is
  fetched and aborts it on cancel/failure.
- **Seek plans are never cached.** `download_plan` may be a `from_segment` slice; only when
  `seek_ms.is_none()` is a writer attached. A seek on a cached track uses the cache file and the
  existing discard-to-sample logic (the file is seekable, so this is cheap).
- **Prefetch.** Once a track's stream is up, the engine calls `cache::prefetch_next(inner)` which:
  peeks the queue's next index, resolves the plan for the configured quality, and if the key is not
  cached, downloads every part straight into a `CacheWriter` (no `StreamBuffer`, no decode, no
  output). One prefetch runs at a time (in-flight flag), it checks `Inner.shutdown`, and all errors
  are logged and swallowed. Prefetch uses the same quality as playback and skips the cache when a
  plan has no `manifest_hash`.
- **Corrupt hit.** If decode fails after a cache hit, the entry is removed and the track is retried
  once over the network. A second failure surfaces normally.

### 5.3 Quality fallback and actual-quality reporting

- `oxitide-tidal`: `RawPlaybackInfo` gains an optional `audioQuality` field; `PlaybackInfo` gains
  `actual_quality: Option<Quality>`. When the field is absent or unknown, quality is derived from the
  manifest: FLAC with 24-bit or >48 kHz → `Max`, other FLAC → `Lossless`, AAC → `High` unless the
  declared bandwidth is low (< 192 kbps → `Low`). If nothing can be derived, `actual_quality` is
  `None` and the track plays (never a false error).
- `StreamPlan` carries `quality: Option<Quality>` (the served quality) and `codec`/`bandwidth` as
  today.
- **Policy in the engine.** `Inner.allow_fallback: AtomicBool` (setter `set_allow_fallback`). In the
  producer, after resolve: if the served quality is known, lower than the requested quality, and
  `!allow_fallback`, the stream fails with a typed error — `"track unavailable at <quality>; enable
  fallback or lower the streaming quality"` — and the queue is kept (existing `StreamError` path).
  Otherwise the track plays and the actual quality is reported. A `None` served quality plays.
- **Reporting.** `StreamEvent::Info`/`StreamStart` carry a `StreamMeta` populated from the plan and
  the decoder's `StreamInfo`:

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct StreamMeta {
    pub quality: Option<Quality>,  // served/actual
    pub codec: Option<String>,     // from the MPD/BTS manifest
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub channels: u16,
    pub bitrate_kbps: Option<u32>, // manifest bandwidth/1000, else bytes*8/duration_ms
}
```

  `PlaybackState` gains `stream: Option<StreamMeta>` (`#[serde(default)]`, additive IPC). Bitrate for
  single-file manifests is computed when the download completes from the byte count the fetch/cache
  path records and `QueueItem.duration_secs`; otherwise `None` and the UI shows a dash.

### 5.4 AAC decode

- Enable the `aac` feature on `symphonia`. The existing `"mp4"` extension hint and isomp4 demuxer
  cover AAC-in-fMP4; ADTS single files are covered by the probe. Nothing else changes: AAC is not a
  bit-perfect candidate, so exclusive mode falls back per policy exactly as float sources do.
- A decode failure stays a typed `StreamError` (queue kept, error visible). No ffmpeg sidecar.

### 5.5 Shuffle and repeat

- `Queue::set_shuffle(true)` shuffles the items **after** the current index once with
  `rand::seq::SliceRandom::shuffle` (a `shuffle_tail_with(rng)` sibling makes tests deterministic
  with `StdRng::seed_from_u64`). The current item and index do not move; disabling leaves the
  shuffled order in place; re-enabling reshuffles the tail. `Queue` gains targeted tests: same
  multiset, current preserved, only the tail moves.
- `EngineCmd::SetShuffle` already exists and now performs the shuffle and emits `State`.
- Repeat is already implemented (`RepeatMode` off/all/one); Plan 4 adds the UI only.

### 5.6 Settings, commands, and config

- New config keys (see §6) with defaults; `Config::load`/`save` unchanged in shape.
- New commands (all registered in `generate_handler!`):
  - `get_cache_status() -> { dir, used_bytes, max_bytes, usable }`
  - `clear_cache()`
  - `set_allow_fallback { enabled }`
  - `set_quality_display { mode: "compact" | "detailed" }`
  - `set_cache_max_size_mb { mb: u64 }`
  - `set_cache_clear_on_exit { enabled }`
- App wiring: `build_player` constructs the `DiskCache` (`DiskCache::new` is infallible; an unusable
  directory simply disables caching) and applies `allow_fallback`; commands persist through `Config`
  like the existing ones; the Tauri `RunEvent::Exit` handler clears the cache directory when
  `cache.clear_on_exit` is set.
- Changing `allow_fallback` applies from the next track (like quality); cache size changes apply at
  once (evict to fit) and persist.

### 5.7 UI

- **`lib/quality.ts` (new, pure):** `formatQuality(meta, mode)`:
  - compact: `Max → "MAX"`, `Lossless → "LOSSLESS"`, `High/Low → "AAC"`, unknown → preferred
    quality's label;
  - detailed: `"24-bit / 96 kHz / 2304 kbps"` (omitting a missing kbps/bit-depth segment with a
    dash, e.g. `"16-bit / 44.1 kHz / —"`).
  Unit-tested in vitest.
- **PlayerBar:** shuffle and repeat buttons (repeat cycles off → all → one) reflecting
  `PlaybackState.shuffle`/`repeat`; quality badge renders via `formatQuality` using
  `PlaybackState.stream` when present and the preferred quality before streaming starts.
- **Settings:** streaming section adds an allow-fallback toggle; display section adds the quality
  display mode select; cache section shows used/cap, a max-size input, clear-on-exit toggle, Clear
  cache button, and the directory read-only. All persist immediately and hydrate from `get_config`.

## 6. Config (`%APPDATA%\OxiTide\config.toml`)

```toml
[audio]
output_device = ""          # "" = system default
exclusive = false           # bit-perfect on/off
exclusive_fallback = "shared" # "shared" | "warn_only"
volume = 1.0

[streaming]
quality = "lossless"        # "low" | "high" | "lossless" | "max"
allow_fallback = true       # play the best available quality and report it

[ui]
quality_display = "compact" # "compact" | "detailed"

[cache]
dir = ""                    # "" = %APPDATA%\OxiTide\cache
max_size_mb = 2048
clear_on_exit = false
```

- Missing keys keep defaults (`#[serde(default)]` on every struct); malformed files still back up to
  `config.toml.bak`. `cache.dir` that cannot be created disables the cache with a logged warning and
  a Settings notice.

## 7. IPC Surface Changes

| Command | Change |
|---|---|
| `get_cache_status` | NEW — `{ dir, used_bytes, max_bytes, usable }` |
| `clear_cache` | NEW |
| `set_allow_fallback` | NEW — `{ enabled }` |
| `set_quality_display` | NEW — `{ mode: "compact" \| "detailed" }` |
| `set_cache_max_size_mb` | NEW — `{ mb }` |
| `set_cache_clear_on_exit` | NEW — `{ enabled }` |
| `get_playback_state` / `state` events | `PlaybackState` gains `stream: Option<StreamMeta>` |

Existing commands and events are unchanged.

## 8. UI Changes

- **Settings:** allow-fallback toggle; quality-display select; cache section (usage, size cap,
  clear-on-exit, Clear cache, read-only dir); a notice when the cache is disabled.
- **PlayerBar:** shuffle/repeat buttons; quality badge in the configured mode.
- **Store:** `PlaybackState` mirror gains `stream`; no reducer shape changes otherwise.
- **api.ts:** new `StreamMeta`, `CacheStatus` types and the new commands; keep the mirror in sync by
  hand.

## 9. Error Handling

| Situation | Behaviour |
|---|---|
| Cache directory cannot be created/written | log; cache disabled; streaming unaffected; Settings notice |
| Corrupt cache entry (decode fails on a hit) | delete entry; retry once from the network |
| Cache over cap | evict LRU except protected/in-flight keys |
| Prefetch network failure | log; abort temp file; playback unaffected |
| Served quality lower than requested, `allow_fallback = false` | typed `StreamError` with a clear message; queue kept |
| `audioQuality` missing/unknown | derive from the manifest; if undecidable, play and report no quality |
| AAC decode failure | typed `StreamError` (existing path) |
| Cache hit while the file vanished mid-read | treated as a miss; stream from the network |
| `clear_on_exit` delete failure | ignored after logging |
| `cache.dir` invalid | fall back to the default directory, else disable the cache |

## 10. Testing Strategy

- **Unit (cache):** key/file-name determinism; FNV manifest hash stability; begin/commit produces the
  final file and no `.part`; abort removes the temp; lookup touches mtime; eviction respects the cap
  and skips protected keys; `clear` empties the dir; stale `.part` cleanup; unwritable dir → `None`
  (cache disabled) without panicking.
- **Unit (tidal/quality):** `audioQuality` parsing (all four values, absent, unknown); manifest
  derivation (24-bit FLAC → Max, 16-bit FLAC → Lossless, AAC bandwidth → High/Low).
- **Unit (queue):** shuffle moves only the tail, keeps the current item, and is deterministic with a
  seeded RNG; repeat behaviour unchanged.
- **Engine (MapFetcher/NullOutput):** cache miss tees bytes and a second play is a hit with **zero**
  fetcher calls; prefetch populates the next track's entry without decoding; `allow_fallback=false`
  errors on a lower served quality while `true` plays and reports `stream.quality`; seek plans are
  not cached; corrupt hit → delete + one network retry.
- **Decode:** `tone_aac.mp4` decodes to the expected sample count/rate; FLAC fixtures unaffected.
- **UI (vitest):** `formatQuality` compact/detailed cases; store mirror fields.
- **Fixtures:** keep the existing tones; add `tone_aac.mp4` (ffmpeg: `-c:a aac -b:a 96k -movflags
  frag_keyframe+empty_moov+default_base_moof`).
- **Manual checklist:** replay an album with zero network (cache hits); cache size grows and is
  capped; Clear cache empties; clear-on-exit; a non-Max track with fallback off errors and with
  fallback on shows the true quality; a Low/High track plays as AAC; the display toggle switches
  compact/detailed; shuffle/repeat behave; settings persist across restart.
- **CI gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui test`, `npm --prefix ui run check`.

## 11. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Task | Exit criteria |
|---|---|---|
| P4-1 | Disk cache module | cache unit tests green; no partial files on abort |
| P4-2 | Cache-aware playback | miss tees + hit makes zero fetcher calls; protect/unprotect |
| P4-3 | Next-track prefetch | prefetch fills the next entry without decoding |
| P4-4 | Actual quality + `allow_fallback` | parsing/derivation tests; policy tests; `PlaybackState.stream` |
| P4-5 | AAC decode | AAC fixture decodes; FLAC unaffected |
| P4-6 | Shuffle + repeat UI | seeded shuffle tests; player-bar controls |
| P4-7 | Config, commands, Settings, display modes | config round-trips; Settings hydrates/persists; formatter tests |
| P4-8 | Verification | full CI + manual checklist recorded |

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Cache corruption or half-written blobs | temp + fsync + atomic rename; abort on cancel; corrupt-hit delete + refetch |
| Prefetch wasting bandwidth or disk | next-track only, single in-flight, cancellable, capped, non-fatal |
| symphonia AAC gaps on real tracks | typed errors; fixture test; manual checklist; ffmpeg reconsidered only if real tracks fail |
| `audioQuality` field/value drift | optional parse + manifest derivation + display fallback to preferred |
| Bitrate unknown for single-file manifests | compute from bytes ÷ duration; otherwise dash in the UI |
| Shuffle flakiness in tests | seeded RNG |
| Disk surprises from the cache | default cap 2 GiB, usage shown, Clear action, clear-on-exit off by default |
| Cache hit hides manifest/auth problems | none — `playbackinfo` still runs; the cache is bytes-only |

## 13. Decisions Log (Plan 4)

| # | Decision | Rationale |
|---|---|---|
| P4-D1 | Track-blob cache inside `oxitide-audio`, not per-segment and not app-layer. | Track-level keying/LRU and prefetch coordination; decode/seek paths untouched. |
| P4-D2 | Only full-track plans are cached. | A seek-truncated plan is a partial track; caching it would poison replays. |
| P4-D3 | Read-through + next-track prefetch; not whole-queue. | Instant replays with bounded speculative traffic. |
| P4-D4 | Best-available quality with the actual value reported; policy enforced in the engine. | Matches how TIDAL serves playbackinfo; no redundant retries; the UI always tells the truth. |
| P4-D5 | `symphonia` AAC only; no ffmpeg. | YAGNI until real tracks fail; keeps the app self-contained. |
| P4-D6 | Shuffle-once over the tail; no original-order restore. | Simple, visible in the queue rail, matches the parked requirement. |
| P4-D7 | `ui.quality_display` with compact default. | Compact is the daily-driver default; detailed is opt-in. |
| P4-D8 | Cache dir read-only in the UI (config-file override), no dialog plugin. | Avoids a new Tauri plugin/capability for a rarely changed path. |
| P4-D9 | Cache lives under `%APPDATA%\OxiTide\cache` by default. | Consistent with the config location; `cache.dir` can override. |

## 14. Out of Scope (tracked for later phases)

- **Phase 2:** playlist/favorites editing, synced lyrics, discovery polish.
- **Phase 3:** offline downloads/playback, Last.fm, Discord RPC, SMTC/media keys.
- **Phase 4:** DSP/EQ (opt-in), casting/TIDAL Connect, Dolby Atmos.
- **Explicitly not in this plan:** per-segment cache, quality retry chains, ffmpeg decode, cache
  folder picker, cache encryption/statistics.
