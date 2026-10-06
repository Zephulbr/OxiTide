# Track 10.3 history-and-stats smoke checklist (2026-10-04)

Human check with a real account and audio. Run `npm --prefix ui run app:dev`
(Vite + Tauri). Mark each item pass/fail with a note; failures become plan
fixes, not silent exceptions.

1. Recently played
   - Play a track past 30 s, skip to the next, then open History from the
     sidebar.
   - PASS = the track appears under "Today" with its cover, title, artist,
     album, and a relative time; clicking it starts playback from that row.
2. Skips are not recorded
   - Skip a track after ~10 s. PASS = it does not appear in Recently played.
3. In-flight play
   - Start a track and open History while it plays.
     PASS = the current track is the first row, before it has been finalized.
4. Stats
   - Open the Stats tab. PASS = all-time/7-day/30-day listening time and play
     counts are plausible; top artists/tracks/albums list real plays; the
     30-day chart shows bars for the days you played, and the in-flight track
     is not counted until it finalizes.
5. Restart persistence
   - Quit and relaunch. PASS = the same history and stats return.
6. Day grouping
   - Play something, then (optionally) change the system clock forward a day
     and play again. PASS = the two plays sit under "Yesterday" and "Today".
7. Clear
   - Settings → Storage → Clear history (confirm).
     PASS = Recently played and Stats are empty and history.jsonl is gone.
8. Disable
   - Settings → Storage, turn off "Record listening history", play a track
     past 30 s. PASS = the play does not appear; re-enabling records new plays
     again.
9. Max entries (optional)
   - Set Max entries to 100, finalize more than 100 plays (or inspect the
     file). PASS = history.jsonl holds at most 100 lines, newest kept.

Record results below.

- [x] 1 recently played
- [x] 2 skips not recorded
- [x] 3 in-flight play
- [x] 4 stats + chart
- [x] 5 restart persistence
- [x] 6 day grouping
- [x] 7 clear
- [x] 8 disable (found: the in-flight track survived a clear/disable while playing; fixed in 1ad2a9d, re-check passed)
- [x] 9 max entries
