# Track 7.2 crossfade smoke checklist (2026-10-07)

Human check with a real account and audio. Run `npm --prefix ui run app:dev`.
Settings → Playback → Sound → Crossfade. Mark each item pass/fail with a note.

1. Basic crossfade
   - Crossfade 6 s, build a queue of tracks from **different** albums, seek to
     ~15 s before the end. PASS = the next track fades in while the current one
     fades out over ~6 s; no gap, no loudness dip or bump; Now Playing switches
     at the start of the blend and the position restarts at 0:00.
2. Albums stay gapless
   - "Keep albums gapless" on, play a gapless album (live album or DJ mix)
     across a track boundary. PASS = no fade, no gap. Turn it off: PASS = the
     same boundary now crossfades.
3. Skips cut
   - With crossfade on, press Next mid-track and seek within a track.
     PASS = immediate cut, no blend.
4. Format change
   - Max quality; queue a 44.1 kHz track followed by a 96 kHz track (different
     albums). PASS = the first track fades out, then the second starts (no
     overlap, no error, no audio from the old track after the switch).
5. Exclusive output
   - Exclusive on, crossfade on, different albums. PASS = the blend plays
     correctly on the exclusive device; the badge shows DSP (crossfade) only
     during the blend and Bit-perfect returns afterward.
6. Sleep timer end-of-track
   - Crossfade 6 s, arm End of current track ~20 s before the end.
     PASS = the fade/pause happens on the current track; nothing of the next
     track is heard.
7. History
   - Let a crossfaded track hand over, then open History. PASS = it is logged
     as a completed play.
8. Live change
   - Change the crossfade length or turn it off while a track plays.
     PASS = no audible glitch; the new setting applies at the next boundary.
9. Queue end
   - Crossfade on, last track in the queue. PASS = the track plays to its real
     end (nothing is cut).

Record results below.
