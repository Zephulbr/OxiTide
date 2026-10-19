# Multi-level sorting — live payload probe (2026-10-19)

- **Authority:** `docs/superpowers/specs/2026-10-19-oxitide-multi-sort-design.md` §6
- **Method:** a throwaway `oxitide-tidal` example (deleted after the run, never committed) loaded
  the stored tokens through `KeyringStore`, and fetched through `TidalClient::get` /
  `get_openapi` with `limit=3`. It printed key names and date/number values only, with no titles
  and no tokens. The app was not running. Automated tests stay wiremock-only; this note is the
  only live capture.
- **Session region:** `countryCode=GR`.

## Results (first item of each response)

| Request | `dateAdded` | `popularity` | `album.releaseDate` | `streamStartDate` |
|---|---|---|---|---|
| `users/{id}/favorites/tracks` | absent; wrapper `created` = `2025-12-16T02:34:19.199+0000` | 32 | **absent** (album: `cover, id, title, vibrantColor, videoCover`) | `2021-08-27T00:00:00.000+0000` |
| `playlists/{uuid}/tracks` | `2025-08-06T17:50:35.372+0000` | 71 | **`1992-09-21`** | `2008-06-02T00:00:00.000+0000` |
| `playlists/{uuid}/items` (`{cut, item, type}`) | same as `/tracks` | 71 | `1992-09-21` | same |
| `albums/{id}/tracks` | absent | 8 | absent | `2021-08-27T00:00:00.000+0000` |
| openapi `albums?filter[id]=` | — | (album-level) | `releaseDate` = `2021-08-27` | — |

Playlist track objects also carry `index` and `itemUuid`. Every v1 track object carries `bpm`,
`key` and `keyScale` as well, which could become future sort keys.

## Conclusions

- **Date added:** favorites use the wrapper's `created`, and playlists use `dateAdded` on the
  existing `/tracks` endpoint. No loader switch is needed.
- **Popularity:** present on every v1 track (0–100).
- **Release date:** embedded in the album object only for playlist tracks. Favorites, album,
  mix and search tracks need the openapi `albums?filter[id]=` batch (cached on disk).
- **`streamStartDate` is not a usable release date.** The playlist sample shows 2008 against an
  actual 1992 release. It stays a last resort only.
