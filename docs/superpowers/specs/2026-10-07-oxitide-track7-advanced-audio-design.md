# OxiTide — Track 7 Advanced Audio (DSP core)

- **Status:** Done — Plans 7.1–7.2 implemented and owner smoke passed (2026-10-07)
- **Date:** 2026-10-07
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 7; PM-D1, PM-D6);
  `plans/notes/2026-10-01-asio-spike.md`; `plans/notes/2026-10-02-listen-along-assessment.md`
- **Process note:** the owner called Track 7 on 2026-10-07 and chose the **DSP core** scope
  (T7-D1). Track 9.4 (a11y) was planned but not executed; its plan was committed first and
  resumes after Track 7.

---

## 1. Purpose & Outcome

Track 7 gives the engine an opt-in signal-processing stage without weakening the
bit-perfect promise: the listener can level loudness across tracks (ReplayGain), shape the
sound (10-band EQ), and blend track changes (crossfade) — and the UI always says when the
output is no longer bit-perfect and why.

**Outcome:**

- ReplayGain from TIDAL's `playbackinfo` normalizes loudness per track or per album, with
  clipping prevention (§6.1).
- A 10-band graphic EQ with presets and an automatic headroom preamp (§6.2).
- Crossfade of 1–12 s on natural track ends, skipping consecutive same-album tracks by
  default so gapless albums stay gapless (§6.3).
- Every change applies live, with no track restart (T7-D3).
- `PlaybackState.bit_perfect` is false, and `dsp_stages` names the active stages, exactly
  when a stage alters samples (T7-D2).

**Success =** the scripted gates stay green (`cargo test --workspace`,
`cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`,
`npm --prefix ui test`); the Plan 7.1/7.2 smoke checklists pass with a real account; the
identity path (all stages off) stays sample-exact — pinned by an engine test that compares
exclusive `i32` output with the decoded source.

## 2. Scope

**In scope (committed):** DSP family (ReplayGain + EQ), crossfade, their config/commands,
the Settings → Playback "Sound" UI, and the bit-perfect status contract.

**Out of scope (stay parked in the roadmap; unchanged status):**

- Casting / TIDAL Connect (protocol research first), Dolby Atmos (DRM spike first),
  360 Reality Audio (difficulty-gated), ASIO output (maybe), listen-along (after casting).
- Automix (beat/tempo-aligned mixing) — research only (T7-D7).
- Resampling. Crossfade between tracks of different formats degrades to a fade-out (T7-D6).
- Dither. Float→integer conversion in exclusive mode rounds to the 32-bit container; the
  device's valid bits truncate below that (documented, §9).

## 3. Decisions (owner-confirmed 2026-10-07)

| ID | Decision |
|---|---|
| T7-D1 | Scope is the DSP core: ReplayGain, EQ, crossfade. Everything else in roadmap Track 7 stays parked. |
| T7-D2 | **Bit-perfect is decided by reality, not by toggles.** A stage that would leave samples untouched (EQ disabled or flat with 0 dB preamp, ReplayGain off or exactly 0 dB, crossfade off or outside a transition) is skipped entirely, so the exclusive `i32` path stays sample-exact. When any stage alters samples, `bit_perfect = false` and `PlaybackState.dsp_stages` names the stages; the output notice is left alone (an opted-in choice is not a warning) and the UI swaps the Bit-perfect badge for a "DSP" badge listing them. Exclusive mode keeps exclusive device access with DSP on: processing runs in float and is converted back to the left-aligned `i32` container. |
| T7-D3 | DSP settings apply live. A settings version counter is checked once per buffer; EQ coefficients are rebuilt while the filter state is kept (no click, no restart). |
| T7-D4 | Chain order: decode → ReplayGain → EQ (preamp then bands) → crossfade mix → output volume. A final clamp to [-1, 1] protects the integer conversion. |
| T7-D5 | ReplayGain values come from `playbackinfo` (`trackReplayGain`, `trackPeakAmplitude`, `albumReplayGain`, `albumPeakAmplitude`; dB gain and linear peak). Modes `off` / `track` / `album` (album falls back to track). A track without data gets the "untagged" gain (default 0 dB = untouched). Prevent-clipping (default on) caps the linear gain at `1 / peak`. |
| T7-D6 | Crossfade is 0 (off) to 12 s, equal-power (sin/cos). It only applies on a **natural** track end; Next/Prev/seek/new context are cuts. The default `skip_same_album` keeps consecutive same-album tracks gapless. Overlap requires an identical output spec (rate, channels, format); otherwise the tail fades out and the next track starts normally (no resampler). |
| T7-D7 | Automix (beat-aligned) stays research; not part of Track 7's committed scope. |
| T7-D8 | One command, `set_dsp_settings(settings)`, replaces the whole `[dsp]` block; it is persisted in `config.toml` and applied at engine build. EQ presets live in the UI (`lib/dsp.ts`). |

