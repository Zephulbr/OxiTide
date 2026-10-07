# Track 7.1 — DSP Pipeline, ReplayGain, EQ Implementation Plan

**Goal:** Add an opt-in DSP stage (ReplayGain + 10-band EQ) to the playback worker that applies live, keeps the identity path sample-exact, and reports the bit-perfect trade-off honestly.

**Spec:** `docs/superpowers/specs/2026-10-07-oxitide-track7-advanced-audio-design.md` (§3 decisions, §5 architecture, §6.1–6.2, §7 tests)

## Global Constraints

- HTTP tests use `wiremock`; never hit the live TIDAL API.
- New `#[tauri::command]` fns are listed in `generate_handler![...]`.
- Rust DTOs are the source of truth; `ui/src/lib/api.ts` is kept in sync by hand.
- Svelte 5 runes only; semantic tokens only; Lucide icons via `Icon.svelte`.
- Every task keeps `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, and `npm --prefix ui test` green before its commit.

### Task 1: ReplayGain from `playbackinfo`

- [x] `oxitide-tidal/src/stream.rs`: `ReplayGain` struct (four `Option<f32>`), parsed from `trackReplayGain`/`trackPeakAmplitude`/`albumReplayGain`/`albumPeakAmplitude`; `PlaybackInfo.replay_gain`.
- [x] wiremock tests: fields parsed; absent fields are `None`.
- [x] `oxitide-audio/src/source.rs`: `StreamPlan.replay_gain` copied in both manifest branches; every literal construction updated.

### Task 2: `dsp.rs` (pure)

- [x] `DspSettings` (`EqSettings`, `ReplayGainSettings`, `CrossfadeSettings`) with serde defaults (all off) and clamping (`sanitized`).
- [x] `Biquad` (RBJ peaking, `f64` TDF-II), `Equalizer` (per-channel state, `retune` keeps state).
- [x] `replay_gain_linear(settings, data) -> f32`; `TrackDsp` (`new(info, data)`, `update(settings)`, `is_identity`, `active_stages`, `process(PcmBuffer) -> PcmBuffer`).
- [x] Unit tests from spec §7.

### Task 3: Engine integration

- [x] `Inner`: `dsp`, `dsp_version`, `dsp_stages`; `Engine::set_dsp`; `PlaybackState.dsp_active`; `build_state` folds stages into `bit_perfect`/notice; `StreamMeta.replay_gain_db`.
- [x] Worker: build `TrackDsp` per track from `StreamStart`; re-check version per buffer; process before `output.write`; publish a state event when the active-stage set changes.
- [x] Engine tests: identity sample-exact (exclusive `i32`), RG gain on the fixture, live toggle flips `bit_perfect`.

### Task 4: App config + command

- [x] `config.rs`: `Config.dsp: DspSettings` (`#[serde(default)]`), round-trip test.
- [x] `commands.rs`: `set_dsp_settings`; `build_player` applies config; `main.rs` handler list.

### Task 5: UI

- [x] `api.ts`: `DspSettings` types, `Config.dsp`, `PlaybackState.dsp_active`, `setDspSettings`.
- [x] `lib/dsp.ts` + `lib/dsp.test.ts`: band labels, presets, `effectivePreamp`, `clampDb`, `presetMatching`.
- [x] `views/Settings.svelte` Playback tab "Sound" section; trade-off copy; one labelled horizontal slider row per band (`aria-valuetext` + dB readout; vertical range inputs are not reliable on WebKitGTK).

### Task 6: Verification + smoke checklist

- [x] All four gates; write `plans/notes/DONE-2026-10-07-track7-1-dsp-smoke.md`.
