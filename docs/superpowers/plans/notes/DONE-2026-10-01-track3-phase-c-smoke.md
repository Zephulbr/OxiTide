# Track 3 Phase C — owner smoke checklist (pending)

- **Date:** 2026-10-01
- **Build:** `track3-ui-library` worktree at `fd17319`, debug + Vite dev.
- **Status:** PENDING — owner-run, real account/library. Check each box when verified; record anything that differs from pre-Phase-C behavior.

## Tracks (large library)

- [ ] Type in the filter while the list is still paging in: typing stays responsive, the filtered list is correct once pages land (no duplicates or missing rows).
- [ ] Filter to a small subset, sort by Title asc → desc → original, then clear the filter: the visible order matches the same operations before this phase.
- [ ] Double-click a filtered row: playback starts on that row; **Play all** after filtering queues the visible (filtered/sorted) list in that order.
- [ ] Unfavorite a row while a sort/filter is engaged: the row disappears once, counts stay right, and re-favoriting brings it back in the next refresh.
- [ ] No visible layout, row height, or ordering change versus before the phase.

## Playlists

- [ ] Open a playlist that contains the same track twice; both occurrences are visible and playable.
- [ ] Drag-reorder a row (and arrow-move from the context menu); re-open the playlist from the sidebar: the order persists.
- [ ] Remove a row from an owned playlist: it disappears once and survives a re-open.

## Modes

- [ ] Instrument: fling-scroll the tracks list and run the filter/sort quicks; no stutter or long freeze (the S6 run measured filter p95 23 ms / sort p95 36 ms).
- [ ] Atmosphere: same actions; no mode-specific jank (frame p95 7.1 ≤ 20, p99 13.9 ≤ 33 measured).

## Perf-session-only fixture (must not leak)

- [ ] `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S6 -Runs 1`: during the run the Tracks view shows the 10,000-row synthetic list (titles like "The Midnight Dream 00000").
- [ ] After that run, launch normally (`npm --prefix ui run app:dev`) and open My Collection → Tracks: the real favorites are shown (no synthetic rows).

## Notes

- Any failure or surprise: record it here with the steps and (if a perf session) the run id from `.superpowers/perf/runs/`.
