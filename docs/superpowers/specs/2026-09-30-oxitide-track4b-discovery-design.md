# OxiTide — Track 4b: Discovery (For You mixes, radio, similar artists)

- **Status:** Draft for review
- **Date:** 2026-09-30
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 4
- **Related:** `docs/superpowers/plans/notes/2026-09-29-track4-api-research.md` (API spike)
- **Process note:** surfaces, radio behavior, and naming were brainstormed live with the owner
  (2026-09-30). Track 4a (lyrics) is a separate shipped spec; this document is the standalone
  source of truth for sub-track 4b.

---

## 1. Purpose & Outcome

Close the discovery half of Track 4 using the legacy v1 endpoints the existing token can reach:
a For You mix section in Mixes & Radio, immediate radio playback from a track or an artist, and
a Similar Artists row on the artist page. The release-activity Feed stays exactly as it is.

**Outcome:** with a real account, Mixes & Radio lists the TIDAL For You mixes above the user's
favorite mixes and plays any of them like a favorite mix; a track's context menu gained
**Track Radio** (official-client wording) which replaces the queue and starts playing; the
artist page has a radio button and a Similar Artists card row whose cards navigate to that
artist. Region gaps and endpoint failures degrade to a hidden section or a toast — never a
broken view. No new auth scopes; no changes to Feed, Home, playback internals, or routing.

Success = all three surfaces work on Windows 10/11 with the owner's account; the discovery
fetches are best-effort and isolated from the rest of their views; radio plays with the same
queue semantics as mixes; the gates (`cargo test`, `cargo clippy -D warnings`,
`svelte-check`, `vitest`) pass; a human smoke checklist is recorded.

## 2. Scope

**In scope:** `pages/for_you`; `tracks/{id}/radio`; `artists/{id}/radio`;
`artists/{id}/similar`; client methods in `oxitide-tidal` with wiremock tests; four Tauri
commands + `generate_handler!` registration; `api.ts` surface and a new `radio.ts` helper with
vitest tests; the Mixes & Radio integration; the Artist radio button + Similar Artists row; the
`TrackTable` context-menu item with the `toasts` prop plumbing at its four call sites; an
inlined Lucide `radio` icon; a human GUI smoke checklist.

**Out of scope:** Feed changes (it keeps showing release activity); new routes or dedicated
radio/For You views; endless/DJ radio, radio history, thumbs up/down; the official JSON:API
discovery resources (`userDiscoveryMixes` etc.) and any auth/scope changes; non-Windows-specific
work; downloads; visual redesign; pagination/infinite scroll for discovery lists.

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| D-1 | For You mixes live in the existing **Mixes & Radio** view, above the favorites; **Feed** stays a release-activity timeline. | Owner choice: discovery mixes and favorites are both mix collections; Feed keeps its current meaning. |
| D-2 | Radio **plays immediately**, replacing the queue; there is no radio-specific view. | Radio endpoints return a finite list; one-shot playback is the smallest useful surface. |
| D-3 | **Similar Artists** render as a card row on the artist page; clicking navigates through the existing `artist` route. | No new surface; reuses the established card pattern. |
| D-4 | The track context-menu entry is named **"Track Radio"**, matching the official client. | Owner wording. |
| D-5 | **Legacy v1 endpoints only**, on the existing token; no scope or auth changes. | The research spike found the official JSON:API discovery resources are out (INTERNAL tier / `recommendations.read`); the legacy paths need nothing new. |
| D-6 | **Thin reuse:** for_you renders through `PageSections.svelte`; radio reuses `playTracks`; `radio.ts` is the only new UI module. | Keeps the plan small and the behavior identical to mixes. |

## 4. Research summary (Track 4 API spike, 2026-09-29)

| Endpoint | Use | Access | Live status |
|---|---|---|---|
| `pages/for_you?deviceType=BROWSER&platform=WEB` | daily/discovery mix feed | existing token, no new scope | reachable per community client; contents for this account/region unverified |
| `tracks/{id}/radio?limit=N` | track radio | existing token | shape/availability unverified |
| `artists/{id}/radio?limit=N` | artist radio | existing token | shape/availability unverified |
| `artists/{id}/similar?limit=N` | similar artists | existing token | shape/availability unverified |
| official `userDiscoveryMixes` / `userDailyMixes` / `userNewReleaseMixes` | — | needs `recommendations.read` against a registered client | **out** |

The spike deliberately did not read the keyring token, so every payload shape above is
unverified; the plan's Task 0 captures real responses and pins the parsers and fixtures before
the rest of the work.

## 5. Architecture & components

### 5.1 Client — `crates/oxitide-tidal`

`pages.rs` gains one method next to `home_page`/`explore_page` (pages.rs:146/151), reusing
`page_sections_at` and its `deviceType`/`platform` params:

```rust
/// The discovery mix feed (daily/discovery mixes), same rows/modules shape
/// as home and explore.
pub async fn for_you_page(&self) -> Result<Vec<PageSection>>;
```

New `discovery.rs` module (peer of `feed.rs`/`pages.rs`) exposes:

