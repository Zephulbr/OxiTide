# AutoMix spike — beat/tempo-aligned transitions (2026-10-07)

- **Status:** Research only (T7-D7); no code. Owner decision needed on the phasing below.
- **Owner:** V
- **Context:** Track 7 shipped an equal-power crossfade (`2026-10-07-oxitide-track7-advanced-audio-design.md`
  §6.3). This note scopes an Apple-AutoMix-style mixer on top of it.

## 1. What AutoMix is (public information)

Apple describes AutoMix (iOS 26 / macOS Tahoe) as analyzing the audio of each song and
building transitions with **time stretching and beat matching**. Press coverage adds that it
looks at **tempo and key** and can ease one track's tempo toward the next. Apple's support text
says it **picks a transition per song**: for some tracks it only trims leading/trailing silence,
for others it uses a plain crossfade, and **albums and some genres play without transitions**.
The algorithm is unpublished. Settings offer AutoMix / Crossfade / Off. Reception is mixed:
some listeners turn it off to keep full endings.

Sources: [9to5Mac](https://9to5mac.com/2025/06/16/automix-apple-music-ios-26/),
[iGeeksBlog](https://www.igeeksblog.com/apple-music-automix-in-ios-26/),
[TechRadar](https://www.techradar.com/audio/apple-music/automix-is-the-apple-music-feature-that-made-me-love-listening-to-music-on-my-iphone-again).

## 2. What OxiTide already has

- A hold-back FIFO that captures the outgoing track's tail and mixes it into the prefetched
  next stream (`engine.rs` `run_session`, `dsp.rs` `Tail`).
- Same-album skip (Apple's "albums play without transitions") and live settings.
- **Gaps:** fixed overlap length, fixed equal-power curve, no silence detection, no resampler
  (a rate change degrades to a fade-out), no analysis of either track.

## 3. Building blocks (Rust ecosystem, checked 2026-10-07)

| Need | Candidate | Notes |
|---|---|---|
| Tempo + beat grid | `stratum-dsp` | Pure Rust, no FFI; tempogram + HMM beat tracking with drift correction; README self-reports 87.7 % within ±2 BPM on 155 DJ tracks (unverified); tuned for 44.1 kHz. |
| Tempo + beat grid | `aubio` / `aubio-rs` | Mature C library bindings; GPL-3.0 (compatible with OxiTide's AGPL-3.0-or-later); `aubio-rs` last released 2021; builds the C library or links a system copy. |
| Beat tracking (neural) | `beat-this` | Port of the JKU Linz neural tracker; pure-Rust `rten` or ONNX Runtime backend; needs model weights; heaviest runtime. |
| Beat grid + stretch | `timestretch` | Exposes `detect_beat_grid` alongside time stretching; maturity unknown. |
| Time stretch | `ssstretch` (Signalsmith Stretch via `cxx`) | Upstream C++ is MIT; quality comparable to Rubber Band; best for 0.75×–1.5×; adds a C++ build step (MSVC on Windows, g++ on Linux). Verify the wrapper crate's own license. |
| Resampling | `rubato` (from memory — verify) | Needed anyway to overlap tracks of different rates. |

Sources: [stratum-dsp](https://gitblind.noratr.app/HLLMR/stratum-dsp),
[aubio-rs](https://docs.rs/crate/aubio-rs/latest), [aubio](https://docs.rs/crate/aubio/latest),
[beat-this](https://docs.rs/crate/beat-this/0.3.0), [timestretch](https://docs.rs/crate/timestretch/0.9.0),
[ssstretch](https://docs.rs/ssstretch),
[Signalsmith Stretch license (Qt attribution)](https://doc.qt.io/QT-6/qtmultimedia-attribution-signalsmith-stretch.html),
[OpenMPT comparison](https://bugs.openmpt.org/view.php?id=1808).

## 4. Engine fit and costs

- **Analysis windows, not whole tracks:** only the outgoing tail (~20–30 s) and the incoming
  head need analysis. The FIFO already holds the tail; the prefetch stream decodes the head
  ahead of time. Results cache per track (`cache/analysis.json`), so a replay costs nothing.
- **Mix-point choice:** with a beat grid, the transition can start on a downbeat and last a
  whole number of bars instead of a fixed time, and trailing silence/fade-outs can be skipped.
- **Tempo matching:** stretching the outgoing tail toward the incoming tempo (or ramping both)
  changes the tail's length; the FIFO model handles that, because the tail is fully buffered.
- **Hi-Res cost:** analysis at 96/192 kHz should downmix + downsample to 22–44 kHz first;
  stretching runs only during the transition (seconds per track).
- **Risk:** wrong beat grids (live recordings, rubato, ambient, classical) sound worse than a
  plain crossfade. Apple sidesteps this by choosing per song; OxiTide needs the same confidence
  gate (fall back to crossfade below a threshold).
- **Measurement:** no ground truth exists in-repo; tuning needs a hand-labelled set of owner
  tracks and the human ear. PM-D1 budgets are unaffected while it is off.

## 5. Recommended phasing

1. **Smart transitions (no beats) — small, high value.** Trim leading/trailing silence by
   level threshold, match the transition to a detected fade-out, and add a resampler so
   rate-mismatched tracks can overlap. Uses only data already in the FIFO/prefetch.
2. **Beat-aligned crossfade — medium.** Beat grid on the tail/head windows (`stratum-dsp`
   first, pure Rust), align downbeats, bar-length overlaps, confidence-gated fallback to the
   plain crossfade. No stretching: only for pairs within ~±3 % tempo.
3. **Tempo ramp — large.** Time-stretch the transition (`ssstretch`/Signalsmith) for pairs
   within 0.75×–1.5×; optional key awareness. Only after Phase 2 proves its grids on the
   owner's library.

**Recommendation:** take Phase 1 when the owner wants transition work again. Gate Phases 2–3
behind a prototype that measures beat-grid accuracy on ~50 owner tracks, and keep a plain
"Crossfade" option beside any "AutoMix" setting, as Apple does.

## 6. Open questions for the owner

- Is Phase 1 (silence trim + rate-mismatch overlap) wanted on its own?
- Which genres dominate the library (sets the beat-grid accuracy bar)?
- Is a C++ build dependency (Signalsmith) acceptable, or pure Rust only?
