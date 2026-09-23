# OxiTide Plan 4 — Streaming, Cache, Quality & Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make streaming fast, resilient, and fully visible: a whole-track disk cache with LRU eviction and next-track prefetch, best-available quality with the served quality reported end-to-end, AAC playback, shuffle/repeat controls, and the full Settings surface (streaming, display, cache).

**Architecture:** `oxitide-audio` gains a `cache` module (track blobs keyed by track id + requested quality + a token-stable manifest hash; temp file + fsync + rename; LRU by mtime with protected keys), a cache-aware `fetch::download` that tees fetched parts into a `CacheWriter`, a seekable `demux::FileSource` for cache hits, and a `StreamMeta` that travels on `StreamEvent::Info` into `PlaybackState.stream`. `Engine::new` receives the injected `Arc<DiskCache>` and an `allow_fallback` flag; `oxitide-app` builds the cache from `config.toml` and exposes six new commands; the Svelte UI gains the quality formatter, shuffle/repeat buttons, and the streaming/display/cache settings.

**Tech Stack:** Rust 2021 (stable), Tokio, reqwest (+ `blocking`), symphonia 0.6.1 (adds the `aac` feature), `rand = "0.10"` (queue shuffle), Tauri 2, Svelte 5 runes + Vite + TypeScript, Vitest, wiremock.

**Spec:** `docs/superpowers/specs/2026-09-23-oxitide-plan4-streaming-cache-design.md`

## Global Constraints

- Rust edition 2021; toolchain pinned by `rust-toolchain.toml` (clippy + rustfmt). Node 20+. Tauri 2.x. Svelte 5 runes only (`$state`/`$props`/`$effect`; no stores, no `export let`).
- Windows-first. Config/app dir: `%APPDATA%\OxiTide`; file `config.toml`; default cache dir `%APPDATA%\OxiTide\cache`.
- `oxitide-audio` must not depend on Tauri or UI code, and must never read `config.toml` itself (the cache and settings are injected through `Engine::new`/setters).
- Every new `#[tauri::command]` must also be added to `generate_handler![...]` in `crates/oxitide-app/src/main.rs`.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case (`position_ms` ↔ `positionMs`). Rust DTOs are the source of truth; update `ui/src/lib/api.ts` by hand.
- No DRM, no key extraction, no credential/token logging. Tokens stay in the OS keyring; unit tests use `MemoryStore`.
- No live network in tests: HTTP tests use `wiremock`; audio tests use committed tone fixtures, `MapFetcher`, and `NullOutput`. Cache tests use temp directories.
- Verified dependency floors: `symphonia = "0.6.1"` with features `["flac", "isomp4", "pcm", "aac"]`; `rand = "0.10"` (API: `rand::rng()`, `rand::seq::SliceRandom::shuffle`, `rand::rngs::StdRng`, `rand::SeedableRng`); `wasapi = "0.24"`; `windows = "0.62"`; `toml = "1"`; `directories = "6"`.
- CI gates (all must pass at the end of every task that touches Rust/UI): `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.
- Conventional commits (`feat:`, `fix:`, `test:`, `chore:`, `docs:`). Commit at the end of every task. Never commit generated installers or secrets.

## Review Focus

Uncovered input classes / failure modes most likely to bite a real user; each is pinned to a test in its owning task:

1. **Signed CDN tokens rotate between `playbackinfo` calls** — the cache must still hit on replay, or the feature is useless. Owning task: 2 (`manifest_hash_ignores_signed_tokens`).
2. **Corrupt or truncated cache blob** — the entry must be deleted and the track retried once over the network; the queue must not stop. Owning task: 4 (`corrupt_cache_entry_is_removed_and_refetched`).
3. **Unwritable cache directory** (a file at that path, read-only volume) — caching disables, playback is unaffected, nothing panics. Owning task: 3 (`unusable_dir_disables_the_cache`).
4. **Served quality lower than requested with fallback off** — a typed error with a clear message, and the queue is kept. Owning task: 6 (`fallback_disabled_errors_on_a_lower_served_quality`).
5. **Missing `audioQuality` and missing manifest bandwidth** — derive what can be derived, play anyway, show a dash rather than a false error. Owning tasks: 2 (`quality_is_derived_when_audio_quality_is_absent`) and 6 (`single_file_bitrate_is_computed_from_bytes_and_duration`).

## Plan-level decisions (deviations from the spec text, with rationale)

| # | Decision | Rationale |
|---|---|---|
| D1 | **The manifest hash is normalized**: FNV-1a 64 over init/media URLs with `?query` stripped plus codec/sample rate/bit depth/timescale. | User-approved during planning. TIDAL manifests embed signed, short-lived CDN tokens; hashing raw bytes would change the key on every `playbackinfo` call and the cache would never hit. Stripping the query keeps the key stable while still detecting a different master. |
| D2 | `StreamPlan` gains `bitrate_kbps: Option<u32>` (DASH `bandwidth / 1000`). | The spec says "codec/bandwidth as today", but `StreamPlan` has no bandwidth field today; `StreamMeta` needs the value. |
| D3 | `PlaybackState.stream: Option<StreamMeta>` has no `#[serde(default)]`. | `PlaybackState` is `Serialize`-only (no `Deserialize`); the UI mirror ships in the same task, so an additive field with a default attribute buys nothing and risks a serde attribute error. |
| D4 | The worker emits `EngineEvent::State` once per track start (after `wait_for_info`). | Otherwise the quality badge would never update mid-session: `State` was only emitted at queue boundaries. |
| D5 | `App.svelte` owns `qualityDisplay` (loaded from config, passed to `PlayerBar`; `Settings` reports changes up). | Keeps the badge reactive when Settings changes the mode, without inventing a new store. |
| D6 | `DiskCache::begin_write(self: &Arc<Self>, ...)`; `CacheWriter::commit()/abort()` delegate to `DiskCache::commit/abort`. | The writer must hold the cache to publish itself. The spec's `DiskCache::commit(writer)` API is preserved as the underlying implementation. |
| D7 | Task 4 temporarily wires `build_player` with `default_cache_dir()` and `allow_fallback = true`. | Keeps the workspace compiling between tasks; Task 9 replaces both with the real `Config` values. |
| D8 | P4-3 is implemented by making the **existing next-track lookahead prefetch** cache-aware (Task 4) plus a **single-flight writer rule** in `DiskCache` (Task 5), instead of adding a second bytes-only prefetch. | The engine already downloads/decodes the next item while the current plays; a second writer for the same key would race it and double-fetch. Serializing the two makes a byte prefetch a no-op. Cost if wrong: no cache fill while paused, and a track skipped before its download completes is not cached. |

---

### Task 1: Quality truth in the TIDAL client

**Milestone:** P4-4 (part 1).