```rust
pub const RADIO_LIMIT: u32 = 50;
pub const SIMILAR_ARTISTS_LIMIT: u32 = 20;

impl TidalClient {
    /// `tracks/{id}/radio?limit=N` — audio-only radio seeded on a track.
    pub async fn track_radio(&self, id: u64) -> Result<Vec<Track>>;
    /// `artists/{id}/radio?limit=N` — audio-only radio seeded on an artist.
    pub async fn artist_radio(&self, id: u64) -> Result<Vec<Track>>;
    /// `artists/{id}/similar?limit=N` — similar artists for the artist page.
    pub async fn similar_artists(&self, id: u64) -> Result<Vec<Artist>>;
}
```

Parsing rules (all GETs through `TidalClient::get`, so single-flight refresh-on-401 applies):

- **Radio** responses are accepted as either `{ "items": [ … ] }` or a bare array (the live
  shape is unverified). Each item is parsed as `serde_json::Value`; entries whose `type` is
  `VIDEO` are dropped (the engine is audio-only, the same rule as `mix_page`, pages.rs:161);
  the rest are mapped with `serde_json::from_value::<Track>` and unparseable entries are
  skipped, mirroring `mix_detail_from_json` (pages.rs:333).
- **Similar artists** is parsed the same lenient way into `Artist`.
- Limits are fixed constants (`RADIO_LIMIT`, `SIMILAR_ARTISTS_LIMIT`); the UI does not
  choose them.

No auth or client-construction changes.

### 5.2 App commands — `crates/oxitide-app`

Four thin wrappers in `commands.rs`, following `get_home`/`get_mix`/`get_artist`
(commands.rs:429/559/673), each `require_client(&state)?` + `map_err(|e| state.fail_tidal(e))`:

```rust
pub async fn get_for_you(state) -> Result<Vec<PageSection>, String>;
pub async fn get_track_radio(state, track_id: u64) -> Result<Vec<Track>, String>;
pub async fn get_artist_radio(state, artist_id: u64) -> Result<Vec<Track>, String>;
pub async fn get_similar_artists(state, artist_id: u64) -> Result<Vec<Artist>, String>;
```

All four are registered in `generate_handler![…]` in `main.rs` (next to
`get_home`/`get_feed`/`get_mix`/`get_artist`, main.rs:243-253) — a missed registration is a
runtime `invoke` failure, so the plan verifies it.

### 5.3 API contract & playback helper — `ui/src/lib`

`api.ts` additions:

```ts
getForYou: () => invoke<PageSection[]>("get_for_you"),
getTrackRadio: (trackId: number) => invoke<Track[]>("get_track_radio", { trackId }),
getArtistRadio: (artistId: number) => invoke<Track[]>("get_artist_radio", { artistId }),
getSimilarArtists: (artistId: number) => invoke<Artist[]>("get_similar_artists", { artistId }),
```

New `ui/src/lib/radio.ts` mirrors `mixPlay.ts`:

```ts
export async function playTrackRadio(trackId: number, toasts: Toasts): Promise<void>;
export async function playArtistRadio(artistId: number, toasts: Toasts): Promise<void>;
```

Both fetch their list, then: empty list → info toast "No radio tracks available."; success →
`playSource.set(null)` + `api.playTracks(tracks.map(trackToQueueItem), 0)`; failure → error
toast via `errorMessage(e)`. No new state.

### 5.4 UI surfaces

**Mixes & Radio (`ui/src/views/Mixes.svelte`; App.svelte:391 also passes `{nav}`):**

- Loads `api.getForYou()` and `api.getFavoriteMixes()` in parallel and independently. A
  for_you failure is swallowed and the section is omitted; a favorites failure keeps today's
  `ErrorState` behavior.
- for_you sections render through the existing `PageSections.svelte` with
  `onPlayMix={(id) => void playMix(id, toasts)}` and the `nav` prop, so mix cards (and any
  other item type the feed returns) behave exactly as on Home/Explore.
- When favorites exist, an `<h3>Favorite mixes</h3>` heading precedes today's grid. The
  `EmptyState` "No mixes yet." shows only when both sources are empty.

**Artist (`ui/src/views/Artist.svelte`; App.svelte:397 passes `{nav}` and `{toasts}`):**

