# OxiTide — Phase 1 (MVP) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-21
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md`
- **Phase 0 result:** see parent doc §5b — **stereo streams are DRM-free; no CDM needed.**

---

## 1. Purpose & Outcome

Phase 1 delivers a **daily-driver MVP**: log in with a real TIDAL account, browse the
catalog and your library, and play music at every stereo tier (Low → Max 24-bit/192 kHz)
**faster and more reliably than the official client**, with optional **bit-perfect** output.

Success = the owner can replace the official desktop app for normal listening.

## 2. Scope

### In scope (Phase 1)
- PKCE OAuth2 login; token storage + refresh.
- Home page (TIDAL personal feed rows **plus** library-driven rows).
- Album, Playlist, Artist, Track pages (browse/read).
- Global search (tracks, albums, artists, playlists).
- Playback of Low / High / Lossless / Max (24-bit FLAC) tiers.
- Queue with reorder, plus the right-side **collapsible queue rail**.
- Bottom now-playing bar with transport, seek, volume, quality badge.
- WASAPI **shared** and **exclusive (bit-perfect)** output; device selection.
- Aggressive prefetch + disk cache (performance differentiator); gapless playback.
- Settings: audio output & bit-perfect, streaming quality + fallback, cache/storage.

### Explicitly deferred
Playlist/favorites **editing**, synced lyrics, offline downloads UI, Last.fm / Discord RPC /
SMTC & media keys, DSP/EQ, casting / TIDAL Connect, Dolby Atmos, multiple accounts, theming.

## 3. Tech Stack

- **Shell:** Tauri 2 (WebView2), Rust backend.
- **UI:** Svelte 5 + Vite + TypeScript.
- **Audio engine:** pure Rust behind pluggable traits.
- **Auth/API:** TIDAL reverse-engineered API (community endpoints) with PKCE.

### Candidate dependencies (validate during M1/M2)
| Concern | Candidate | Notes |
|---|---|---|
| Async runtime | `tokio` | network, prefetch |
| HTTP | `reqwest` | rustls or native-tls |
| JSON/XML | `serde`, `serde_json`, `quick-xml`/`dash-mpd` | MPD parse |
| Decode | `symphonia` (+ `symphonia-codec-aac`) | FLAC/AAC; ffmpeg fallback proven |
| Output | `wasapi` (henquist) | shared + exclusive, event-driven |
| Ring buffer | `rtrb` / `ringbuf` | lock-free decode→output |
| Secrets | `keyring` | Windows Credential Manager |
| Config | `toml` + `serde` | `%APPDATA%\OxiTide\config.toml` |
| Errors/logs | `thiserror`, `tracing` | |
| Paths | `directories` | AppData resolution |
| UI virtual list | `@tanstack/svelte-virtual` or similar | large track tables |

## 4. Repository Layout

```
OxiTide/
  Cargo.toml                 # workspace
  rust-toolchain.toml
  crates/
    oxitide-tidal/           # auth + API client
    oxitide-audio/           # playback engine
    oxitide-app/             # Tauri app + IPC + config
  ui/                        # Svelte 5 front end
  docs/superpowers/specs/
  .superpowers/              # gitignored (visual companion)
  .gitignore
```

## 5. Architecture

### 5.1 Components
- **`oxitide-tidal`**: PKCE OAuth2, token refresh, catalog, library, search, playlists,
  lyrics, home feed. Pure async, no UI.
- **`oxitide-audio`**: the engine.
  - `queue` — playback state machine (play/pause/seek/next/prev/shuffle/repeat).
  - `source` — resolve `playbackinfo`, parse **MPD (DASH)** and **BTS** manifests.
  - `fetch` — parallel segment downloader + prefetch buffer + disk cache.
  - `decode` — demux fMP4 (+ bare FLAC) and decode to PCM (`symphonia`).
  - `output` — trait `AudioOutput`; `WasapiOutput` (shared/exclusive).
- **`oxitide-app`**: Tauri commands (UI→Rust), event emitters (Rust→UI), config, keyring,
  wiring.
- **UI (`ui/`)**: views (Home, Album, Playlist, Artist, Search, Library, Settings),
  NowPlaying bar, Queue rail, global stores mirroring engine state.

### 5.2 Pluggable boundaries
`AudioOutput` trait: `enumerate_devices`, `open(device, mode, format)`, `start`,
`write(&[f32|i32])`, `flush`, `stop`. Decode is also swappable so `libmpv`/`ffmpeg` can be
added later without touching the control plane.

### 5.3 State ownership
**Rust owns authoritative playback state.** UI sends commands and renders from events. No
playback logic in the WebView.

## 6. Playback Pipeline

```
UI: play(trackId)
  -> engine.queue.set_current(trackId)
  -> source.resolve(trackId, quality)  [tidal: PKCE token]
       -> MPD (DASH SegmentTemplate) | BTS (direct URL)
  -> fetch: GET init + media segments (ahead N, parallel, capped)
  -> decode: demux fMP4 -> FLAC/AAC frames -> PCM
  -> output: WASAPI shared | exclusive
  -> events: state / position / queue / errors -> UI
