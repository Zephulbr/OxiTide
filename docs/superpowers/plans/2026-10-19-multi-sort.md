# Multi-level track sorting

Spec: `docs/superpowers/specs/2026-10-19-oxitide-multi-sort-design.md`. Probe:
`docs/superpowers/plans/notes/2026-10-19-multi-sort-probe.md`. Work on `main` in the owner's
checkout; do not commit without owner approval.

## Steps

1. **Rust DTOs** (`oxitide-tidal/src/models.rs`, `library.rs`):
   - add `Track.date_added` (`dateAdded`), `Track.popularity` and `AlbumRef.release_date`
     (`releaseDate`), all `Option` with `skip_serializing_if`;
   - favorites copy the wrapper's `created` into `date_added`.

   Tests: wiremock favorites page carries `created`; the playlist track fixture round-trips
   `dateAdded` / `album.releaseDate` / `popularity`.
2. **Album release dates** (`oxitide-tidal/src/catalog.rs` `album_release_dates(ids)`, public v2
   `albums?filter[id]=`). The `oxitide-app/src/album_dates.rs` store follows the `track_flags.rs`
   pattern: memory map + `cache/album_dates.json`, batches of 20, ids the API omits are
   negative-cached, fail open, only `Unauthorized` surfaces.

   Command `get_album_release_dates(ids)`. Tests: batching, caching, fail-open, disk round-trip.
3. **Play counts:** `HistoryStore::play_counts(ids)` counts finalized records per track id.
   Command `get_play_counts(ids)`. Test: counts and unknown ids.
4. **Wire up:** `main.rs` (`generate_handler!` + store setup), `remote/rpc.rs` `NOT_EXPOSED`,
   and `api.ts` types and wrappers.
5. **Pure sort core** (`ui/src/lib/trackSort.ts`):
   - keys, labels, default directions, `MAX_LEVELS`;
   - level operations (`setPrimary` cycle, `addLevel`, `flipLevel`, `removeLevel`, `moveLevel`);
   - the comparator (missing values last in both directions, then source order);
   - per-table key availability (`keysFor(kind)`).

   Tests in node.
6. **Extra sort data** (`ui/src/lib/sortData.svelte.ts`): reactive `plays` and `releaseDates`
   maps. `ensure(keys, tracks)` fetches only what is missing; playlist embeds seed
   `releaseDates`. Tests with a mocked api.
7. **Sort memory** (`ui/src/lib/sortMemory.svelte.ts`):
   - mode `page` / `session` / `always`, stored in `localStorage` (`oxitide.track-sort-memory`),
     with saved sorts under `oxitide.track-sorts` (always mode only);
   - per-page keys as in MS-D13.

   Tests with fake storage.
8. **`TrackView`:**
   - `levels` (UI state) vs `applied` (what `items` sorts by, swapped in after any data load);
   - `memoryKey` option, with restore deferred to a microtask (Playlist builds its view inside
     `$derived`);
   - `toggleSort` stays as an alias of `setPrimary` (perf hooks).

   Update `trackView.test.ts`.
9. **Columns** (`ui/src/lib/trackColumns.svelte.ts`): optional `added` / `released` /
   `popularity` / `plays`. Visibility is per table kind in `localStorage`; Date added is on by
   default. A sorted key's column always shows.
10. **UI:**
    - `SortBar.svelte`: chips with flip, ×, and Alt+←/→ to reorder; "+ Add sort" popover; a quiet
      "Sort" button when there are no levels.
    - `TrackTable.svelte`:
      - a `kind` prop;
      - Artist / Album / Quality / optional columns become sortable;
      - rank badges, plus a hover/focus "+" on each sortable header (Shift+click adds a level);
      - a Columns context menu on the header;
      - the optional columns hide under narrow container widths;
      - visible rows ensure plays / release dates.
    - Views pass `kind` and `memoryKey`.
    - Settings gets "Remember track sorting" (+ `settingsIndex.ts`).
11. **Docs:** `ui/design/DESIGN.md` (sort bar, rank badge, header "+"), and the spec status.
12. **Gates:**
    - `cargo test --workspace`
    - `cargo clippy --workspace -- -D warnings`
    - `npm --prefix ui run check`
    - `npm --prefix ui test`

    The owner check in the running app stays pending.
