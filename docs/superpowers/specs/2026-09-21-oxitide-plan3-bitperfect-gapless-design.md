# OxiTide — Plan 3 (Bit-perfect, Gapless & Seek) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-21
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md`
- **Predecessor:** `2026-09-21-oxitide-plan2-playback-design.md`
- **Phase:** Phase 1 (MVP) — this is the third implementation **plan** within Phase 1 and covers
  milestone **M6** (bit-perfect exclusive output, correct sample rates, gapless) plus working
  seek. Plan 4 will cover M7 (prefetch/cache/quality fallback/AAC/full Settings).

---

## 1. Purpose & Outcome

Plan 3 turns the working player into a **bit-perfect daily driver**. It delivers WASAPI
**exclusive** output at the source's native sample rate and bit depth, **gapless** playback
across queue items, a working **seek** bar, **device hot-plug** handling, and the first real
configuration file with a bit-perfect Settings toggle.

Success = the owner can play an album end-to-end in exclusive mode with no gaps, no resampling,
and no float round-trip in the data path, and the player survives unplugging the DAC.

## 2. Scope

### In scope (Plan 3)
- Verification of Plan 2's real playback (human smoke) and fixes for anything it surfaces.
- **Typed PCM path**: decode converts once to the sink's preferred representation
  (f32 for shared, native integer for bit-perfect), preserving exact samples.
- `AudioFormat`/`StreamInfo` carrying sample rate, channels, and bits per sample.
- **WASAPI exclusive** output using polling mode, opened at the source's native rate and bit
  depth; format negotiation with a fallback chain.
- **Endpoint volume** for exclusive mode (via the `windows` crate), with a documented
  software-scaling fallback and a "bit-perfect compromised" notice.
- **Playback worker** restructure: one long-lived worker owning a persistent output.
- **Gapless** chaining across queue items whose `StreamInfo` matches; reopen only on format
  change.
- **Bounded lookahead**: prefetch the next track's first ~10 s of decoded PCM, discarding
  consumed bytes (also retires Plan 2's parked unbounded-buffer finding).
- **Seek**: segment-index-based restart with sample-accurate discard to the requested offset.
- **Device hot-plug**: detect removal, pause with a notice, recover when the device returns or
  on the next user action, falling back to the default device.
- **Minimal config**: `%APPDATA%\OxiTide\config.toml` with `audio.*` and `streaming.quality`,
  plus Settings toggles for bit-perfect and the fallback policy.

### Explicitly deferred (Plan 4 = M7)
- Aggressive prefetch, disk cache, LRU eviction.
- Quality fallback chain (Max → Lossless → High → Low).
- AAC (Low/High) playback.
- `streaming.allow_fallback` and `cache.*` config keys; full Settings UI.
- Shuffle ordering and shuffle/repeat UI (parked from Plan 2).

## 3. Tech Stack Additions

| Concern | Crate | Notes |
|---|---|---|
| Endpoint volume | `windows = "0.62"` | `IMMDeviceEnumerator` → `IActivate` → `IAudioEndpointVolume`; version matches `wasapi`'s `windows` dependency |
| Config file | `toml = "1"` | serde (de)serialization |
| Config path | `directories = "6"` | `%APPDATA%\OxiTide` |

Existing stack (`symphonia`, `rtrb`, `wasapi 0.24`, `tokio`, `reqwest`, `serde`) is reused.

## 4. Repository Layout Changes

```
crates/oxitide-audio/src/
  decode.rs            # + PcmFormat, PcmBuffer, StreamInfo; typed PcmSink
  output.rs            # + OutputMode, ExclusiveFallback, AudioOutputSpec; trait rework
  wasapi_output.rs     # + exclusive polling, native format, endpoint volume, hot-plug
  endpoint_volume.rs   # NEW: windows-crate endpoint volume helper
  engine.rs            # per-track threads -> long-lived playback worker; gapless; seek
  fetch.rs             # + bounded lookahead (discard consumed bytes)
  seek.rs              # NEW: segment-index math + discard planning
crates/oxitide-app/src/
  config.rs            # NEW: config.toml load/save (audio.*, streaming.quality)
  commands.rs          # + set_exclusive, set_exclusive_fallback; seek becomes real
