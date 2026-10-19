# Multi-level sorting — owner smoke checklist (pending)

Spec `2026-10-19-oxitide-multi-sort-design.md`; run with `npm --prefix ui run app:dev`.

## Favorites → Tracks

- [ ] The Date added column shows real dates.
- [ ] Clicking Artist sorts A→Z; a second click reverses it; a third returns to the original order.
- [ ] With Artist active, hovering Album shows a `+`. Clicking it adds Album as level 2: the
      headers show rank dots 1/2 and the sort bar shows two chips.
- [ ] Shift+click on Date added adds it as the next level.
- [ ] Clicking a chip reverses that level; × removes it; Alt+←/→ moves it; "Clear" resets.
- [ ] "Add sort" → Release date: "Loading all tracks…" shows, then the order applies; the
      Release date column appears while it is sorted.
- [ ] "Add sort" → Plays: counts match History (0 for unplayed).

## Playlists

- [ ] Release dates appear instantly, with no wait (embedded).
- [ ] Sort, leave the playlist, come back: the sort is still there. Other playlists are not
      affected.
- [ ] Drag-reorder is off while sorted and works again after "Clear".

## Columns and pages

- [ ] Right-clicking the header row opens "Columns". Toggling Popularity or Plays persists per
      page type.
- [ ] Narrowing the window (or opening the queue rail) hides the optional columns cleanly
      during the glide.
- [ ] Album, Artist (top tracks and all tracks), Mix and search results sort by multiple levels.
      There is no Date added outside favorites and playlists, and no Album / Release date on an
      album page.

## Settings → Remember track sorting

- [ ] *Until I leave*: lists always open in original order.
- [ ] *Until restart*: sorts survive navigation but not a restart.
- [ ] *Always*: sorts survive a restart.

## Modes

- [ ] Atmosphere, Instrument and Rack: chips, rank dots and the `+` are legible, in both the
      light and dark themes.
