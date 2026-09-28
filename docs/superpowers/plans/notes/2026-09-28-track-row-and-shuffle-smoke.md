# Track row rework + Play/Shuffle — Manual Smoke Checklist

Run against a real account in `npm --prefix ui run app:dev`. Record pass/fail
and notes per item.

## Gate Results

- cargo test --workspace: 61 + 152 + 1 + 102 passed, 0 failed (1 ignored, audio-device test)
- cargo clippy --workspace -- -D warnings: clean
- npm --prefix ui test: 23 files / 200 tests passed
- npm --prefix ui run check: 0 errors, 0 warnings

## Track rows and header

- [ ] Rows show, left to right: full-height cover, #, Title, Artist, Album,
      Time, Quality badge, then the add-to-playlist and heart icons at the far
      right. No other row icons.
- [ ] Clicking the cover plays that track (single click); double-clicking
      elsewhere on the row still plays it; the play overlay shows on hover and
      the cover gets an accent ring while playing.
- [ ] The header matches the columns (#, Title, Artist, Album, Time, Quality);
      Title and Time still cycle asc/desc/original; # resets the order. The
      header has no Play all button.
- [ ] Album page: the Album column is hidden, the cover column is still shown.
- [ ] Long titles/artists/albums ellipsize; a narrow window keeps the icons at
      the right edge without horizontal scroll.
- [ ] Right-click menu still offers Play, Add to queue, Add to playlist,
      Favorite, Copy title, and (owned playlist) Move up/down + Remove.
- [ ] Owned playlist: dragging a row (no grip icon) still reorders with the
      2 px indicator; moves are absent while a filter/sort is active.
- [ ] Compact density: rows stay 28 px aligned after scrolling a long list
      (no drift from the full-height covers).

## List height

- [ ] Playlist, Album, and Library (Favorite tracks): the table fills all
      leftover window height and scrolls inside itself; short lists grow with
      empty space instead of stopping mid-page.
- [ ] Search keeps its capped tracks table so the Albums/Artists/Playlists
      sections below stay reachable.

## Play all / Shuffle

- [ ] Album: the labeled Play button under the artwork starts at track 1;
      Shuffle starts on a random track and the PlayerBar shuffle toggle lights
      up active. Both sit under the image, not over it.
- [ ] Playlist: same buttons under the artwork. On a 2,000+ track playlist,
      Play waits for the full list and starts at track 1; Shuffle starts the
      full list in random order (not necessarily the first track).
- [ ] Library: play/shuffle icon buttons sit beside the "Favorite tracks"
      heading; Search: beside "Tracks".
- [ ] Buttons are disabled when the list is empty (empty playlist / album /
      search results with no tracks).
- [ ] With a sort or filter active, Play all and Shuffle use the visible order.
- [ ] Shuffle on an album then Next/Prev: playback follows the shuffled queue
      order; toggling the PlayerBar shuffle off leaves the order in place.

## Header frost and library editing

- [ ] A track list at rest (both modes): the sticky header is un-tinted and
      blends seamlessly with the surface above it — no lighter band, no top
      seam.
- [ ] Scroll a long track list (atmosphere mode): the header stays un-tinted
      while rows passing under it blur — cover thumbnails included; its bottom
      edge fades softly into the rows (shadow) instead of a hard line.
- [ ] Same in instrument mode (no blur): the opaque veil fades in once rows
      scroll under and still fades at the bottom edge.
- [ ] Library → Playlists, owned playlist row: a single pencil button sits at
      the row's right edge; clicking it opens the inline rename editor; no
      Rename/Delete text buttons remain. Right-click still offers Open/
      Rename/Delete (Delete in red).