## 4. Research summary

- **Engine shape:** the worker already decodes the next queue item concurrently
  (`start_prefetch`), so crossfade needs no second decoder thread type — the prefetched
  `TrackStream` is the second voice.
- **End detection:** the worker learns a track ended only when the producer channel
  disconnects. Crossfade therefore keeps a **hold-back FIFO**: while crossfade is armed, the
  worker reads ahead from the producer (which decodes faster than real time) until the FIFO
  holds the crossfade length, writing from the FIFO head. At disconnect the FIFO is exactly
  the track's tail. No duration estimate is needed and TTFS is unaffected (writes start with
  an empty FIFO). A slow producer shortens the fade instead of stalling.
- **Formats:** shared WASAPI opens float at the source rate (`autoconvert`), so DSP runs on
  native-rate float. Exclusive opens a 32-bit container with the source's valid bits; float
  results are converted back with `round(x · 2^31)` clamped to `i32`.
- **Filters:** RBJ peaking biquads in `f64` Transposed Direct Form II — `f32` coefficient
  precision is inadequate for a 31 Hz band at 192 kHz. Cost at 10 bands × 2 ch × 192 kHz
  ≈ 4 M biquad steps/s: negligible next to decode.

## 5. Architecture

### 5.1 `oxitide-tidal`

`PlaybackInfo` gains `replay_gain: ReplayGain { track_gain_db, track_peak, album_gain_db,
album_peak }` (all `Option<f32>`), parsed from `playbackinfo`.

### 5.2 `oxitide-audio`

- `source::StreamPlan` gains `replay_gain: ReplayGain` (copied from `PlaybackInfo`).
- New `dsp.rs` (pure, unit-tested): `DspSettings` (`eq`, `replay_gain`, `crossfade`,
  serde), `Biquad`, `Equalizer`, `replay_gain_linear`, `TrackDsp` (per-track chain:
  identity check + in-place `f32` processing + `PcmBuffer` round-trip), and crossfade helpers
  (`equal_power`, `Tail`).
- `engine.rs`: `Inner` gains `dsp: Mutex<DspSettings>`, `dsp_version: AtomicU64`,
  `dsp_stages: Mutex<Vec<&'static str>>`; `Engine::set_dsp(settings)`; `PlaybackState`
  gains `dsp_active: bool` and `dsp_stages`; `build_state` folds the DSP state into
  `bit_perfect`. `StreamMeta` gains `replay_gain_db: Option<f32>` (the gain actually applied).

### 5.3 `oxitide-app` and UI

- `config.rs`: `[dsp]` = `DspSettings` (serde defaults: everything off).
- `commands.rs`: `set_dsp_settings(settings)`; `build_player` applies the stored settings.
- `ui/src/lib/api.ts`: `DspSettings` types + `setDspSettings`; `PlaybackState.dsp_active`.
- `ui/src/lib/dsp.ts`: presets, band labels, auto-preamp math, clamping (vitest).
- `views/Settings.svelte` Playback tab: a "Sound" section — ReplayGain (mode, preamp,
  untagged gain, prevent clipping), EQ (enable, preset, auto preamp, 10 vertical sliders),
  crossfade (duration, skip same album) — with the explicit trade-off text: "Any active
  sound processing turns bit-perfect off."

