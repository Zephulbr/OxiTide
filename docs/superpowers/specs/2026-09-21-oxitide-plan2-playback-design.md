# OxiTide — Plan 2 (Playback Engine & Queue) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-21
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md`
- **Predecessor:** `2026-09-21-oxitide-phase1-design.md`
- **Phase:** Phase 1 (MVP) — this is the second implementation **plan** within Phase 1 and
  covers milestones **M4 (first sound)** and **M5 (queue)**. (Parent §9's roadmap "Phase 2"
  is a different thing — Library & discovery — and is not this document.)
- **Phase 0 result:** stereo streams (Low/High/Lossless/Max) are plain, DRM-free MPEG-DASH
  fMP4 — no CDM needed (parent §5b).

---

## 1. Purpose & Outcome

Plan 2 makes OxiTide **audible**. It delivers the first end-to-end playback path — resolve a
track's stream, fetch segments, decode to PCM, and play out through WASAPI — plus a
Rust-owned queue with working transport, wired into the existing UI shell.

Success = clicking a track on an Album, Playlist, or Track view plays it, and the now-playing
bar and queue rail behave correctly for normal listening.

Milestones M6–M8 of Phase 1 remain deferred to Plan 3.

## 2. Scope

### In scope (Plan 2)
- `oxitide-tidal`: a `playbackinfo` endpoint returning the stream manifest.
- `oxitide-audio` crate: `source`, `fetch`, `demux`, `decode`, `output`, `queue`, `engine`.
- FLAC playback (Lossless and Max/Hi-Res, 16/44.1 and 24-bit up to 192 kHz).
- WASAPI **shared** output; software volume.
- Queue state machine: ordered items, current index, play/pause/seek/next/prev,
  shuffle, repeat (off/all/one), auto-advance on track end.
- Rust → UI event stream; Tauri commands wrapping engine operations.
- UI: functional `PlayerBar`, functional `QueueRail`, play actions on track tables, and a
  minimal Settings slice (output device + preferred quality).
- Fixture-driven pipeline integration test (generated tone, no live network).

### Explicitly deferred (Plan 3 = M6+M7)
- WASAPI **exclusive** mode / bit-perfect, device hot-plug recovery.
- Gapless playback across queue items.
- Aggressive prefetch buffer and disk cache.
- Quality **fallback** chain (walk Max → Lossless → High → Low on failure).
- Full Settings UI (cache/storage, exclusive fallback policy).
- AAC (Low/High) playback — see §6.4; lands behind the same decoder seam once FLAC is proven.

## 3. Tech Stack Additions

| Concern | Crate | Notes |
|---|---|---|
| XML (MPD) | `quick-xml` | custom minimal parser (§5) |
| Decode | `symphonia` | `flac`, `isomp4`, `pcm` features; FLAC first |
| Output | `wasapi` (henquist) | shared mode; exclusive later |
| Ring buffer | `rtrb` | lock-free decode → output |
| Base64 | `base64` | playbackinfo `manifest` field |

Existing stack (Tokio, reqwest, serde, thiserror, keyring, wiremock) is reused unchanged.

## 4. Repository Layout

```
crates/
  oxitide-tidal/           # + src/stream.rs
  oxitide-audio/           # NEW
    src/lib.rs
    src/error.rs
    src/source.rs          # playbackinfo -> resolved segment plan
    src/mpd.rs             # minimal DASH MPD parser
    src/fetch.rs           # segment downloader
    src/demux.rs           # fMP4 -> packets (symphonia isomp4)
    src/decode.rs          # packets -> PCM (symphonia flac)
    src/output.rs          # AudioOutput trait; WasapiOutput; NullOutput (test)
    src/queue.rs           # queue state machine
    src/engine.rs          # glue + EngineEvent
  oxitide-app/             # + playback commands, event emitter, engine wiring
