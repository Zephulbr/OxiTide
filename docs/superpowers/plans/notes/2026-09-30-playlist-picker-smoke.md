# Add-to-playlist picker — manual smoke checklist

GUI smoke checks for the PlaylistPicker layout pass: roomier right-aligned
rows, accent-tinted shared modal, New playlist on top, recently-added
ordering via TIDAL timestamps, and overflow handling (2026-09-30). Requires
a human, a real account, and a browser login. Run with
`npm --prefix ui run app:dev`.

## Layout

- [ ] Open the picker from a track row: "New playlist…" sits at the top with
      a plus glyph, followed by a thin separator, then the playlists.
- [ ] Rows have breathing room (not bullet-compact): title on the left,
      track count right-aligned, muted, and never hugging the title.
- [ ] Long playlist names ellipsize instead of wrapping, and the count stays
      flush right.
- [ ] A one-track playlist reads "1 track" (not "1 tracks").
- [ ] Clicking anywhere on a row (including the gap before the count) adds to
      that playlist.
- [ ] No bullet/list markers remain.

## Overlay recipe

- [ ] Play a track with bright artwork, open the picker and the track context
      menu side by side: both carry the same faint accent tint that follows
      the cover, neither turns light beige.
- [ ] Panel fill/border/blur match the context menu and search suggestions.
- [ ] Other dialogs (delete confirm, rename playlist/folder, create) still
      look correct with the shared tint — dark and legible over any content.
- [ ] Looks correct in both Atmosphere and Instrument modes.

## Backdrop and dismissal (all dialogs)

- [ ] Atmosphere mode: opening any dialog frosts the content behind it
      (slight 8px blur over the dim veil).
- [ ] Instrument mode: no blur on the backdrop — flat veil, matching the
      mode's no-glass look.
- [ ] Mousedown on the dimmed area closes the dialog; clicks inside the
      panel never do.
- [ ] Drag-selecting text inside a panel and releasing the mouse outside
      does not close it.
- [ ] Escape and the header X close every dialog as before.
- [ ] Check all four surfaces: Add to playlist, delete confirmation, the
      collection rename/delete/move dialogs, and New folder.
- [ ] Clicking the backdrop while an add is in flight closes the picker
      cleanly; the add still completes and its toast appears.

## Ordering (needs the real API)

- [ ] Add a track to a playlist, reopen the picker: that playlist is at the
      top (TIDAL's `lastItemAddedAt` reflected on reopen).
- [ ] Playlists never added to (`lastItemAddedAt: null`) fall back to
      `lastUpdated`; playlists without any timestamps keep the API's order.
- [ ] Create a playlist with tracks and reopen: it appears at the top.
- [ ] If TIDAL's list response omits the timestamp fields, the picker keeps
      the API order without errors (no sorting regression) — note the real
      response shape if so.

## Overflow

- [ ] With many playlists (or a short window), the list scrolls internally;
      the header, New playlist row, and separator stay pinned.
- [ ] The modal never grows past its max height; the page behind does not
      scroll when the wheel reaches the list's ends.
- [ ] Zero playlists shows "No playlists yet" under the New playlist row.

## Unchanged flows

- [ ] "New playlist…" opens the create form; Cancel returns to the list.
- [ ] Adding a track that exists already still opens the duplicates step, and
      Keep one / Add both / Confirm behave as before.
- [ ] The picker from the album header opens the same surface.
