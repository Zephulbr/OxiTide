# AutoMix transition quality — research (2026-10-08)

- **Status:** Research; no engine change. Recommendation in §5 awaits the owner.
- **Owner:** V
- **Trigger:** owner A/B listening on `track7b-automix` (`2026-10-08-track7b-automix-smoke.md`):
  transitions are either **distracting / not smooth** or **so faint they are not heard**.
- **Related:** `2026-10-07-automix-spike.md` (on `main`), `2026-10-08-tempo-stretch-research.md`,
  spec `2026-10-08-oxitide-track7b-automix-design.md`.

## 1. What Apple publishes about AutoMix

Apple says that AutoMix "uses AI to analyze audio features" and builds each transition "with
time stretching and beat matching". Its support page says it **chooses the transition per
pair**. Sometimes it only removes silence at the start or end, sometimes it does a simple
crossfade, and **albums and some genres play without transitions**. It also skips the
transition when it can't find a seamless one. Press reports say it looks at **tempo and key**
and picks the "perfect moment". The DSP is unpublished: no patent or Apple ML Research paper
turned up, and no review describes filter sweeps, echo, or risers. The most common complaint
is that it **eats song endings and intros**. That suggests Apple moves the mix point into the
outro/intro *sections* rather than overlapping the last few seconds.

Spotify's "Mix" (beta) is more explicit about its building blocks. It offers volume curves
(Smooth crossfade, Overlap, Fade in/out, Cut in, Center cut, …) and **EQ modes: Center bass
(default), End bass swap, 3-band fade, None**. Transition points come from waveform and beat
data, and it shows key and BPM. The default is notable: **the bass is handled separately from
the rest of the spectrum.**

## 2. What real DJ transitions look like (research)

- **Kim et al., ISMIR 2020** (1,557 real mixes, 20,765 transitions): transition lengths and
  cue-outs cluster at **multiples of 32 beats** (phrases; ≈15 s at 127 BPM). 86 % of tracks
  are tempo-adjusted by < 5 %. Only 2.5 % are key-transposed.
- **Kim, Yang, Nam, DAFx 2022** (estimating fader + 3-band EQ gains from 5,040 real mixes):
  a **constant-power crossfade was the worst reconstruction** of real mixes, behind EQ+fader
  models. In their words, real DJ mixes "have relatively **constant energy through
  transitions**". The example in the paper shows a separate **low-shelf move**, where the bass
  of the incoming track comes up at one point. EQ bands they used: low shelf at 180 Hz, mid
  peak at 1 kHz, high shelf at 3 kHz.
- **Vande Veire & De Bie, EURASIP 2018** (open-source automatic DJ for Drum and Bass): the
  crossfade is **per frequency band**, each band with its own speed and profile. Cue points
  sit on **phrase boundaries** (downbeat-aligned), and it has three transition types (double
  drop, rolling, relaxed) with fade lengths counted in measures.

## 3. Why OxiTide's transitions sound the way they do

There is one mixing law for everything: `dsp.rs` `equal_power` applies a full-band cos/sin curve
across the overlap, and `plan_transition` always overlaps the **last N audible seconds**
(audible means above −54 dBFS). That accounts for both complaints:

| Symptom | Cause in the current design |
|---|---|
| **Faint / not heard** | Many tracks already have a mastered **fade-out**. "Audible" at −54 dBFS includes most of that fade, so the overlap lands inside it, and we apply our cos curve on top: a fade on a fade. The incoming track starts at sin(0) = 0 and often has a quiet intro too, so the middle of the overlap is a dip. No loudness matching unless ReplayGain is on. |
| **Distracting / not smooth** | Tracks with a **hard ending** (final chord, last hit) are overlapped at full energy by an unrelated song. Two **full-band** mixes summed at −3 dB each means two basslines, two kicks, and two vocals at once. Keys are never checked. Beat matching allows ±30 ms drift (audible flams on kicks) and also mixes the full low end. |

So the analysis (silence, beats) is not the main problem. The **gain law and the choice of
mix point** are. Apple, Spotify, and the research all point the same way: pick the window from
the music's **energy envelope**, keep **total energy steady**, and **treat bass separately**.

## 4. Candidate improvements (pure Rust; reuses `dsp.rs` biquads and `automix.rs` analysis)

1. **Energy-aware mix point and gain law (fixes "faint").** Compute a short-term loudness
   envelope (RMS, ~400 ms windows, K-weighted if cheap) of the tail and head.
   - Detect a **fade-out**: the body level is the median of the tail's first seconds. The fade
     starts where the envelope stays ≥ 3 dB below the body level and keeps falling. Start the
     overlap at the fade's start and **don't add our fade on top**: the outgoing gain stays 1
     while its own fade runs, and the incoming rises against it.
   - Detect a **quiet intro** the same way on the head, and give the incoming more lead time.
   - **Loudness-match** the incoming to the outgoing body level during the overlap, then ramp
     to its normal gain (ReplayGain when on), capped at ±6 dB.
   - Shape the curves so the **summed loudness stays near the body level** (no dip, no
     bump). This is the "constant energy" finding.