## 6. Features

### 6.1 ReplayGain (Plan 7.1)

`gain_db = (album or track gain per mode) + preamp_db`, or `untagged_db` without data.
`linear = 10^(gain_db/20)`; with prevent-clipping and a known peak `p > 0`,
`linear = min(linear, 1/p)`. A linear gain within `1e-6` of 1.0 is identity.

### 6.2 Equalizer (Plan 7.1)

Bands at 31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz; gain −12…+12 dB; Q = √2 (one
octave). Bands at or above 0.45 · rate are skipped. Preamp −12…+12 dB; `auto_preamp`
(default on) uses `−max(0, max band gain)` instead of the manual preamp. Enabled with all
bands 0 dB and effective preamp 0 dB is identity.

### 6.3 Crossfade (Plan 7.2)

Armed when `duration_ms > 0` and the next item is known and not excluded by
`skip_same_album` (same non-empty `album` title; artist is ignored because compilation tracks carry per-track artists). A running sleep fade disarms the crossfade so the fade pauses on its own track; the UI fires end-of-track sleep `crossfade_ms` early (`PlaybackState.crossfade_ms`), and history counts a hand-over within `crossfade_ms` of the end as completed. At a natural end, the FIFO tail
(`L ≤ duration` frames) is mixed into the next stream's first `L` frames:
`out = next · sin(πt/2) + tail · cos(πt/2)`, `t = i/L`. `NowPlaying` switches at the start
of the overlap and the position restarts at 0. If the next track needs a different output
spec, the tail is written with a cos fade-out before the reopen. A seek/skip discards the
FIFO with the session.

## 7. Testing and verification

- **Rust unit (dsp.rs):** biquad unity at 0 dB; ±6 dB at the band centre (sine RMS within
  0.3 dB); RG math (modes, fallback, preamp, clip cap); identity detection; `i32` round-trip;
  equal-power curve endpoints and constant power.
- **Rust engine:** identity settings keep exclusive `i32` output sample-exact; RG halves a
  known fixture's amplitude (−6.02 dB); live `set_dsp` flips `bit_perfect` and `dsp_stages`;
  crossfade produces fewer total frames than two tracks played back to back (overlap) and
  blends the boundary; `skip_same_album` keeps the gapless total; Next during playback does
  not crossfade.
- **wiremock:** `playbackinfo` parses the four ReplayGain fields and tolerates their absence.
- **UI vitest:** presets, auto preamp, clamping, settings patch helpers.
- **Smoke (human):** `plans/notes/DONE-2026-10-07-track7-1-dsp-smoke.md`,
  `plans/notes/DONE-2026-10-07-track7-2-crossfade-smoke.md`.

## 8. Performance (PM-D1)

The identity path adds one atomic load and a few branch checks per buffer — no conversion,
no allocation. Active DSP converts and processes each buffer once. The crossfade FIFO holds
at most 12 s of PCM (≈ 18 MB at 192 kHz stereo `f32`; ≈ 4 MB at 44.1 kHz). Budgets in
`scripts/perf/budgets.json` are unchanged; S3 (TTFS) and S4 (gapless advance with crossfade
off) are the regression rows to re-check in the smoke.

## 9. Risks and open items

- TIDAL's ReplayGain reference level is undocumented; values are applied as given. The
  smoke compares loud and quiet masters by ear.
- No dither when exclusive DSP output feeds a 16-valid-bit device (truncation noise at
  −96 dBFS; inaudible in practice).
- Crossfade across a format change is a fade-out, not an overlap (no resampler).
- Crossfade near the end of a slow network stream may shorten.

## 10. Plan sequence and closure

1. **Plan 7.1** — DSP pipeline, ReplayGain, EQ, settings UI (`plans/2026-10-07-track7-1-dsp-replaygain-eq.md`).
2. **Plan 7.2** — Crossfade (`plans/2026-10-07-track7-2-crossfade.md`).

Track 7 closes with `plans/notes/2026-10-07-track7-completion.md`, with an explicit
verified/pending split for the human smoke rows, and a roadmap revision line.
