# OxiTide — Multi-level track sorting (2026-10-19)

- **Status:** Implemented 2026-10-19 (plan `docs/superpowers/plans/2026-10-19-multi-sort.md`);
  gates green; owner check in the running app pending. Deviations from the draft are listed
  in §7.
- **Owner:** V
- **Scope:** Every sortable `TrackTable`: Favorites (`CollectionTracks.svelte`), playlists
  (`Playlist.svelte`), Album, Artist top tracks, Artist "all tracks", Mix, and Explore search
  results. Each table offers only the keys its rows have data for (MS-D11). Rack gets the
  shared bar unchanged for now (owner: revisit later). The phone remote is out of scope.

## 1. Problem

Track tables sort through their headers by one key only (`TrackSort = "original" | "title" |
"duration"`, `trackView.svelte.ts`). A secondary sort is meaningless with those two keys
(titles and lengths almost never tie), and there is no way to say "by artist, then newest
added first". The sort also resets every time the page unmounts.

## 2. Decisions

| # | Decision |
|---|---|
| MS-D1 | **Keys:** Title, Artist, Album, Time and Quality, which the client already has, plus Date added, Release date, Popularity and Plays (§4). Keys with no column live only in the sort bar. |
| MS-D2 | **Persistence:** the sort is remembered per page (`favorites`, `playlist:<uuid>`) in memory until the app restarts. A setting, **Track sorting → Remember**, offers *Until I leave the page* / *Until the app closes* (default) / *Always*. *Always* persists to `localStorage` like `playlistView.svelte.ts`. Only the sort is remembered, never the filter text. |
| MS-D3 | **No implied levels:** adding a key adds exactly that key. The hidden tiebreak is source order only, as it is today. |
| MS-D4 | **Rack:** uses the same bar through its semantic tokens; a Rack-specific skin is deferred. |
| MS-D5 | **Levels:** at most 3. Adding a fourth replaces the last level. |
| MS-D6 | **Header interaction:** clicking a sortable header makes it the only key (asc → desc → off, as today). Once a sort is active, other sortable headers show a `+` button on hover/focus that appends them as the next level; Shift+click does the same. An active header shows its rank and direction (`1▲`, `2▼`) with the accent colour, and `aria-sort` stays on the primary only (ARIA allows one). |
| MS-D7 | **Sort bar** (next to the filter, shown only while a sort is active, plus a quiet `Sort` button when none is): one chip per level, in order. Clicking a chip flips its direction and its `×` removes it. `+ Add sort` opens a `SortSelect`-style list of the unused keys. The `#` header still clears all. Chips can be reordered with Alt+←/→ (keyboard) and by dragging. |
| MS-D8 | **Default directions:** text keys ascending; Date added, Release date, Popularity and Plays descending (newest/most first); Time and Quality ascending. |
| MS-D9 | **Missing values** (no release date, no plays yet) sort last in either direction, so flipping a key does not bring the blanks to the top. |
| MS-D10 | While any sort is active, playlist drag-reorder and disc sections stay off (existing behaviour, `engaged`). Playing a row plays the visible order (existing). |
| MS-D11 | **Per-table keys:** a table offers a key only when its rows carry that data and it can differ between rows. Date added appears only in Favorites and playlists. Release date is left out on the Album page, where every row has the same date. Album is left out on the Album page. Search results get the keys but no memory, since the query changes the list. |
| MS-D12 | **Columns:** Date added, Release date, Popularity and Plays become table columns. Date added is shown by default where it exists. The others are hidden by default and toggled from a **Columns** submenu on the header's context menu; the choice persists per table kind (`[ui] track_columns`). Sorting by a hidden column's key shows that column. Column widths come from CSS container queries (AGENTS: no JS width counting), and the optional columns drop out first on narrow widths. |
| MS-D13 | **Memory keys:** `favorites`, `playlist:<uuid>`, `album:<id>`, `artist:<id>`, `artist-all:<id>`, `mix:<id>`. Search results are not remembered. |

## 3. Engine

`TrackView` changes from `sort` + `direction` to `levels: { key: TrackSortKey; dir }[]`
(empty = original order). The API becomes `setPrimary(key)`, `addLevel(key)`,
`flipLevel(i)`, `removeLevel(i)`, `moveLevel(i, to)` and `clearSort()`. The existing
`toggleSort` maps to `setPrimary` for the other tables. `applyView` builds one comparator
that walks the levels and falls back to source index. Per-track keys (collation strings,
quality rank, parsed dates) reuse the `WeakMap` caches and `warmTrackSearch`.

Keys that need extra data (§4) join `ensureAll`: engaging such a key first awaits its loader,
using the same `preparing` / `error` / `retry` states as loading the full list does now.

Persistence is a small `lib/sortMemory.svelte.ts` module: an in-memory map keyed by page id,
reading and writing `localStorage` only in *Always* mode. *Until I leave the page* skips the
map entirely. Pages pass their key to `createTrackView`.

## 4. Data sources for the new keys

