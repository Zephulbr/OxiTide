# OxiTide — Home rhythm & card language (Track 9 slice)

- **Status:** Draft for review
- **Date:** 2026-10-03
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 9 (UI completion)
- **Related:** `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (§8 shapes/hover,
  §9 motion, §14 rejected options), `2026-09-30-oxitide-track4b-discovery-design.md` (mix cards
  navigate, no autoplay), `2026-09-28-oxitide-sidebar-collection-navigation-design.md` (§8.1
  shared section renderer)
- **Process note:** designed live with the owner (2026-10-03) from reference screenshots of the
  official client; the reference informed the rhythm and tag ideas but nothing is copied
  verbatim. Hi-Fi Rack remains deferred to its own spec (convergence §2) — this document does
  not introduce a new mode.

---

## 1. Purpose & Outcome

Home currently renders every feed section with one identical recipe — a heading plus a wrapping
grid of square cards (`PageSections.svelte`) — so albums, playlists, mixes and tracks are
visually indistinguishable and long sections push the page down without rhythm. This design
gives Home three section treatments assigned by section kind, a pinned quick-access strip, a
type-keyed shape language on one shared `MediaCard`, a generic View-all page, and playable feed
tracks.

**Outcome:** with a real account, Home shows a quick-access strip (your playlists, then saved
mixes) above the feed; playlist/editorial sections render as two-row tile grids with **View
all**; album/mix/radio sections render as arrow-controlled carousels; track/video-heavy sections
render as dense two-column lists; artists are circles, albums rounded squares, playlists sharp
squares with a deck edge, and mixes carry a radio badge. Clicking a card navigates exactly as
today (mix cards open the mix page), clicking a track row plays that track, and a track section
offers **Play all**. View all opens a `section` page whose back/forward/history behavior is the
app's normal one. Every failure degrades per section or per strip — never a broken Home.

Success = the surfaces above work on Windows 10/11 with the owner's account; the classification
and shape functions are pure and unit-tested; existing navigation and playback semantics are
unchanged; the gates (`cargo test`, `cargo clippy -D warnings`, `svelte-check`, `vitest`) pass;
a human smoke checklist is recorded; Track 3 budgets (S1/S2/S5) are re-checked.

## 2. Scope

**In scope:** `PageSection.kind` in `oxitide-tidal::pages` and `api.ts`; track subtitle fallback
from `artists[].name`; the `play_track_ids` Tauri command + `generate_handler!` registration;
`api.playTrackIds`; the pure `ui/src/lib/media.ts` module and its vitest suite; a shared
`MediaCard.svelte`; a rewritten `PageSections.svelte` with three treatments; Home's quick-access
strip; the `section` route, `Section.svelte` page and `App.svelte` dispatch; `CollectionAlbums`
and the Mixes favorites grid switching to `MediaCard`; `ARTIST` item routing; two new semantic
tokens; `toasts` plumbing into `PageSections`; tests and a smoke checklist.

**Out of scope:** hover play buttons on non-track cards; now-playing row highlight on feed
items; video playback; Hi-Fi Rack (own spec) and any mode branching; light mode; i18n; feed
pagination/infinite scroll; Feed (release activity) changes; sidebar/queue/now-playing changes;
`ui.cover_shape` (stays a Now Playing preference); new TIDAL endpoints beyond `tracks/{id}`
(already used by catalog).

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| D-1 | **Rhythm by role.** Sections get one of three treatments — tile grid, carousel, dense list — assigned by a pure classifier over section kind + item types. | Owner: Home should not look "the same" everywhere; texture follows what a section *is*. |
| D-2 | **Pinned quick-access strip** at the top of Home, composed UI-side from `getUserPlaylists` + `getFavoriteMixes` (existing commands), playlists first, ≤6 tiles. Hidden when empty or failed; never blocks the feed. | Reference's "Created by me" block; one-click resume of your own stuff without leaving Home. |
| D-3 | **Full silhouettes:** artist circle, album rounded square, playlist sharp square + deck edge, mix/radio rounded square + corner radio badge, everything else plain square. | Owner picked the scannable option; circle keeps its established "artist" meaning, mix badge matches the reference's tag, deck makes sharp-vs-rounded readable. |
| D-4 | Section treatment is driven by the new **`PageSection.kind`** (TIDAL module type) and item types — **never localized titles**. Unknown kinds fall back to carousel. | Titles are locale-/account-dependent; module types are stable. |
| D-5 | **Generic section route** `{ name: "section", source, title }` re-fetches the source feed and renders the matching section uncapped. | View all needs a destination with working history; re-fetching avoids storing feed payloads in persisted nav. |
| D-6 | Feed tracks are resolved **server-side** by a new `play_track_ids` command (`TidalClient::track`), so queue metadata is always complete. | Feed payloads may omit artist/duration/album; resolving guarantees correct queue/scrobble metadata. |
| D-7 | One shared **`MediaCard`** replaces the hand-rolled card recipes in `PageSections`, `CollectionAlbums` and Mixes' favorites grid. | Shape language must be identical everywhere; deletes duplication. |
| D-8 | Track rows play on click; track-major sections get **Play all**. Other cards keep navigate-only behavior. | Owner accepted the added scope; matches the official client's row behavior. |
| D-9 | `ARTIST` page items with numeric ids route to the existing artist page. | With artist cards visible and circular, an inert artist card would read as a bug; the route already exists. |
| D-10 | Explicit non-goals (see §11): hover play on cards, playing-row highlight, video playback, Hi-Fi Rack. | Keeps this slice layout/shape-focused; each is independently addable later. |

## 4. Architecture & components

### 4.1 Client — `crates/oxitide-tidal/src/pages.rs`

- `PageSection` gains `#[serde(default)] pub kind: String`. `sections_from_page` (pages.rs:182)
  sets it from `module.module_type` verbatim; an itemless module is still dropped as today.
