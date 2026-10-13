# Track 7b.2 — Transition styles and the replay lab (2026-10-08)

- **Status:** In progress on `track7b-automix`
- **Owner:** V
- **Authority:** `plans/notes/2026-10-08-automix-transition-quality-research.md` §4–§5; owner call
  2026-10-08 after the first A/B pass ("either distracting or too faint"; rock and phonk/pop
  both failed). The owner suspects the mix starts too early and asked for both a **shorter**
  and an **Apple-style cutting** transition, plus a dev replay tool.

## Decisions

| # | Decision |
|---|---|
| AM-D8 | `CrossfadeSettings.style`: `classic` (Track 7 equal-power over the last N audible seconds, unchanged, kept as the A/B baseline), `quick`, `blend`. Default `classic`. |
| AM-D9 | **Quick:** a short overlap (≤ 1.5 s, never longer than the crossfade setting) placed where the outgoing track has died down: its smoothed level stays ≥ 18 dB under its body level from then on (fade-outs, ringing final chords); with no such drop, the last 1.5 s of audible audio. The incoming track reaches full level within the first 35 % of the overlap, or at once when it opens cold (loud from its first 150 ms). |
| AM-D10 | **Blend (Apple-style cut):** the overlap **ends where the outgoing fade starts** (level stays ≥ 4 dB under the body from then on), so its own fade-out is cut, at most 12 s of audible ending is dropped. Overlap length = the crossfade setting. The incoming track skips a slow intro so it reaches its body level as the overlap ends (at most 8 s beyond silence). Lows (< 180 Hz, Linkwitz-Riley split, complementary so the sum is exact) **swap within 250 ms at the overlap centre**; mids/highs cross with equal power across the overlap. With beat matching on and confident grids, the window is beat-aligned (existing `align`) and the swap lands on the centre beat. |
| AM-D11 | Any AutoMix option (style ≠ classic, trim, beat match) holds back crossfade + 14 s of the outgoing tail and pre-reads 20 s of the next track, so every style can be planned (and replayed) from one transition. All toggles off stays the exact Track 7 path. |
| AM-D12 | **Replay lab:** while AutoMix is on, the engine keeps the last transition's raw material (pre-DSP tail and head, `f32`) in memory only. `automix_render(style)` re-plans that pair with the current settings and the chosen style and returns a 16-bit WAV in memory (≈ 6 s before the overlap → overlap → ≈ 6 s after); the Settings page plays it through a `blob:` audio element (CSP `media-src 'self' blob:`). No audio is written to disk (PM-D2). Replays skip EQ/ReplayGain. |
| AM-D13 | The report gains `style`, `cut_out_ms` (audible ending dropped), and `cut_in_ms` (intro skipped beyond silence). |
| AM-D14 | **Blend level ride** (owner A/B 2026-10-13: "the next starts too loud"): the planner compares what is heard — tail and head scaled by their ReplayGain — and the incoming track enters at the outgoing level across the overlap (its mean level vs the head's body, never a boost, at most −6 dB), easing back to unity (smoothstep in dB) by 4 s after the overlap. Report `ride_db`. Replays now keep ReplayGain (still skip EQ). |
| AM-D15 | **Sparse endings** (owner: endings with only vocals left feel jarring): when the level after the fade point holds (slope > −1.5 dB/s for ≥ 2 s) it is a sparse ending, not a fade. Blend keeps it — the overlap ends where it dies down — crosses full band (no bass swap, nothing to swap) and skips beat matching; note "sparse ending kept". |
| AM-D16 | **Shared output keeps the device across format changes** (owner: an audio click "like the DAC stops and starts"; the log showed `resampled=true` transitions). On a natural advance in shared mode with AutoMix (or `resample`) on, the open device is kept: a bit-depth change needs nothing (WASAPI shared takes `f32`), a rate change converts the incoming track to the device rate with a streaming FFT resampler (`dsp::StreamResampler`) for the rest of that track. Previously `stop()` dropped the queued ring audio and reopened mid-overlap. Exclusive output still reopens (it must run at the track's rate). |

| AM-D17 | **Quick retune** (owner 2026-10-13, second pass: "too abrupt, the tracks are too close"): after a decay the overlap *starts* where the outgoing track has dropped 18 dB (no longer centred on it) and may run on into its trailing silence, up to 3 s; the incoming track fades in over 1 s (0.2 s for a loud opening, which used to be instant). A hard stop (under 0.75 s of decline) still overlaps its last 1.5 s. |
| AM-D18 | **Auto style.** Every AutoMix plan measures the pair (`TransitionFeatures`: ending kind hard stop / ring-out / fade-out / sparse, decline length, trailing and lead silence, cold open, intro length, both body levels with ReplayGain) and logs it (`transition features into track …`). Auto picks: beat-matched Blend when beat match is on and both grids agree; continuous recording (hard stop, < 0.15 s silence at both edges, loud opening) → gapless; sparse → Quick; fade-out ≥ 6 s → Blend, shorter → Quick; ring-out → Quick; hard stop into an intro ≥ 2 s → Classic, else Quick. The report names the pick and `auto_reason` ("Auto → Blend · Why: long fade-out: cut it"). Thresholds are first guesses, to be tuned against the logged features and the owner's verdicts. |

## Fixes from the 2026-10-13 A/B pass

- **Transport froze near a track's end (owner: "couldn't skip to the previous track").** With AutoMix on, the planned preroll (up to the 14 s hold plus the crossfade) went to the device in one blocking write, so Prev/Next/Pause/Seek waited for it. It is now written in ~46 ms slices that stop when the session is superseded and hold while paused (`write_responsive`), with the position following.
- **Live Blend often was not Blend.** A beat analysis that missed the tail fell back to a plain equal-power crossfade, dropping the style. The style is now planned without beats up front and used whenever the analysis is late. `oxitide-audio` builds at `opt-level = 2` in dev so the analysis keeps up in `app:dev`.
- The transition log line now carries `style`, `cut_out_ms`, `cut_in_ms`, `ride_db`, and `kept_device`.
- Second pass (same day): all 11 Blend transitions kept the device (one across a rate change, no
  click). 6 of 11 still logged "finished late; plain crossfade": with a short held-back tail the
  first buffer went out before the stand-in check. Fixed (re-check before writing; a test forces
  a late analysis on a tail with and without preroll). A plain fallback's readout no longer shows
  the beat plan's cuts and ride.
- Open: the outgoing beat confidence is very often 0.00 on real tracks (rock/metal/phonk); the grid confidence (periodicity × impulsiveness × steadiness) needs tuning against labelled owner tracks before beat matching is judged.

## Tasks

1. `dsp.rs`: `TransitionStyle`, `MixShape` (`EqualPower` / `Quick { attack }` / `BassSwap { swap_start, swap_end }`, fractions of the overlap so a resampled tail keeps them), `Tail::set_shape`, LR4 low-band split, `Tail::from_normalized`. Tests: shapes keep the summed signal exact where the gains meet, the bass swap is complementary, `converted` keeps the shape.
2. `automix.rs`: short-term level envelope, `ending` (body, fade start, quiet point), `opening` (entry, cold), `plan_quick`, `plan_blend` (+ beat-aligned variant), report fields, hold/head sizes, `render_transition`. Tests on synthetic fade-outs, hard endings, slow intros, and cold opens.
3. `engine.rs`: plans carry the shape into the handoff tail, capture the last transition's material, `Engine::render_last_transition`.
4. `oxitide-app`: `automix_render` command (WAV encoder), registered in `main.rs`; CSP `media-src`.
5. UI: style picker, report lines, replay buttons (pause playback, play/stop the render), `nearEndTarget` → 35 s so the longer hold fills.
6. Gates + update the A/B smoke checklist with the style rows.