**Files:**
- Modify: `crates/oxitide-tidal/src/stream.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces (used by Tasks 2, 3, 4, 6):
  - `Quality` now derives `PartialOrd, Ord, Hash` (variant order `Low < High < Lossless < Max`).
  - `Quality::label(self) -> &'static str` → `"low" | "high" | "lossless" | "max"`.
  - `Quality::from_tidal_param(value: &str) -> Option<Quality>` (case-insensitive; `LOW`, `HIGH`, `LOSSLESS`, `HI_RES_LOSSLESS`, `HI_RES`, `MASTER`).
  - `PlaybackInfo { quality: Quality, actual_quality: Option<Quality>, manifest_mime: String, manifest: Vec<u8> }`.

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` in `crates/oxitide-tidal/src/stream.rs`:

```rust
    #[test]
    fn quality_label_is_lowercase() {
        assert_eq!(Quality::Low.label(), "low");
        assert_eq!(Quality::High.label(), "high");
        assert_eq!(Quality::Lossless.label(), "lossless");
        assert_eq!(Quality::Max.label(), "max");
    }

    #[test]
    fn quality_orders_from_low_to_max() {
        assert!(Quality::Low < Quality::High);
        assert!(Quality::High < Quality::Lossless);
        assert!(Quality::Lossless < Quality::Max);
    }

    #[test]
    fn from_tidal_param_parses_known_values() {
        assert_eq!(Quality::from_tidal_param("LOW"), Some(Quality::Low));
        assert_eq!(Quality::from_tidal_param("HIGH"), Some(Quality::High));
        assert_eq!(Quality::from_tidal_param("LOSSLESS"), Some(Quality::Lossless));
        assert_eq!(Quality::from_tidal_param("HI_RES_LOSSLESS"), Some(Quality::Max));
        assert_eq!(Quality::from_tidal_param("hi_res"), Some(Quality::Max));
        assert_eq!(Quality::from_tidal_param("MASTER"), Some(Quality::Max));
        assert_eq!(Quality::from_tidal_param("something_else"), None);
    }

    #[tokio::test]
    async fn playback_info_parses_audio_quality() {
        let server = MockServer::start().await;
        let mpd = "<MPD></MPD>";
        let encoded = base64::engine::general_purpose::STANDARD.encode(mpd);
        Mock::given(method("GET"))
            .and(path("/tracks/7/playbackinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifestMimeType": "application/dash+xml",
                "manifest": encoded,
                "audioQuality": "LOSSLESS"
            })))
            .mount(&server)
            .await;
        let info = client(&server).playback_info(7, Quality::Max).await.unwrap();
        assert_eq!(info.actual_quality, Some(Quality::Lossless));
    }

    #[tokio::test]
    async fn playback_info_without_audio_quality_is_none() {
        let server = MockServer::start().await;
        let encoded = base64::engine::general_purpose::STANDARD.encode("<MPD></MPD>");
        Mock::given(method("GET"))
            .and(path("/tracks/8/playbackinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifestMimeType": "application/dash+xml",
                "manifest": encoded
            })))
            .mount(&server)
            .await;
        let info = client(&server).playback_info(8, Quality::Lossless).await.unwrap();
        assert_eq!(info.actual_quality, None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal stream`
Expected: compile errors — `no method named label`, `from_tidal_param`, `actual_quality`.

- [ ] **Step 3: Implement**

Change the `Quality` derive and add the helpers:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Low,
    High,
    Lossless,
    Max,
}

impl Quality {
    pub fn tidal_param(self) -> &'static str {
        match self {
            Quality::Low => "LOW",
            Quality::High => "HIGH",
            Quality::Lossless => "LOSSLESS",
            Quality::Max => "HI_RES_LOSSLESS",
        }
    }

    /// Lowercase name used in cache file names and user-facing errors.
    pub fn label(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::High => "high",
            Quality::Lossless => "lossless",
            Quality::Max => "max",
        }
    }

    /// Parse the `audioQuality` value returned by `playbackinfo`.
    pub fn from_tidal_param(value: &str) -> Option<Quality> {
        match value.to_ascii_uppercase().as_str() {
            "LOW" => Some(Quality::Low),
            "HIGH" => Some(Quality::High),
            "LOSSLESS" => Some(Quality::Lossless),
            "HI_RES_LOSSLESS" | "HI_RES" | "MASTER" => Some(Quality::Max),
            _ => None,
        }
    }
}
```

Add `actual_quality` to `PlaybackInfo` and the raw response:

```rust
/// The decoded `playbackinfo` response. `manifest` is raw bytes: XML for DASH,
/// JSON for BTS.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackInfo {
    /// The quality that was requested.
    pub quality: Quality,
    /// The quality the API says it served, when reported.
    pub actual_quality: Option<Quality>,
    pub manifest_mime: String,
    pub manifest: Vec<u8>,
}

#[derive(Deserialize)]
struct RawPlaybackInfo {
    #[serde(default, rename = "manifestMimeType")]
    manifest_mime_type: String,
    #[serde(default)]
    manifest: String,
    #[serde(default, rename = "audioQuality")]
    audio_quality: Option<String>,
}
```

And in `playback_info`:

```rust
        Ok(PlaybackInfo {
            quality,
            actual_quality: raw.audio_quality.as_deref().and_then(Quality::from_tidal_param),
            manifest_mime: raw.manifest_mime_type,
            manifest,
        })
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal stream`
Expected: all stream tests PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-tidal/src/stream.rs
git commit -m "feat(tidal): expose the served audio quality"
```

---

### Task 2: StreamPlan quality, normalized manifest hash, and bitrate

**Milestone:** P4-4 (part 2).

**Files:**
- Modify: `crates/oxitide-audio/src/source.rs`
- Modify: `crates/oxitide-audio/src/fetch.rs` (5 test literals)
- Modify: `crates/oxitide-audio/src/seek.rs` (1 test literal)
- Modify: `crates/oxitide-audio/src/engine.rs` (2 test literals)
- Modify: `crates/oxitide-audio/tests/pipeline.rs` (1 `PlaybackInfo` literal)

**Interfaces:**
- Consumes: Task 1 (`Quality::label`, `Quality::from_tidal_param`, `PlaybackInfo.actual_quality`).
- Produces (used by Tasks 4, 5, 6):
  - `StreamPlan` gains `bitrate_kbps: Option<u32>`, `quality: Option<Quality>`, `manifest_hash: Option<u64>`.
  - `pub fn fnv1a64(bytes: &[u8]) -> u64`.

- [ ] **Step 1: Write the failing tests**

Replace the `info` helper in `crates/oxitide-audio/src/source.rs` tests and add the new tests:

```rust
    fn info(mime: &str, body: &[u8]) -> PlaybackInfo {
        PlaybackInfo {
            quality: Quality::Max,
            actual_quality: None,
            manifest_mime: mime.into(),
            manifest: body.to_vec(),
        }
    }

    fn info_with_quality(mime: &str, body: &[u8], actual: Option<Quality>) -> PlaybackInfo {
        PlaybackInfo {
            quality: Quality::Max,
            actual_quality: actual,
            manifest_mime: mime.into(),
            manifest: body.to_vec(),
        }
    }

    fn flac_mpd(codec: &str, rate: u32, bits: u32) -> String {
        format!(
            r#"<MPD mediaPresentationDuration="PT8S"><Period><AdaptationSet>
              <Representation id="audio" codecs="{codec}" audioSamplingRate="{rate}" bitDepth="{bits}">
                <SegmentTemplate initialization="init.mp4" media="seg-$Number$.mp4" startNumber="1" duration="4" timescale="1"/>
              </Representation></AdaptationSet></Period></MPD>"#
        )
    }

    fn mpd_with_token(token: &str) -> String {
        format!(
            r#"<MPD mediaPresentationDuration="PT8S"><Period><AdaptationSet>
              <Representation id="audio" codecs="flac" audioSamplingRate="48000" bitDepth="24">
                <SegmentTemplate initialization="init.mp4?token={token}" media="seg-$Number$.mp4?token={token}" startNumber="1" duration="4" timescale="1"/>
              </Representation></AdaptationSet></Period></MPD>"#
        )
    }

    #[test]
    fn explicit_audio_quality_wins_over_derivation() {
        let mpd = flac_mpd("flac", 44100, 16);
        let plan =
            plan_from_info(&info_with_quality(DASH_MIME, mpd.as_bytes(), Some(Quality::High))).unwrap();
        assert_eq!(plan.quality, Some(Quality::High));
    }

    #[test]
    fn quality_is_derived_when_audio_quality_is_absent() {
        let hi_res = flac_mpd("flac", 48000, 24);
        let lossless = flac_mpd("flac", 44100, 16);
        let aac_low = flac_mpd("mp4a.40.2", 44100, 16).replace("bitDepth=\"16\"", "bandwidth=\"128000\"");
        let aac_high = flac_mpd("mp4a.40.2", 44100, 16).replace("bitDepth=\"16\"", "bandwidth=\"320000\"");

        assert_eq!(
            plan_from_info(&info(DASH_MIME, hi_res.as_bytes())).unwrap().quality,
            Some(Quality::Max)
        );
        assert_eq!(
            plan_from_info(&info(DASH_MIME, lossless.as_bytes())).unwrap().quality,
            Some(Quality::Lossless)
        );
        assert_eq!(
            plan_from_info(&info(DASH_MIME, aac_low.as_bytes())).unwrap().quality,
            Some(Quality::Low)
        );
        assert_eq!(
            plan_from_info(&info(DASH_MIME, aac_high.as_bytes())).unwrap().quality,
            Some(Quality::High)
        );
    }

    #[test]
    fn unknown_codec_derives_no_quality() {
        let mpd = flac_mpd("opus", 44100, 16);
        let plan = plan_from_info(&info(DASH_MIME, mpd.as_bytes())).unwrap();
        assert_eq!(plan.quality, None);
    }

    #[test]
    fn bitrate_comes_from_dash_bandwidth() {
        let mpd = flac_mpd("flac", 44100, 16).replace("bitDepth=\"16\"", "bitDepth=\"16\" bandwidth=\"828801\"");
        let plan = plan_from_info(&info(DASH_MIME, mpd.as_bytes())).unwrap();
        assert_eq!(plan.bitrate_kbps, Some(828));
    }

    #[test]
    fn bts_plan_has_no_bitrate() {
        let json = r#"{"mimeType":"audio/flac","codecs":"flac","urls":["https://cdn/a.flac?token=A"]}"#;
        let plan = plan_from_info(&info(BTS_MIME, json.as_bytes())).unwrap();
        assert_eq!(plan.bitrate_kbps, None);
        assert_eq!(plan.quality, Some(Quality::Lossless));
    }

    #[test]
    fn manifest_hash_ignores_signed_tokens() {
        let a = plan_from_info(&info(DASH_MIME, mpd_with_token("A").as_bytes())).unwrap();
        let b = plan_from_info(&info(DASH_MIME, mpd_with_token("B").as_bytes())).unwrap();
        assert!(a.manifest_hash.is_some());
        assert_eq!(a.manifest_hash, b.manifest_hash, "token rotation changed the cache key");
    }

    #[test]
    fn manifest_hash_detects_a_different_stream() {
        let a = plan_from_info(&info(DASH_MIME, mpd_with_token("A").as_bytes())).unwrap();
        let other = mpd_with_token("A").replace("bitDepth=\"24\"", "bitDepth=\"16\"");
        let b = plan_from_info(&info(DASH_MIME, other.as_bytes())).unwrap();
        assert_ne!(a.manifest_hash, b.manifest_hash);
    }

    #[test]
    fn fnv1a64_is_deterministic() {
        assert_eq!(fnv1a64(b"abc"), fnv1a64(b"abc"));
        assert_ne!(fnv1a64(b"abc"), fnv1a64(b"abd"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio source`
Expected: compile errors — missing `actual_quality`, `quality`, `bitrate_kbps`, `manifest_hash`, `fnv1a64`.

- [ ] **Step 3: Implement**

Extend `StreamPlan`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct StreamPlan {
    pub init_url: Option<String>,
    pub segments: Vec<Segment>,
    /// Ticks per second for `Segment.start_ticks`/`duration_ticks`; 0 for a
    /// single-file source with no timing information.
    pub timescale: u64,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    /// Manifest bandwidth in kbps, when the manifest declares one.
    pub bitrate_kbps: Option<u32>,
    /// The quality the API says it served, or a value derived from the manifest.
    pub quality: Option<Quality>,
    /// FNV-1a 64 over the query-stripped URLs and format fields. `None` for
    /// plans built directly (tests, `StreamPlan::single`) — such plans bypass
    /// the cache.
    pub manifest_hash: Option<u64>,
}
```

Update `StreamPlan::single` and the two `StreamPlan { ... }` literals in `plan_from_info` (all three new fields default to `None` in `single`). Add the helpers:

```rust
/// FNV-1a 64 over `bytes`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Signed CDN tokens live in the query string and rotate between
/// `playbackinfo` calls; hash only the stable part of a URL.
fn strip_query(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

fn manifest_hash(
    init_url: Option<&str>,
    segments: &[Segment],
    codec: Option<&str>,
    sample_rate: Option<u32>,
    bit_depth: Option<u32>,
    timescale: u64,
) -> u64 {
    let mut canonical = String::new();
    if let Some(init) = init_url {
        canonical.push_str(strip_query(init));
    }
    canonical.push('\n');
    for segment in segments {
        canonical.push_str(strip_query(&segment.url));
        canonical.push('\n');
    }
    canonical.push_str(&format!(
        "{codec:?}|{sample_rate:?}|{bit_depth:?}|{timescale}"
    ));
    fnv1a64(canonical.as_bytes())
}

/// Best-effort quality for a manifest that does not declare `audioQuality`.
fn derive_quality(
    codec: Option<&str>,
    bit_depth: Option<u32>,
    sample_rate: Option<u32>,
    bandwidth: Option<u64>,
) -> Option<Quality> {
    let codec = codec?.to_ascii_lowercase();
    if codec.contains("flac") || codec.contains("alac") {
        let hi_res = bit_depth.unwrap_or(0) >= 24 || sample_rate.unwrap_or(0) > 48_000;
        Some(if hi_res { Quality::Max } else { Quality::Lossless })
    } else if codec.contains("mp4a") || codec.contains("aac") {
        Some(if bandwidth.is_some_and(|b| b < 192_000) { Quality::Low } else { Quality::High })
    } else {
        None
    }
}
```

In the DASH arm, before constructing the plan:

```rust
            let bitrate_kbps = repr.bandwidth.map(|b| (b / 1000) as u32);
            let quality = info.actual_quality.or_else(|| {
                derive_quality(repr.codecs.as_deref(), repr.bit_depth, repr.sample_rate, repr.bandwidth)
            });
            let hash = manifest_hash(
                init_url.as_deref(),
                &segments,
                repr.codecs.as_deref(),
                repr.sample_rate,
                repr.bit_depth,
                timescale,
            );
            Ok(StreamPlan {
                init_url,
                segments,
                timescale,
                codec: repr.codecs.clone(),
                sample_rate: repr.sample_rate,
                bit_depth: repr.bit_depth,
                bitrate_kbps,
                quality,
                manifest_hash: Some(hash),
            })
```

In the BTS arm:

```rust
            let segments: Vec<Segment> = parsed
                .urls
                .into_iter()
                .map(|url| Segment { url, start_ticks: 0, duration_ticks: 0 })
                .collect();
            let quality = info
                .actual_quality
                .or_else(|| derive_quality(parsed.codecs.as_deref(), None, None, None));
            let hash = manifest_hash(None, &segments, parsed.codecs.as_deref(), None, None, 0);
            Ok(StreamPlan {
                init_url: None,
                segments,
                timescale: 0,
                codec: parsed.codecs,
                sample_rate: None,
                bit_depth: None,
                bitrate_kbps: None,
                quality,
                manifest_hash: Some(hash),
            })
```

- [ ] **Step 4: Update every other `StreamPlan` / `PlaybackInfo` literal**

Add `bitrate_kbps: None, quality: None, manifest_hash: None` to:
- `crates/oxitide-audio/src/fetch.rs` (5 literals in the tests module)
- `crates/oxitide-audio/src/seek.rs` (`plan_with_segments`)
- `crates/oxitide-audio/src/engine.rs` (`fixture_plan`, `plan_for`)

Add `actual_quality: None` to the `PlaybackInfo { ... }` literal in `crates/oxitide-audio/tests/pipeline.rs`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio`
Expected: all source tests PASS; existing fetch/seek/engine/pipeline tests still PASS.

- [ ] **Step 6: Run the workspace gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add crates/oxitide-audio/src/source.rs crates/oxitide-audio/src/fetch.rs crates/oxitide-audio/src/seek.rs crates/oxitide-audio/src/engine.rs crates/oxitide-audio/tests/pipeline.rs
git commit -m "feat(audio): derive served quality and a token-stable manifest hash"
```

---

### Task 3: Disk cache module

**Milestone:** P4-1.

**Files:**
- Create: `crates/oxitide-audio/src/cache.rs`
- Modify: `crates/oxitide-audio/src/lib.rs` (add `pub mod cache;` before `pub mod decode;`)

**Interfaces:**
- Consumes: Task 1 (`Quality::label`).
- Produces (used by Tasks 4, 5, 9):
  - `CacheKey { track_id: u64, quality: Quality, manifest_hash: u64 }` with `file_name() -> String`.
  - `DiskCache::new(dir: PathBuf, max_bytes: u64) -> Arc<DiskCache>` (infallible).
  - `usable() -> bool`, `dir() -> &Path`, `max_bytes() -> u64`.
  - `lookup(&CacheKey) -> Option<PathBuf>`, `begin_write(self: &Arc<Self>, &CacheKey) -> Option<CacheWriter>`.
  - `commit(CacheWriter)`, `abort(CacheWriter)`, `remove(&CacheKey)`, `size_bytes() -> u64`, `clear()`, `set_max_bytes(u64)`, `protect(&CacheKey)`, `unprotect(&CacheKey)`, `cleanup()`.
  - `CacheWriter::write_all(&[u8]) -> io::Result<()>`, `bytes() -> u64`, `commit(self)`, `abort(self)`.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-audio/src/cache.rs` with the full test module first (the implementation types do not exist yet):

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio cache`
Expected: compile errors — `CacheKey`, `DiskCache`, `CacheWriter` not defined.

- [ ] **Step 3: Implement the module**

Prepend to `cache.rs` (above the tests):

```rust
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
```

- [ ] **Step 4: Register the module**

In `crates/oxitide-audio/src/lib.rs`, add `pub mod cache;` (alphabetically, before `pub mod decode;`).

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio cache`
Expected: all 8 cache tests PASS.

- [ ] **Step 6: Run the workspace gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add crates/oxitide-audio/src/cache.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add the track-blob disk cache"
```

---

### Task 4: Cache-aware playback

**Milestone:** P4-2.

**Files:**
- Modify: `crates/oxitide-audio/src/fetch.rs`
- Modify: `crates/oxitide-audio/src/demux.rs`
- Modify: `crates/oxitide-audio/src/engine.rs`
- Modify: `crates/oxitide-audio/tests/pipeline.rs` (the `download` call)
- Modify: `crates/oxitide-app/src/config.rs` (add `default_cache_dir`)
- Modify: `crates/oxitide-app/src/commands.rs` (`build_player` signature + cache construction)

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces (used by Tasks 5, 6, 9):
  - `StreamBuffer::total_pushed() -> u64`.
  - `fetch::download(buffer, fetcher, plan, cache_writer: Option<CacheWriter>)`.
  - `demux::FileSource::open(&Path) -> io::Result<FileSource>` (seekable `MediaSource`).
  - `Engine::new(resolver, make_fetcher, output, quality, cache: Arc<DiskCache>, allow_fallback: bool)`.
  - `config::default_cache_dir() -> Option<PathBuf>`.

- [ ] **Step 1: Write the failing fetch tests**

In `crates/oxitide-audio/src/fetch.rs` tests, add the imports and helpers:

```rust
    use crate::cache::{CacheKey, DiskCache};
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
        assert_eq!(buf.total_pushed(), 8);
        let path = cache.lookup(&key).expect("committed entry");
        assert_eq!(std::fs::metadata(path).unwrap().len(), 8);
        assert_eq!(cache.size_bytes(), 8);
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio fetch`
Expected: compile errors — `total_pushed`, wrong `download` arity.

- [ ] **Step 3: Implement the fetch tee**

Add `total_pushed: AtomicU64` to `StreamBuffer`, initialize it in `new()`, increment it in `push` after `s.data.extend_from_slice(bytes)`, and add:

```rust
    /// Total bytes pushed into the buffer (not the currently buffered count).
    pub fn total_pushed(&self) -> u64 {
        self.total_pushed.load(Ordering::SeqCst)
    }
```

Add `use crate::cache::CacheWriter;` and change `download`:

```rust
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
        match get_with_retry(fetcher.as_ref(), &url) {
            Ok(bytes) => {
                if let Some(writer) = writer.as_mut() {
                    if let Err(e) = writer.write_all(&bytes) {
                        tracing::warn!("cache write failed: {e}");
                        if let Some(writer) = writer.take() {
                            writer.abort();
                        }
                    }
                }
                buffer.push(&bytes);
            }
            Err(e) => {
                buffer.fail(e.to_string());
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
```

Update all existing `download(...)` calls in `fetch.rs` tests and in `crates/oxitide-audio/tests/pipeline.rs` to pass `None`.

- [ ] **Step 4: Write the failing demux test**

Add to `crates/oxitide-audio/src/demux.rs` tests:

```rust
    #[test]
    fn file_source_reads_seeks_and_reports_len() {
        let dir = std::env::temp_dir().join(format!("oxitide-demux-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("blob.bin");
        std::fs::write(&path, b"0123456789").unwrap();
        let mut src = FileSource::open(&path).unwrap();
        assert_eq!(src.byte_len(), Some(10));
        assert!(src.is_seekable());
        let mut out = [0u8; 4];
        assert_eq!(src.read(&mut out).unwrap(), 4);
        assert_eq!(&out, b"0123");
        src.seek(SeekFrom::Start(8)).unwrap();
        assert_eq!(src.read(&mut out).unwrap(), 2);
        assert_eq!(&out[..2], b"89");
        let _ = std::fs::remove_dir_all(dir);
    }
```

Run: `cargo test -p oxitide-audio demux` → FAIL (`FileSource` not defined).

- [ ] **Step 5: Implement `FileSource`**

Add to `crates/oxitide-audio/src/demux.rs`:

```rust
/// A seekable file source for cache hits.
pub struct FileSource {
    file: std::fs::File,
    len: u64,
    pos: u64,
}

impl FileSource {
    pub fn open(path: &std::path::Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { file, len, pos: 0 })
    }
}

impl Read for FileSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read(out)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for FileSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.pos = self.file.seek(pos)?;
        Ok(self.pos)
    }
}

impl MediaSource for FileSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}
```

- [ ] **Step 6: Wire the cache into the engine**

In `crates/oxitide-audio/src/engine.rs`:

Add imports:

```rust
use crate::cache::{CacheKey, DiskCache};
```

Add `cache: Arc<DiskCache>` to `Inner`, set it from `Engine::new`, and change the constructor:

```rust
    pub fn new(
        resolver: Arc<dyn StreamResolver>,
        make_fetcher: impl FnOnce() -> Arc<dyn Fetcher> + Send + 'static,
        output: Box<dyn AudioOutput>,
        quality: Quality,
        cache: Arc<DiskCache>,
        allow_fallback: bool,
    ) -> Self {
        let inner = Arc::new(Inner {
            // ... existing fields ...
            cache,
            allow_fallback: AtomicBool::new(allow_fallback),
            // ... existing fields ...
        });
        // ... unchanged ...
    }
```

Add `allow_fallback: AtomicBool` to `Inner` now (Task 6 uses it; the constructor already takes it).

Replace `decode_track` with the cache-aware version:

```rust
fn decode_track(
    inner: Arc<Inner>,
    item: QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: SyncSender<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
) {
    let quality = *inner.quality.lock().unwrap();
    let plan = match inner.resolver.resolve(item.track_id, quality) {
        Ok(plan) => plan,
        Err(e) => {
            let _ = tx.send(Err(e));
            return;
        }
    };
    tracing::debug!(
        "resolved track {}: codec={:?} rate={:?} bits={:?} timescale={} segments={} init={} first_media={:?}",
        item.track_id,
        plan.codec,
        plan.sample_rate,
        plan.bit_depth,
        plan.timescale,
        plan.segments.len(),
        plan.init_url.is_some(),
        plan.segments.first().map(|s| s.url.split('?').next().unwrap_or("")),
    );
    let cache_key = plan.manifest_hash.map(|manifest_hash| CacheKey {
        track_id: item.track_id,
        quality,
        manifest_hash,
    });
    if let Some(key) = cache_key {
        if let Some(path) = inner.cache.lookup(&key) {
            inner.cache.protect(&key);
            let result = decode_from_file(&inner, &item, mode, generation, seek_ms, &tx, &path, &plan);
            inner.cache.unprotect(&key);
            match result {
                Ok(()) => return,
                Err(e) => {
                    if inner.generation.load(Ordering::SeqCst) != generation {
                        let _ = tx.send(Err(e));
                        return;
                    }
                    tracing::warn!("cache entry {} failed to decode ({e}); refetching", key.file_name());
                    inner.cache.remove(&key);
                }
            }
        }
    }
    decode_from_network(inner, item, mode, generation, seek_ms, tx, buffer, plan, cache_key);
}

#[allow(clippy::too_many_arguments)]
fn decode_from_network(
    inner: Arc<Inner>,
    item: QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: SyncSender<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
    plan: StreamPlan,
    cache_key: Option<CacheKey>,
) {
    // Seek by segment index: media URLs are an ordered list, so restarting
    // at the segment covering the target needs no HTTP Range.
    let download_plan = match seek_ms {
        Some(ms) if plan.sample_rate.is_some() => {
            let target = seek::target_segment(&plan, ms, plan.sample_rate.unwrap());
            plan.from_segment(target.segment_index)
        }
        _ => plan.clone(),
    };
    // Only full-track plans are cached; a seek-truncated plan is a partial
    // track and would poison replays.
    let writer = match (seek_ms, cache_key) {
        (None, Some(key)) => inner.cache.begin_write(&key),
        _ => None,
    };
    let fetch_buffer = buffer.clone();
    let fetcher = inner.fetcher.get().expect("fetcher initialized by the worker").clone();
    let downloader = std::thread::spawn(move || fetch::download(fetch_buffer, fetcher, download_plan, writer));
    let source = crate::demux::StreamBufferSource::new(buffer);
    let result = decode_source(inner, &item, mode, generation, seek_ms, &tx, source, plan);
    let _ = downloader.join();
    if let Err(e) = result {
        let _ = tx.send(Err(e));
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_from_file(
    inner: &Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: &SyncSender<Result<StreamEvent>>,
    path: &std::path::Path,
    plan: &StreamPlan,
) -> Result<()> {
    let source = crate::demux::FileSource::open(path)?;
    decode_source(inner.clone(), item, mode, generation, seek_ms, tx, source, plan.clone())
}

#[allow(clippy::too_many_arguments)]
fn decode_source(
    inner: Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: &SyncSender<Result<StreamEvent>>,
    source: impl symphonia::core::io::MediaSource + 'static,
    plan: StreamPlan,
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
        discarder: Discarder::default(),
        start_frames: 0,
    };
    let _ = item;
    crate::decode::run(source, "mp4", &mut sink)
}
```

(`item` is unused in Task 4's `decode_source`; Task 6 uses it for `duration_secs`. Keep the parameter and the `let _ = item;` line — Task 6 replaces it.)

- [ ] **Step 7: Update the engine tests**

In `crates/oxitide-audio/src/engine.rs` tests:

Add imports and the cache helper:

```rust
    use crate::cache::{CacheKey, DiskCache};

    fn test_cache() -> Arc<DiskCache> {
        static CACHE_COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "oxitide-engine-cache-{}-{}",
            std::process::id(),
            CACHE_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        DiskCache::new(dir, 64 * 1024 * 1024)
    }

    fn build_engine(
        resolver: Arc<dyn StreamResolver>,
        fetcher: Arc<dyn Fetcher>,
        output: Box<dyn AudioOutput>,
        quality: Quality,
    ) -> Engine {
        Engine::new(resolver, move || fetcher, output, quality, test_cache(), true)
    }

    fn cached_plan(url: &str, manifest_hash: u64) -> StreamPlan {
        StreamPlan {
            init_url: None,
            segments: vec![Segment { url: url.into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            bitrate_kbps: None,
            quality: None,
            manifest_hash: Some(manifest_hash),
        }
    }

    struct CountingFetcher {
        files: HashMap<String, Vec<u8>>,
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Fetcher for CountingFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.files
                .get(url)
                .cloned()
                .ok_or_else(|| Error::Fetch(format!("no fixture for {url}")))
        }
    }
```

Update the direct `Engine::new` call in `fetcher_is_built_off_the_async_runtime` to pass `test_cache(), true`.

Add the cache tests:

```rust
    #[test]
    fn cache_miss_writes_the_entry_then_a_replay_makes_zero_fetcher_calls() {
        let (_, files) = fixture_plan();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let output = NullOutput::new();
        let cache = test_cache();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan: cached_plan("mem://tone", 0xabcd) }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: 0xabcd };

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert!(cache.lookup(&key).is_some(), "a cache miss did not write the entry");
        let first = calls.load(Ordering::SeqCst);
        assert_eq!(first, 1, "the first play should fetch exactly once");

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        assert_eq!(calls.load(Ordering::SeqCst), first, "a cache hit still fetched");
    }

    #[test]
    fn seek_plans_are_not_cached() {
        let (_, files) = fixture_plan();
        let output = NullOutput::new();
        let cache = test_cache();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan: cached_plan("mem://tone", 0xabcd) }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output),
            Quality::Lossless,
            cache.clone(),
            true,
        );
        let (tx, _rx) = std::sync::mpsc::sync_channel(512);
        let generation = engine.inner.generation.load(Ordering::SeqCst);
        let buffer = StreamBuffer::new();
        decode_track(engine.inner.clone(), item(1), OutputMode::Shared, generation, Some(500), tx, buffer);
        assert_eq!(cache.size_bytes(), 0, "a seek plan must not be cached");
    }
```

```rust
    #[test]
    fn corrupt_cache_entry_is_removed_and_refetched() {
        let (_, files) = fixture_plan();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let output = NullOutput::new();
        let cache = test_cache();
        let key = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: 0xabcd };
        {
            let mut writer = cache.begin_write(&key).unwrap();
            writer.write_all(b"this is not an mp4").unwrap();
            writer.commit();
        }
        let engine = Engine::new(
            Arc::new(FakeResolver { plan: cached_plan("mem://tone", 0xabcd) }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(output.clone()),
            Quality::Lossless,
            cache.clone(),
            true,
        );

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(output.samples().len(), 176_400);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the network retry must run exactly once");
        let replaced = cache.lookup(&key).expect("the corrupt entry was not replaced");
        assert_eq!(
            std::fs::metadata(replaced).unwrap().len(),
            files["mem://tone"].len() as u64,
            "the refetched bytes were not re-cached"
        );
    }
```

- [ ] **Step 8: Keep the app compiling**

In `crates/oxitide-app/src/config.rs`, add:

```rust
/// `%APPDATA%\OxiTide\cache`.
pub fn default_cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide").map(|dirs| dirs.config_dir().join("cache"))
}
```

In `crates/oxitide-app/src/commands.rs`, change `build_player` to take a cache and pass the new arguments:

```rust
fn build_player(
    app: &tauri::AppHandle,
    client: TidalClient,
    config: &Config,
    cache: std::sync::Arc<oxitide_audio::cache::DiskCache>,
) -> Result<Player, String> {
    let resolver = crate::player::TidalResolver::new(client).map_err(|e| e.to_string())?;
    let engine = Engine::new(
        Arc::new(resolver),
        || Arc::new(HttpFetcher::default()),
        crate::player::real_output(),
        config.streaming.quality.to_quality(),
        cache,
        true, // Task 9 replaces this with config.streaming.allow_fallback
    );
    // ... unchanged ...
}
```

In `auth_complete` and `auth_restore`, before `state.set_player(...)`:

```rust
            let cache_dir = crate::config::default_cache_dir()
                .unwrap_or_else(|| std::env::temp_dir().join("oxitide-cache"));
            let cache = oxitide_audio::cache::DiskCache::new(cache_dir, 2 * 1024 * 1024 * 1024);
            state.set_player(build_player(&app, client, &config, cache)?);
```

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio` then `cargo test --workspace`
Expected: all tests PASS.

- [ ] **Step 10: Run the gates and commit**

```powershell
cargo clippy --workspace -- -D warnings
git add crates/oxitide-audio/src/fetch.rs crates/oxitide-audio/src/demux.rs crates/oxitide-audio/src/engine.rs crates/oxitide-audio/tests/pipeline.rs crates/oxitide-app/src/config.rs crates/oxitide-app/src/commands.rs
git commit -m "feat(audio): play from and tee into the disk cache"
```

---

### Task 5: Next-track prefetch and single-flight cache writers

**Milestone:** P4-3.

**Files:**
- Modify: `crates/oxitide-audio/src/cache.rs`
- Modify: `crates/oxitide-audio/src/engine.rs` (tests only)

**Interfaces:**
- Consumes: Tasks 3, 4.
- Produces: `DiskCache` refuses a second writer for a key already being written; `CacheWriter` aborts itself if dropped without `commit()`/`abort()`.

**Ruling (plan amendment, recorded in the ledger):** the engine already runs a next-track PCM lookahead prefetch (`start_prefetch`) that downloads and decodes the next queue item while the current one plays. With Task 4's cache tee, that path already fills the next track's cache entry (committed when its download completes). A second, bytes-only prefetch for the same track would race the first writer on the same cache key and duplicate its network traffic; serializing the two makes the byte prefetch a no-op. P4-3 is therefore implemented as: the existing next-track prefetch is cache-aware, guarded by a single-flight writer rule so concurrent playback/prefetch writers can never corrupt an entry. `download_into_cache`/`prefetch_next` are not added. Cost if wrong: the cache is not filled while the current track is paused, and a track skipped before its download completes is not cached.

- [ ] **Step 1: Write the failing cache tests**

Add to `crates/oxitide-audio/src/cache.rs` tests:

```rust
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
```

Update `cleanup_removes_stale_parts` in the same module (dropping a writer now aborts it, so the stale part must be created directly):

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio cache`
Expected: FAIL — a second writer is currently allowed.

- [ ] **Step 3: Implement single-flight writers**

In `crates/oxitide-audio/src/cache.rs`:

Add the in-flight set to `DiskCache`:

```rust
pub struct DiskCache {
    dir: PathBuf,
    max_bytes: AtomicU64,
    protected: Mutex<HashSet<PathBuf>>,
    /// Keys with a writer in flight; a second writer for the same key is refused.
    in_flight: Mutex<HashSet<CacheKey>>,
    usable: bool,
}
```

Initialize it in `new` (`in_flight: Mutex::new(HashSet::new())`).

Replace `begin_write`:

```rust
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
```

Replace `commit` and `abort`:

```rust
    /// Atomic publish: fsync temp, rename to the final name, touch mtime, evict.
    pub fn commit(&self, mut writer: CacheWriter) {
        writer.finish_commit();
    }

    /// Deletes a temp file after a cancelled/failed download.
    pub fn abort(&self, mut writer: CacheWriter) {
        writer.finish_abort();
    }
```

Replace the `CacheWriter` struct and impl:

```rust
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
        self.cache.in_flight.lock().unwrap().remove(&self.key);
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
        if let Err(e) = fs::rename(&self.part, &final_path) {
            tracing::warn!("cache commit for {} failed: {e}", self.key.file_name());
            let _ = fs::remove_file(&self.part);
            return;
        }
        if let Ok(file) = OpenOptions::new().write(true).open(&final_path) {
            let _ = file.set_modified(SystemTime::now());
        }
        self.cache.evict_to_fit();
    }

    fn finish_abort(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.cache.in_flight.lock().unwrap().remove(&self.key);
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
```

- [ ] **Step 4: Write the failing engine test**

Add to `crates/oxitide-audio/src/engine.rs` tests:

```rust
    /// P4-3: the next queue item is downloaded into the cache while the current
    /// one plays (via the existing lookahead prefetch, now cache-teed), and a
    /// replay of the queue fetches nothing.
    #[test]
    fn next_track_is_prefetched_into_the_cache() {
        let tone = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let mut files = HashMap::new();
        files.insert("mem://t1".to_string(), tone.clone());
        files.insert("mem://t2".to_string(), tone);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut plans = HashMap::new();
        plans.insert(1, cached_plan("mem://t1", 0x1111));
        plans.insert(2, cached_plan("mem://t2", 0x2222));
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = NullOutput::new();
        let cache = test_cache();
        let engine = Engine::new(
            Arc::new(PerTrackResolver { plans, order }),
            {
                let calls = calls.clone();
                let files = files.clone();
                move || -> Arc<dyn Fetcher> { Arc::new(CountingFetcher { files, calls }) }
            },
            Box::new(ThrottledOutput { inner: output.clone(), delay: Duration::from_millis(20) }),
            Quality::Lossless,
            cache.clone(),
            true,
        );
        let key1 = CacheKey { track_id: 1, quality: Quality::Lossless, manifest_hash: 0x1111 };
        let key2 = CacheKey { track_id: 2, quality: Quality::Lossless, manifest_hash: 0x2222 };

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for(|| cache.lookup(&key2).is_some());
        assert!(cache.lookup(&key2).is_some(), "the prefetch never filled track 2's entry");
        assert!(
            output.samples().len() < 176_400,
            "track 1 finished before the prefetch was observable"
        );
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        assert!(cache.lookup(&key1).is_some(), "track 1 was not cached when it finished");
        let after_first_play = calls.load(Ordering::SeqCst);
        assert_eq!(after_first_play, 2, "each track should be fetched exactly once");

        // Replaying the queue is a pure cache read.
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 4);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            after_first_play,
            "a replay of a cached queue still fetched"
        );
    }
```

- [ ] **Step 5: No engine code change**

Task 4 already routes the next-track lookahead prefetch (`start_prefetch` → `decode_track`) through the cache tee. This step confirms the Step 4 test passes against that path; if it fails, fix the cache integration, not the test.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio cache engine`
Expected: all cache and engine tests PASS.

- [ ] **Step 7: Run the gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add crates/oxitide-audio/src/cache.rs crates/oxitide-audio/src/engine.rs
git commit -m "feat(audio): make the next-track prefetch cache-aware and single-flight"
```

---

### Task 6: Fallback policy and StreamMeta reporting

**Milestone:** P4-4 (part 3).

**Files:**
- Modify: `crates/oxitide-audio/src/error.rs`
- Modify: `crates/oxitide-audio/src/engine.rs`

**Interfaces:**
- Consumes: Tasks 1, 2, 4.
- Produces (used by Tasks 9, 10):
  - `Error::Unavailable(String)`.
  - `Engine::set_allow_fallback(&self, allow: bool)`.
  - `StreamMeta { quality: Option<Quality>, codec: Option<String>, sample_rate: u32, bit_depth: u16, channels: u16, bitrate_kbps: Option<u32> }` (Serialize).
  - `PlaybackState.stream: Option<StreamMeta>`.

- [ ] **Step 1: Write the failing tests**

In `crates/oxitide-audio/src/error.rs` tests:

```rust
    #[test]
    fn unavailable_error_has_stable_message() {
        assert_eq!(
            Error::Unavailable("lossless".into()).to_string(),
            "track unavailable at lossless; enable fallback or lower the streaming quality"
        );
    }
```

In `crates/oxitide-audio/src/engine.rs` tests:

```rust
    fn engine_with_policy(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
        allow_fallback: bool,
    ) -> (Engine, NullOutput) {
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
            test_cache(),
            allow_fallback,
        );
        (engine, output)
    }

    #[test]
    fn fallback_disabled_errors_on_a_lower_served_quality() {
        let (mut plan, files) = fixture_plan();
        plan.quality = Some(Quality::Low);
        let (engine, output) = engine_with_policy(plan, files, false);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| {
            seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. }))
        });
        let message = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                EngineEvent::Error { message, .. } => Some(message.clone()),
                _ => None,
            })
            .expect("no error event");
        assert!(
            message.contains("track unavailable at low"),
            "unexpected error message: {message}"
        );
        assert!(!engine.state().playing);
        assert_eq!(engine.state().queue.len(), 1, "the queue must be kept");
        assert!(output.samples().is_empty(), "a rejected stream must not write audio");
    }

    #[test]
    fn fallback_enabled_plays_and_reports_the_served_quality() {
        let (mut plan, files) = fixture_plan();
        plan.quality = Some(Quality::Low);
        let (engine, output) = engine_with_policy(plan, files, true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);

        let meta = engine.state().stream.expect("no stream metadata");
        assert_eq!(meta.quality, Some(Quality::Low));
        assert_eq!(meta.sample_rate, 44100);
        assert_eq!(meta.bit_depth, 16);
        assert_eq!(meta.channels, 2);
        assert_eq!(meta.codec.as_deref(), Some("flac"));
    }

    #[test]
    fn unknown_served_quality_plays() {
        let (mut plan, files) = fixture_plan();
        plan.quality = None;
        let (engine, output) = engine_with_policy(plan, files, false);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(output.samples().len(), 176_400);
        assert_eq!(engine.state().stream.and_then(|m| m.quality), None);
    }

    #[test]
    fn single_file_bitrate_is_computed_from_bytes_and_duration() {
        let (plan, files) = fixture_plan();
        let bytes = files["mem://tone"].len() as u64;
        let (engine, output) = engine_with_policy(plan, files, true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);

        let meta = engine.state().stream.expect("no stream metadata");
        assert_eq!(meta.bitrate_kbps, Some((bytes * 8 / 2 / 1000) as u32));
    }

    #[test]
    fn manifest_bitrate_wins_over_the_computed_one() {
        let (mut plan, files) = fixture_plan();
        plan.bitrate_kbps = Some(828);
        let (engine, output) = engine_with_policy(plan, files, true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(engine.state().stream.and_then(|m| m.bitrate_kbps), Some(828));
    }

    #[test]
    fn set_allow_fallback_applies_from_the_next_track() {
        let (mut plan, files) = fixture_plan();
        plan.quality = Some(Quality::Low);
        let (engine, output) = engine_with_policy(plan, files, false);
        engine.set_allow_fallback(true);

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400);
        assert_eq!(output.samples().len(), 176_400);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio error engine`
Expected: compile errors — `Unavailable`, `set_allow_fallback`, `state().stream`.

- [ ] **Step 3: Implement the typed error**

In `crates/oxitide-audio/src/error.rs`, add to `Error`:

```rust
    #[error("track unavailable at {0}; enable fallback or lower the streaming quality")]
    Unavailable(String),
```

- [ ] **Step 4: Implement the policy and StreamMeta**

In `crates/oxitide-audio/src/engine.rs`:

Add `stream_meta: Mutex<Option<StreamMeta>>` to `Inner` (initialize `Mutex::new(None)`).

Add `StreamMeta` above `PlaybackState` and the new field:

```rust
/// The stream that was actually served and decoded for the current track.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct StreamMeta {
    /// The served/actual quality (`None` when undecidable).
    pub quality: Option<Quality>,
    /// Codec name from the manifest.
    pub codec: Option<String>,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub channels: u16,
    /// Manifest bandwidth, else computed from the fetched bytes.
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlaybackState {
    // ... existing fields ...
    pub queue: Vec<QueueItem>,
    pub stream: Option<StreamMeta>,
}
```

In `build_state`:

```rust
        stream: inner.stream_meta.lock().unwrap().clone(),
```

Add the setter on `Engine`:

```rust
    /// Whether a lower-than-requested served quality plays or errors. Applies
    /// from the next track.
    pub fn set_allow_fallback(&self, allow: bool) {
        self.inner.allow_fallback.store(allow, Ordering::SeqCst);
    }
```

In `decode_track`, after resolving and before the cache lookup:

```rust
    let served = plan.quality;
    if let Some(served) = served {
        if !inner.allow_fallback.load(Ordering::SeqCst) && served < quality {
            let _ = tx.send(Err(Error::Unavailable(served.label().to_string())));
            return;
        }
    }
```

Add `meta: StreamMeta` to `StreamStart`:

```rust
struct StreamStart {
    info: StreamInfo,
    start_frames: u64,
    meta: StreamMeta,
}
```

Extend `ChannelSink` with the duration and a byte-count source, and build the meta in `on_stream_info`:

```rust
struct ChannelSink {
    inner: Arc<Inner>,
    generation: u64,
    tx: SyncSender<Result<StreamEvent>>,
    mode: OutputMode,
    out_format: PcmFormat,
    channels: u16,
    /// The original (untruncated) plan, for seek discard math.
    plan: StreamPlan,
    seek_ms: Option<u64>,
    discarder: Discarder,
    start_frames: u64,
    /// The queue item's duration, for the bitrate fallback.
    duration_secs: u32,
    /// Total source bytes fetched so far (buffer for network, file length for a
    /// cache hit).
    total_bytes: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl ChannelSink {
    fn send(&self, event: Result<StreamEvent>) -> Result<()> {
        self.tx.send(event).map_err(|_| Error::Output("playback stream closed".into()))
    }

    /// Fallback bitrate for a single-file source: fetched bytes over the item's
    /// duration. Multi-segment manifests carry their own bandwidth.
    fn computed_bitrate_kbps(&self) -> Option<u32> {
        if self.plan.timescale != 0 || self.plan.segments.len() != 1 || self.duration_secs == 0 {
            return None;
        }
        let bytes = (self.total_bytes)();
        if bytes == 0 {
            return None;
        }
        Some((bytes.saturating_mul(8) / self.duration_secs as u64 / 1000) as u32)
    }
}
```

In `on_stream_info`, replace the final `self.send(...)` with:

```rust
        let meta = StreamMeta {
            quality: self.plan.quality,
            codec: self.plan.codec.clone(),
            sample_rate: info.sample_rate,
            bit_depth: info.bits_per_sample,
            channels: info.channels,
            bitrate_kbps: self.plan.bitrate_kbps.or_else(|| self.computed_bitrate_kbps()),
        };
        self.send(Ok(StreamEvent::Info(StreamStart {
            info: *info,
            start_frames: self.start_frames,
            meta,
        })))
```

Update `decode_source` to take the item and build the sink fields. Change its signature and body:

```rust
#[allow(clippy::too_many_arguments)]
fn decode_source(
    inner: Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
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
        discarder: Discarder::default(),
        start_frames: 0,
        duration_secs: item.duration_secs,
        total_bytes,
    };
    crate::decode::run(source, "mp4", &mut sink)
}
```

In `decode_from_network`, build the byte-count source and pass it through:

```rust
    let source = crate::demux::StreamBufferSource::new(buffer.clone());
    let total_bytes = {
        let buffer = buffer.clone();
        Arc::new(move || buffer.total_pushed()) as Arc<dyn Fn() -> u64 + Send + Sync>
    };
    let result = decode_source(inner, &item, mode, generation, seek_ms, &tx, source, plan, total_bytes);
```

In `decode_from_file`, use the file length:

```rust
#[allow(clippy::too_many_arguments)]
fn decode_from_file(
    inner: &Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: &SyncSender<Result<StreamEvent>>,
    path: &std::path::Path,
    plan: &StreamPlan,
) -> Result<()> {
    let len = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let source = crate::demux::FileSource::open(path)?;
    let total_bytes = Arc::new(move || len) as Arc<dyn Fn() -> u64 + Send + Sync>;
    decode_source(inner.clone(), item, mode, generation, seek_ms, tx, source, plan.clone(), total_bytes)
}
```

In `run_session`, destructure the meta, store it, and emit a State so the badge updates mid-session:

```rust
        let start = match wait_for_info(inner, &mut stream, gen) {
            Ok(start) => start,
            Err(_) if inner.generation.load(Ordering::SeqCst) != gen => {
                return SessionOutcome::Superseded;
            }
            Err(e) => return SessionOutcome::StreamError(e),
        };
        let StreamStart { info, start_frames, meta } = start;
        *inner.stream_meta.lock().unwrap() = Some(meta);
        emit(inner, EngineEvent::State(build_state(inner)));
        // The stream's absolute starting frame (non-zero only for a seek).
        frames = start_frames;
```

Clear the meta at the start of every track iteration (before `wait_for_info`) and on `Clear` / new `PlayTracks` / worker error paths:

- In `run_session`'s loop, immediately before `let start = match wait_for_info(...)`, add `*inner.stream_meta.lock().unwrap() = None;`.
- In `Engine::handle`, `EngineCmd::PlayTracks` and `EngineCmd::Clear` arms: `*self.inner.stream_meta.lock().unwrap() = None;`.
- In `worker_main`'s `OutputError` and `StreamError` arms: `*inner.stream_meta.lock().unwrap() = None;`.
- In `Engine::play_track_into`, after `wait_for_info` returns: `*self.inner.stream_meta.lock().unwrap() = Some(start.meta.clone());`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio`
Expected: all tests PASS.

- [ ] **Step 6: Run the gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add crates/oxitide-audio/src/error.rs crates/oxitide-audio/src/engine.rs
git commit -m "feat(audio): enforce fallback policy and report the real stream format"
```

---

### Task 7: AAC decode

**Milestone:** P4-5.

**Files:**
- Modify: root `Cargo.toml` (`symphonia` features)
- Create: `crates/oxitide-audio/tests/fixtures/tone_aac.mp4`
- Modify: `crates/oxitide-audio/src/decode.rs` (tests)

**Interfaces:**
- Consumes: nothing new.
- Produces: AAC-LC playback via symphonia; AAC is a float source, so exclusive mode falls back per policy exactly as float sources do today.

- [ ] **Step 1: Generate the fixture**

```powershell
ffmpeg -y -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=2" -ac 2 -c:a aac -b:a 96k -movflags frag_keyframe+empty_moov+default_base_moof crates/oxitide-audio/tests/fixtures/tone_aac.mp4
ffprobe -v error -show_entries stream=codec_name,sample_rate,channels,duration -of default=noprint_wrappers=1 crates/oxitide-audio/tests/fixtures/tone_aac.mp4
```

Expected: `aac`, 44100, 2, ~2.0 s.

- [ ] **Step 2: Write the failing test**

Add to `crates/oxitide-audio/src/decode.rs` tests:

```rust
    #[test]
    fn decodes_fragmented_mp4_aac() {
        let sink = decode_fixture_f32("tone_aac.mp4", "mp4");
        let info = sink.info.unwrap();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert!(
            !info.is_bit_perfect_candidate(),
            "AAC must not be treated as a bit-perfect candidate"
        );
        // AAC-LC adds up to one frame of encoder delay/padding (1024 samples
        // per channel) around the 2 s tone.
        assert!(
            sink.samples.len() >= 176_400 && sink.samples.len() <= 176_400 + 2 * 2 * 1024,
            "unexpected AAC sample count: {}",
            sink.samples.len()
        );
        assert!(sink.samples.iter().all(|s| s.is_finite()));
    }
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio decode`
Expected: FAIL — "decode error: unsupported codec" (or no audio track).

- [ ] **Step 4: Enable the feature**

In root `Cargo.toml`:

```toml
symphonia = { version = "0.6", features = ["flac", "isomp4", "pcm", "aac"] }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio decode`
Expected: all decode tests PASS (AAC plus the existing FLAC tests). If the sample count falls outside the range, run the test once with the count printed and tighten the bound to the observed value ± 1024.

- [ ] **Step 6: Run the gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add Cargo.toml Cargo.lock crates/oxitide-audio/tests/fixtures/tone_aac.mp4 crates/oxitide-audio/src/decode.rs
git commit -m "feat(audio): decode AAC via symphonia"
```

---

### Task 8: Shuffle-once over the queue tail

**Milestone:** P4-6 (part 1).

**Files:**
- Modify: root `Cargo.toml` (`rand = "0.10"` in `[workspace.dependencies]`)
- Modify: `crates/oxitide-audio/Cargo.toml` (`rand.workspace = true`)
- Modify: `crates/oxitide-audio/src/queue.rs`

**Interfaces:**
- Consumes: `EngineCmd::SetShuffle` already exists and calls `Queue::set_shuffle`.
- Produces: `Queue::set_shuffle(true)` shuffles the tail with `rand::rng()`; `fn shuffle_tail_with<R: rand::Rng + ?Sized>(&mut self, rng: &mut R)` (private, seeded by tests).

- [ ] **Step 1: Write the failing tests**

Add to `crates/oxitide-audio/src/queue.rs` tests:

```rust
    #[test]
    fn set_shuffle_moves_only_the_tail() {
        use rand::SeedableRng;
        let mut q = queue(8);
        q.next(); // current = index 1
        let head: Vec<u64> = q.items()[..=1].iter().map(|i| i.track_id).collect();
        let tail_before: Vec<u64> = q.items()[2..].iter().map(|i| i.track_id).collect();
        q.shuffle_tail_with(&mut rand::rngs::StdRng::seed_from_u64(7));
        let head_after: Vec<u64> = q.items()[..=1].iter().map(|i| i.track_id).collect();
        let tail_after: Vec<u64> = q.items()[2..].iter().map(|i| i.track_id).collect();
        assert_eq!(head_after, head, "shuffle moved the current item or the head");
        let mut sorted_before = tail_before.clone();
        sorted_before.sort();
        let mut sorted_after = tail_after.clone();
        sorted_after.sort();
        assert_eq!(sorted_after, sorted_before, "shuffle changed the tail's items");
        assert_ne!(
            tail_after, tail_before,
            "seed 7 produced the identity order; pick another seed"
        );
    }

    #[test]
    fn set_shuffle_flag_and_tail() {
        let mut q = queue(8);
        q.next();
        let head: Vec<u64> = q.items()[..=1].iter().map(|i| i.track_id).collect();
        q.set_shuffle(true);
        assert!(q.shuffle());
        assert_eq!(q.items()[..=1].iter().map(|i| i.track_id).collect::<Vec<_>>(), head);
        assert_eq!(q.current().unwrap().track_id, 2);
    }

    #[test]
    fn disabling_shuffle_keeps_the_order() {
        use rand::SeedableRng;
        let mut q = queue(6);
        q.shuffle_tail_with(&mut rand::rngs::StdRng::seed_from_u64(3));
        let shuffled: Vec<u64> = q.items().iter().map(|i| i.track_id).collect();
        q.set_shuffle(false);
        assert!(!q.shuffle());
        assert_eq!(q.items().iter().map(|i| i.track_id).collect::<Vec<_>>(), shuffled);
    }

    #[test]
    fn shuffle_without_a_current_track_shuffles_everything() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.push((1..=8).map(item).collect());
        q.shuffle_tail_with(&mut rand::rngs::StdRng::seed_from_u64(5));
        let mut ids: Vec<u64> = q.items().iter().map(|i| i.track_id).collect();
        ids.sort();
        assert_eq!(ids, (1..=8).collect::<Vec<_>>());
        assert_eq!(q.current_index(), None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio queue`
Expected: compile errors — `shuffle_tail_with` not defined, `rand` crate missing.

- [ ] **Step 3: Add the dependency**

Root `Cargo.toml` `[workspace.dependencies]`:

```toml
rand = "0.10"
```

`crates/oxitide-audio/Cargo.toml` `[dependencies]`:

```toml
rand.workspace = true
```

- [ ] **Step 4: Implement the shuffle**

In `crates/oxitide-audio/src/queue.rs`:

```rust
use rand::seq::SliceRandom;
use rand::Rng;
```

Replace `set_shuffle` and add the helper:

```rust
    /// Enabling shuffles the items after the current index once; disabling
    /// leaves the shuffled order in place. The current item never moves.
    pub fn set_shuffle(&mut self, on: bool) {
        if on {
            self.shuffle_tail_with(&mut rand::rng());
        }
        self.shuffle = on;
    }

    /// One Fisher–Yates shuffle of the tail. Seeded by tests via `StdRng`.
    fn shuffle_tail_with<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        match self.current {
            Some(current) if current + 1 < self.items.len() => {
                self.items[current + 1..].shuffle(rng);
            }
            Some(_) => {}
            None => self.items.shuffle(rng),
        }
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio queue`
Expected: all queue tests PASS. If `set_shuffle_moves_only_the_tail` fails on `assert_ne!`, change the seed from `7` to `8` and re-run.

- [ ] **Step 6: Run the gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add Cargo.toml Cargo.lock crates/oxitide-audio/Cargo.toml crates/oxitide-audio/src/queue.rs
git commit -m "feat(audio): shuffle the queue tail once"
```

---

### Task 9: Config, commands, and app wiring

**Milestone:** P4-7 (part 1).

**Files:**
- Modify: `crates/oxitide-app/src/config.rs`
- Modify: `crates/oxitide-app/src/player.rs`
- Modify: `crates/oxitide-app/src/state.rs`
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs`

**Interfaces:**
- Consumes: Tasks 3, 4, 6 (`DiskCache`, `Engine::new`, `Engine::set_allow_fallback`).
- Produces (used by Task 10):
  - `Config { audio, streaming: { quality, allow_fallback }, ui: { quality_display }, cache: { dir, max_size_mb, clear_on_exit } }` with `#[serde(default)]` everywhere.
  - `config::QualityDisplay { Compact, Detailed }` (serializes lowercase).
  - `config::default_cache_dir() -> Option<PathBuf>`.
  - `player::build_cache(&Config) -> Arc<DiskCache>`.
  - `AppState::{set_cache, cache}`.
  - Commands: `get_cache_status`, `clear_cache`, `set_allow_fallback`, `set_quality_display`, `set_cache_max_size_mb`, `set_cache_clear_on_exit`.
  - `RunEvent::Exit` clears the cache when `cache.clear_on_exit`.

- [ ] **Step 1: Write the failing config tests**

In `crates/oxitide-app/src/config.rs` tests, update `save_and_load_round_trip` and `partial_file_fills_defaults`, and add the round-trip test:

```rust
    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let config = Config {
            audio: AudioConfig {
                output_device: "dev-1".into(),
                exclusive: true,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
                volume: 0.42,
            },
            streaming: StreamingConfig { quality: QualityDto::Max, allow_fallback: false },
            ui: UiConfig { quality_display: QualityDisplay::Detailed },
            cache: CacheConfig {
                dir: "D:\\cache".into(),
                max_size_mb: 512,
                clear_on_exit: true,
            },
        };
        config.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), config);
        assert!(!dir.join("config.toml.tmp").exists(), "temp file left behind");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[audio]\nexclusive = true\n").unwrap();
        let config = Config::load_from(&path);
        assert!(config.audio.exclusive);
        assert_eq!(config.audio.output_device, "");
        assert_eq!(config.audio.exclusive_fallback, ExclusiveFallback::Shared);
        assert_eq!(config.audio.volume, 1.0);
        assert_eq!(config.streaming.quality, QualityDto::Lossless);
        assert!(config.streaming.allow_fallback);
        assert_eq!(config.ui.quality_display, QualityDisplay::Compact);
        assert_eq!(config.cache.dir, "");
        assert_eq!(config.cache.max_size_mb, 2048);
        assert!(!config.cache.clear_on_exit);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn new_sections_round_trip_as_documented_toml() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.streaming.allow_fallback = false;
        config.ui.quality_display = QualityDisplay::Detailed;
        config.cache.max_size_mb = 1024;
        config.cache.clear_on_exit = true;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[streaming]"));
        assert!(text.contains("allow_fallback = false"));
        assert!(text.contains("quality_display = \"detailed\""));
        assert!(text.contains("[cache]"));
        assert!(text.contains("max_size_mb = 1024"));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app config`
Expected: compile errors — `allow_fallback`, `ui`, `cache`, `QualityDisplay`.

- [ ] **Step 3: Implement the config**

In `crates/oxitide-app/src/config.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub audio: AudioConfig,
    pub streaming: StreamingConfig,
    pub ui: UiConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamingConfig {
    pub quality: QualityDto,
    /// Play the best available quality and report it instead of erroring.
    pub allow_fallback: bool,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self { quality: QualityDto::Lossless, allow_fallback: true }
    }
}

/// How the player bar renders the stream quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityDisplay {
    Compact,
    Detailed,
}

impl Default for QualityDisplay {
    fn default() -> Self {
        Self::Compact
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub quality_display: QualityDisplay,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { quality_display: QualityDisplay::Compact }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Empty string means `%APPDATA%\OxiTide\cache`.
    pub dir: String,
    pub max_size_mb: u64,
    pub clear_on_exit: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self { dir: String::new(), max_size_mb: 2048, clear_on_exit: false }
    }
}

/// `%APPDATA%\OxiTide\cache`.
pub fn default_cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide").map(|dirs| dirs.config_dir().join("cache"))
}
```

(Remove the `default_cache_dir` placeholder added by Task 4 and keep the one above.)

- [ ] **Step 4: Implement the cache builder and state handle**

In `crates/oxitide-app/src/player.rs`:

```rust
use std::sync::Arc;

use oxitide_audio::cache::DiskCache;

use crate::config::{default_cache_dir, Config};

/// Build the disk cache from config: the configured directory when usable,
/// else the default, else a temp directory. `DiskCache::new` is infallible.
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

In `crates/oxitide-app/src/state.rs`:

```rust
use oxitide_audio::cache::DiskCache;

#[derive(Default)]
pub struct AppState {
    pub pending_pkce: Mutex<Option<Pkce>>,
    pub client: RwLock<Option<TidalClient>>,
    pub player: RwLock<Option<Arc<Player>>>,
    pub config: Mutex<Config>,
    pub cache: Mutex<Option<Arc<DiskCache>>>,
}

impl AppState {
    // ... existing methods ...

    pub fn set_cache(&self, cache: Arc<DiskCache>) {
        *self.cache.lock().unwrap() = Some(cache);
    }

    pub fn cache(&self) -> Option<Arc<DiskCache>> {
        self.cache.lock().unwrap().clone()
    }
}
```

- [ ] **Step 5: Implement the commands**

In `crates/oxitide-app/src/commands.rs`:

```rust
use oxitide_audio::cache::DiskCache;

use crate::config::{Config, QualityDisplay};

#[derive(Serialize)]
pub struct CacheStatusDto {
    pub dir: String,
    pub used_bytes: u64,
    pub max_bytes: u64,
    pub usable: bool,
}

fn require_cache(state: &State<'_, AppState>) -> Result<Arc<DiskCache>, String> {
    state.cache().ok_or_else(|| "cache unavailable".to_string())
}

#[tauri::command]
pub fn get_cache_status(state: State<'_, AppState>) -> Result<CacheStatusDto, String> {
    let cache = require_cache(&state)?;
    Ok(CacheStatusDto {
        dir: cache.dir().display().to_string(),
        used_bytes: cache.size_bytes(),
        max_bytes: cache.max_bytes(),
        usable: cache.usable(),
    })
}

#[tauri::command]
pub fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    require_cache(&state)?.clear();
    Ok(())
}

#[tauri::command]
pub fn set_allow_fallback(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_allow_fallback(enabled);
    let mut config = state.config();
    config.streaming.allow_fallback = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_quality_display(state: State<'_, AppState>, mode: QualityDisplay) -> Result<(), String> {
    let mut config = state.config();
    config.ui.quality_display = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_cache_max_size_mb(state: State<'_, AppState>, mb: u64) -> Result<(), String> {
    require_cache(&state)?.set_max_bytes(mb.saturating_mul(1024 * 1024));
    let mut config = state.config();
    config.cache.max_size_mb = mb;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_cache_clear_on_exit(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.cache.clear_on_exit = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

Update `build_player` to take the cache and use the configured fallback:

```rust
fn build_player(
    app: &tauri::AppHandle,
    client: TidalClient,
    config: &Config,
    cache: Arc<DiskCache>,
) -> Result<Player, String> {
    let resolver = crate::player::TidalResolver::new(client).map_err(|e| e.to_string())?;
    let engine = Engine::new(
        Arc::new(resolver),
        || Arc::new(HttpFetcher::default()),
        crate::player::real_output(),
        config.streaming.quality.to_quality(),
        cache,
        config.streaming.allow_fallback,
    );
    // ... unchanged ...
}
```

In `auth_complete` and `auth_restore`, replace the Task 4 cache construction with:

```rust
            let config = Config::load();
            state.set_config(config.clone());
            let cache = state
                .cache()
                .unwrap_or_else(|| crate::player::build_cache(&config));
            state.set_player(build_player(&app, client, &config, cache)?);
```

- [ ] **Step 6: Wire `main.rs`**

```rust
mod commands;
mod config;
mod player;
mod state;

use tauri::Manager;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = config::Config::load();
    let state = AppState::default();
    state.set_cache(player::build_cache(&config));
    state.set_config(config);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // ... existing commands ...
            commands::get_cache_status,
            commands::clear_cache,
            commands::set_allow_fallback,
            commands::set_quality_display,
            commands::set_cache_max_size_mb,
            commands::set_cache_clear_on_exit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OxiTide")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                let config = state.config();
                if config.cache.clear_on_exit {
                    if let Some(cache) = state.cache() {
                        cache.clear();
                    }
                }
            }
        });
}
```

- [ ] **Step 7: Run the tests and gates**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: all green.

- [ ] **Step 8: Commit**

```powershell
git add crates/oxitide-app/src/config.rs crates/oxitide-app/src/player.rs crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add cache and streaming settings"
```

---

### Task 10: UI — quality formatter, shuffle/repeat, Settings

**Milestone:** P4-7 (part 2).

**Files:**
- Create: `ui/src/lib/quality.ts`
- Create: `ui/src/lib/quality.test.ts`
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/player.svelte.ts`
- Modify: `ui/src/lib/player.test.ts`
- Modify: `ui/src/lib/components/PlayerBar.svelte`
- Modify: `ui/src/views/Settings.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: Task 9 (commands/config), Task 6 (`PlaybackState.stream`).
- Produces: `formatQuality(meta, mode, preferred) -> string`; `QualityDisplay`; `StreamMeta`, `CacheStatus`; `PlayerState.stream`; player-bar controls; settings sections.

- [ ] **Step 1: Write the failing formatter tests**

Create `ui/src/lib/quality.ts` with only the type export (implementation comes in Step 3), and `ui/src/lib/quality.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { formatQuality } from "./quality";
import type { StreamMeta } from "./api";

function meta(over: Partial<StreamMeta> = {}): StreamMeta {
  return {
    quality: "lossless",
    codec: "flac",
    sample_rate: 44100,
    bit_depth: 16,
    channels: 2,
    bitrate_kbps: 1000,
    ...over,
  };
}

describe("formatQuality compact", () => {
  it("labels the served quality", () => {
    expect(formatQuality(meta({ quality: "max" }), "compact", "lossless")).toBe("MAX");
    expect(formatQuality(meta({ quality: "lossless" }), "compact", "max")).toBe("LOSSLESS");
    expect(formatQuality(meta({ quality: "high" }), "compact", "max")).toBe("AAC");
    expect(formatQuality(meta({ quality: "low" }), "compact", "max")).toBe("AAC");
  });

  it("falls back to the preferred quality before streaming starts", () => {
    expect(formatQuality(null, "compact", "max")).toBe("MAX");
    expect(formatQuality(undefined, "compact", "high")).toBe("HIGH");
    expect(formatQuality(meta({ quality: null }), "compact", "lossless")).toBe("LOSSLESS");
  });
});

describe("formatQuality detailed", () => {
  it("shows the exact format", () => {
    expect(
      formatQuality(meta({ bit_depth: 24, sample_rate: 96000, bitrate_kbps: 2304 }), "detailed", "max"),
    ).toBe("24-bit / 96 kHz / 2304 kbps");
  });

  it("dashes missing segments", () => {
    expect(formatQuality(meta({ bitrate_kbps: null }), "detailed", "max")).toBe(
      "16-bit / 44.1 kHz / —",
    );
  });

  it("falls back to the preferred quality without stream metadata", () => {
    expect(formatQuality(null, "detailed", "lossless")).toBe("LOSSLESS");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/quality.test.ts`
Expected: FAIL — `formatQuality` is not exported.

- [ ] **Step 3: Implement the formatter**

Replace `ui/src/lib/quality.ts`:

```ts
import type { Quality, StreamMeta } from "./api";

export type QualityDisplay = "compact" | "detailed";

function preferredLabel(quality: Quality): string {
  switch (quality) {
    case "max":
      return "MAX";
    case "lossless":
      return "LOSSLESS";
    case "low":
      return "LOW";
    case "high":
      return "HIGH";
  }
}

function compactLabel(quality: Quality): string {
  switch (quality) {
    case "max":
      return "MAX";
    case "lossless":
      return "LOSSLESS";
    case "low":
    case "high":
      return "AAC";
  }
}

function formatRate(rate: number): string {
  const khz = rate / 1000;
  return `${Number.isInteger(khz) ? khz.toString() : khz.toFixed(1)} kHz`;
}

/** Compact (`MAX`/`LOSSLESS`/`AAC`) or detailed (`24-bit / 96 kHz / 2304 kbps`). */
export function formatQuality(
  meta: StreamMeta | null | undefined,
  mode: QualityDisplay,
  preferred: Quality,
): string {
  if (!meta) return preferredLabel(preferred);
  if (mode === "compact") return compactLabel(meta.quality ?? preferred);
  const bits = meta.bit_depth > 0 ? `${meta.bit_depth}-bit` : "—";
  const rate = meta.sample_rate > 0 ? formatRate(meta.sample_rate) : "—";
  const kbps = meta.bitrate_kbps ? `${meta.bitrate_kbps} kbps` : "—";
  return `${bits} / ${rate} / ${kbps}`;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/quality.test.ts`
Expected: PASS.

- [ ] **Step 5: Update `api.ts`**

```ts
export type Quality = "low" | "high" | "lossless" | "max";

export interface StreamMeta {
  quality?: Quality | null;
  codec?: string | null;
  sample_rate: number;
  bit_depth: number;
  channels: number;
  bitrate_kbps?: number | null;
}

export interface CacheStatus {
  dir: string;
  used_bytes: number;
  max_bytes: number;
  usable: boolean;
}
```

In `PlaybackState`, change `preferred_quality: "low" | "high" | "lossless" | "max";` to `preferred_quality: Quality;` and add `stream?: StreamMeta | null;`.

In `Config`:

```ts
export interface Config {
  audio: {
    output_device: string;
    exclusive: boolean;
    exclusive_fallback: "shared" | "warn_only";
    volume: number;
  };
  streaming: { quality: Quality; allow_fallback: boolean };
  ui: { quality_display: "compact" | "detailed" };
  cache: { dir: string; max_size_mb: number; clear_on_exit: boolean };
}
```

Add to the `api` object:

```ts
  setAllowFallback: (enabled: boolean) => invoke<void>("set_allow_fallback", { enabled }),
  setQualityDisplay: (mode: "compact" | "detailed") =>
    invoke<void>("set_quality_display", { mode }),
  setCacheMaxSizeMb: (mb: number) => invoke<void>("set_cache_max_size_mb", { mb }),
  setCacheClearOnExit: (enabled: boolean) =>
    invoke<void>("set_cache_clear_on_exit", { enabled }),
  getCacheStatus: () => invoke<CacheStatus>("get_cache_status"),
  clearCache: () => invoke<void>("clear_cache"),
```

- [ ] **Step 6: Update the player store and its test**

In `ui/src/lib/player.svelte.ts`:

```ts
import { api, type EngineEvent, type PlaybackState, type QueueItem, type RepeatMode, type StreamMeta } from "./api";

export interface PlayerState {
  // ... existing fields ...
  queue: QueueItem[];
  stream: StreamMeta | null;
  error: string;
}

export function emptyState(): PlayerState {
  return {
    // ... existing fields ...
    queue: [], stream: null, error: "",
  };
}
```

In `ui/src/lib/player.test.ts`, add `stream: null` to the state-event fixture in the first test and add:

```ts
  it("mirrors the stream metadata", () => {
    const next = reduce(emptyState(), {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 1, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      queue: [],
      stream: { quality: "lossless", codec: "flac", sample_rate: 44100, bit_depth: 16, channels: 2, bitrate_kbps: 1000 },
    });
    expect(next.stream?.sample_rate).toBe(44100);
  });
```

- [ ] **Step 7: Update `PlayerBar.svelte`**

```svelte
<script lang="ts">
  import { formatDuration } from "../format";
  import { formatQuality, type QualityDisplay } from "../quality";
  import type { RepeatMode } from "../api";
  import type { Player } from "../player.svelte";
  let { player, qualityDisplay }: { player: Player; qualityDisplay: QualityDisplay } = $props();

  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const quality = $derived(
    formatQuality(player.state.stream, qualityDisplay, player.state.preferred_quality),
  );

  function nextRepeat(mode: RepeatMode): RepeatMode {
    if (mode === "off") return "all";
    if (mode === "all") return "one";
    return "off";
  }
</script>
```

Inside the `{#if player.state.current}` block, after the next button:

```svelte
    <button
      class:active={player.state.shuffle}
      onclick={() => player.setShuffle(!player.state.shuffle)}
      aria-label="Shuffle"
      title="Shuffle the rest of the queue"
    >🔀</button>
    <button
      class:active={player.state.repeat !== "off"}
      onclick={() => player.setRepeat(nextRepeat(player.state.repeat))}
      aria-label="Repeat"
      title={player.state.repeat === "one" ? "Repeat one" : player.state.repeat === "all" ? "Repeat all" : "Repeat off"}
    >{player.state.repeat === "one" ? "🔂" : "🔁"}</button>
```

Replace the badge line with:

```svelte
      <span class="badge" title={player.state.stream ? "Actual stream quality" : "Preferred quality"}>{quality}</span>
```

Add a style block at the end of the file:

```svelte
<style>
  .active { color: var(--accent); }
</style>
```

- [ ] **Step 8: Update `Settings.svelte`**

Extend the script:

```svelte
  import { api, type CacheStatus, type Config, type OutputDevice } from "../lib/api";
  import type { QualityDisplay } from "../lib/quality";
  import type { Player } from "../lib/player.svelte";
  let {
    player,
    onQualityDisplay,
  }: { player: Player; onQualityDisplay: (mode: QualityDisplay) => void } = $props();
  let devices = $state<OutputDevice[]>([]);
  let config = $state<Config | null>(null);
  let cacheStatus = $state<CacheStatus | null>(null);
  let error = $state("");

  function refreshCacheStatus() {
    api.getCacheStatus().then((status) => (cacheStatus = status)).catch(apply);
  }

  $effect(() => {
    Promise.all([api.getConfig(), api.getOutputDevices(), api.getCacheStatus()])
      .then(([loaded, list, status]) => {
        devices = list;
        cacheStatus = status;
        // ... existing stored-device fallback ...
        config = next;
      })
      .catch(apply);
  });

  function setAllowFallback(enabled: boolean) {
    if (!config) return;
    config = { ...config, streaming: { ...config.streaming, allow_fallback: enabled } };
    api.setAllowFallback(enabled).catch(apply);
  }

  function setDisplay(mode: QualityDisplay) {
    if (!config) return;
    config = { ...config, ui: { ...config.ui, quality_display: mode } };
    onQualityDisplay(mode);
    api.setQualityDisplay(mode).catch(apply);
  }

  function setCacheMaxMb(mb: number) {
    if (!config) return;
    const clamped = Math.max(0, Math.floor(mb));
    config = { ...config, cache: { ...config.cache, max_size_mb: clamped } };
    api.setCacheMaxSizeMb(clamped).then(refreshCacheStatus).catch(apply);
  }

  function setClearOnExit(enabled: boolean) {
    if (!config) return;
    config = { ...config, cache: { ...config.cache, clear_on_exit: enabled } };
    api.setCacheClearOnExit(enabled).catch(apply);
  }

  function clearCache() {
    api.clearCache().then(refreshCacheStatus).catch(apply);
  }

  function formatMb(bytes: number): string {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
```

Add the sections after the streaming-quality select:

```svelte
  <p>
    <label>
      <input
        type="checkbox"
        checked={config.streaming.allow_fallback}
        onchange={(e) => setAllowFallback((e.target as HTMLInputElement).checked)}
      />
      Play the best available quality and report it
    </label>
  </p>

  <h3>Display</h3>
  <p>
    <label>
      Quality display:
      <select
        value={config.ui.quality_display}
        onchange={(e) => setDisplay((e.target as HTMLSelectElement).value as QualityDisplay)}
      >
        <option value="compact">Compact (MAX / LOSSLESS / AAC)</option>
        <option value="detailed">Detailed (bit depth / rate / bitrate)</option>
      </select>
    </label>
  </p>

  <h3>Cache</h3>
  {#if cacheStatus}
    {#if !cacheStatus.usable}
      <p style="color:#e0a030">
        Cache is disabled: the directory is not writable. Streaming is unaffected.
      </p>
    {/if}
    <p class="muted">{formatMb(cacheStatus.used_bytes)} of {formatMb(cacheStatus.max_bytes)} used</p>
    <p>
      <label>
        Max size (MB):
        <input
          type="number"
          min="0"
          value={config.cache.max_size_mb}
          onchange={(e) => setCacheMaxMb(Number((e.target as HTMLInputElement).value))}
        />
      </label>
    </p>
    <p>
      <label>
        <input
          type="checkbox"
          checked={config.cache.clear_on_exit}
          onchange={(e) => setClearOnExit((e.target as HTMLInputElement).checked)}
        />
        Clear cache on exit
      </label>
    </p>
    <p><button onclick={clearCache}>Clear cache</button></p>
    <p class="muted">Directory: {cacheStatus.dir} (edit cache.dir in config.toml)</p>
  {/if}
```

- [ ] **Step 9: Update `App.svelte`**

```svelte
  import { createPlayer } from "./lib/player.svelte";
  import { api } from "./lib/api";
  import type { QualityDisplay } from "./lib/quality";

  const nav = createNav();
  const player = createPlayer();
  let qualityDisplay = $state<QualityDisplay>("compact");

  $effect(() => {
    if (loggedIn) {
      api
        .getConfig()
        .then((config) => (qualityDisplay = config.ui.quality_display))
        .catch(() => {});
    }
  });
```

And in the markup:

```svelte
      {:else if nav.route.name === "settings"}<Settings {player} onQualityDisplay={(mode) => (qualityDisplay = mode)} />{/if}
    </main>
    {#if nav.queueOpen}<aside class="queue"><QueueRail {player} /></aside>{/if}
    <PlayerBar {player} {qualityDisplay} />
```

- [ ] **Step 10: Run the UI gates**

```powershell
npm --prefix ui test
npm --prefix ui run check
```

Expected: all tests PASS; svelte-check reports 0 errors.

- [ ] **Step 11: Commit**

```powershell
git add ui/src/lib/quality.ts ui/src/lib/quality.test.ts ui/src/lib/api.ts ui/src/lib/player.svelte.ts ui/src/lib/player.test.ts ui/src/lib/components/PlayerBar.svelte ui/src/views/Settings.svelte ui/src/App.svelte
git commit -m "feat(ui): quality display, shuffle/repeat, and cache settings"
```

---

### Task 11: Verification

**Milestone:** P4-8.

**Files:**
- Create: `docs/superpowers/plans/notes/plan4-smoke.md`
- Modify (only if a smoke run surfaces a real bug): whatever file the bug lives in

**Interfaces:**
- Consumes: the shipped Plan 4 build.
- Produces: recorded automated results and a human checklist.

- [ ] **Step 1: Run the automated gates and record exact results**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Note the counts (Rust tests per crate, UI tests per file). If anything fails, fix it first via `superpowers:systematic-debugging` (failing test first) and commit the fix separately.

- [ ] **Step 2: Run the Plan 4 human checklist**

Start the app with a real account and a real audio device:

```powershell
npm --prefix ui run app:dev
```

Have the human run every item below and report pass/fail:

```
- [ ] Play an album once, then replay it: the second play fetches no segments (cache hits).
- [ ] The cache directory grows while playing and never exceeds the configured cap.
- [ ] Settings → Cache shows used/cap, and Clear cache empties the directory.
- [ ] Clear on exit empties the cache when the app closes.
- [ ] With fallback off, a track that cannot be served at the requested quality errors with a clear message; the queue is kept.
- [ ] With fallback on, the same track plays and the player bar shows its true quality.
- [ ] A Low or High track plays as AAC.
- [ ] The display toggle switches the badge between compact and detailed.
- [ ] Detailed mode shows bit depth / sample rate / bitrate, with a dash where unknown.
- [ ] Shuffle shuffles the rest of the queue once; the current track keeps playing.
- [ ] Repeat cycles off → all → one from the player bar.
- [ ] Streaming, display, and cache settings survive an app restart.
- [ ] Making the cache directory unwritable (or pointing cache.dir at a file) disables the cache with a Settings notice and playback still works.
```

- [ ] **Step 3: Record the results**

Create `docs/superpowers/plans/notes/plan4-smoke.md`:

```markdown
# Plan 4 Smoke Checklist

> Requires a human, a logged-in TIDAL account, and an audio device. Run with
> `npm --prefix ui run app:dev`.

## Automated checks (tree at <commit>)
- cargo test --workspace: <counts>
- cargo clippy --workspace -- -D warnings: <clean/warnings>
- npm --prefix ui test: <counts>
- npm --prefix ui run check: <errors/warnings>

## Manual checks
<the checklist with [x]/[ ] and one note per failure>
```

- [ ] **Step 4: Fix any real bugs**

For each failure, use `superpowers:systematic-debugging`: capture the symptom, write a failing test (a Rust unit test where possible, otherwise a documented manual repro), fix, re-run the gate(s), and commit each fix as its own `fix(...)` commit.

- [ ] **Step 5: Commit**

```powershell
git add docs/superpowers/plans/notes/plan4-smoke.md
git commit -m "test: record plan 4 verification results"
```

If Step 4 produced fixes, commit those first (one commit per fix).

---

## Self-Review

**Spec coverage:**

| Spec section | Task(s) |
|---|---|
| §5.1 disk cache (`CacheKey`, `DiskCache`, LRU, clear, protect, cleanup) | 3 |
| §5.2 playback integration (hit, miss tee, seek not cached, corrupt hit) | 4 |
| §5.2 prefetch | 5 |
| §5.3 quality fallback + actual-quality reporting (`StreamMeta`) | 1, 2, 6 |
| §5.4 AAC | 7 |
| §5.5 shuffle (repeat UI in 10) | 8, 10 |
| §5.6 settings/commands/config/clear-on-exit | 9 |
| §5.7 UI (formatter, PlayerBar, Settings) | 10 |
| §6 config keys | 9 |
| §7 IPC surface | 6, 9 |
| §8 UI changes | 10 |
| §9 error handling table | 3, 4, 6 |
| §10 testing strategy | every task |
| §11 milestones P4-1…P4-8 | 3, 4, 5, 1+2+6, 7, 8, 9+10, 11 |

**Deferred scope untouched:** offline playback/downloads, per-segment cache, explicit quality retry chains, ffmpeg sidecar, cache folder picker, encryption/statistics, DSP/EQ, casting, Atmos, lyrics, Last.fm, Discord RPC, SMTC.

**Placeholder scan:** every step carries exact commands and full code; the only runtime-adjustable values are the seeded shuffle (`7`, with an explicit fallback instruction) and the AAC sample-count bound (with an explicit tighten instruction).

**Type consistency:** `manifest_hash`/`quality`/`bitrate_kbps` are introduced in Task 2 and consumed unchanged in Tasks 4–6; `CacheKey`/`CacheWriter` from Task 3 are consumed in Tasks 4–5; `StreamMeta` in Task 6 matches `StreamMeta` in Task 10's `api.ts`; `QualityDisplay` in Task 9 matches the TS union in Task 10.

**Review Focus → owning tests:** all five items are pinned (Tasks 2, 4, 3, 6, 2+6).