- `RawItem` gains `#[serde(default)] pub artists: Vec<RawArtist>` (`RawArtist { name }`) and the
  mapping falls back for track items: subtitle precedence becomes `subTitle`/`subtitle` →
  `description` → joined `artists[].name` **only when the resolved `item_type` contains
  `TRACK`**. Display-only; no effect on ids/artwork.
- No new endpoint; `catalog.rs::track(id)` (catalog.rs:10) is reused by 4.2.

### 4.2 App commands — `crates/oxitide-app/src/commands.rs`

New async command, registered in `main.rs`'s `generate_handler![...]` next to `play_tracks`
(main.rs:318):

```rust
#[tauri::command]
pub async fn play_track_ids(
    state: State<'_, AppState>,
    ids: Vec<u64>,
    start: usize,
    shuffle: Option<bool>,
) -> Result<(), String>;
```

- Empty `ids` is an error (`"no tracks"`), matching the UI's disabled Play all.
- Resolution is factored into a free, wiremock-testable helper:
  `async fn queue_items_from_ids(client: &TidalClient, ids: &[u64]) -> Result<Vec<QueueItem>, Error>`
  — one `track(id)` per id via a tokio `JoinSet` capped at 4 in-flight tasks, with results
  re-ordered to the input sequence (tokio is already a dependency, no new crates), fail-fast on
  the first error.
- `queue_item_from_track(&Track) -> QueueItem` mirrors the UI's `trackToQueueItem`
  (api.ts:189): `track_id`, `title`, `artists` joined with ", ", `album.title` or `""`,
  `duration`, `album.cover`.
- On success it dispatches the existing engine path exactly like `play_tracks`
  (`EngineCmd::PlayTracks { items, start }` or `PlayShuffled` when `shuffle`), and calls
  `crate::perf::note_play_command()` so the Track 3 harness keeps counting plays.
- `start` semantics match `play_tracks` (engine handles bounds).

### 4.3 API contract — `ui/src/lib/api.ts`

```ts
export interface PageSection { title: string; items: PageItem[]; kind: string }
playTrackIds: (ids: number[], start: number, shuffle = false) =>
  invoke<void>("play_track_ids", { ids, start, shuffle }),
```

Field naming stays verbatim (`kind`, camelCase invoke args per AGENTS.md §IPC).

### 4.4 Pure UI logic — `ui/src/lib/media.ts` (new, vitest-covered)

```ts
export type CardShape = "artist" | "album" | "playlist" | "mix" | "plain";
export type SectionTreatment = "tiles" | "carousel" | "list";

export function shapeForItemType(itemType: string): CardShape;
export function classifySection(kind: string, items: PageItem[]): SectionTreatment;
export function isPlayableTrack(item: PageItem): boolean;
```

- **Shape precedence** (uppercased type): `ARTIST` → artist; `ALBUM` → album; `MIX` → mix
  (`MIX`, `MIX_LIST`); `PLAYLIST`/`EDITORIAL` → playlist; otherwise plain (`TRACK`, `VIDEO`,
  unknown).