ui/src/                    # PlayerBar, QueueRail, play actions, settings slice
```

`crates/oxitide-audio` is added to the workspace `members` and must not depend on Tauri or
on the UI.

## 5. Architecture

### 5.1 Component boundaries

- **`oxitide-tidal::stream`** — pure API: given `(track_id, quality)`, call `playbackinfo`,
  return a typed `PlaybackInfo { quality, manifest_mime, manifest_bytes }`. Handles 401
  refresh via the existing client. No manifest interpretation.
- **`source`** — turns `PlaybackInfo` into a `StreamPlan`: an ordered list of absolute URLs
  (one init + N media segments) plus the codec/DASH parameters needed by `demux`. Delegates
  MPD parsing to `mpd`.
- **`mpd`** — minimal DASH parser. Extracts `Representation` attributes
  (`audioSamplingRate`, `bitDepth`, codecs), the `SegmentTemplate` fields
  (`initialization`, `media`, `startNumber`, `duration`, `timescale`), and a
  `SegmentTimeline` when present. Expands `$Number$` (with `startNumber`) and `$Time$`
  placeholders.
- **`fetch`** — downloads the init segment and media segments in order, keeping a small
  bounded lookahead in memory. Emits byte chunks to `demux`. Disk cache is out of scope.
- **`demux`** — accepts the concatenated init+media bytes and yields codec frames. Wraps
  symphonia's ISOMP4 reader behind a `Demuxer` trait so it can be swapped if fragmented-MP4
  support proves inadequate.
- **`decode`** — wraps a symphonia decoder producing interleaved `f32` PCM at the source's
  native sample rate and channel count. Behind a `Decoder` trait.
- **`output`** — `AudioOutput` trait: `enumerate_devices()`, `open(device, mode, format)`,
  `start()`, `write(&[f32])`, `flush()`, `stop()`. `WasapiOutput` in shared mode;
  `NullOutput` for tests/headless.
- **`queue`** — the authoritative playback state machine (§7). Pure logic, no I/O.
- **`engine`** — owns the queue, the output device, and a worker thread; pumps fetch →
  demux → decode → ring buffer → output; publishes `EngineEvent`s and consumes `EngineCmd`s.

### 5.2 State ownership

**Rust owns all playback state.** The UI sends commands and renders events. The renderer
holds only a local mirror of the last state received, and re-syncs once via
`get_playback_state` on mount. No playback logic lives in the WebView (parent D6/D8 carry
over).

### 5.3 Concurrency

- A dedicated **engine worker thread** drives fetch/demux/decode and writes PCM into an
  `rtrb` ring buffer. The WASAPI output thread/event callback drains the buffer.
- Commands arrive on a channel (`EngineCmd`); events leave on an emitter callback
  (`EngineEvent`). The Tauri layer adapts these to `invoke` handlers and `emit`.
- Buffer underrun (empty ring) is not fatal: output writes silence for the missed frames and
  the engine records `EngineEvent` with `buffering: true`.

## 6. Stream Resolution & Pipeline

### 6.1 Playback pipeline

```
UI: play_tracks(ids, index, label)
  -> engine.queue.set_current(index)
  -> source.resolve(trackId, quality)          [oxitide-tidal::stream::playbackinfo]
       -> StreamPlan { init_url, media_urls[], codec, sample_rate, bit_depth, channels }
  -> fetch: GET init, then media segments in order (bounded lookahead)
  -> demux: fMP4 -> codec frames
  -> decode: FLAC frames -> PCM f32
  -> output: WASAPI shared
  -> events: state / now_playing / position (throttled) / queue / volume / error -> UI
