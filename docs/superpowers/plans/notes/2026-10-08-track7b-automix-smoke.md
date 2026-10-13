# Track 7b AutoMix — A/B smoke checklist (2026-10-08)

- **Status:** Scripted gates green; owner A/B listening **paused 2026-10-08** for transition-quality research
- **Authority:** `specs/2026-10-08-oxitide-track7b-automix-design.md` §8
- **Build under test:** branch `track7b-automix`

Run `npm --prefix ui run app:dev`. Settings → Playback → Sound: set Crossfade to 6 s and turn
**off** "Keep albums gapless" while testing (otherwise same-album pairs never transition).
Use **Jump near the end of this track** to reach each transition quickly; the **Last
transition** readout and the log (`%APPDATA%\OxiTide\logs`, lines "transition into track")
record what happened. Build queues that mix the genres below.

For every toggle, listen to ~10 transitions **off**, then the same pairs **on**, and record a
verdict: **keep / drop / tune** with a note. Mixed genres to cover: rock, metal, alternative,
J-pop, shoegaze, symphonic (plus anything else you play a lot).

## A. Baseline (all AutoMix toggles off)

1. Transitions sound exactly like the Track 7 crossfade (no regression).
2. Readout shows "Crossfade" with the overlap length; a 44.1 → 96 kHz change shows
   "Fade-out (no overlap)".

## B. Skip silence at track edges

3. Tracks that end in silence or a long fade: the overlap now starts under audible music (no
   dead air in the middle of the blend). Readout shows "Silence skipped".
4. Tracks with quiet, deliberate intros (symphonic, shoegaze swells): the intro is not cut
   (leading trim is capped at 4 s). Note any intro that felt clipped.
5. Hidden tracks / long silent gaps before a bonus track: note what happens.

Verdict: keep / drop / tune — notes:

## C. Overlap across sample-rate changes

6. Queue a 44.1 kHz (Lossless) track before a 96/192 kHz (Max) track. With the toggle on the
   readout says "Resampled across a format change" and the outgoing track overlaps instead of
   fading out first. Listen for clicks or a dropout at the device reopen.
7. Exclusive mode: same check; the DAC re-locks at the switch.

Verdict: keep / drop / tune — notes:

## D. Beat-match when tempos fit

8. Pairs with near-identical tempos (same artist/genre often works): readout "Beat-matched",
   overlap in beats, both BPMs with confidence. Do the beats land together (no flams)?
9. Pairs with different tempos: readout falls back to "Crossfade" with "tempos too far apart".
10. Half/double-time pairs (e.g. metal 180 → 90 BPM groove): matched on the slower grid?
11. Rubato / ambient / shoegaze / symphonic: expect "beat confidence too low" fallbacks.
    Note any **confidently wrong** grids (beat-matched but audibly off) with the track names —
    these tune the confidence floor.
12. Rapid skips during a transition: no glitch, no stuck audio.

Verdict: keep / drop / tune — notes:

## D2. Transition styles and the replay lab (Track 7b.2, 2026-10-08)

Plan: `plans/2026-10-08-track7b-2-transition-styles.md`. Crossfade 6 s, any AutoMix option on.
Let one track end on its own (Jump near the end now seeks 35 s back), then use **Replay last
transition as: Classic / Quick / Blend** to hear the same pair three ways. Replays pause
playback and skip EQ/ReplayGain; the line under the buttons gives the overlap position in the
replay (it starts ~6 s in) and what was cut.

12a. Rock pair (fade-out ending) — Classic vs Quick vs Blend. Is Quick's incoming entry on time
     (lands as the old track dies away, no dead air)? Does Blend's cut of the fade feel natural?
12b. Phonk/pop pair (hard or looped ending, loud cold open) — Quick should not fade in the
     opening hit ("next track opens loud"); Blend's bass swap: no double kick/808 mud.
12c. A slow-intro incoming track — Blend skips into the intro ("Cut: … of the intro"); is the
     skip acceptable or too greedy (cap 8 s)?
12d. Blend + Beat-match on a same-tempo pair: readout "Blend (beat-matched)"; does the bass swap
     land on the beat?
12e. Pick a style and live-play ~10 transitions: does the live result match its replay?

Verdict per style: keep / drop / tune — notes:

## E. Regression rows (PM-D1)

13. AutoMix toggles off: gapless album playback (S4) and time-to-first-sound (S3) feel unchanged.
14. CPU stays calm during transitions (Task Manager); no underruns in `app:dev`.

## Results

Record results below.

- **2026-10-08 (owner, first pass):** transitions are either distracting / not smooth or so faint
  they are not heard. Paused for research: `2026-10-08-automix-transition-quality-research.md`.
- **2026-10-13 (owner, 7b.2 styles, shared mode, 6 s, ReplayGain track):**
  - Classic: keep. Quick: keep, may need small tuning (details pending).
  - Blend: mixed — very good on some pairs, barely different on others, a little jarring on
    endings with only vocals left; incoming track often too loud against a fading ending; an
    audible click mid-blend (log: `resampled=true`, a device reopen).
  - Bug: Prev did nothing near the end of "I Can't Handle Change".
  - Beat match: nearly every live transition logged "beat analysis finished late"; outgoing
    confidence mostly 0.00.
  - Changes in response: AM-D14–D16 and the fixes listed in
    `plans/2026-10-08-track7b-2-transition-styles.md`. Re-test rows 12a–12e (Blend especially),
    plus: 12f vocal-only ending into a loud track (readout "sparse ending kept", "Level ride");
    12g a Lossless ↔ Max pair (no click; log `kept_device=true`); 12h Prev/Pause in the last
    ~20 s of a track.
- **2026-10-13 (owner, second pass):** better overall, click gone (log agrees: every transition
  `kept_device=true`, one across a rate change). Quick "too abrupt, tracks too close" → AM-D17.
  Auto style added (AM-D18). Next rows:
  - 12i Quick retuned: fade-out pairs and a hard stop into a loud opening — still abrupt? any
    dead air now?
  - 12j Auto over ~20 live transitions of mixed genres: for each, readout "Auto → … · Why: …"
    and a verdict (right pick / wrong pick, which style would have been better). Note any
    "continuous recording" gapless pick that was not a live album or mix.