- **Classification precedence** (kind uppercased; item majority = strictly more than half):
  1. empty items → carousel;
  2. track/video majority (type contains `TRACK` or `VIDEO`) → list;
  3. `kind` or any item type contains `MIX`/`RADIO` → carousel;
  4. `kind` contains `ALBUM` → carousel;
  5. playlist/editorial majority or `kind` contains `PLAYLIST`/`EDITORIAL` → tiles;
  6. otherwise carousel (the safe default for unknown kinds).
- **`isPlayableTrack`:** type contains `TRACK` and `id` is numeric. Track playback always uses
  the numeric id list; non-numeric/unknown items stay inert.
- The exact keyword table is intentionally conservative; it is tuned against the real account
  during implementation (Task 0 of the plan captures real `pages/home` output) and pinned by
  tests. Tuning only changes predicate constants, never the UI contract.

### 4.5 Components — `ui/src/lib/components`

- **`MediaCard.svelte` (new):** props `item: PageItem`, `variant: "tile" | "carousel" | "row"`,
  `onclick?: () => void`, `thumb?: number` (row variant only, default 40), and an optional
  `art` snippet (used by the strip to render `PlaylistCover` instead of `Cover`). Applies the
  shape class from `shapeForItemType` (radius, deck edge, mix badge with
  the inlined `radio` icon, `aria-hidden` badge), title and optional muted subtitle. Renders a
  `<button>` when `onclick` is set and a plain non-interactive `<div>` otherwise (no hover
  affordance on inert items). Track rows show a hover/focus play glyph and use
  `aria-label="Play {title}"`.
- **`PageSections.svelte` (rewrite):** per section, a header (title + controls) and the
  treatment container:
  - tiles: `MediaCard variant="tile"`, `items.slice(0, 12)`, header shows **View all** when
    `items.length > 12`;
  - carousel: `MediaCard variant="carousel"` in a scroll-snap row, header shows arrow buttons
    (enabled/disabled from scroll position), no View all;
  - list: `MediaCard variant="row"` in a two-column grid, `items.slice(0, 8)`, **View all**
    when `items.length > 8`; track-major sections additionally show **Play all**.
  - Additions to the existing props: required `toasts` (play failures) and required `source`
    (`"home" | "explore" | "forYou"`) so View all can push `{ name: "section", source, title }`.
    Untroutable items render inert rather than as buttons.
- **`Section.svelte` (new view):** loads its source (`home` → `getHome`, `explore` →
    `getExplore`, `forYou` → `getForYou`), finds the section by exact title, and renders it
    uncapped with its classification treatment (carousel collapses to a plain grid). Missing
    title → `EmptyState` "Section no longer available."; fetch failure → `ErrorState` with
    retry; heading shows the title plus item count.
- **`Home.svelte`:** keeps its feed loading, adds the quick-access strip (4.6/5.1) and passes
  `toasts` through to `PageSections`.
- **`CollectionAlbums.svelte` / `Mixes.svelte`:** their card recipes become `MediaCard`
  (album tiles; mix tiles) for visual parity; Mixes' favorite-mix grid is otherwise unchanged.
- **`Icon.svelte`:** no new icons — `radio`, `play`, `chevron-left/right` all exist.

### 4.6 Quick-access strip (Home only)

Composed in `Home.svelte` from existing commands, loaded independently of the feed with
`Promise.allSettled` so one source failing still renders the other:

- entries = owned playlists (`getUserPlaylists`) first, then saved mixes (`getFavoriteMixes`),
  truncated to 6; no dedupe needed (separate namespaces);
- each entry: `{ kind: "playlist" | "mix", id, title, subtitle, art }` with subtitles
  "Playlist" / "Saved mix";
- playlist tiles use `PlaylistCover` (`owned`), so user playlists show the 2×2 collage;
- click navigates to `{ name: "playlist" | "mix", id }` via the existing nav;
- both empty or both failed → the strip is not rendered at all (no heading, no empty box); no
  toast (non-critical, matches the per-section degradation rule).

### 4.7 Routing — `ui/src/lib/nav.svelte.ts` + `App.svelte`

- `Route` gains:

  ```ts
  | { name: "section"; source: "home" | "explore" | "forYou"; title: string }
  ```

- `parseRoute` accepts only those three sources and a non-empty string title.
- **`sameRoute` must treat `section` as distinct by `source` + `title`** — today it returns
  `true` for same-name routes without an `id`, which would make navigating between two View-all
  pages a silent no-op.
- `routeForPageItem` gains the `ARTIST` mapping (D-9) for numeric ids; mixes, albums,
  playlists/editorial stay as they are.