```

Playback begins as soon as the init segment and the first media segment have decoded —
never wait for the whole track.

### 6.2 Quality selection

Requests exactly the user's configured preferred quality (`low|high|lossless|max`). The
**fallback chain is deferred to Plan 3**; if the requested quality's manifest cannot be
resolved, the engine surfaces an error and the UI reports it. Max maps to `HI_RES_LOSSLESS`.

### 6.3 Manifest shapes

- **DASH:** `manifestMimeType = application/dash+xml`; the `manifest` field is a base64 of
  the MPD XML. Parsed by `mpd` into a `StreamPlan`.
- **BTS (direct URL):** `manifestMimeType = vnd.tidal.bts`; the `manifest` field is JSON
  containing direct URLs. Treated as the degenerate `StreamPlan` (init is the file itself,
  no segments). If the shape is not as expected, `source` returns a typed error rather than
  panicking.

### 6.4 AAC

Low/High are AAC in fMP4 and are **not** in Plan 2's exit criteria. The `Decoder` seam is
written so adding `symphonia-codec-aac` is a new match arm. If symphonia's AAC proves
inadequate when Plan 3 adds it, that is the trigger to revisit the demux/decode decision
(parent risk table).

## 7. Queue State Machine

Pure, synchronous, fully unit-testable. State:

```
items: Vec<QueueItem>          // track id, title, artist, album, duration, cover
current: Option<usize>
playing: bool
shuffle: bool
repeat: RepeatMode             // Off | All | One
volume: f32                    // 0.0..=1.0
preferred_quality: Quality
```

Operations (each returns the events it caused):

- `play_context(items, start_index)` — replace the queue and start at the index.
- `toggle_play_pause`, `pause`, `resume`.
- `next` / `prev` — respect `shuffle` and `repeat` (`One` repeats the current item on
  auto-advance but is bypassed by explicit `next`).
- `seek(position)`.
- `on_track_ended` — advance per `repeat`/`shuffle`; stop at the end when `repeat == Off`.
- `reorder(from, to)`, `remove(index)`, `clear()`.
- `set_volume`, `set_shuffle`, `set_repeat`.

`current` is adjusted correctly across `reorder`/`remove` so the playing item is preserved.
Shuffle uses a stored permutation so `prev` is consistent.

## 8. IPC Surface

### 8.1 Commands (Tauri `invoke`)

| Command | Args | Result |
|---|---|---|
| `play_tracks` | `trackIds: string[]`, `startIndex: number`, `contextLabel?: string` | `()` |
| `play_pause` | — | `PlaybackState` |
| `next` / `prev` | — | `PlaybackState` |
| `seek` | `positionMs: number` | `()` |
| `set_volume` | `volume: number` (0..1) | `()` |
| `set_shuffle` | `enabled: boolean` | `()` |
| `set_repeat` | `mode: "off" \| "all" \| "one"` | `()` |
| `queue_reorder` | `from: number`, `to: number` | `QueueSnapshot` |
| `queue_remove` | `index: number` | `QueueSnapshot` |
| `queue_clear` | — | `QueueSnapshot` |
| `get_playback_state` | — | `PlaybackState` |
| `get_output_devices` | — | `OutputDevice[]` |
| `set_output_device` | `deviceId: string` | `()` |
| `set_preferred_quality` | `quality: "low" \| "high" \| "lossless" \| "max"` | `()` |

`PlaybackState` is a single snapshot type containing `playing`, `buffering`,
`nowPlaying?`, `positionMs`, `durationMs`, `volume`, `shuffle`, `repeat`,
`preferredQuality`, and `queue: QueueSnapshot`.

### 8.2 Events (Rust → UI)

`playback_state_changed` (snapshot), `now_playing_changed` (track meta),
`position_changed` (throttled to ~250 ms while playing), `queue_changed` (snapshot),
`volume_changed`, `engine_error` (`{ code, message }`). Every event carries the same
`PlaybackState`/snapshot shapes as the getters so the UI has one reducer.

## 9. UI Wiring

- **PlayerBar** — cover, title, artist, play/pause, next/prev, seek slider (with
  drag-vs-event reconciliation), volume slider, quality badge, buffering indicator.
- **QueueRail** — ordered list from `queue_changed`, click-to-jump, drag reorder
  (`queue_reorder`), remove, clear, current-item highlight.
- **TrackTable / Album / Playlist** — a play action on each row and a "play all" header
  action, both calling `play_tracks` with the view's ordered ids.
- **Settings (minimal slice)** — output-device dropdown (`get_output_devices` /
  `set_output_device`) and preferred-quality select (`set_preferred_quality`). Remaining
  settings are deferred to Plan 3.

The UI reducer subscribes to all events and calls `get_playback_state` once on mount.

## 10. Error Handling

| Situation | Behaviour |
|---|---|
| 401/403 during resolve | refresh once and retry (existing client); on failure emit `engine_error` with an auth-required code so the UI can re-prompt login |
| Requested quality unavailable | typed error + `engine_error` (no fallback in Plan 2) |
| Network error mid-stream | retry a bounded number of times; if the buffer cannot be refilled, stop with `engine_error` |
| 429 rate limit | honour `Retry-After` (existing client behaviour) |
| MPD parse error / unexpected manifest shape | typed error, no panic |
| Decode error on a segment | typed error, stop playback, `engine_error` |
| No audio device / device open failure | typed error, `engine_error`; UI shows a notice |
| Ring-buffer underrun | output silence, mark `buffering`, keep going |

Playback errors never panic the app and never leave the worker thread wedged.

## 11. Testing Strategy

- **Unit:** MPD template expansion (`$Number$` incl. `startNumber`, `$Time$`), `source`
  plan construction (DASH + BTS shapes), queue transitions (next/prev at bounds,
  repeat One/All/Off, shuffle stability, reorder/remove preserving current), event
  throttling logic, config load/save of the new settings.
- **Fixture integration (no live network):** a committed ~2 s generated tone (our own
  content) as a standalone FLAC **and** wrapped as fMP4 with a matching MPD. `wiremock`
  serves the `playbackinfo` JSON and the segments; the real pipeline runs through a
  `NullOutput`; the test asserts sample rate, channel count, and a PCM checksum against a
  locally generated reference. This is the M4 exit test.
- **Engine:** commands in → expected event sequence out, using `NullOutput`.
- **UI:** Vitest for the event reducer and queue store; `svelte-check`.
- **CI:** `cargo test`, `cargo clippy -- -D warnings`, `svelte-check`, `vite build`.

## 12. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Milestone | Exit criteria |
|---|---|---|
| P1 | `oxitide-audio` scaffold + `oxitide-tidal::stream` | playbackinfo call + typed shapes, wiremock-tested |
| P2 | `mpd` + `source` | MPD parses to a `StreamPlan` from fixtures |
| P3 | `fetch` + `demux` + `decode` | fixture fMP4 → PCM matching reference; `NullOutput` |
| P4 | `output` | `WasapiOutput` shared mode opens a device and plays PCM |
| P5 | `queue` | state-machine unit tests green |
| P6 | `engine` + Tauri commands/events | commands drive events end-to-end |
| P7 | UI wiring | PlayerBar, QueueRail, play actions, settings slice |
| P8 | Verification | full CI + smoke checklist for a human playback run |

## 13. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| symphonia fragmented-MP4 reader inadequate | `Demuxer`/`Decoder` seams; fixture test fails loudly; fallback options documented (parent risk table) |
| symphonia AAC gaps (Plan 3) | deferred; seam ready; revisit trigger noted (§6.4) |
| WASAPI shared-mode init quirks | `AudioOutput` trait + `NullOutput` keeps logic testable headless |
| Long-running engine thread wedging | bounded retries, typed errors, explicit stop path |
| UI position-slider fighting event updates | drag-vs-event reconciliation called out in §9 |
| Unofficial API drift | all API shapes isolated in `oxitide-tidal`; fixture tests |

## 14. Decisions Log (Plan 2)

| # | Decision | Rationale |
|---|---|---|
| P2-D1 | Plan 2 = M4+M5 only. | Smallest reviewable slice that produces audible music and a usable player. |
| P2-D2 | symphonia-only, FLAC first. | Matches stack; no native deps; AAC added later behind the same seam. |
| P2-D3 | Custom minimal MPD parser (`quick-xml`) over `dash-mpd`. | Tiny surface, full control, fixture-tested. |
| P2-D4 | Generated-tone fixtures committed, served via `wiremock`. | Deterministic, no copyright content, no live network. |
| P2-D5 | `wasapi` crate; decode thread + `rtrb` ring buffer. | One output layer designed once; exclusive mode added in Plan 3. |
| P2-D6 | Rust owns playback state; UI renders events. | Carries parent D6/D8; avoids WebView playback logic. |
| P2-D7 | No quality fallback in Plan 2. | Keeps scope tight; fallback is M7 (Plan 3). |

## 15. Out of Scope (tracked for later phases)

- **Plan 3 (Phase 1, M6+M7):** exclusive/bit-perfect, device hot-plug, gapless, prefetch
  buffer, disk cache, quality fallback, full Settings, AAC.
- **Parent roadmap Phase 2:** playlist/favorites editing, synced lyrics, discovery polish.
- **Parent roadmap Phase 3:** offline downloads, Last.fm, Discord RPC, SMTC/media keys.
- **Parent roadmap Phase 4:** DSP/EQ (opt-in), casting/TIDAL Connect, Dolby Atmos.
