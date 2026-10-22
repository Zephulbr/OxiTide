# Drag to playlist — owner smoke checklist (pending)

Spec `2026-10-22-oxitide-drag-to-playlist-design.md`, plan `2026-10-22-drag-to-playlist.md`;
run with `npm --prefix ui run app:dev`, signed in. Windows first; the Linux items join the
batched bare-metal run.

## Multi-select (any track table)

- [ ] A click on a row selects it; Ctrl+click toggles; Shift+click selects a range; a click
      on the cover, a name link or a row button does not change the selection.
- [ ] Ctrl+A selects all loaded rows; Escape clears; Shift+↑/↓ extends; Ctrl+Space toggles.
- [ ] Double-click and Enter still play; Space still plays.
- [ ] Changing the sort or the filter clears the selection; scrolling to load more keeps it.
- [ ] Right-click inside a selection of 2+: "Play N next", "Add N to queue",
      "Add N to playlist" (opens the picker with all of them).
- [ ] Owned playlist: "Remove N from playlist" removes exactly those rows (also while sorted).
- [ ] Right-click on an unselected row selects it and shows the usual track menu.

## Drag to a playlist

- [ ] One row → an owned sidebar playlist: the row gets the accent ring, the ghost says
      "Add to …", it adds without asking, a toast confirms with **Open**, and the sidebar
      count updates.
- [ ] A 5-row selection → playlist: "Add 5 tracks to …?" appears; **Add** lands all five in
      table order; **Cancel** (or Escape) adds nothing.
- [ ] Album card, playlist card, mix card → playlist: the confirm names the collection
      ("Add 12 tracks from … to …?"). A track card (Home) adds at once.
- [ ] A single (one-track album) → playlist: adds at once, no confirm.
- [ ] Album / playlist / mix page header cover → playlist (confirm shown).
- [ ] Sidebar playlist row → another sidebar playlist (confirm shown); → itself is refused
      ("Already in this playlist"); → the queue inserts it. The row doesn't open after the drag.
- [ ] Player bar cover → playlist, and player bar title/artist → playlist (the playing track,
      no confirm). A plain click on the title still opens its album.
- [ ] A favorited playlist you don't own: dimmed, "You can't edit this playlist", nothing
      happens on release.
- [ ] Rows of a playlist dropped on that same playlist's row: refused ("Already in this playlist").
- [ ] A drop that hits tracks already in the playlist opens the duplicates step; both
      "Keep one" and "Add both" work, "All" sets every row, "Add N copies" counts the
      choices, and "Skip duplicates" closes without a second add.
- [ ] Very long playlist / album names wrap to two lines in the confirm and duplicates
      dialogs (no sideways scroll); hovering shows the full name.
- [ ] A big playlist card (500+ tracks) → playlist: the spinner shows over the target's
      cover until the add finishes.
- [ ] Hovering a collapsed folder for ~0.6 s opens it; a nested playlist accepts the drop.
- [ ] Collapsed sidebar rail: resting on it for ~0.4 s peeks it open (with hover panels
      both off and on); it closes when the drag ends.
- [ ] Long sidebar: holding the drag near its top/bottom edge scrolls it.
- [ ] Library → Playlists grid: a card gets the cover ring and accepts the drop.
- [ ] Library → Playlists grid: a card (cover or title) drags its playlist onto a sidebar
      playlist or another card; the title underlines on hover and still opens on click.
- [ ] With an owned playlist's page open, drag the player-bar cover onto that playlist's
      sidebar row: the page's list and header count refresh in place.

## Drag to the queue

- [ ] Queue open, music playing: dropping between two rows inserts there (insertion line
      shown) without restarting or skipping the playing track.
- [ ] Dropping above the playing track keeps it playing and highlighted.
- [ ] Dropping below the last row appends; dropping on an empty queue fills it (nothing starts).
- [ ] Content-filtered tracks in a dropped album are left out.
- [ ] Floating queue card and the hover-peek queue accept drops too.

## Reorder (owned playlist)

- [ ] Dragging one row up and down reorders it; the drop line shows the slot.
- [ ] Dragging a multi-row selection inside the table says "Can't move several rows yet"
      (until the Task 1 spike lands).
- [ ] Reorder is off while sorted/filtered (rows can still be dragged to other targets).

## Gesture details

- [ ] Escape mid-drag cancels; nothing is added and nothing opens.
- [ ] Releasing over empty space does nothing.
- [ ] A drag never opens the card, plays the row, or navigates the tree row it started on or
      ended over.
- [ ] Alt+Tab mid-drag cancels cleanly; the next click works normally.
- [ ] Text never gets selected during a drag; the cursor is "grabbing" / "no-drop".
- [ ] The queue rail's own row reorder still works as before.

## Shells and modes

- [ ] Rack shell: the playlist tree accepts drops; the queue drawer/changer accepts drops;
      the ghost is readable in the skin.
- [ ] Atmosphere, Instrument and Rack, light and dark: ghost, ring, selection tint legible.
- [ ] Reduced motion: no ghost scale-in; springs still open.
- [ ] Perf: no frame loop or listeners at rest (PerfHud idle CPU unchanged after a few drags).
