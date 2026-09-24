# Plan 6 Smoke Checklist

> Requires a human, a logged-in TIDAL account, and a browser login. Run with
> `npm --prefix ui run app:dev` (or the installed app).

## Automated checks (tree at 3973a38)
- cargo test --workspace: 227 passed, 0 failed, 1 ignored (oxitide-app 32, oxitide-audio 139 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 55, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 53 passed, 0 failed across 6 files (format 2, quality 5, player 6, player.restore 3, pager 28, favorites 9)
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

## Manual checks
- [x] Hearts are filled for a track, an album, and an artist already in the library (check Album view, Artist view, Search results, PlayerBar).
- [x] Favorite a track from Album view → open Library → the track is in the list and the count matches.
- [x] Unfavorite a track from the Library list → the row disappears immediately and the count drops by one.
- [x] Unfavorite a track in Library, re-favorite it from the PlayerBar, then press Refresh → the track stays and the count is right.
- [x] Unfavorite a track from Album view → open Library → the row is gone and the count is right.
- [x] Favorite/unfavorite an album and an artist from their pages → Library reflects it on the next visit.
- [x] Network off → click a heart → it reverts and the notice strip shows the error; dismiss works.
- [x] Session expiry (delete the keyring entry, then click Library) → re-login → hearts hydrate again.
- [x] Restart the app → hearts still reflect the library.

## Known caveats
- A favorited track that TIDAL marks unavailable is omitted from the favorites list, so its heart can read hollow.
- Favoriting a track while the Library is already mounted does not insert the row until Refresh or a remount.