ui/src/
  views/Settings.svelte          # bit-perfect toggle + fallback select, persisted
  lib/components/PlayerBar.svelte # seek slider interactive; bit-perfect badge/notice
  lib/player.svelte.ts, api.ts    # new commands + audio status fields
```

## 5. Architecture

### 5.1 Typed PCM (`decode`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmFormat {
    F32,
    I32 { bits: u16 }, // valid bits, 16 or 24
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub format: PcmFormat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PcmBuffer {
    F32(Vec<f32>),
    I32(Vec<i32>),
}

pub trait PcmSink {
    fn preferred_format(&self) -> PcmFormat;
    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()>;
    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()>;
}
```

- `decode::run` reads `bits_per_sample` (and `sample_format`) from the codec parameters,
  builds `StreamInfo`, calls `on_stream_info` once, and converts every decoded frame **once**
  to `preferred_format()` before calling `on_samples`.
- The I32 path uses symphonia's `copy_to_slice_interleaved::<i32>`, which preserves the exact
  integer samples for FLAC 16/24-bit. The F32 path is unchanged in behavior.
- `bits_per_sample` defaults to the intrinsic container width when the codec omits it
  (16 for 16-bit, 24 for 24-bit), and is clamped to 16/24 for the exclusive container.

### 5.2 Output trait and volume (`output`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode { Shared, Exclusive }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusiveFallback { Shared, WarnOnly }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioOutputSpec {
    pub device_id: Option<String>,
    pub info: StreamInfo,
    pub mode: OutputMode,
    pub exclusive_fallback: ExclusiveFallback,
}

pub trait AudioOutput: Send {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn write(&mut self, buf: &PcmBuffer) -> Result<()>;
    fn set_volume(&mut self, volume: f32) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}
```

- **Volume moves into the output.** Shared mode scales samples; exclusive mode drives endpoint
  volume. `NullOutput` scales and records.
- `NullOutput` gains observable state for tests: `opens: usize`, `stops: usize`,
  `last_spec: Option<AudioOutputSpec>`, and `volumes: Vec<f32>`.
- This is a breaking change to the Plan 2 `AudioOutput`/`PcmSink` traits; every call site is
  updated in the same plan (only `engine.rs`, `wasapi_output.rs`, `output.rs`, `decode.rs` and
  their tests use them).

### 5.3 Playback worker (`engine`)

Replace the per-track `start_playback` thread + `OutputFactory` with **one long-lived
playback worker**:

- The worker owns the `AudioOutput` and the queue cursor; commands mutate queue state under the
  existing `playback_lock` and wake the worker (a `Condvar`/channel).
- For each track it resolves a plan, fetches bytes through the bounded lookahead, decodes, and
  writes PCM to the output.
- **Gapless:** when the next queue item's `StreamInfo` equals the current one, the worker
  continues writing the next track's PCM into the same open output — no `stop`/`open` between
  tracks. When the format differs, it stops and reopens at the boundary.
- **Lookahead:** while track A plays, the worker resolves B and prefetches/decode-buffers its
  first ~10 s of PCM, bounded; if B's format differs, the buffer is used to start B immediately
  after the reopen.
- Generation-based cancellation is retained for `Next`/`Prev`/`Clear`/`Seek`; the worker is
  the single place that touches the output, so `AudioClient` never crosses a thread.
- `EngineCmd`/`EngineEvent`/IPC stay compatible; `PlaybackState` gains
  `output_mode: "shared" | "exclusive"`, `bit_perfect: bool`, and `bit_perfect_notice: Option<String>`.

### 5.4 Exclusive output (`wasapi_output`, `endpoint_volume`)

- Open with `StreamMode::PollingExclusive { buffer_duration_hns, period_hns }` (the wasapi
  docs warn event-driven exclusive can stutter on standard USB drivers; polling supports more
  devices). Format is the source's native rate/channels with `SampleType::Int` and a 32-bit
  container whose valid bits are 16 or 24.
- Negotiate with `is_supported_exclusive_with_quirks`; if the exact format is unsupported:
  - `ExclusiveFallback::Shared` → reopen in shared mode, set `bit_perfect = false`, emit a notice.
  - `ExclusiveFallback::WarnOnly` → stay paused, emit `engine_error` with a clear message.
- Shared mode continues to use `SampleType::Float` with `autoconvert: true`.
- **Endpoint volume:** `endpoint_volume::set_master_volume(device_id, v)` resolves the device
  via the `windows` crate and calls `IAudioEndpointVolume::SetMasterVolumeLevelScalar`. If it
  fails or is unsupported in exclusive mode, fall back to software scaling and set
  `bit_perfect_notice = Some("volume applied in software; bit-perfect compromised")`.
- **Hot-plug:** register `DeviceEventCallbacks`; on device removal the worker pauses and emits
  a notice. Recovery happens when the device returns or on the next user action; if the stored
  device id no longer resolves, fall back to the default device.

### 5.5 Seek (`seek`)

- `seek::target_segment(plan, position_ms) -> SeekTarget { segment_index, discard_samples }`
  computed from the MPD's segment timing (`SegmentTemplate` duration/timescale, or
  `SegmentTimeline` offsets) and the source sample rate.
- The worker abandons the current fetch, starts fetching from `segment_index` (media URLs are
  already an ordered list — no HTTP Range required), decodes, discards `discard_samples` frames,
  then resumes writing.
- Seeking is exact to the sample; `position_ms` is updated from frames written after the seek.

## 6. Config (`%APPDATA%\OxiTide\config.toml`)

```toml
[audio]
output_device = ""          # "" = system default
exclusive = false           # bit-perfect on/off
exclusive_fallback = "shared" # "shared" | "warn_only"

