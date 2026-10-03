# Artist page spike — live API findings (2026-10-03)

Feasibility probe run before the Track 9 artist-page spec. Confirms what the
API actually returns with the real OxiTide session token, so the spec is built
on payloads, not documentation.

## Method

Throwaway example `crates/oxitide-tidal/examples/artist_probe.rs` (deleted after
this note) loaded the real keyring token, refreshed it when needed, and dumped
raw payloads for every artist-page candidate endpoint. Samples: The Notwist
(`3529689`, the documented `/links` example) and AC/DC (`945`, resolved through
v1 search). Account country `GR`. Raw captures kept in
`.superpowers/artist-spike/raw-payloads-utf8.txt` (gitignored). No token
material was printed.

## Findings

### Fan count — AVAILABLE (decides the open question)

`GET openapi/v2/artists/{id}?countryCode={cc}` attributes include
`numberOfFollowers`:

- The Notwist: `9672`
- AC/DC: `1106540`

The public schema doesn't document it, but the host returns it with our
session token. The same attributes carry `popularity` (0.0–1.0), `ownerType`,
`spotlighted`, `contributionsEnabled`, and `externalLinks`. No extra request,
no internal endpoint.

### Bio — AVAILABLE

`GET openapi/v2/artists/{id}?include=biography&countryCode={cc}` returns an
`artistBiographies` resource in `included` with `source: "TiVo"` and `text`.
The v1 `artists/{id}/bio` endpoint also works and carries `lastUpdated` plus an
(empty) `summary`. Both use the same markup: `[wimpLink artistId=".."]Name[/wimpLink]`,
`[wimpLink albumId=".."]Title[/wimpLink]`, and `<br/>` line breaks.

Implication: one OpenAPI call with `include=biography` supplies fans, socials,
**and** bio; the v1 bio endpoint is redundant for this surface.

### Socials — AVAILABLE, coverage is artist-dependent

OpenAPI `attributes.externalLinks` is `[{ href, meta: { type } }]`:

- AC/DC: `TWITTER` → `https://twitter.com/acdc`, plus `TIDAL_SHARING`
- The Notwist: `TIDAL_SHARING` only (no socials — section stays hidden)

`TIDAL_SHARING` is a self-link, not a social and should be filtered out. The v1
`artists/{id}/links` endpoint is unreliable: AC/DC returned `[{siteName:
TWITTER, url}]`, The Notwist `404`. Implication: use `externalLinks` only; drop
v1 `/links` from the design.

### Banner art — NOT AVAILABLE as wide art (derived backdrop stands)

OpenAPI `profileArt` (`include=profileArt`) is the same **square** artist image
as v1 `picture` (750×750 max, 480/320/160 variants), with `blurHash` and
`selectedPaletteColor` metadata. The `pages/artist` header module also carries
only the square `picture`. Nothing returns a wide banner. The approved design
(palette gradient + blurred portrait, upgraded only if real art existed)
proceeds as the final look; no CSP changes needed — `resources.tidal.com` is
already allowlisted.

### Discography — CONFIRMED, filters behave as expected

`GET v1 artists/{id}/albums` totals per filter (`totalNumberOfItems`):

| Filter | The Notwist | AC/DC | Item types seen |
|---|---|---|---|
| (none) | 21 | 23 | ALBUM |
| `ALBUMS` | 21 | 23 | ALBUM |
| `EPSANDSINGLES` | 35 | 3 | SINGLE, EP |
| `COMPILATIONS` | 12 | 4 | ALBUM |

Items match the existing Rust `Album` DTO fields (`cover`, `artists`,
`releaseDate`, `numberOfTracks`, `duration`, `audioQuality`). `COMPILATIONS` is
"Appears on" (type ALBUM, not COMPILATION).

### Top tracks — CONFIRMED

`GET v1 artists/{id}/toptracks?limit=N` returns full `Track` objects; totals
can be large (Notwist 200, AC/DC 284), so the page fetches a bounded window
(5 shown, up to 10 expanded).

### Endpoints that did NOT work

- `GET v2/artists/{id}` → `not found` on both samples (internal v2 has no
  single-artist resource; the usual `get_v2` path is not applicable).
- `GET openapi/v2/artists/{id}/relationships/followers` works but returns
  paginated follower user ids, not a count — unnecessary given the attribute.

### Other payload notes

- v1 `artists/{id}` carries `mixes.ARTIST_MIX` (a `pages/mix`-playable mix
  uuid), `artistRoles`/`artistTypes`, `popularity` (0–100), `handle: null`,
  `userId: null`. Not needed for this design; the artist mix is a candidate
  future action.
- `pages/artist?artistId=` works (header module + rows/modules) but offers no
  fans, no wide art, and its page-item shape loses full track DTOs. Explicit
  endpoints stay cleaner and were chosen.

## Implications for the spec

1. Add one `ArtistProfile` DTO and one client method (`artist_profile`) joining
   the existing v1 artist fetch best-effort-style (same pattern as playlist
   follower counts) with
   `openapi artists/{id}?include=biography&countryCode={cc}`.
2. Scope drops: `artist_bio` command, `artist_links` command, banner/profileArt
   spike work.
3. Socials render only entries that exist and are not `TIDAL_SHARING`; unknown
   types fall back to a generic link icon + hostname.
4. Fan count is `Option<u64>`; UI formats compactly (`1,106,540` → `1.1M`).
5. Bio markup stripping (`wimpLink`, `<br/>`) is a pure Rust function with unit
   tests; both bio sources agree on markup.
