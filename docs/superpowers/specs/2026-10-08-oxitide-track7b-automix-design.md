# OxiTide — Track 7b AutoMix (smart and beat-matched transitions)

- **Status:** Approved scope (owner, 2026-10-08); implementation via Plans 7.3–7.4; Phase 3 research
- **Date:** 2026-10-08
- **Owner:** V
- **Parents:** `2026-10-07-oxitide-track7-advanced-audio-design.md` (T7-D6 crossfade, T7-D7);
  `plans/notes/2026-10-07-automix-spike.md` (phasing and building blocks)

---

## 1. Purpose & Outcome

Make track changes sound deliberate across the owner's library — rock, metal, alternative,
J-pop, shoegaze, symphonic, and more — while keeping every behavior **individually
toggleable** so the owner can A/B test and keep what works.

**Outcome:**

- **Phase 1 — smart transitions (Plan 7.3):** the crossfade skips trailing silence of the
  outgoing track and leading silence of the incoming one, and can overlap tracks of different
  sample rates/channel counts (resampling the held-back tail) instead of fading out.
- **Phase 2 — beat matching (Plan 7.4):** when both sides have a confident beat grid and
  compatible tempos, the overlap is aligned beat-to-beat and sized in whole beats; otherwise the
  transition falls back to the Phase 1 crossfade and says why.
- **A/B instrumentation:** each transition publishes a report (kind, overlap, trims, BPMs,
  confidences, fallback reason) shown in Settings, plus a "jump near the end" control.
- **Phase 3 — tempo stretch:** research and an evaluation path for a pure-Rust time stretcher
  (§7); no engine change in this track.

**Success =** scripted gates green; unit tests pin silence detection, tempo/beat accuracy on
synthetic material, alignment math, and resampling; with every new toggle off, the Track 7
crossfade behavior and tests are unchanged; the owner's A/B smoke (§8) records keep/drop
verdicts per toggle.

## 2. Scope

**In:** silence trim, cross-format overlap (resampling), beat-matched alignment without
stretching, transition reports, Settings toggles + report readout, Phase 3 research.

**Out:** tempo stretching in the engine (Phase 3 implementation), key detection/harmonic
mixing, EQ "bass swap" curves, per-genre presets, cloud analysis.

## 3. Decisions (owner-confirmed 2026-10-08)

| ID | Decision |
|---|---|
| AM-D1 | Phases 1 and 2 ship together; each behavior is its own toggle (`trim_silence`, `resample`, `beat_match`) under Crossfade, **all off by default**, for A/B testing. They only apply while crossfade is on. |
| AM-D2 | **Pure Rust only.** New dependencies: `rustfft` (MIT/Apache-2.0) for onset analysis and `rubato` 0.16 (MIT) for resampling. The beat tracker is written in-repo (spectral-flux onsets → autocorrelation tempo with a log-Gaussian prior → Ellis-style dynamic-programming beat tracking → least-squares grid fit). |
| AM-D3 | Genre robustness comes from **confidence gating, not genre rules**: beat matching requires both grids above a confidence floor and tempos within ±4 % (allowing 2:1 / 1:2 octave relations, common in metal and J-pop half-time sections). Rubato, ambient, shoegaze washes, and symphonic material are expected to fall back to the smart crossfade. |
| AM-D4 | Alignment never stretches (Phase 2): the overlap length is chosen in whole beats (16 → 8 → 4) so the beat drift across the overlap stays within ±30 ms, anchored at the overlap's centre beat. |
| AM-D5 | Analysis runs only on audio already in memory: the held-back tail (FIFO) and an incoming **head buffer** pre-read from the prefetch stream during the outgoing track. No extra network, no disk cache in this track. |
| AM-D6 | Same-album skip (T7-D6) still wins: albums stay gapless regardless of the new toggles. |
| AM-D7 | Phase 3 stays research: evaluate the `timestretch` crate (hybrid phase vocoder + WSOLA, rustfft-only) against an in-repo phase-locked vocoder, by listening, before any engine work. |

## 4. Architecture

### 4.1 Settings (`dsp.rs` `CrossfadeSettings`)

`trim_silence`, `resample`, `beat_match` (`bool`, default `false`, serde defaults). The UI
shows them only while crossfade is on.

### 4.2 Analysis (`oxitide-audio/src/automix.rs`, pure)

- `Clip { samples: &[f64] (interleaved, normalized), channels, rate }`.
- `audible_bounds(clip) -> (start, end)` — first/last frame whose 10 ms window peak exceeds
  −54 dBFS.
- `analyze_beats(clip) -> Option<BeatGrid { bpm, period_s, beats_s: Vec<f64>, confidence }>`:
  mono downmix, box-decimation to ≈11–12 kHz, 512-point Hann STFT at a ≈5.8 ms hop,
  log-magnitude spectral flux, moving-average detrend, autocorrelation over 60–200 BPM with a
  log-Gaussian prior at 120 BPM, parabolic peak refinement, dynamic-programming beat tracking,
  least-squares fit of the beat times. Confidence combines beat salience (envelope at beats vs
  overall) and interval regularity.
- `plan_transition(tail, head, settings) -> TransitionPlan { preroll_frames, mix_frames,
  incoming_skip_frames, report }` (tail frames are in the tail's rate, skip in the head's).

### 4.3 Engine (`engine.rs`)

- FIFO hold-back = crossfade + 6 s when trim or beat match is on (≤ 18 s).
- `TrackStream.pending`: events pre-read from the prefetch stream (Info + samples up to 8 s)
  while the current track plays; `wait_for_info` and sample reads consume `pending` first.
