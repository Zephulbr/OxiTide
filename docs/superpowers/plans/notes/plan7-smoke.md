# Plan 7 Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and the official
> TIDAL app for cross-checking. Run with `npm --prefix ui run app:dev` (or the
> installed app).

## Automated checks (tree at 1f86ed9)
- cargo test --workspace: 244 passed, 0 failed, 1 ignored (oxitide-app 33, oxitide-audio 139 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 71, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 71 passed, 0 failed across 8 files (format 2, quality 5, notice 3, playlists 12, favorites 12, player.restore 3, player 6, pager 28)
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

## Manual checks
- [ ] Create a playlist from Library → it appears in Library and in the official TIDAL app.
- [ ] Create a playlist from the picker on an album ("New playlist…") → the album's tracks are added and the app navigates to the new playlist.
- [ ] Add a single track from Search, Album, Library, and Playlist views → the picker lists playlists with correct counts → the notice reports the number added.
- [ ] Add a whole album from the Album header → every track lands, order preserved.
- [ ] Add tracks that already exist → the duplicates step shows both cards per collision; "Keep one" leaves the playlist unchanged; "Add both" duplicates the track (verify in the official app); "Add all duplicates" applies to every row.
- [ ] Remove one of two identical rows in a playlist → only the clicked occurrence disappears (verify counts in the official app).
- [ ] Move a row up and down → order matches the official app after the refetch; buttons are disabled at the first/last row and while loading.
- [ ] Rename a playlist from Library (inline) and from the Playlist header → both surfaces and the official app agree.
- [ ] Delete a playlist from Library and from the Playlist view → gone from both; deleting the open playlist navigates to Library.
- [ ] Delete the playlist that is currently playing → playback continues unaffected.
- [ ] Unavailable-row index probe: open a playlist that contains an unavailable track, then remove/move the row after it → the intended track changes (not its neighbor).
- [ ] Large playlist (>1k tracks): open, add, remove, and move → refetch completes without a visible stall; editing buttons are disabled until the list is done.
- [ ] Network off → any write shows the notice with the error and changes nothing.
- [ ] Session expiry during a write → re-login; the picker is closed and later edits work.
- [ ] Restart the app → playlists and their contents match the official app.

## Known caveats
- Writes are index-based; a concurrent edit from the official app between load
  and write can shift indices — the refetch after the write shows the result.
- No `If-None-Match`/ETag guard yet (P7-D8).
- Only your own playlists are editable; TIDAL rejects others with 403.
- Pre-existing duplicates are never auto-deduped.
- If TIDAL omits `addedItemIds`, the client assumes every requested track was
  added and the duplicate dialog is skipped.