| Key | Source | Cost |
|---|---|---|
| Artist / Album | `Track.artists[0].name` / `Track.album.title` (collated; Album tiebreaks on volume then track number, since that is "album order" and not an extra level) | none |
| Quality | `audioQuality` + `HIRES_LOSSLESS` tag, ranked LOW < HIGH < LOSSLESS < HI_RES | none |
| Date added | Verified (§6). Favorites: the `created` field on the `{created, item}` wrapper, which `library.rs` currently discards in `Favorited<T>`. Playlists: `dateAdded` on each track object of the existing `playlists/{uuid}/tracks`. Carried as `Track.dateAdded: Option<String>` (`skip_serializing_if`). | none at runtime; Rust DTO + `api.ts` change |
| Popularity | Verified (§6): every v1 track object carries `popularity` (0–100). Add `Track.popularity: Option<u32>`. This is global TIDAL popularity, not personal, and the column header says so in its tooltip. | none |
| Plays | Local history (`history.jsonl`): a new command `history_play_counts(ids) -> {id: count}` counted from `HistoryStore`. Only covers plays since history was enabled and within `history.max_entries`. Plays from other devices do not count. Refreshed when the history changes. | one IPC call per engage |
| Release date | Verified (§6). **Playlists:** the embedded `album.releaseDate` is present, so add `AlbumRef.release_date` (free). **Favorites, Artist, Mix, search:** the embed lacks it, so batch public v2 `albums?filter[id]=…` (the `flags.rs` pattern, verified to return `releaseDate`) for the list's distinct album ids that have no embedded date. Cache them on disk (`cache/album_dates.json`, since dates never change) and in memory, and seed the cache from playlist embeds. **Last resort** on failure: v1 `streamStartDate`, which the probe showed as wrong for old catalogue (2008 against a 1992 release), so these rows are marked as approximate. | free in playlists; ~N/20 calls for N uncached albums elsewhere |

New command(s) must be listed in `generate_handler!` and classified in `remote/rpc.rs`
(`NOT_EXPOSED`).

## 5. Files (expected)

- Rust: `oxitide-tidal/src/models.rs` (`date_added`, `popularity`), `library.rs`
  (keep `created`), `playlists.rs` (keep `dateAdded`), a release-date batch call;
  `oxitide-app/src/history.rs` + `commands.rs` (`history_play_counts`, album dates),
  `main.rs`, `remote/rpc.rs`; config `[ui] track_sort_memory`.
- UI: `lib/trackView.svelte.ts` (+ tests), `lib/sortMemory.svelte.ts`,
  `lib/components/TrackTable.svelte` (headers, `+`), new `lib/components/SortBar.svelte`,
  `views/CollectionTracks.svelte`, `views/Playlist.svelte`, `views/Album.svelte`,
  `views/Artist.svelte`, `views/ArtistAll.svelte`, `views/Mix.svelte`,
  `views/Explore.svelte`, `views/Settings.svelte` +
  `lib/settingsIndex.ts`, `lib/api.ts`; `ui/design/DESIGN.md` (sort bar, rank badge).

## 6. Verification (done)

Live probe on 2026-10-19: `docs/superpowers/plans/notes/2026-10-19-multi-sort-probe.md`.
- Favorites `created` and playlist `dateAdded` are present, so the playlist loader stays on
  `/tracks`.
- `popularity` is on every track.
- `album.releaseDate` is embedded only in playlist tracks; every other table uses the v2 batch.
- The probe also found `bpm`, `key` and `keyScale` on every track. They are candidate future
  keys, out of scope here.

## 7. As built (deviations from the draft)

- **Storage:** both prefs live in `localStorage`, like the sidebar playlist-view prefs, rather
  than the TOML `[ui]` config. Remember mode and always-mode sorts use
  `oxitide.track-sort-memory` / `oxitide.track-sorts`; columns use `oxitide.track-columns`.
  They are UI-only and need no Rust command.
- **Header click on an active key:**
  - when it is the only level, the click cycles it (default → flipped → off);
  - when it is one of several, the click flips it in place instead of replacing the others, so
    the primary's direction can be changed from its header.

  Shift+click (or `+`) on an active key also flips it.
- **Plays:** a count of 0 is a real value (sorting ascending puts the least-played first). Only a
  count that has not loaded yet sorts last.
- **Album level:** it groups by album title. After all levels compare equal, tracks fall back to
  album id, disc and track number, then source order. This keeps each album in track order and
  stops same-named albums from interleaving.
- **Chip reordering** is Alt+←/→ only; drag reordering was dropped.
- **Release dates:** embedded playlist dates are read directly rather than seeded into the
  store. Other lists fetch dates for the visible rows when the column is shown, and for the
  whole list when the sort is engaged. Albums whose lookup failed are retried on the next
  engage.
- **The `+` at the cap:** it stays available at three levels and replaces the last level
  (MS-D5).
- **Settings:** "Remember track sorting" is a `Segmented` (Until I leave / Until restart /
  Always) in Look & feel → Typography & layout.
