# Track 9.2 — API research spike (credits + album info)

- **Date:** 2026-10-06
- **Status:** Spike complete — public v2 credits and album facts are accessible; Tasks 3/4 run as written (one credits-pagination caveat recorded under Decision 1)
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §5.3; plan `docs/superpowers/plans/2026-10-06-track9-2-credits-album-info.md` Task 0
- **Parent:** `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 9

## Method

- A temporary `debug_get_openapi` Tauri command (removed before this note was committed) forwarded
  raw paths/includes to `TidalClient::get_openapi` (`openapi.tidal.com/v2`, `countryCode` from the
  live session). It was driven over the WebView2 CDP endpoint by a throwaway Node probe
  (`.superpowers/sdd/2026-10-06-track9-2-credits-album-info/probe.mjs`, gitignored scratch) against
  the debug app with a real logged-in account. Raw output: `probe-output.txt` beside it.
- **Session region:** `countryCode=GR`. No tokens were logged; the payloads contain none. Automated
  tests stay wiremock-only — this note is the only live capture.
- **Sample:** track `564178214` "Draw You Out" (from `get_home` → "New Tracks") and album
  `565212937` "Rubber Soul (Super Deluxe)" (from `get_home` → "New Albums").
- All four requests returned **HTTP 200 JSON**. No 403/400/empty result was observed.

## Probe a — `GET tracks/564178214?include=credits.artist` (200)

```json
{
  "data": {
    "id": "564178214",
    "type": "tracks",
    "attributes": {
      "title": "Draw You Out",
      "duration": "PT4M16S",
      "explicit": false,
      "isrc": "USUG12608043",
      "copyright": { "text": "℗ 2026 Mercury Records, a division of UMG Recordings, Inc." }
    },
    "relationships": {
      "credits": {
        "data": [
          { "id": "XAl6FHNtoYJXU2RAWHioDQZUPlH68187toaLkatTQg7oYkC1tMr2TX6t86A", "type": "credits" },
          { "id": "XAl6FHNtoYJXU2RAgzmtkZEUabrkHz5j1iFHd8JMQXeDqRFAcLyVdlBPQuY", "type": "credits" },
          "…18 more credit refs on this page…"
        ],
        "links": {
          "meta": { "nextCursor": "3nI1Esi" },
          "next": "/tracks/564178214/relationships/credits?countryCode=GR&page%5Bcursor%5D=3nI1Esi",
          "self": "/tracks/564178214/relationships/credits?countryCode=GR"
        }
      }
    }
  },
  "included": [
    {
      "id": "XAl6FHNtoYJXU2RAWHioDQZUPlH68187toaLkatTQg7oYkC1tMr2TX6t86A",
      "type": "credits",
      "attributes": { "name": "Aaron Dessner", "role": "Producer" },
      "relationships": { "artist": { "data": { "id": "5957579", "type": "artists" } } }
    },
    {
      "id": "XAl6FHNtoYJXU2RAV5oFuVI0dnZW064gymtoVOWg1jtOQGD7dnTxTTT3x6h",
      "type": "credits",
      "attributes": { "name": "Aaron Dessner", "role": "Banjo" },
      "relationships": { "artist": { "data": { "id": "5957579", "type": "artists" } } }
    },
    "…18 more credit resources (name/role + optional relationships.artist.data.id)…",
    {
      "id": "5957579",
      "type": "artists",
      "attributes": { "name": "Aaron Dessner", "numberOfFollowers": 993 }
    },
    "…5 more artist resources (12933242, 16889484, 8006304, 8231320, 8451939)…"
  ],
  "links": { "self": "/tracks/564178214?include=credits.artist&countryCode=GR" }
}
```

- Page 1 carries **20 credit resources**, each with `attributes.name` + `attributes.role` and a
  `relationships.artist.data.id` (see Decision 3), plus the referenced `artists` resources.
- The credits relationship is **cursor-paginated**: `links.meta.nextCursor` is present, so 20 is a
  page size, not necessarily the full credit list (see Decision 1 caveat).

## Probe b — `GET tracks/564178214` (200)

```json
{
  "data": {
    "id": "564178214",
    "type": "tracks",
    "attributes": {
      "title": "Draw You Out",
      "duration": "PT4M16S",
      "explicit": false,
      "isrc": "USUG12608043",
      "copyright": { "text": "℗ 2026 Mercury Records, a division of UMG Recordings, Inc." }
    }
  },
  "links": { "self": "/tracks/564178214?countryCode=GR" }
}
```

- No `relationships` node without the include, as expected; track metadata (`title`, `duration`,
  `explicit`, `isrc`, `copyright`) is present.

## Probe c — `GET albums/565212937` (200)

```json
{
  "data": {
    "id": "565212937",
    "type": "albums",
    "attributes": {
      "title": "Rubber Soul (Super Deluxe)",
      "version": "Super Deluxe",
      "type": "ALBUM",
      "albumType": "ALBUM",
      "releaseDate": "2026-10-02",
      "copyright": { "text": "© 2026 Apple Corps Ltd" },
      "barcodeId": "00881061135678",
      "numberOfVolumes": 4,
      "numberOfItems": 68,
      "duration": "PT2H53M39S",
      "explicit": false
    }
  },
  "links": { "self": "/albums/565212937?countryCode=GR" }
}
```

- All five `AlbumInfo` fields have live sources (Decision 2). No `label` key anywhere in the
  document.

## Probe d — `GET tracks/564178214/relationships/credits` (200)

```json
{
  "data": [
    { "id": "XAl6FHNtoYJXU2RAWHioDQZUPlH68187toaLkatTQg7oYkC1tMr2TX6t86A", "type": "credits" },
    { "id": "XAl6FHNtoYJXU2RAgzmtkZEUabrkHz5j1iFHd8JMQXeDqRFAcLyVdlBPQuY", "type": "credits" },
    "…18 more credit refs…"
  ],
  "links": {
    "meta": { "nextCursor": "3nI1Esi" },
    "next": "/tracks/564178214/relationships/credits?countryCode=GR&page%5Bcursor%5D=3nI1Esi",
    "self": "/tracks/564178214/relationships/credits?countryCode=GR"
  }
}
```

- The dedicated relationship endpoint is **not** blocked for this token/region: it returned 200
  linkage (`id`/`type` only, no attributes). The public docs' INTERNAL tier did not gate the live
  call, but it is also not useful for the surface on its own — names/roles still require the
  `include=credits.artist` document (or per-credit resource fetches), so Task 3's parser path is
  unchanged.

## Decisions

1. **Credits — accessible.** `tracks/{id}?include=credits.artist` returned 200 with role/name
   credit resources and artist links; the dedicated relationship endpoint returned 200 as well
   (no entitlement error). **Tasks 3/4 run as written.**
   - **Caveat (recorded, no scope change):** the credits relationship is cursor-paginated at 20 per
     page (`links.meta.nextCursor`). Task 3's `credits_from_document` parses one track document, so
     tracks with more than 20 credits render the first page only. Follow-up if that proves visible:
     follow `links.meta.nextCursor` with a page loop (cursor, not offset). This sample track is
     already at a next cursor, so the limit is real.
2. **Album facts — present.** `releaseDate` = `"2026-10-02"`, `copyright.text` =
   `"© 2026 Apple Corps Ltd"`, `barcodeId` = `"00881061135678"`, `numberOfVolumes` = `4`,
   `numberOfItems` = `68`. **`label` is absent** (confirmed on the live payload; only `copyright`).
   All five `AlbumInfo` fields ship; the facts row omits label by design.
3. **Credit artist ids — present.** `include=credits.artist` returned numeric artist ids both as
   `included` `artists` resources (e.g. `"5957579"` Aaron Dessner) and as each credit resource's
   `relationships.artist.data.id`. **Contributor → artist links ship.** (A credit without that
   relationship still renders as plain text per the plan.)

## Follow-ups

- Credits pagination (Decision 1 caveat) — only if the 20-row first page proves visible in use.
- Region coverage: probed under `countryCode=GR` only; other regions untested.
- Track metadata for the Info tab keeps using the existing `TidalClient::track` (v1) path; this
  spike only settled the public-v2 credits/album-facts questions.

## Throwaway

`debug_get_openapi` was added to `commands.rs` + `main.rs`, built, exercised, then deleted before
this commit; `git diff` for the task contains only this note. The probe script, runner, and raw
output live under gitignored `.superpowers/sdd/2026-10-06-track9-2-credits-album-info/`. No product
code was written.
