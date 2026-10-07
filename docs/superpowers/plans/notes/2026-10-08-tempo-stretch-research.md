# Tempo stretch (AutoMix Phase 3) — pure-Rust research (2026-10-08)

- **Status:** Research; no engine change. Recommendation in §4 awaits the owner.
- **Owner:** V
- **Context:** Track 7b beat matching aligns tempos within ±4 % and caps the overlap so the
  drift stays within ±30 ms (spec AM-D4). Stretching removes the drift and widens the range.
  Owner constraint: **pure Rust** (AM-D2).

## 1. What "stretch" means for a transition

| Technique | Pitch | Cost | Typical use |
|---|---|---|---|
| **Varispeed** (resample by the tempo ratio) | moves with tempo (1 % ≈ 17 cents) | trivial — we already resample with `rubato` | DJ decks with keylock off; ±1–3 % nudges |
| **Time-domain SOLA/WSOLA** | preserved | low | speech; drums/bass OK, dense harmony smears |
| **Phase vocoder (phase-locked)** | preserved | medium (FFT) | tonal/polyphonic music; transients need care |
| **Hybrid** (PV for tonal + time-domain for transients/bass) | preserved | medium | high-quality DJ/DAW stretchers |

## 2. Pure-Rust options (checked 2026-10-08)

- **`timestretch` 0.15 (MIT, repo `robmorgan/timestretch-rs`)** — `#![forbid(unsafe_code)]`,
  only external DSP dependency is `rustfft`. Current design: a **windowed-sinc varispeed** tempo
  axis plus a two-band **keylock** (120 Hz Linkwitz-Riley split; SOLA-class correction per
  band) that engages only beyond about ±1–2 % deviation; the phase vocoder is reserved for pitch
  shifting/formants. Built for EDM DJ decks (beat grids, loops), CI-gated against Rubber Band
  references on a public corpus. ~25 stars, 1k+ commits — young but active. Best ready-made
  candidate; needs listening on our genres (dense distorted guitars, orchestras).
- **`wsola` 0.1 (MIT)** — tiny, streaming WSOLA; author positions it for speech. Likely to
  smear dense mixes; a baseline only.
- **`ssstretch` / `signalsmith-stretch`** — C++ (Signalsmith Stretch, MIT) through FFI: excluded
  by the pure-Rust constraint. Its ideas (documented in Signalsmith's ADC22 "Four Ways To Write
  A Pitch-Shifter" example code) are open to a clean-room Rust implementation.
- **Our own** — see §3.

Sources: [timestretch-rs](https://github.com/robmorgan/timestretch-rs),
[timestretch docs](https://docs.rs/crate/timestretch/0.9.0), [wsola](https://docs.rs/crate/wsola/0.1.0),
[ssstretch](https://docs.rs/ssstretch),
[Signalsmith pitch/time example code](https://github.com/Signalsmith-Audio/pitch-time-example-code),
[Signalsmith license (Qt attribution)](https://doc.qt.io/QT-6/qtmultimedia-attribution-signalsmith-stretch.html),
[peak-locked phase vocoder (arXiv 1612.05156)](https://www.arxiv.org/abs/1612.05156v1).

## 3. Building our own

A transition-only stretcher is a much smaller problem than a general one: it runs offline on a
≤ 12 s tail, ratios are close to 1, and the outgoing side is fading out anyway.

1. **Varispeed tail (Phase 3a) — ~50 lines, no new dependency.** Resample the held-back tail by
   `P_in / P_out` with the existing `rubato` path (`Tail::converted` already does offline
   resampling). Beats then stay aligned for the whole overlap (16 beats instead of the
   drift-limited 4–8), and the pitch moves by the tempo ratio on a track that is already fading
   out — inaudible for ±2 % (≈ 35 cents), noticeable near ±4 %. Gate: |Δtempo| ≤ 3 %.
2. **Keylocked tail (Phase 3b) — ~500–800 lines on `rustfft`.** A phase-locked vocoder
   (identity phase locking around spectral peaks, Laroche–Dolson style) with transient detection
   that resets phases on onsets (reusing our spectral-flux envelope), applied to the tail only.
   Bass below ~150 Hz can stay varispeed (it is mostly the kick and the fading bass line).
   Ratios up to ±10–15 % would let rock/J-pop pairs of different tempos beat-match.
3. **Tempo ramp (Phase 3c).** Ramp the ratio across the overlap (outgoing eases into the
   incoming tempo) instead of a fixed ratio — needs a time-varying stretcher (both 3b and
   `timestretch` support streaming ratios).

## 4. Recommendation

1. **Do Phase 3a next** (varispeed tail, ≤ 3 %, behind a fourth toggle "Nudge tempo"). It is
   cheap, pure Rust with what we have, and directly fixes the drift limit the A/B smoke is
   likely to expose on long overlaps.
2. **Run a listening shoot-out before 3b:** an offline example (`cargo run --example
   stretch-ab`) rendering the owner's genre excerpts at 0.94 / 0.97 / 1.03 / 1.06 through
   (a) varispeed, (b) `timestretch` keylock, (c) an in-repo phase-locked vocoder prototype, to
   WAV for blind listening, with CPU time per audio second.
3. Adopt `timestretch` if it wins on our material; otherwise grow the in-repo prototype into
   Phase 3b. Either way the engine change is confined to the tail conversion step.