[streaming]
quality = "lossless"        # "low" | "high" | "lossless" | "max"
```

- `oxitide-app::config::Config` with `load()`/`save()`; missing file yields defaults; malformed
  file is backed up to `config.toml.bak` and defaults are used (never crash on bad config).
- Writes are atomic (temp file + rename).
- The engine receives audio settings through commands at startup and on change; it never reads
  the file itself (keeps `oxitide-audio` free of path/config concerns).

## 7. IPC Surface Changes

| Command | Change |
|---|---|
| `seek` | now functional (was a no-op) |
| `set_exclusive` | NEW — `{ enabled: boolean }` |
| `set_exclusive_fallback` | NEW — `{ mode: "shared" \| "warn_only" }` |
| `get_audio_status` | NEW — `{ output_mode, bit_perfect, notice }` |

`PlaybackState` gains `output_mode`, `bit_perfect`, and `bit_perfect_notice`. Existing commands
and events are unchanged.

## 8. UI Changes

- **Settings:** a "Bit-perfect (exclusive mode)" toggle and a fallback select; device and
  quality selects persist to `config.toml`; changing any of them applies immediately.
- **PlayerBar:** the seek slider becomes interactive (drag-to-seek with the existing
  scrub reconciliation); a small "Bit-perfect" badge when active, and an amber notice when a
  notice is present (fallback to shared, or software volume).
- **Store:** `PlaybackState` mirror gains the new fields; `api.ts` gains the new commands.

## 9. Error Handling

| Situation | Behaviour |
|---|---|
| Exclusive format unsupported | fallback per `exclusive_fallback`; notice; never panic |
| Endpoint volume unavailable | software scaling + "bit-perfect compromised" notice |
| Device removed mid-playback | pause, emit notice, keep the queue; recover on return/next action |
| Stored device id no longer exists | fall back to the default device and update the config |
| Seek target beyond track length | clamp to the last segment; no error |
| Malformed `config.toml` | back up and use defaults |
| Exclusive open fails mid-album (format change) | stop, reopen; if reopen fails, fall back per policy |

## 10. Testing Strategy

- **Unit:**
  - PCM conversion: an I32 sink receives exact i32 samples for the 16- and 24-bit fixtures;
    an F32 sink receives f32; `bits_per_sample` is propagated into `StreamInfo`.
  - Exclusive format negotiation: 16/24-bit sources map to the right container/valid bits.
  - Seek math: segment index and discard counts for duration-based and timeline MPDs,
    including clamp-at-end.
  - Bounded lookahead: consumed bytes are released; a long stream does not grow without bound.
  - Config: defaults when missing, round-trip save/load, malformed file backs up and defaults.
- **Engine (NullOutput):**
  - Gapless: two fixture tracks with matching formats produce a continuous sample stream with
    `opens == 1` and `stops == 0` between tracks; mismatched formats produce `opens == 2`.
  - Seek: seeking into a fixture yields samples starting at the requested offset (assert the
    first returned sample index via the sink's frame count).
  - Hot-plug simulation: a failing output surfaces a notice and pauses without panicking.
- **Fixtures:** keep `tone.flac`/`tone.mp4`; add a second tone at a different sample rate
  (e.g. 48 kHz) to exercise the reopen path.
- **Manual checklist:** exclusive opens at the native rate; bit-perfect audible; endpoint
  volume; gapless album; seek; DAC unplug/replug.
- **CI gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui test`, `npm --prefix ui run check`.

