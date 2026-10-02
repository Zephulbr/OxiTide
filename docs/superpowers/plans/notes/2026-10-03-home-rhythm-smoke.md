# Home rhythm & card language — smoke checklist (2026-10-03)

Implementation: `docs/superpowers/plans/2026-10-03-home-rhythm-card-language.md`
Spec: `docs/superpowers/specs/2026-10-03-home-layout-card-language-design.md`

## Automated gates (run 2026-10-03)

- `cargo test --workspace` — 207 + 166 + 1 + 155 passed, 1 ignored, 0 failed.
- `cargo clippy --workspace -- -D warnings` — clean.
- `npm --prefix ui run check` — 0 errors, 0 warnings.
- `npm --prefix ui test` — 49 files / 463 tests passed.

## Live tuning (pending owner)

Unknown module kinds fall back to carousel + plain shapes and are pinned in
`ui/src/lib/media.test.ts`. To tune against the real account:

1. Temporarily add `{@const _debug = console.debug("section", section.kind, section.items.map((i) => i.item_type))}`
   at the top of the `{#each}` body in `ui/src/lib/components/PageSections.svelte`.
2. `npm --prefix ui run app:dev`, open Home, read the WebView2 console (Inspect) or the Vite terminal.
3. If a kind needs a different treatment, change only the predicate constants in
   `ui/src/lib/media.ts`, add a `classifySection` test for that kind, remove the debug line,
   and commit `fix(ui): tune section classification to the live feed`.

## Human GUI smoke (pending owner)

- [ ] Quick-access strip shows playlists first, then saved mixes, max 6; hidden when both sources are empty.
- [ ] With the network offline (or TIDAL unreachable), the strip hides and Home still renders its feed error state.
- [ ] Playlist sections render as two-row tile grids with View all when > 12 items.
- [ ] Album/mix/radio sections render as carousels; arrows disable at both ends and on short rows; shift+wheel scrolls.
- [ ] Track-heavy sections render as two-column rows with Play all.
- [ ] Album = rounded square; playlist = sharp square with deck edge; mix = radio badge; artist = circle.
- [ ] Mix cards still open the mix page (no autoplay); artist cards open the artist page.
- [ ] View all opens the section page; back and forward return through each section visited.
- [ ] Track row click plays the track; Play all plays the section from the top; queue metadata (title/artist/album/cover) is correct in the queue rail.
- [ ] Player error toast appears if a track resolve fails (e.g. disable network before clicking).
- [ ] Explore and Mixes render the same treatments and shapes.
- [ ] Reduced motion: carousel arrow clicks jump without smooth scrolling.

## Performance re-check (pending owner)

Single app, foreground, Vite warm (AGENTS.md §Performance harness):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 3
node scripts/perf/check.mjs
```

If the strip pushes S1/S2 over budget, drop `collage` from the strip's `MediaCard`
(plain `Cover` thumbs) and re-run.
