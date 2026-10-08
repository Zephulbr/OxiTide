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

## Tasks

1. `dsp.rs`: `TransitionStyle`, `MixShape` (`EqualPower` / `Quick { attack }` / `BassSwap { swap_start, swap_end }`, fractions of the overlap so a resampled tail keeps them), `Tail::set_shape`, LR4 low-band split, `Tail::from_normalized`. Tests: shapes keep the summed signal exact where the gains meet, the bass swap is complementary, `converted` keeps the shape.
2. `automix.rs`: short-term level envelope, `ending` (body, fade start, quiet point), `opening` (entry, cold), `plan_quick`, `plan_blend` (+ beat-aligned variant), report fields, hold/head sizes, `render_transition`. Tests on synthetic fade-outs, hard endings, slow intros, and cold opens.
3. `engine.rs`: plans carry the shape into the handoff tail, capture the last transition's material, `Engine::render_last_transition`.
4. `oxitide-app`: `automix_render` command (WAV encoder), registered in `main.rs`; CSP `media-src`.
5. UI: style picker, report lines, replay buttons (pause playback, play/stop the render), `nearEndTarget` → 35 s so the longer hold fills.
6. Gates + update the A/B smoke checklist with the style rows.
