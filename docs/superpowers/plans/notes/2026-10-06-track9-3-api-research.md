# Track 9.3 — API research spike (page payloads, playlist dates, bio markup)

- **Date:** 2026-10-06
- **Status:** Spike complete — page track items carry the full embedded metadata set; collection
  playlists carry all three date fields; bio `wimpLink artistId` links are present. Tasks 1/2/3
  run as written, Task 8 takes the real added-date branch, Task 7 ships artist navigation.
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §5.4, §6.3,
  T9-D6; plan `docs/superpowers/plans/2026-10-06-track9-3-polish-bundle.md` Task 0
- **Parent:** `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 9

## Carry-in (Step 1)

- `docs/superpowers/plans/notes/2026-10-06-track9-carry-in-triage.md` — triage complete,
  **no code defects**; 11 queued owner runs (artist-page, home-rhythm, light-mode UI, plan4
  re-checks) are verification, not fix work. The home-rhythm re-run re-checks the 9.3 card rows;
  the artist-page re-run's "leftovers" are exactly Tasks 6/7's surfaces. Nothing folds in; no
  scope change (T9-D9).
- `docs/superpowers/plans/notes/2026-10-05-track9-1-light-mode-smoke.md` — S5-light passes except
  `memory_max_mb` 681/675, and the S1/S2 quiet re-run is blocked by machine load; first-pass
  attribution is environmental (app-owned memory inside the Phase D range; growth in WebView2
  154.0.4258.53), no functional defect. The adjudication stays deferred to a quiet run and touches
  no 9.3 surface.
- Net deltas landing on 9.3 surfaces (feed cards, settings toggles, hotkeys, artist page,
  playlists, purchases, now playing): **none**. The note records the carry-in state only.

## Method

- Two temporary Tauri commands (removed before this note was committed) forwarded raw requests:
  `debug_get_v1` → `TidalClient::get` (v1 pages) and `debug_get_openapi` → `TidalClient::get_openapi`
  (`countryCode` from the live session). They were driven over the WebView2 CDP endpoint by
  throwaway Node probes against the debug app with the real logged-in account. Raw outputs:
  `probe-output.json` / `probe-bio-output.json` beside the scripts under gitignored
  `.superpowers/sdd/2026-10-06-track9-3-polish-bundle/`.
- **Session region:** same account/session as the 9.2 spike (`countryCode=GR`).
- All requests returned **HTTP 200 JSON**. No 403/400/empty result was observed. No tokens were
  logged; the payloads contain none. Automated tests stay wiremock-only — this note is the only
  live capture.
- The app instance and Vite server started for the probe were stopped afterwards (ports 1420 and
  9222 confirmed closed).

## Probe a — `GET pages/home?deviceType=BROWSER&platform=WEB` (200)

- 10 track items in the `TRACK_LIST` module "New Tracks". **All 10** carried `title`, `duration`,
  `explicit`, `audioQuality`, `artists[].name`, `album.title`, and artwork via `album.cover`;
  the raw track item has no own `type` (the module type is the discriminator) and no top-level
  `cover`.
- Trimmed sample (item 564178214; unrelated keys such as `mixes`, `replayGain`, `popularity`,
  `url`, `mediaMetadata` elided):

```json
{
  "id": 564178214,
  "title": "Draw You Out",
  "duration": 256,
  "explicit": false,
  "audioQuality": "LOSSLESS",
  "artists": [
    { "id": 8451939, "name": "Noah Kahan", "type": "MAIN" },
    { "id": 86841321, "name": "The Hunger Games", "type": "MAIN" }
  ],
  "album": {
    "id": 564178213,
    "title": "Draw You Out (From The Hunger Games: Sunrise on the Reaping - The Album)",
    "cover": "016f5fd0-7d80-4ee9-9238-62b63bab593b"
  },
  "trackNumber": 1,
  "volumeNumber": 1
}
```

- Second sampled item: `566123443` "Cold Shoulder" — `duration: 235`, `explicit: true`,
  `audioQuality: "LOSSLESS"`, three named artists (Drake, Don Toliver, Yebba), album
  `566123429` "HABIBTI (FOMO)".
- `pages/explore` returned **no track rows at all** this session — only `PAGE_LINKS` (5) and
  `PAGE_LINKS_CLOUD` (40) modules. Nothing to enrich there yet; the Task 3 wire-up stays as
  written and is inert until the page serves track rows (recorded under Follow-ups).

## Probe b — `get_collection_items(folderId: "root")` (existing command, 200)

- 16 entries: 1 folder + **15 playlists; 15/15 carry non-null `created`, `lastItemAddedAt`, and
  `lastUpdated`** through the existing v2 `my-collection/playlists/folders` payload parsed by
  `Playlist`. Sample (titles redacted — private account data):

```json
[
  { "title": "…", "created": "2024-04-29T15:37:44.640+0000",
    "lastItemAddedAt": "2026-10-02T17:53:18.456+0000", "lastUpdated": "2026-10-02T17:53:18.456+0000" },
  { "title": "…", "created": "2024-05-11T00:36:01.036+0000",
    "lastItemAddedAt": "2026-10-05T21:45:51.041+0000", "lastUpdated": "2026-10-05T21:45:51.041+0000" },
  { "title": "…", "created": "2026-10-02T18:53:37.689+0000",
    "lastItemAddedAt": "2026-10-02T18:53:52.928+0000", "lastUpdated": "2026-10-03T18:03:54.727+0000" }
]
```

- Counts over the full response: `created` 15, `lastItemAddedAt` 15, `lastUpdated` 15. The
  sample spans playlists whose `lastItemAddedAt` is older, newer, and equal to `lastUpdated`, so
  the field is a real per-playlist value, not a constant fallback.

## Probe c — `GET artists/{id}?include=biography` (public v2, 200)

- Artist ids were taken from the live `pages/home` track payload's `artists[]` (the home page
  served no album/artist modules). All four probes returned an
  `included[]` resource of `type: "artistBiographies"` with `attributes.text`:

| Artist id | Bio length | `[wimpLink` open tags | `artistId` tags | `albumId` tags |
| --- | --- | --- | --- | --- |
| 3632070 (Drake) | 15,080 | 119 | **93** | 26 |
| 9279273 (Don Toliver) | 7,860 | 74 | **53** | 21 |
| 8451939 (Noah Kahan) | 1,410 | 0 | 0 | 0 |
| 86841321 (The Hunger Games) | 1,532 | 0 | 0 | 0 |

- Every `[wimpLink` open tag in the linked bios was one of the two known forms; no other
  attribute shape appeared. Raw markup (trimmed):

```text
Canadian rapper and vocalist [wimpLink artistId="3632070"]Drake[/wimpLink] has retained a
bigger-than-life commercial presence… with hits by the likes of [wimpLink artistId="27518"]Lil
Wayne[/wimpLink], [wimpLink artistId="10665"]Rihanna[/wimpLink]…

…a Top 40 U.S. pop hit off [wimpLink artistId="5198891"]Travis Scott[/wimpLink]'s
[wimpLink albumId="92967282"]Astroworld[/wimpLink] LP…
```

- Two bios were plain text with HTML `<br/>` breaks and no links — the parser must degrade to a
  single plain segment (already the planned behavior).

## Decisions

1. **Page metadata — present.** All five fields (`duration`, `explicit`, `audioQuality`,
   `artists[].name`, `album.title`) were present on 10/10 live `TRACK_LIST` items, artwork via
   `album.cover`. **Tasks 1/2/3 run as written.** Fields stay optional in the DTO so a partial or
   different module still falls back to the per-item resolve; no field-based task branch.
2. **Playlist dates — present.** 15/15 collection playlists carry `created`, `lastItemAddedAt`,
   and `lastUpdated`. **Task 8 takes the real added-date branch** (plan Step 2 + date test
   update); the toggle copy stays "Sort by date added" (Step 3), since it becomes truthful.
3. **Bio links — present.** `[wimpLink artistId="…"]Name[/wimpLink]` was observed in live bios
   (Drake 93, Don Toliver 53). **Task 7 ships artist navigation** via `bio_segments`;
   `albumId`-only links and link-free bios stay plain text per the plan.

## Follow-ups

- **Explore has no track rows today** (link modules only). The Task 3 Explore integration is
  wired regardless; if the page later serves track rows, enrichment applies with no code change.
  Worth a glance during the PM-D1/owner passes.
- Region coverage: probed under the 9.2 session's `countryCode=GR` only; other regions untested.
- The playlist-date capture came through the existing `get_collection_items` (v2 folders)
  command; no new origin or endpoint was exercised.

## Throwaway

`debug_get_v1` and `debug_get_openapi` were added to `commands.rs` + `main.rs`, built, exercised,
then deleted before this commit; `git diff` for the task contains only this note. The probe
scripts, launcher, and raw outputs live under gitignored
`.superpowers/sdd/2026-10-06-track9-3-polish-bundle/`. No product code was written.
