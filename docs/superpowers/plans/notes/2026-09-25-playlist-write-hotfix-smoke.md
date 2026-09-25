# Playlist Write Hotfix Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and the official
> TIDAL app for cross-checking. Run with `npm --prefix ui run app:dev` (or the
> installed app).

## Automated checks (tree at c3fe7c7)
- cargo test --workspace: 254 passed, 0 failed, 1 ignored (oxitide-app 33, oxitide-audio 139 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 81, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 167 passed, 0 failed across 17 files
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

Counts recorded after the review fix pass (`c3fe7c7`), which pinned the persistent-412 and permission-401 outcomes with two integration tests (tidal 79 → 81).

## Manual checks
- [ ] Create a playlist with a track from the picker → it is created and the track lands (no 412 toast).
- [ ] Add tracks to an existing playlist (single and whole album) → no 412; order matches the official app.
- [ ] Remove and move rows in your own playlist → both succeed.
- [ ] Rename and delete your own playlist → both succeed.
- [ ] Rename or delete a playlist you do not own → superseded by the 2026-09-25 list-interactions smoke: the controls are now hidden; open one to confirm no edit UI appears.
- [ ] While a playlist is open, edit it in the official app, then remove/move a row here → the write either retries transparently or shows the stale-playlist copy, never raw JSON.
- [ ] Add more than 100 tracks (or a >100-track album) to a playlist → all land; the chunk boundary does not 412.

## Known caveats
- If TIDAL returns no `etag` on the playlist GET, writes fall back to the old header-less behavior and any 412 shows the friendly conflict copy.
- The retry is once per request by design; a second conflict surfaces to the user.
- The 412 retry refetches the playlist before retrying, so a concurrent edit from the official app may still reorder indices (pre-existing P7-D8 index caveat).
