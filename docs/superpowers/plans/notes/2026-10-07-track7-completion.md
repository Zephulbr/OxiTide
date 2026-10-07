# Track 7 completion note (2026-10-07)

- **Status:** Closed 2026-10-07 — owner smoke passed (7.1 DSP and 7.2 crossfade checklists);
  scripted gates green on Windows and WSL2
- **Owner:** V
- **Scope:** DSP core (T7-D1) — Plans 7.1 (DSP pipeline, ReplayGain, EQ) and 7.2 (crossfade).
  Casting/TIDAL Connect, Dolby Atmos, 360 RA, ASIO, listen-along, and automix stay parked.
- **Authority:** `docs/superpowers/specs/2026-10-07-oxitide-track7-advanced-audio-design.md` §10

## What shipped

- **ReplayGain** — `playbackinfo` `track/albumReplayGain` + peak amplitudes parsed into
  `PlaybackInfo` → `StreamPlan`; modes off/track/album (album → track fallback), preamp,
  untagged gain, prevent clipping (`1/peak` cap). Applied gain shown per track
  (`StreamMeta.replay_gain_db`).
- **EQ** — 10 ISO octave bands (31 Hz–16 kHz), ±12 dB, RBJ peaking biquads in `f64` TDF-II,
  auto/manual preamp, UI presets (Flat, Bass boost/cut, Treble boost, Vocal, Loudness).
- **Crossfade** — 0.5–12 s equal-power on natural ends via a hold-back FIFO and the existing
  prefetch stream; same-album transitions stay gapless by default; a format change fades the
  tail out instead of overlapping; skips/seeks cut; a sleep fade disarms it.
- **Bit-perfect contract (T7-D2)** — identity settings keep exclusive `i32` output
  sample-exact (engine test against the decoded source); any altering stage clears
  `bit_perfect` and is listed in `PlaybackState.dsp_stages`, shown as a **DSP** badge in place
  of Bit-perfect. Exclusive mode keeps the device with DSP on.
- **Live apply (T7-D3)** — `Engine::set_dsp` + version counter; no restart or reopen.
- **Config/UI** — `[dsp]` in `config.toml`, `set_dsp_settings` command, Settings → Playback →
  Sound (debounced sliders), sleep-timer end-of-track lead and history completion aware of
  `PlaybackState.crossfade_ms`.

## Verified (scripted, 2026-10-07)

- Windows: `cargo test --workspace` exit 0; `cargo clippy --workspace -- -D warnings` exit 0;
  `npm --prefix ui run check` 0 errors / 0 warnings; `npm --prefix ui test` 621 passed.
- Linux (WSL2 Ubuntu 22.04 clone, head `84e9e29`): `cargo test --workspace` exit 0 (audio
  231 passed, app 342 passed); `cargo clippy --workspace -- -D warnings` exit 0.
- New coverage: 18 `dsp.rs` unit tests (band gain within 0.3 dB at 44.1/48/192 kHz, RG
  modes/clipping, identity, `i32` round-trip, equal-power, `Tail` mixing/fade-out); engine
  tests for identity sample-exactness, RG scaling + `bit_perfect`, live toggling without a
  reopen, idle stage clearing, crossfade overlap / same-album gapless / opt-out / lone track / format change, and
  sleep-fade disarm (verified to fail without the fix); wiremock `playbackinfo` RG parsing;
  history crossfade completion; UI `dsp.test.ts` and the sleep-timer lead.
- The crossfade engine tests ran 6× back to back without a flake.

## Owner smoke (2026-10-07)

- `plans/notes/DONE-2026-10-07-track7-1-dsp-smoke.md` — passed (including the S3/S4 regression
  rows).
- `plans/notes/DONE-2026-10-07-track7-2-crossfade-smoke.md` — passed.
- Bare-metal Linux stays under the Track 8 exemption.

## Deferred

- **Automix** (beat/tempo-aligned, Apple AutoMix-style) remains research only (T7-D7); the
  equal-power crossfade is the committed transition. Revisit with its own spike → spec cycle.
- Casting/TIDAL Connect, Dolby Atmos, 360 RA, ASIO, and listen-along keep their parked status.

## Known limitations (spec §9)

- TIDAL's ReplayGain reference level is undocumented; values are applied as given.
- No dither when exclusive DSP output feeds a 16-valid-bit device.
- No resampler: crossfade across a sample-rate change is a fade-out, not an overlap.
- Same-album detection compares album titles from the queue item.