- `App.svelte` adds
  `{:else if nav.route.name === "section"}<Section source={nav.route.source} title={nav.route.title} {nav} {toasts} />`
  and passes `toasts` into `Home` (line 463) and `Mixes` (line 469); `Explore` already receives
  it and forwards it to `PageSections`.

### 4.8 Tokens — `ui/src/styles/tokens.css`

Add to `:root` (semantic layer, next to the radii block):

```css
--radius-card-sharp: 4px;  /* playlist/collage silhouettes */
--card-badge: 20px;        /* mix/radio tag chip */
```

No mode overrides; components continue to read semantic tokens only.

## 5. Surface specification

### 5.1 Home anatomy (top to bottom)

1. `<h2>Home</h2>` — unchanged.
2. Quick-access strip — up to 6 wide tiles (`MediaCard variant="row"`, `thumb=56`), responsive
   grid (`auto-fill, minmax(280px, 1fr)`), no View all.
3. Feed sections — each header (`h3` + controls) followed by its treatment; spacing and
   typography per the existing foundation; the feed's own empty sections were already dropped
   in Rust.
4. `Loading` / `ErrorState` / `EmptyState` behavior for the feed itself is unchanged.

### 5.2 Treatments

| Treatment | Used for | Layout | Cap | Header control |
|---|---|---|---|---|
| Tile grid | playlist/editorial sections | `grid`, `auto-fill minmax(160px, 1fr)`, gap 16 | 12 shown | View all when capped |
| Carousel | album, mix, radio, unknown | scroll-snap row, one card per step | all items | arrows only |
| Dense list | track/video-major sections | 2-column grid (`auto-fill minmax(320px, 1fr)`), 40px thumbs | 8 shown | View all + Play all |

Caps are presentation-only; View all exposes the rest. Carousels expose everything by
scrolling, so they need no View all.

### 5.3 Shape language

| Type | Silhouette | Treatment |
|---|---|---|
| Artist | circle (`border-radius: 50%`) | as today for artist art (search/header); cards now match |
| Album | rounded square, `--radius-card` (12px) | unchanged |
| Playlist | sharp square, `--radius-card-sharp` (4px) + deck edge: a static 3px offset layer (`::after`, `--surface-3` fill, 1px `--line` stroke, same radius) behind bottom-right | new |
| Mix / Radio | rounded square 12px + top-right badge pill (`--card-badge`, `--surface-float` fill, `--edge-lit` border, `radio` icon 12px) | new |
| Track / Video / unknown | rounded square 12px, no badge; track rows are list rows | unchanged cards, new rows |

Hover/press/focus remain the foundation's: hover `--surface-2` + cover zoom 1.03
(`--dur-state`), press scale 0.98, keyboard-only 2px accent focus ring. The deck edge and badge
are static (never animated) and do not branch on mode.

### 5.4 Track playback

- Track rows (dense list and track-only tile/carousel cards) are buttons:
  `playSource.set(null)` then `api.playTrackIds([id], 0)`; failure → error toast via the
  existing `errorMessage` helper.
- **Play all** (track-major sections) calls `api.playTrackIds(ids, 0)` with every playable
  track id in the section, is disabled while the invoke is in flight, and toasts on failure.
- Queue replacement semantics are exactly those of any `playTracks` call — the engine's normal
  state events drive the UI.
- Nothing autoplays on view load; only explicit clicks play.

## 6. Behavior specification

- **Navigation unchanged:** album/playlist/mix/editorial cards use `routeForPageItem` exactly as
  before (mix cards open the mix page, per Track 4b D-7); artist cards now open the artist page;
  everything unroutable is inert with no hover affordance.
- **View all** from a capped section pushes `{ name: "section", source, title }`; the page
  re-fetches that source, renders the found section uncapped, and normal back/forward works;
  the entry persists across restarts (payload-free).
- **Quick access** tiles navigate like the sidebar entries; the strip hides entirely when it has
  nothing to show.
- **Section headers** keep the server-provided titles; no renaming.
- **Explore and Mixes** feed sections get the same treatments and shapes, so the language is
  consistent app-wide; the quick-access strip is Home-only.

## 7. Error handling & degradation

- Feed-level fetch failure keeps today's `ErrorState` + retry on Home/Explore/Mixes.
- Quick-access strip: per-source failure is swallowed; the strip renders whatever succeeded and
  disappears if nothing did.
- Playback failures surface as standard error toasts; playback state is untouched on failure
  (the command fails before dispatching when resolution fails).
