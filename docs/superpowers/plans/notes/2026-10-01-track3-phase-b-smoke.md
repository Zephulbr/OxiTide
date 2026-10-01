# Track 3 Phase B — owner smoke checklist (pending)

- **Date opened:** 2026-10-01
- **Build:** `track3-startup-ttfs` (`468630f`), debug + Vite dev; real account, real audio output.
- **Why:** Phase B changed the startup path (gated/backgrounded work), made artwork/lyrics/scrobbler caches lazy, and replaced the audio worker's 200 ms poll with event-driven waits. Scripted perf runs cannot exercise device/network edge cases, account state, or feel.
- **Status:** ALL ITEMS PENDING — the owner runs these and records pass/fail + notes below. Each item gets the build stamped in the notes column.

## A. Audio path (Task 4 — event-driven worker wakeups)

- [ ] **A1 — next/prev during a slow network load.** Start a track, then throttle the network (or pick a track that buffers), and hit next/prev repeatedly while the new track is still resolving/fetching. Expected: the newest request wins promptly (no stuck spinner, no ghost playback of a superseded track, no audible artifact); the old request is abandoned.
- [ ] **A2 — seek.** Seek forward/backward in a streamed (uncached) track and in a fully cached track. Expected: playback repositions correctly in both; position readout and audio agree; a seek near the end still advances naturally when the track ends. (Note: Phase A found a cached-seek defect — pure cached `decode_from_file` did not reposition; Phase D is tracking it. Record whether this build still shows it so Phase D has a fresh observation.)
- [ ] **A3 — gapless album advance.** Play an album with continuous transitions (repeat off) and let at least two natural track transitions happen. Expected: no gap beyond a breath, no double-play/stutter, correct now-playing and queue position.
- [ ] **A4 — device hot-plug.** While playing, unplug the active output device (or disable it in Windows). Reconnect/re-enable. Expected: playback pauses on loss, device list refreshes, selecting the device resumes cleanly (no crash, no stuck "playing" state).
- [ ] **A5 — quality switch.** While playing, switch quality (e.g. High ↔ Max, and one unsupported/HE-AAC-only track if available). Expected: the switch applies on the next resolve without wedging the worker; fallback messages still appear when expected.
- [ ] **A6 — logout while playing.** With audio playing, log out. Expected: playback stops, settings/auth state clears, no orphaned worker or repeated keyring prompts; app remains usable.

## B. Startup (Tasks 1–3 — gated window, lazy loads)

- [ ] **B1 — S1 cold start feel.** Quit fully, relaunch cold. Expected: window appears promptly, no long white/blank stare before first paint; UI becomes usable without a visible stall. Record rough feel (fast/normal/slow) and whether the window ever shows before content.
- [ ] **B2 — S2 warm start feel.** Relaunch within 5 minutes. Expected: same or better than cold; no flash of stale content; window geometry restored.
- [ ] **B3 — deep-link login cold start.** Start the app cold via a `tidal://login/auth?code=...` link. Expected: no stale login screen; login completes to the signed-in state; no need to paste the URL manually.
- [ ] **B4 — palette/cover color after login.** Sign in and open a page with artwork. Expected: cover art loads and the dynamic palette still colors the UI after login (no missing `analyze_cover` results due to the lazy cache load); lyrics still appear in the lyrics view, including a synced track.
- [ ] **B5 — scrobbler status immediately after launch.** Open Settings → Connections right after launch (both Last.fm and ListenBrainz configured). Expected: the connection status shown is correct immediately (no stale "disconnected" that flips once you interact, no spurious auth error toast); now-playing still scrobbles.

## Sign-off

- Owner: _pending_
- Date: _pending_
- Result summary: _pending_
- Any item failing gets a dated follow-up note next to this file and a roadmap/Track 3 chase item; do not edit this checklist in place beyond appending results.