## 11. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Task | Exit criteria |
|---|---|---|
| P3-1 | Plan 2 smoke verification | checklist run; any real playback bugs fixed |
| P3-2 | Typed PCM path | `PcmFormat`/`PcmBuffer`/`StreamInfo`; decode tests green |
| P3-3 | Output trait + NullOutput recording | trait rework compiles; NullOutput tests |
| P3-4 | WASAPI exclusive + endpoint volume | exclusive opens at native format; manual check |
| P3-5 | Bounded lookahead | memory bound test |
| P3-6 | Playback worker + gapless | gapless scheduler tests with NullOutput |
| P3-7 | Seek | seek math + engine seek test |
| P3-8 | Device hot-plug | failure path test + manual check |
| P3-9 | Config + Settings | config tests; UI toggles persist |
| P3-10 | Verification | full CI + manual checklist |

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Exclusive mode unsupported on some devices/drivers | negotiation + fallback chain; manual checklist |
| Endpoint volume is a no-op on some exclusive drivers | software fallback + visible notice |
| Worker rewrite destabilises Plan 2 behaviour | keep IPC/events; port generation/lock tests; NullOutput scheduler tests |
| Bit-perfect cannot be verified automatically | exact i32 identity unit test + native-rate open assertion; manual listening |
| Hot-plug notifications flaky | pause/notify is guaranteed; recovery also on next user action |
| `windows` crate version drift with `wasapi` | pin both to 0.62 |

## 13. Decisions Log (Plan 3)

| # | Decision | Rationale |
|---|---|---|
| P3-D1 | Plan 3 = M6 only; M7 deferred to Plan 4. | Keeps each plan independently shippable. |
| P3-D2 | Typed PCM (`PcmBuffer`) end-to-end, one decode pass. | Preserves exact samples for bit-perfect without a second decode. |
| P3-D3 | Volume moves into the output; exclusive uses endpoint volume. | Keeps the data path untouched; slider still works. |
| P3-D4 | `PollingExclusive` default. | Avoids the documented USB-driver stutter of event-driven exclusive; supports more devices. |
| P3-D5 | One long-lived playback worker with a persistent output. | Gapless is impossible with per-track device open/stop. |
| P3-D6 | Seek by segment index, no HTTP Range. | Media URLs are already an ordered list; simplest correct approach. |
| P3-D7 | Bounded lookahead in Plan 3. | Gapless needs next-track PCM; bounding retires Plan 2's parked memory finding. |
| P3-D8 | Minimal `audio.*` config now; Plan 4 extends it. | Bit-perfect needs persistence; full Settings is M7. |

## 14. Out of Scope (tracked for later phases)

- **Plan 4 (Phase 1, M7):** aggressive prefetch, disk cache + LRU, quality fallback, AAC,
  `streaming.allow_fallback`, `cache.*`, full Settings, shuffle/repeat UI.
- **Parent roadmap Phase 2:** playlist/favorites editing, synced lyrics, discovery polish.
- **Parent roadmap Phase 3:** offline downloads, Last.fm, Discord RPC, SMTC/media keys.
- **Parent roadmap Phase 4:** DSP/EQ (opt-in), casting/TIDAL Connect, Dolby Atmos.