- The header actions gain a radio icon button (`Icon name="radio"`, aria-label/title "Start
  artist radio") calling `playArtistRadio(artist.id, toasts)`.
- Below the header, a "Similar artists" section loads `api.getSimilarArtists(id)` in the same
  effect as the artist fetch, with its own failure swallowed and the section hidden when empty
  or failed. Cards use `Cover` (artist `picture`) and navigate with
  `nav.goto({ name: "artist", id })`.

**Track context menu (`ui/src/lib/menus.ts`, `TrackTable.svelte`):**

- `TrackMenuContext` gains a required `startRadio(): void`; `trackMenuItems` inserts
  `{ id: "radio", label: "Track Radio", run: () => ctx.startRadio() }` directly after "Play".
- `TrackTable.svelte` gains a required `toasts: Toasts` prop and supplies
  `startRadio: () => void playTrackRadio(t.id, toasts)`.
- Call sites: `Explore.svelte:227` and `Playlist.svelte:247` already hold `toasts` and pass it;
  `Album.svelte:96` and `CollectionTracks.svelte:96` gain a `toasts` prop, passed from
  App.svelte:389/395.

**Icon (`ui/src/lib/components/Icon.svelte`):** add an inlined Lucide `radio` path, verbatim
from the Lucide sources, per the no-unicode-glyph rule (AGENTS.md).

**Routing:** unchanged. No new `Route` variants; radio never navigates.

## 6. Behavior specification

- **Open Mixes & Radio:** For You sections appear first (with the server-provided titles, e.g.
  a "Made for you" row), each card's artwork/title/subtitle exactly as TIDAL returns; the
  favorites grid follows under its heading. Clicking a For You mix card plays it through the
  existing `playMix` path (fetch detail → replace queue → play).
- **Context menu → Track Radio:** fetches the radio list for that track, replaces the queue
  (stopping the current source identity, as `playMix` does), and starts at index 0. The playing
  row highlight and all player/queue behavior are the engine's normal ones.
- **Artist radio button:** same behavior seeded on the artist.
- **Similar artist card:** navigates to that artist's page; back/forward history works as with
  any artist click.
- **Disabled/empty radio:** an empty or fully filtered list shows the info toast and leaves
  playback untouched.
- **Feed/Home/Explore:** no behavior changes.

## 7. Error handling & degradation

- for_you and similar-artists are **best-effort**: any error (network, 403/404, region gap,
  parse) omits that section silently; the rest of the view renders normally and no toast is
  raised.
- Radio errors surface as a standard error toast; an empty result (including after video
  filtering) surfaces the info toast; neither changes the queue.
- Parser leniency limits blast radius on API drift: unknown item types are skipped, video
  entries dropped, and a wrong top-level shape yields an empty/`NotFound` error rather than a
  panic.
- If `pages/for_you` proves unavailable for this account/region, the Mixes view is unchanged
  in practice — the feature degrades to "favorites only".

## 8. Performance

PM-D1 standing gate. Open Mixes adds one extra GET, fetched in parallel with favorites; open
Artist adds one small GET; radio fetches happen only on explicit click. No polling loops, no
new threads, no audio-path or engine changes, no extra Tauri events. All lists are fixed-size
(≤50 tracks / ≤20 artists).

## 9. Testing & verification

**Rust (wiremock only; never the live API):**

- `pages.rs`: `for_you_page` hits `/pages/for_you` with `deviceType=BROWSER&platform=WEB`,
  parses sections/items like home (reuse existing helpers).
- `discovery.rs`: `track_radio` and `artist_radio` hit the right path with the limit param;
  both `{items:[…]}` and bare-array bodies parse; `VIDEO` entries are dropped; unparseable
  entries are skipped; empty body returns an empty vec; 404 maps to `Error::NotFound` (pattern
  of catalog.rs:130). `similar_artists` maps id/name/picture.

**UI (vitest):**

- `radio.test.ts` mirrors `mixPlay.test.ts`: success plays the mapped tracks at index 0;
  empty list → info toast and no `playTracks`; fetch/play failure → error toast.
- `menus.test.ts`: the context menu includes "Track Radio" in the expected position and its
  `run` invokes `startRadio`.

**Checks:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`;
`npm --prefix ui run check`; `npm --prefix ui test`.

**Human GUI smoke** (`docs/superpowers/plans/notes/2026-09-30-track4b-discovery-smoke.md`,
created during implementation): For You mixes visible and playable; Mixes view unchanged when
for_you is empty/unavailable; Track Radio from album/playlist/favorites/explore; artist radio;
similar artists render and navigate; region-gap behavior recorded; Feed still shows release
activity; playback and queue behave as before.

## 10. Risks & open verifications

- **Live shapes.** Radio/similar/for_you payloads were never captured; the plan's Task 0
  captures one real response per endpoint and pins fixtures/parsers before the remaining tasks.
  The lenient parser above is the designed hedge.
- **Region availability.** `pages/for_you`, radio, and similar may be missing for the owner's
  account/region; the design hides or toasts gracefully, and the smoke record documents what
  was observed rather than assuming availability.
- **TrackTable prop ripple.** Adding required `toasts` touches four call sites and two views
  (`Album`, `CollectionTracks`); mechanical, but the plan lists them explicitly so none is
  missed.
- **API drift.** Any future change in item typing is absorbed by the lenient item mapping;
  a change in the top-level envelope (items vs array) is already handled for radio/similar.

## 11. Future / explicitly deferred

Endless/auto-extending radio; radio history and seed switching; thumbs up/down feedback;
a dedicated For You page or nav entry; mix-creation endpoints; the official JSON:API
discovery resources (revisit only if the app's auth story changes); pagination for similar
artists. None of these change this design's public surface (four commands + one helper module)
beyond adding methods.