2. **Bass swap / 3-band blend (fixes "distracting").** Split both signals with a
   Linkwitz-Riley crossover at ~180 Hz (and optionally ~3 kHz). Mids and highs crossfade
   across the whole overlap. The **lows swap within about one beat** at the overlap midpoint,
   on a beat when a confident grid exists, otherwise a ~250 ms swap. The highs of the incoming
   can come in early (DJ "rolling" style). This removes the bass/kick clash, which is the
   biggest source of mud in any overlap, and is what Spotify uses by default.
3. **Choose the transition per pair (Apple's rule).** Classify each end and start:
   *fade-out*, *hard ending* (last loud onset followed by decay/silence), *sustained*. Same
   for starts: *fade-in*, *cold open* (a loud onset in the first 200 ms). Then pick:
   - fade-out → long energy-aware blend (item 1)
   - hard ending → no overlap, trim the silence and leave a short musical gap (~1 beat or
     300 ms), or a very short overlap after the final hit decays
   - cold open → never overlap the opening hit; the incoming starts when the outgoing is done
   - both rhythmic with confident grids → beat-matched bass-swap blend (item 2)
   - otherwise → short smart crossfade
   The report states which rule fired, so A/B notes map to code.
4. **Phrase-aware beat matching (Phase 2 tuning).** Lengthen the overlap to 16/32 beats when
   grids are confident, which needs the varispeed tail from the tempo-stretch note (Phase 3a)
   to stop the drift. Tighten the drift budget from ±30 ms to ±10 ms, or drop to the bass-swap
   blend without beat alignment.
5. **Key compatibility (later).** A chroma-based key estimate on the tail and head. With
   incompatible keys (not the same key, relative, or Camelot ±1), shorten the
   mid/high overlap. This is cheap once the STFT exists, but less important than 1–3.

## 5. Recommendation

1. **Build an A/B harness first.** The engine keeps the last transition's tail and head in
   memory. Add a dev-only **"Replay last transition as…"** control that re-renders the same
   pair with each variant (current / energy-aware / bass swap / per-pair rule) without writing
   audio to disk (PM-D2). Every later step is then judged on the same material by ear in
   seconds, instead of waiting for the next natural track end.
2. **Then items 1 + 3 together** (energy-aware law + per-pair rule). These address "faint"
   and the worst "distracting" cases (hard endings, cold opens) with no new dependencies.
3. **Then item 2** (bass swap), A/B'd against item 1 alone on rock/metal/J-pop pairs.
4. Items 4–5 only after 1–3 pass the owner's listening test. Track 7b stays unmerged until then.

## 6. Questions for the owner

- Which pairs sounded worst? Track names, or the "Last transition" readout (kind, overlap,
  trims), would show whether the bad ones were fades, hard endings, or beat-matched.
- Accept Apple's trade-off (cut into outros/intros for smoother blends) or keep more of each
  song (shorter, safer transitions)? This decides how far item 1 may move the mix point.
- Is the dev-only replay control acceptable in the app UI (hidden behind a flag), or should
  it be an offline `cargo run --example` that reads audio from a local file you supply?

## Sources

- [Apple Newsroom — services this fall (AutoMix)](https://www.apple.com/newsroom/2025/06/apple-services-deliver-powerful-features-and-intelligent-updates-to-users-this-fall/)
- [Apple Support — AutoMix / Crossfade](https://support.apple.com/en-us/105067)
- [MacRumors — iOS 26 AutoMix](https://www.macrumors.com/2025/06/12/ios-26-automix-apple-music/)
- [BGR — AutoMix complaints](https://www.bgr.com/2059450/how-to-turn-off-automix-apple-music-worst-feature/)
- [How-To Geek — disabling AutoMix](https://www.howtogeek.com/i-disabled-apple-music-automix/)
- [RouteNote — Spotify Mix transitions](https://routenote.com/blog/spotify-mix-customizable-transitions/)
- [Kim et al., ISMIR 2020 — Computational analysis of real-world DJ mixes](https://arxiv.org/abs/2008.10267)
- [Kim, Yang, Nam, DAFx 2022 — Joint estimation of fader and EQ gains](https://mac.kaist.ac.kr/pubs/KimYangNam-dafx2022.pdf)
- [Kim, Yang, Nam, NIME 2021 — Sub-band analysis of transition regions](https://nime.org/proc/nime2021_87/)
- [Vande Veire & De Bie, EURASIP 2018 — Automated DJ for Drum and Bass](https://asmp-eurasipjournals.springeropen.com/articles/10.1186/s13636-018-0134-8)
- [Chen et al. — Automatic DJ transitions with differentiable audio effects and GANs](https://arxiv.org/abs/2110.06525)
