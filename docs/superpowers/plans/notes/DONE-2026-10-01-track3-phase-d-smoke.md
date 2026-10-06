# Track 3 Phase D — owner smoke checklist

- **Date:** 2026-10-01 (checklist) / 2026-10-02 (run)
- **Build:** `track3-memory-cache` worktree at HEAD, debug + Vite dev.
- **Status:** COMPLETE — owner-run 2026-10-02, real account/audio.

## Cached seek (the Phase D fix)

- [x] Play one track to completion so it is fully cached (the S8/S9 runs warm several).
- [x] Replay it, seek mid-track (e.g. 2:00 into a 3-minute track): audio jumps to the target immediately, the position never exceeds the duration, and the next track advances naturally at the end. Owner: "seems to work well."
- [x] Repeat the seek twice in a row and seek backwards: each seek repositions correctly.
- [x] Seek an uncached track (network path): unchanged from before (starts at the target, no double-skip).

## Device hot-plug (S10)

- [x] Play a track, unplug/disable the output device: playback pauses with a visible notice, the queue is intact.
- [x] Re-enable the device and Resume: playback continues from the paused position.
- [x] Switch the default device mid-playback from Settings: the stream reopens on the new device without crashing.

Result: works as expected (owner, 2026-10-02).

## Cache clear-on-exit integrity (S10)

- [x] Enable Settings → cache clear-on-exit, play a track, quit from the app: exit is clean (no hang).
- [x] Relaunch and check the cache directory: empty/no partial entries, and `get_cache_status` (Settings) reports 0 used bytes.
- [x] With clear-on-exit off, play a track, quit, relaunch and replay it: it is a cache hit (no re-download) and no `.part` files are left behind.

Result: works as expected (owner, 2026-10-02).

## Memory behavior (context for the renegotiation)

- [x] Idle after login feels unchanged; the Task Manager total (app + WebView2 children) is recorded in the final note from the S9/S1 corpora, not here.
- [x] Time to first sound and scrolling feel unchanged (Phase B/C budgets hold in the final run).

## Official-client sanity (non-gating, T3-D3)

Owner note, 2026-10-02: "Official client is way too slow to compare with ours — there is a
visible wait when loading track batches on large lists; memory is ~1.1–1.6 GB when Now
Playing is on." Context: OxiTide's S6 10k-list interactions measure 22.1 ms filter /
33.1 ms sort / 17.6 ms album open, and its working set across S1–S9 is 363–655 MB.

## Notes

- No failures or surprises. No perf-session run id involved (manual smoke only).
