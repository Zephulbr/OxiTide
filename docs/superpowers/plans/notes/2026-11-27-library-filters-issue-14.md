# Albums and Playlists filters (issue #14)

**Status:** Done on `release/1.x` (`a355ffd6`), ships in v1.1.11; owner smoke passed
2026-11-27 on a local build of `a355ffd6`.

Issue: https://github.com/Zephulbr/OxiTide/issues/14 (a filter box on Albums, as in TIDAL's
own app).

## Scope

- **Albums:** a filter box beside the sort menu; matches the album title or any credited
  artist (`filterAlbums` in `ui/src/lib/albumSort.ts`).
- **Playlists (root and folders):** matches playlist and folder names in the page's folder
  and every folder below it (`ui/src/lib/collectionSearch.ts`). This level answers at once;
  the folders below are walked (at most 200 folders, 5 levels, 6 requests at a time) when
  the box takes focus or a query is typed, through `fetchRecent` in `lib/viewCache.ts`, so
  the folders the grid's cover mosaics just loaded are not asked for again. The walk is kept
  for later keystrokes and redone when the page's list reloads (an edit, a sort change).
- **Not done, on purpose:** Mixes (mostly TIDAL's own rows; favourite mixes are few) and
  Purchases (the page loads only the first 100, so a filter would miss items silently).
- Not remembered across navigation: Back to Albums shows the whole grid again, as on Tracks
  and Profiles.
- `users/<id>/playlists` (the picker's flat list) was not used: it holds owned playlists
  only, not favourited ones filed in the collection.
- Seven new `app.json` messages, machine-drafted for the 23 shipped languages.

## Owner smoke

On a build of this branch, signed in, with at least one folder holding a playlist (ideally a
folder inside a folder):

- [x] Albums: type part of an album title, then an artist name with its accent left out
      ("beyonce"); the grid narrows, and the count under the title stays the total. Type
      nonsense: "No albums match …". Clear it: the whole grid is back, in the chosen sort.
- [x] Playlists (All playlists): type part of the name of a playlist that lives in a folder.
      It shows with "<folder> · N tracks" under it; open it, play it from its cover, and
      check its menu's Move dialog starts from that folder.
- [x] Playlists: a folder name matches too and opens the folder.
- [x] Inside a folder: the box searches that folder (and any below it) only.
- [x] Rename or move a playlist, then search for it: the new name/folder shows.
- [x] Hi-Fi Rack mode: the box takes the Rack field skin on both pages.
