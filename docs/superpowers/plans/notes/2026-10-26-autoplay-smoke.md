# Autoplay — owner smoke (2026-10-26)

Plan: `docs/superpowers/plans/2026-10-26-autoplay.md`. Windows, a real account, `npm --prefix ui run app:dev`.
Autoplay logs `autoplay: queued N tracks after track <id>` (or why it didn't) in the app log.

- [x] 1. Settings → Playback → Queue shows **Autoplay**, on by default. Settings search finds "autoplay".
- [x] 2. Play the last track of an album. Within a few seconds about 10 tracks appear under an
      "Autoplay" divider in the queue. The move into the first one is gapless.
- [x] 3. Same as 2 with crossfade on (different-album rule): the transition crossfades.
- [x] 4. A suggestion is playing: the player bar reads "From Autoplay". Near the end of the
      suggestions, about 10 more are appended (from the pool, no visible wait).
- [x] 5. Next on the last track (after the suggestions appeared): it advances into them.
- [x] 6. Repeat All, then Repeat One, on the last track: nothing is appended.
- [x] 7. Block explicit on: no explicit suggestions (check the E labels).
- [x] 8. Turn Autoplay off mid-suggestions: the upcoming suggestions disappear, and the current track keeps playing.
- [x] 9. Restart the app with suggestions queued: the restored queue still shows the divider.
- [x] 10. Hide to the tray on the last track: the queue still extends (check after reopening).
- [x] 11. Offline (Wi-Fi off) at the last track: the queue ends quietly. Back online during the
      track (within 10 min): it fills.
- [x] 12. Phone remote: suggestions show "Autoplay ·" before the artist.
- [x] 13. "Play next" / "Add to queue" on a last track (Autoplay off): the move into the added
      track is gapless (prefetch refresh, AP-D7).
- [x] 14. Perf: `run-scenario.ps1 -Scenario S4 -Release` is within budget.
