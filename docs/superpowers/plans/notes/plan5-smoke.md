# Plan 5 Smoke Checklist

> Requires a human, a logged-in TIDAL account, an audio device, and the built
> installer. Run with `npm --prefix ui run app:dev` (or the installed app).

## Automated checks (tree at dd6622e)
- cargo test --workspace: 216 passed, 0 failed, 1 ignored (oxitide-app 31, oxitide-audio 139 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 45, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 26 passed, 0 failed across 5 files (format 2, quality 5, player 6, player.restore 3, pager 10)
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)
- npm --prefix ui run app:build: exit 0; `target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe` (4,349,769 bytes), `target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi` (6,377,472 bytes); `Test-Path` True for both

## Manual checks
- [x] Install → launch → upgrade-in-place → uninstall (see plan5-release.md).
- [x] Session expiry: while a track plays, revoke the session (TIDAL account → "Log out of all devices"; if unavailable, delete the keyring entry and wait for the access token to expire), then click Library. Expect "Session expired — sign in to continue"; after login the queue and position resume (paused if it was paused).
- [x] Network off while browsing: the view shows the error with Retry; restore the network and press Retry — the view loads.
- [x] Network off mid-stream: the player bar shows the error with Retry; restore the network and press Retry — playback restarts near its last position.
- [x] Window geometry survives a restart; a maximized window reopens maximized.
- [x] A saved position on a disconnected monitor is ignored: the window opens at the default position with its saved size (edit config.toml to an off-screen position to simulate).
- [x] %APPDATA%\OxiTide\logs\oxitide.<date>.log exists and grows; Settings → Logs shows the directory and Open logs folder opens Explorer there.
- [ ] Library with a large favorites list: the first page paints quickly, the
      list completes on its own (no scrolling required), the label reads
      "2141 tracks (225 unavailable)", and "Play all" queues the full list.
- [ ] Playlist paging behaves the same (the 704-track playlist loads without
      scrolling and reads "704 tracks"); revisiting Library/Playlist within a
      session does not refetch (session cache); scrolling a long list is smooth
      (virtualized rows).

## Notes

The manual run on 2026-09-24 passed everything except the two paging items:
paging stopped before the reported total (favorites 95/2366, playlist 682/702),
fixed on `fix/plan5-pager-short-pages` — see the findings below. Re-run those
two items after the fix and tick them.

## Smoke findings (2026-09-24)

- **Paging stopped before the reported total.** `loadMore` ended the walk on any
  short page (`page.items.length < pageSize`), but TIDAL returns short pages
  mid-list (the favorites first page carried 95 of a requested 100; the
  702-track playlist's page at offset 600 carried 82). The walk therefore
  stopped at 95/2366 favorites and 682/702 playlist tracks. Fixed by mirroring
  `TidalClient::get_all_pages`: trust `totalNumberOfItems` when it is reported,
  apply the short-page rule only when no total is reported, and always end on an
  empty page. `ensureAll` / Play all / Queue now walk the full list.
- **Playlists deduped repeated tracks.** The pager dropped ids already loaded,
  so a playlist that repeats a track lost the repeats from the table and the
  queue. `createTrackPager` now takes a `dedupe` flag; Library keeps the default
  (favorites cannot repeat) and Playlist passes `false`, so the queue matches
  the playlist exactly.
- **The favorites count includes unavailable tracks.** TIDAL's
  `totalNumberOfItems` counts tracks that are not available for playback (the
  API client libraries document this), while the endpoint only serves the
  available rows. The official TIDAL app shows 2141 for this library, matching
  what OxiTide loads, but the label presented the server total as a loading
  target ("2141 of 2366"). The pager now exposes
  `unavailable = total - items.length` once the walk is done, and Library and
  Playlist show "2141 tracks (225 unavailable)".
- **The v1 endpoint honors large page sizes; there is no cheap available
  count.** A temporary probe (removed) showed `limit=5000` returns the whole
  available list in one request (2141 items, total 2366), while 500 and 1000
  return ~90% of the requested window — proof that `offset` indexes the full
  list and unavailable rows are omitted, not that the limit is capped. The v2
  `userCollectionTracks` count reports the same inflated 2366, and the
  `my-collection/tracks` and `favorites/tracks` v2 paths 404, so no cheaper
  server-side available count exists.
- **Loading model.** The pager keeps the small first page (~100 rows) for a
  fast paint, then prefetches the remainder as disjoint 1000-row windows, four
  in flight, applied in offset order; above a 10 000-row reported total it
  stays lazy and scroll-paged. Favorites reach the exact
  "2141 tracks (225 unavailable)" label within roughly a second and the whole
  list is in memory, so scrolling is instant. Refresh reloads the same way.
- **The track table is virtualized.** `TrackTable` renders only the visible
  window of rows (36 px rows, sticky header, 8-row overscan), so a fully loaded
  list does not put thousands of rows in the DOM; scrolling the list also drives
  lazy paging (`onEndReached`) where the pager is in lazy mode.
- **Cross-session list persistence is deferred** (owner decision on
  2026-09-24): the session cache plus the prefetch keeps data fresh enough for
  now; an on-disk, per-account list cache is tracked as its own future task.
- Tests: `ui/src/lib/pager.test.ts` grew to 22 cases (short-page fix, in-flight
  sharing, `unavailable`, burst window math/order/concurrency/resumption, lazy
  fallback); 38 UI tests pass, svelte-check 0/0, Rust gates green. The two
  paging checklist items above were rewritten for the new behavior and are
  pending the owner's re-verification.
