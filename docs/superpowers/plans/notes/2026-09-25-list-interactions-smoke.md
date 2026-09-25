# List Interactions and Ownership Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and the official
> TIDAL app for cross-checking. Run with `npm --prefix ui run app:dev` (or the
> installed app).

## Automated checks (tree at aaedb14)
- cargo test --workspace: 261 passed, 0 failed, 1 ignored (oxitide-app 33, oxitide-audio 139 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 88, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 168 passed, 0 failed across 17 files
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

## Manual checks
- [ ] Open an editorial/unowned playlist from Home or Search → no Rename/Delete in the header, no Move/Remove row buttons, rows are not draggable, and the row context menu has no Move/Remove items.
- [ ] Open a playlist you own → Rename/Delete, row editing, drag reorder, and menu items are all present and work.
- [ ] Library playlists show Rename/Delete for your own playlists and the row menu lists Open/Rename/Delete.
- [ ] Copy title on a track row → paste elsewhere shows the track title.
- [ ] Copy title on a queue row → paste elsewhere shows the track title.
- [ ] Double-click a track row in Search, Album, Playlist, and Library favorites → playback starts at that row.
- [ ] Double-click on ♥, ＋, ⊕, ▶, or ✕ → no row playback is triggered by the double-click.
- [ ] Single-click ▶ still plays; text in track rows cannot be selected (no blue flash on double-click).
- [ ] Queue: single-click anywhere on a row (except ✕ and the drag handle) starts that track; Enter/Space on a focused row starts it; Space does not toggle play/pause when a queue row is focused.
- [ ] Queue: ✕ still removes the row; dragging still reorders; right-click still opens Play now / Copy title / Remove.

## Known caveats
- Ownership is creator-based: TIDAL collaborator playlists (editable but created by someone else) are treated as unowned and hide their edit controls.
- If TIDAL omits `creator` on a playlist fetch, the playlist is treated as unowned; on list endpoints the item stays owned.
- Copy title copies the raw title only (no artist).
- If the clipboard does not receive text in some WebView2 state, the fallback path may need the official clipboard plugin as a follow-up.