- At a natural end: the tail is planned against the head, the **preroll** (tail before the
  overlap) is written as the outgoing track's own audio before the queue advances, and the rest
  becomes the pending tail.
- At the next track: same output spec → mix after skipping `incoming_skip_frames` (position
  includes the skip); different spec with `resample` on → convert the tail (channels, then
  `rubato` FFT resampling) and overlap after the reopen; otherwise the Track 7 fade-out.
- `PlaybackState.last_transition: Option<TransitionReport>`; transitions are also logged at
  `info` level for A/B notes.

### 4.4 UI

Settings → Playback → Sound → Crossfade: three toggles ("Skip silence at track edges",
"Overlap across sample-rate changes", "Beat-match when tempos fit"), a "Last transition"
readout (kind, overlap, trims, BPMs/confidence, fallback reason), and a "Jump near the end"
button (seeks to 25 s before the end) for quick A/B listening.

## 5. Algorithms (detail)

- **Silence:** threshold −54 dBFS peak per 10 ms window; leading trim capped at 4 s so quiet
  intros (symphonic, shoegaze swells) are not eaten; trailing trim uncapped within the tail.
- **Tempo prior:** log-Gaussian over BPM centred at 120 with σ = 1 octave; candidates outside
  60–200 BPM are ignored. Octave errors are tolerated by the 2:1 / 1:2 matching (AM-D3).
- **Beat tracking:** score(t) = onset(t) + max over p ∈ [t − 2P, t − P/2] of
  score(p) − λ·ln²((t − p)/P), λ = 100; backtrace from the best score in the last period.
- **Confidence:** `clamp((salience − 1.2) / 1.8) · clamp(1 − 6·cv)` where salience = mean
  onset at beats / mean onset, cv = std(IBI) / mean(IBI). Floor 0.35 (tunable after A/B).
- **Alignment:** pick the effective grids (decimating the denser one by 2 for octave
  relations), require |ΔP|/P ≤ 4 %, choose L ∈ {16, 8, 4} with L·P ≤ 1.25 × crossfade and
  L/2·|ΔP| ≤ 30 ms, take the latest outgoing window [b_k, b_k + L·P] ending before the audible
  end, map incoming beat j₀ + L/2 onto outgoing b_{k+L/2}, and derive preroll/skip/overlap.

## 6. Testing

- `automix.rs`: silence bounds; synthetic kick/hat patterns at 90/128/174 BPM with noise →
  BPM within 1 % (or its octave) and beat phase within 12 ms, confidence ≥ 0.5; white noise
  and sustained chords → confidence < 0.35; plans for trim-only, beat-matched (128 vs 127),
  octave (128 vs 64), and fallbacks (128 vs 100, low confidence) with the aligned beats landing
  within 15 ms at the overlap centre.
- `dsp.rs`: tail channel/rate conversion preserves length ratio and a sine's frequency.
- `engine.rs`: toggles off → identical totals to Track 7; trim on a silence-free fixture →
  crossfade report; beat match on a tone → fallback reason; resample across 44.1/48 kHz →
  overlap (fewer total samples than a fade-out) with `resampled` in the report.
- UI vitest for toggle/report formatting.

## 7. Phase 3 — tempo stretch research (no engine change)

Options for a pure-Rust stretcher (ratios 0.94–1.06 cover most DJ-style ramps; up to
0.75–1.5 for wide pairs):

1. **`timestretch` crate** — hybrid phase vocoder (tonal) + WSOLA (transients), rustfft only,
   `forbid(unsafe_code)`; later versions moved to a varispeed + two-band keylock design. Check
   license and the version's algorithm before use.
2. **`wsola` crate** — small, streaming, speech-oriented; likely to smear dense distorted
   guitars and orchestral textures.
3. **In-repo phase-locked vocoder** (peak-locked phases, transient phase reset) — ~500 lines
   on `rustfft`; the approach behind high-quality stretchers (Signalsmith Stretch is MIT and
   its ideas are documented in Signalsmith's ADC22 "Four Ways To Write A Pitch-Shifter"
   example code, so a clean-room Rust take is legally open).

**Evaluation path:** an offline example (`cargo run --example stretch-ab`) that renders the
same excerpts (one per owner genre) through each candidate at 0.96/1.04/1.10 into WAV files for
blind listening, plus CPU per second of audio. The winner gets its own plan; Phase 2's
alignment then relaxes the ±4 % gate to the stretcher's clean range.

## 8. A/B smoke (owner)

For each toggle, listen to ~10 transitions with it on and off across the genres above and
record keep/drop: `plans/notes/2026-10-08-track7b-automix-smoke.md`.

## 9. Risks

- Beat grids on syncopated or live material can be confidently wrong → audible flams; the
  report makes these visible for tuning the floor.
- Head pre-read needs the next track decoded ~8 s ahead; on a slow network the plan falls back.
- Resampling a ≤ 12 s tail runs synchronously at the next track's start (an FFT resampler,
  optimized even in dev builds); the device reopens at that moment anyway.
- Beat analysis takes ~30 ms in release and ~0.6 s in debug builds, so it runs on a helper
  thread while the tail keeps playing; a plan that arrives after its overlap should have started
  degrades to a plain crossfade with a note.
- Timing-sensitive engine tests (e.g. the Track 10 fade-cancel test) can flake under full-suite
  CPU load; they pass in isolation on this branch and on `main`.
