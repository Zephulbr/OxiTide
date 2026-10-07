# Track 7b AutoMix — A/B smoke checklist (2026-10-08)

- **Status:** Scripted gates green; **owner A/B listening pending**
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

## E. Regression rows (PM-D1)

13. AutoMix toggles off: gapless album playback (S4) and time-to-first-sound (S3) feel unchanged.
14. CPU stays calm during transitions (Task Manager); no underruns in `app:dev`.

## Results

Record results below.
