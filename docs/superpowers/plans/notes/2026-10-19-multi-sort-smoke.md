# Multi-level sorting — owner smoke checklist (pending)

Spec `2026-10-19-oxitide-multi-sort-design.md`; run with `npm --prefix ui run app:dev`.

## Favorites → Tracks

- [x] The Date added column shows real dates.
- [x] Clicking Artist sorts A→Z; a second click reverses it; a third returns to the original order.
- [x] With Artist active, hovering Album shows a `+`. Clicking it adds Album as level 2: the
      headers show rank dots 1/2 and the sort bar shows two chips.
- [x] Shift+click on Date added adds it as the next level.
- [x] Clicking a chip reverses that level; × removes it; Alt+←/→ moves it; "Clear" resets.
- [x] "Add sort" → Release date: "Loading all tracks…" shows, then the order applies; the
      Release date column appears while it is sorted.
- [x] "Add sort" → Plays: counts match History (0 for unplayed).

## Playlists

- [x] Release dates appear instantly, with no wait (embedded).
- [x] Sort, leave the playlist, come back: the sort is still there. Other playlists are not
      affected.
- [x] Drag-reorder is off while sorted and works again after "Clear".

## Columns and pages

- [x] Right-clicking the header row opens "Columns". Toggling Popularity or Plays persists per
      page type.
- [x] Narrowing the window (or opening the queue rail) hides the optional columns cleanly
      during the glide.
- [x] Album, Artist (top tracks and all tracks), Mix and search results sort by multiple levels.
      There is no Date added outside favorites and playlists, and no Album / Release date on an
      album page.

## Settings → Remember track sorting

- [x] *Until I leave*: lists always open in original order.
- [x] *Until restart*: sorts survive navigation but not a restart.
- [x] *Always*: sorts survive a restart.

## Modes

- [x] Atmosphere, Instrument and Rack: chips, rank dots and the `+` are legible, in both the
      light and dark themes.

**Closed 2026-10-26 (owner):** the remaining rows were marked done by the owner, not ticked row by row.