- `Section` page: unknown title → `EmptyState`; fetch failure → `ErrorState` with retry.
- Unknown/missing `kind` or item types degrade to the carousel treatment and plain silhouettes —
  no panics, no title matching.

## 8. Motion, accessibility, performance

- **Carousel:** `scroll-snap-type: x proximity`, hidden scrollbar; arrows scroll by one card
  (`behavior: "smooth"`, forced to `"auto"` under `prefers-reduced-motion`); no wheel hijack —
  shift+wheel keeps native horizontal scrolling. Arrows are labelled (`Scroll left/right`) and
  disabled at the ends; cards remain tabbable buttons.
- **a11y:** carousel rows are `role="region"` with the section title as label; badges are
  `aria-hidden` (the subtitle carries meaning); track rows expose `Play {title}`; the strip is a
  labelled list; target sizes meet the 24px floor; focus styling per the foundation.
- **Performance (PM-D1 / Track 3):** no new fetches for the feed itself; the strip adds two
  invokes plus at most six collage cover requests, loaded after first paint and independent of
  the feed; images stay lazy. S5 (Home/Explore scroll) must be re-run with carousels and re-run
  S1/S2 for the strip; results recorded in the plan notes per AGENTS.md.

## 9. Testing & verification

**Rust (wiremock only; never the live API):**

- `pages.rs`: `kind` is populated from the module type; missing module type defaults to `""`;
  track items without subtitle/description get the joined `artists[].name`; non-track items are
  unaffected; existing artwork/id tests stay green.
- `commands.rs`: `queue_items_from_ids` maps ids in order (title/artist/album/duration/cover),
  preserves order under bounded concurrency, and fails fast on a mid-list 500 — all via
  wiremock against a mock TidalClient. The command wrapper itself stays a thin dispatch (same
  as `play_tracks`, which has no unit test).

**UI (vitest):**

- `media.test.ts`: shape mapping for every type/casing and unknown; classification precedence
  (list > mix/radio > album > playlist; empty → carousel; unknown kind → carousel);
  `isPlayableTrack` numeric/non-numeric.
- `nav.test.ts`: `section` route round-trips parse/persist; `sameRoute` distinguishes two
  sections; `ARTIST` items map to the artist route; existing inert cases unchanged.
- Component behavior is covered by pure functions + `svelte-check`; there is no component-test
  harness in the repo.

**Checks:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`;
`npm --prefix ui run check`; `npm --prefix ui test`.

**Human GUI smoke** (`docs/superpowers/plans/notes/2026-10-03-home-rhythm-smoke.md`, created
during implementation): strip contents and hiding behavior; one section of each treatment;
shape readability (album vs playlist vs mix vs artist); View all page + back/forward; track row
play and Play all (queue, scrobbling metadata sane); mix card still opens the mix page;
Explore/Mixes consistency; reduced-motion carousel; S1/S2/S5 numbers.

## 10. Risks & open verifications

- **Real module kinds.** The classifier's keyword table is written from known module types
  (`PLAYLIST_LIST`, `TRACK_LIST`, `MIX_LIST`, `MIXED_TYPES_LIST`, `PAGE_LINKS_CLOUD`, …) but the
  owner's live feed may use others; Task 0 captures real output and tunes predicates before the
  UI work lands. Fallback (carousel/plain) is safe by construction.
- **Play all latency.** `play_track_ids` fetches metadata before the engine starts; bounded
  concurrency 4 keeps a section-sized batch reasonable but the first click is not instant. If
  the smoke check finds it slow, the future mitigation is enriching `PageItem` with feed
  metadata and skipping the fetch (documented in §11).
- **Strip cost.** Two invokes plus up to six collage requests on Home; if S1/S2 regress, the
  fallback is rendering strip thumbs with plain `Cover` instead of `PlaylistCover` collages.
- **Prop ripple.** `PageSections` gains required `toasts`; Home and Mixes receive it from App,
  Explore already has it. Mechanical, but the plan lists the call sites.
- **`sameRoute` subtlety.** Forgetting the `section` identity would break consecutive View-all
  navigations; the nav test pins it.

## 11. Future / explicitly deferred

Hover play buttons on non-track cards; now-playing highlighting on feed rows; feed track
metadata enrichment (instant play, no resolve fetch); a "hero" first section with larger art;
user-configurable section order/visibility; video playback; Purchases adopting `MediaCard`;
Hi-Fi Rack card treatments (own spec, sequenced last in the Track 9 bucket); light mode; i18n.
None of these change this design's public surface beyond adding behavior to existing components.
