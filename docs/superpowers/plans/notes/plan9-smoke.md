# Plan 9 — Manual Smoke Checklist

Run against a real account in `npm --prefix ui run app:dev`. Record pass/fail
and notes per item; a failed item blocks the plan until fixed or explicitly
accepted.

## Gate Results

- cargo test --workspace: 33 + 139 + 1 + 74 passed, 0 failed (1 ignored, audio-device test)
- cargo clippy --workspace -- -D warnings: clean
- npm --prefix ui test: 17 files / 160 tests passed
- npm --prefix ui run check: 0 errors, 0 warnings

## Sort and filter

- [ ] Open a playlist with 2,000+ tracks. Type in the filter field: the header
      shows "Loading all tracks…", then the list filters over ALL tracks (not
      just the first page); the count line is unchanged.
- [ ] Click Title: ascending ▲, click again descending ▼, click again back to
      original order. Click # while a filter is active: order resets, filter
      stays.
- [ ] Sort by Time ascending/descending; ties keep original order.
- [ ] Filter by an artist name that only appears in the artist column; matching
      rows show.
- [ ] Filter with no matches shows `No tracks match “…”`; clear it with ✕.
- [ ] Remove a row while filtered: the correct track disappears after refresh;
      no moves are offered in the row menu (Remove/Favorite remain).
- [ ] Navigate away and back: sort/filter state is reset; rows hydrate from the
      session cache without a full refetch flash.
- [ ] Repeat on Library → Favorite tracks and on an Album (instant, no loading
      row); Search results table gets the same controls.

## Live search

- [ ] Type "ra" quickly into Search: results appear without Enter after ~300 ms.
- [ ] Type fast through several queries: the final result set matches the final
      text (no older result flashing in).
- [ ] Press Enter with one character: search runs immediately.
- [ ] Clear the box: results clear.
- [ ] A no-match query shows `No results for “…”`; a failed request shows the
      inline error with Retry.
- [ ] `Ctrl+F` still focuses the input; Space/arrows in the input edit text and
      do not control playback.

## Drag reorder

- [ ] Drag a playlist row by the ⠿ handle: a 2 px indicator line tracks the
      insertion slot; the source row dims.
- [ ] Drop mid-list: the row lands there instantly (no whole-table reload
      flash); the server agrees after navigating away and back.
- [ ] Drop a row onto itself or just below the last row: nothing moves.
- [ ] Scroll a long playlist, drag a row near the top/bottom edge: the list
      auto-scrolls and the drop lands where the indicator showed.
- [ ] While a filter/sort is active, the ⠿ handle and ↑/↓ buttons are absent;
      menu has no Move items.
- [ ] ↑/↓ buttons and menu Move still work in original order and are instant.
- [ ] Force a move failure (disconnect the network): an error toast appears and
      the list returns to server order.

## Virtualization

- [ ] Queue an entire large playlist, open the queue rail: scrolling is smooth,
      rows are 40 px aligned, and drag reorder + jump/remove/menu still work.
- [ ] Library playlists and albums scroll within their boxes; rename/edit/
      delete/menu still work; scrolling an edited row out of view and back
      keeps it editable.
- [ ] Rows are pixel-aligned at 36 px in TrackTable/albums, 40 px in the queue,
      44 px in Library playlists.
- [ ] Empty states show for an empty queue, no playlists, and no albums.
