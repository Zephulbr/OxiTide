# Track 7.1 DSP (ReplayGain + EQ) smoke checklist (2026-10-07)

Human check with a real account and audio. Run `npm --prefix ui run app:dev`
(Vite + Tauri). Settings → Playback → **Sound**. Mark each item pass/fail with a
note; failures become plan fixes, not silent exceptions.

1. Defaults
   - Fresh config (or `[dsp]` removed). PASS = ReplayGain Off, Equalizer
     unchecked, Crossfade Off; with exclusive output on, the player bar still
     shows **Bit-perfect** on a 16/24-bit FLAC.
2. ReplayGain level
   - ReplayGain → Track. Alternate a loud modern master and a quiet older
     master. PASS = their loudness is noticeably closer than with Off; the
     Sound section shows "Current track: −x.x dB" and it changes per track.
3. Album mode
   - ReplayGain → Album, play an album in order. PASS = quiet interludes stay
     quiet relative to the loud tracks (album dynamics kept).
4. Preamp / untagged / prevent clipping
   - Move the ReplayGain preamp to +6 dB with Prevent clipping on, then off.
     PASS = audible change with no crash; with it on, loud tracks are not
     raised past full scale (no harsh clipping).
5. EQ live
   - Equalizer on → Preset Bass boost while playing. PASS = bass rises within
     a fraction of a second, no restart, no click/dropout; the preset select
     shows "Custom" after nudging one band.
6. EQ auto preamp
   - With Bass boost, Automatic preamp on: PASS = "Preamp: −6.0 dB" shows and
     there is no clipping distortion on a loud track. Turn it off and set the
     preamp manually; the slider applies live.
7. Bit-perfect trade-off (exclusive)
   - Exclusive output on, play a FLAC. Toggle EQ on with a non-flat curve.
     PASS = the Bit-perfect badge becomes **DSP** (tooltip "EQ active —
     bit-perfect off"), the device stays exclusive (no reopen click), and
     flattening every band (or unchecking Equalizer) brings Bit-perfect back.
8. Bit-perfect trade-off (shared)
   - Shared output. PASS = no warning text appears in the player bar; the DSP
     badge shows while EQ/ReplayGain alter the sound.
9. Persistence
   - Set ReplayGain Album, a custom EQ curve; quit and relaunch. PASS = the
     same settings are shown and applied on the first track.
10. Regression rows (PM-D1)
    - With all Sound settings off: TTFS feels unchanged (S3) and an album
      plays gaplessly (S4).

Record results below.
