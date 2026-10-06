# Track 10.1 session-restore smoke checklist (2026-10-04)

Human check with a real account. Run `npm --prefix ui run app:dev` (Vite + Tauri).
Mark each item pass/fail with a note; failures become plan fixes, not silent exceptions.

1. Queue in, app out
   - Play a playlist (the sidebar row shows the playing marker), pause mid-track,
     note the position, close the window (X).
   - Relaunch, log in if asked.
   - PASS = queue rail shows the same items in the same order, the player bar shows
     the same track at the same position, shuffle/repeat match what was set, the
     playlist marker is still lit, and playback is paused.
2. Resume
   - Press play. PASS = playback resumes at the saved position (no restart from 0,
     no blip before the press).
3. Edits
   - Reorder or remove a queue item, then quit and relaunch.
     PASS = the edited order is restored.
4. Logout
   - Log out. PASS = queue empties and `%APPDATA%\OxiTide\config\session.json` is gone.
   - Log back in. PASS = the app starts with an empty queue, not the old one.
5. Corrupt file
   - Quit. Overwrite `session.json` with `garbage`. Launch.
     PASS = app boots to an empty queue, no error dialog, and `session.json.bak` exists.
6. Mini player
   - With `start_in_mini` on and a saved queue, launch. PASS = mini player shows the
     restored track paused; exit mini shows the full queue.
7. Empty queue
   - Clear the queue, quit, relaunch. PASS = empty state, no errors.

Record results below.

- [x] 1 queue in/out
- [x] 2 resume position
- [x] 3 edits survive
- [x] 4 logout clears + fresh login empty
- [x] 5 corrupt file boots empty
- [x] 6 mini player restore
- [x] 7 empty queue
