# Track 3 Phase D — owner smoke checklist (pending)

- **Date:** 2026-10-01
- **Build:** `track3-memory-cache` worktree at HEAD, debug + Vite dev.
- **Status:** PENDING — owner-run, real account/audio. Check each box when verified; record anything that differs from pre-Phase-D behavior.

## Cached seek (the Phase D fix)

- [ ] Play one track to completion so it is fully cached (the S8/S9 runs warm several).
- [ ] Replay it, seek mid-track (e.g. 2:00 into a 3-minute track): audio jumps to the target immediately, the position never exceeds the duration, and the next track advances naturally at the end.
- [ ] Repeat the seek twice in a row and seek backwards: each seek repositions correctly.
- [ ] Seek an uncached track (network path): unchanged from before (starts at the target, no double-skip).

## Device hot-plug (S10)

- [ ] Play a track, unplug/disable the output device: playback pauses with a visible notice, the queue is intact.
- [ ] Re-enable the device and Resume: playback continues from the paused position.
- [ ] Switch the default device mid-playback from Settings: the stream reopens on the new device without crashing.

## Cache clear-on-exit integrity (S10)

- [ ] Enable Settings → cache clear-on-exit, play a track, quit from the app: exit is clean (no hang).
- [ ] Relaunch and check the cache directory: empty/no partial entries, and `get_cache_status` (Settings) reports 0 used bytes.
- [ ] With clear-on-exit off, play a track, quit, relaunch and replay it: it is a cache hit (no re-download) and no `.part` files are left behind.

## Memory behavior (context for the renegotiation)

- [ ] Idle after login feels unchanged; the Task Manager total (app + WebView2 children) is recorded in the final note from the S9/S1 corpora, not here.
- [ ] Time to first sound and scrolling feel unchanged (Phase B/C budgets hold in the final run).

## Notes

- Any failure or surprise: record it here with the steps and (if a perf session) the run id from `.superpowers/perf/runs/`.