```

Quality negotiation: request preferred tier; walk **Max → Lossless → High → Low** until a
playable manifest is returned.

## 7. Buffering & Prefetch (performance differentiator)

- **Look-ahead:** maintain ~30–60 s buffered, adaptively sized from segment size and observed
  throughput; pause fetch when buffer full, resume when low.
- **Two tiers:** RAM ring buffer for near-term audio; disk cache for the whole track.
- **Fast start:** begin playback once init + first segment(s) are decoded; never block on the
  whole track.
- **Parallel fetch:** bounded concurrency (e.g., 4–8 in flight).
- **Gapless:** decode the next queue item ahead; keep output stream continuous when
  sample-format matches; otherwise re-init the device at the track boundary (with exclusive
  mode this is a clean stop/start aligned to the boundary).

## 8. Auth & Security

- **PKCE** authorization code flow (required to unlock Hi-Res). `appMode=android`,
  `redirect_uri=https://tidal.com/android/login/auth`.
- **Redirect capture UX:** preferred approach is an embedded WebView that intercepts
  navigation to the redirect URI and extracts `code` automatically. Fallback: paste-back
  flow (as used in the spike). Decision deferred to M2.
- **Token storage:** `keyring` (Windows Credential Manager). Refresh token persisted; access
  token refreshed on expiry. Never store credentials plaintext on disk.
- **Client credentials:** configurable; default to the community client id used by the spike
  (acceptable for personal + friends). Public release requires revisiting this.

## 9. Error Handling & Resilience

| Situation | Behaviour |
|---|---|
| 401/403 | refresh token; on failure prompt re-auth |
| Quality unavailable | fall back down the tier chain |
| Device disconnect / hot-plug | pause, notify UI, re-init on return |
| Network error | retry w/ backoff; prefetch failure non-fatal while buffer has audio |
| 429 rate limit | honour `Retry-After`; throttle requests |
| Manifest/segment parse error | log, skip, fall back quality |
| App crash | never corrupt cache; cache writes are atomic (temp + rename) |

## 10. Cache

- Disk cache keyed by `trackId + quality + manifestHash`.
- Stores whole-track fMP4 (or segments) under `%APPDATA%\OxiTide\cache` (configurable).
- LRU eviction with a size cap; "clear cache" action in Settings.
- Repeats/stumbles become instant. Phase 3 layers explicit offline downloads on top.

## 11. Config (`%APPDATA%\OxiTide\config.toml`)

Only v1 settings:
- `audio.output_device`, `audio.exclusive` (bit-perfect), `audio.exclusive_fallback`
  (`shared` | `warn_only`).
- `streaming.quality` (`low|high|lossless|max`), `streaming.allow_fallback`.
- `cache.dir`, `cache.max_size_mb`, `cache.clear_on_exit`.

## 12. Testing Strategy

- **Unit:** MPD parsing, segment-URL templating, quality fallback chain, queue operations,
  config load/save, token-refresh logic.
- **Fixture integration:** serve the recorded spike MPD + segments from a local HTTP server →
  run the real pipeline → assert output format (24-bit/192 kHz), channel count, and a PCM
  checksum against an ffmpeg reference.
- **Bit-perfect:** exclusive-mode test asserting device opens at the source's native rate;
  optional WASAPI loopback capture compared to the reference PCM.
- **Frontend:** Vitest component/store tests; `svelte-check`.
- **CI:** `cargo test`, `cargo clippy -- -D warnings`, `svelte-check`, `vite build`.

## 13. Milestones

| # | Milestone | Exit criteria |
|---|---|---|
| M1 | Scaffold | Cargo workspace + Tauri shell + Svelte UI skeleton builds and runs |
| M2 | Auth | PKCE login, tokens stored/refreshed, `sessions` call succeeds |
| M3 | Browse | Home/Library/Search/Album/Playlist/Artist views against live API |
| M4 | First sound | One track plays end-to-end (shared mode) via pure-Rust pipeline |
| M5 | Queue | Queue ops + collapsible rail + now-playing bar fully wired |
| M6 | Bit-perfect | Exclusive mode, device selection, correct sample rates, gapless |
| M7 | Perf | Prefetch + disk cache + quality fallback + settings UI |
| M8 | Polish | Error handling, tests, packaging (MSI/NSIS) |

## 14. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `symphonia` AAC decode gaps | pluggable decoder; ffmpeg fallback (validated in spike) |
| PKCE redirect capture UX | embedded-WebView interception; paste-back fallback |
| Unofficial API drift/breakage | isolate in `oxitide-tidal`; fixture tests; version-tolerant parsing |
| WASAPI exclusive unsupported on some devices | detect, fall back to shared with clear notice |
| ToS exposure | personal-use; no DRM circumvention; no key extraction |

## 15. Out of Scope (tracked for later phases)

Phase 2: playlist/favorites editing, synced lyrics, discovery polish.
Phase 3: offline downloads, Last.fm, Discord RPC, SMTC/media keys.
Phase 4: DSP/EQ (opt-in), casting/TIDAL Connect, Dolby Atmos.
