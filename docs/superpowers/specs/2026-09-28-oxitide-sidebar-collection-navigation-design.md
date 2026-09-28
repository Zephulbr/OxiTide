# OxiTide — Sidebar, Explore/Feed, and My Collection Pages

- **Status:** Draft for review
- **Date:** 2026-09-28
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` ·
  `2026-09-25-oxitide-plan8-ux-foundations-design.md` ·
  `2026-09-25-oxitide-plan9-data-interaction-design.md` ·
  `2026-09-25-oxitide-2b-visual-convergence.md`
- **Process note:** scope and decisions were brainstormed live with the owner
  (2026-09-28). This document is the standalone source of truth.

---

## 1. Purpose & Outcome

The current sidebar (Home, Search, Library, Queue, Settings) and the single-page Library
view (playlists + albums + favorite tracks stacked) diverge from the official TIDAL
client and feel cluttered. The official client separates navigation into Home, Explore,
Feed, and a *My Collection* area with dedicated pages.

**Outcome:** the sidebar reads top-to-bottom as **Home**, **Explore**, **Feed**, then a
divider and an expandable **My Collection** section containing **Playlists**, **Albums**,
**Tracks**, and a *Show more* reveal of **Mixes & Radio**, **Videos**, **Profiles**,
**Purchases**. Below that, a quick-access tree of the user's created and liked playlists
with folder support. Search moves into Explore; Library is retired into the collection
pages.

Success = every collection page is reachable and functional with real data, folders can
be created/expanded/moved into, the playlist tree reflects mutations immediately, and all
previously-working behavior (search, favorites, playlist edits, favorite-tracks paging)
still works.

## 2. Scope

**In scope:** route/navigation remodel; `Sidebar.svelte` rewrite (sections, collection
headings, rail mode with playlist covers, playlist tree); new Explore and Feed views;
collection pages (Playlists + folder drill-in, Albums, Tracks, Mixes & Radio, Videos,
Profiles, Purchases); shared page-section renderer for Home/Explore/Feed; generalized
`pages/*` parser in `oxitide-tidal`; new folder endpoints and commands; new DTOs; store +
`api.ts` plumbing; tests and a manual GUI checklist.

**Out of scope:** video playback (engine is audio-only); TIDAL's Upload entry; feed
notification badge counts (no API); a videos group in search results; TIDAL Connect /
device handoff; playlist favoriting/unfavoriting UI; drag-and-drop (folder moves go
through context menus); persistence of sidebar expand/collapse state across launches;
macOS/Linux-specific behavior.

## 3. Current state (codebase grounding)

- Routes live in `ui/src/lib/nav.svelte.ts` (hand-rolled `Route` union + history);
  `App.svelte` switches on it. `Sidebar.svelte` is a flat list of buttons and a manual
  rail mode (`<=1080px` auto or toggle).
- `views/Library.svelte` stacks playlists, favorite albums, and a paged favorite-tracks
  `TrackTable`. `views/Search.svelte` owns the `search-input` element that
  `focusSearch` targets. `views/Home.svelte` renders `HomeSection[]` items as a card grid
  and ignores item types it cannot route.
- `crates/oxitide-tidal` has: `home.rs` (v1 `pages/home` rows/modules parser), `search.rs`,
  `catalog.rs`, `library.rs` (favorite tracks/albums/artists), `playlists.rs` (playlist
  writes; the v2 folders `create-playlist` endpoint is already used). No explore, feed,
  folders, videos, mixes, or purchases support.
- All HTTP flows through `TidalClient::get` / `get_all_pages` (`client.rs`).

## 4. Endpoint reference

Confirmed shapes come from the current code and from the unofficial-client ecosystem
(`tidalapi`); candidates marked *probe* are verified against a live account by the plan's
probe task before their command is finalized. Every call uses the existing
`TidalClient::get`/`get_all_pages` single-flight path.

| Purpose | Request (v1 base unless noted) | Shape notes |
|---|---|---|
| Home sections | `GET pages/home?deviceType=BROWSER&platform=WEB` | existing parser |
| Explore sections | `GET pages/explore?deviceType=BROWSER&platform=WEB` | same rows/modules shape as home |
| Feed sections (*probe*) | `GET pages/feed?deviceType=BROWSER` then `pages/my_collection_feed` | first path that parses rows/modules wins; otherwise empty feed |
| Mixes & Radio page | `GET pages/my_collection_my_mixes?deviceType=BROWSER` | rows/modules; items of type `MIX` |
| Mix detail tracks | `GET pages/mix?mixId=…&deviceType=BROWSER` | second category's items are tracks and videos; videos are dropped |
| Playlist/folder tree | `GET /v2/my-collection/playlists/folders?folderId=root&limit=50&order=NAME` | cursor-paginated; items `{type: FOLDER\|PLAYLIST, trn, data, name?}`; folder `name` is top-level, `data.totalNumberOfItems` on `data` |
| Create folder | `PUT /v2/my-collection/playlists/folders/create-folder?name=…&folderId=…` | v2 envelope `{data}` |
| Rename folder | `PUT /v2/my-collection/playlists/folders/rename?trn=trn:folder:…&name=…` | |
| Remove folder / playlist from tree | `PUT /v2/my-collection/playlists/folders/remove?trns=trn:folder:…,trn:playlist:…` | trns are comma-joined with correct prefix |
| Move playlists into folder | `PUT /v2/my-collection/playlists/folders/move?folderId=…&trns=trn:playlist:…,…` | `folderId=root` moves to root |
| Favorite videos | `GET users/{id}/favorites/videos?limit&offset` | `ItemPage<{item: Video}>` |
| Favorite mixes | `GET /v2/favorites/mixes?limit&offset` | v2; parse permissively (mixed `data`/`items` envelopes) |
| Favorite artists (Profiles) | `GET users/{id}/favorites/artists?limit&offset` | already implemented |
| Purchases (*probe*) | `GET users/{id}/purchases?limit&offset` | shape confirmed by probe; empty state when unavailable |

## 5. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| SC-D1 | **Explore absorbs Search; Library is removed.** Explore owns the search input; typing replaces the section content with results; clearing restores sections. | Matches the official client and the owner's request; one home for discovery. |
| SC-D2 | **My Collection is an inline click-expandable section** (chevron header), expanded by default; not a hover flyout. | Owner asked to "expand it"; inline expansion is deterministic in a desktop sidebar. |
| SC-D3 | **Show more reveals the four advanced items** (Mixes & Radio, Videos, Profiles, Purchases) and is collapsed by default. Both expansion states are in-memory only. | Keeps the default sidebar short; no config surface for cosmetic state. |
| SC-D4 | **Quick access lists root folder/playlist entries** from the v2 folders endpoint, mixing user-created and liked playlists. Folders expand recursively and lazily; nested folders supported. | Owner-confirmed; the v2 root already includes favorited playlists. |
| SC-D5 | **Rail mode keeps page icons and shows root playlist covers**; folders render as folder icons; headers/labels/tree text hide. The rail list scrolls. | Owner chose covers in rail; keeps the low-width layout useful. |
| SC-D6 | **Generalize `home.rs` into `pages.rs`** and render Home/Explore/Feed/Mixes with one `PageSections.svelte` component. | One tested parser and renderer instead of four copies. |
| SC-D7 | **Feed probes candidate endpoints** (`pages/feed`, then `pages/my_collection_feed`) and degrades to an empty state ("Feed isn't available for this account.") rather than an error. | Endpoint is unconfirmed; the owner accepted probe-as-we-go. |
| SC-D8 | **Profiles = favorite artists** via the existing favorites endpoint, labeled "Profiles" in the UI. | Owner-confirmed; no separate profile-follow API is known. |
| SC-D9 | **Videos are an inert list** (artwork, title, artist, duration) with a visible note that video playback isn't supported yet. | The audio engine cannot play videos; owner chose inert over browser handoff. |
| SC-D10 | **Mixes are playable**: clicking a mix fetches `pages/mix`, filters to tracks, and plays via the existing `play_tracks` command. | Cheap to support and makes the page useful; videos inside mixes are skipped. |
| SC-D11 | **Purchases probes `users/{id}/purchases`**; if the endpoint or shape differs, the command adapts to the observed response and the page shows an empty state on failure. | Owner chose to probe it. |
| SC-D12 | **Folder management**: create folder, rename folder, remove folder, move playlists (context menu "Move to folder…", root included). Playlists page gets a sort toggle (Name / Date added) applied client-side over the fully fetched list. | Owner chose full folder management; client-side sorting avoids depending on server `order` support. |
| SC-D13 | **Search keeps its four groups** (tracks/albums/artists/playlists); no videos group. | Owner-confirmed. |
| SC-D14 | **Favorite-tracks paging logic moves intact** from Library to the Tracks page, including the unfavorite-drop effect and play/shuffle actions. | No behavior regression during the move. |
| SC-D15 | **Home mix/video items become consistent with shared rendering**: mixes play (SC-D10); videos stay inert. | The shared renderer shouldn't change Home's look, only make mix cards actionable. |

## 6. Routing

| Route | View | Notes |
|---|---|---|
| `home` | `views/Home.svelte` | renders via shared `PageSections` |
| `explore` | `views/Explore.svelte` | replaces `search`; owns `#search-input` |
| `feed` | `views/Feed.svelte` | sections or empty state |
| `playlists` | `views/Playlists.svelte` | "All playlists", root contents, management |
| `folder; id` | `views/Playlists.svelte` | same component with `folderId` prop; breadcrumb back to `playlists` |
| `albums` | `views/CollectionAlbums.svelte` | favorite albums grid |
| `tracks` | `views/CollectionTracks.svelte` | favorite tracks table (from Library) |
| `mixes` | `views/Mixes.svelte` | favorite mixes & radio cards |
| `videos` | `views/Videos.svelte` | favorite videos, inert |
| `profiles` | `views/Profiles.svelte` | favorite artists grid |
| `purchases` | `views/Purchases.svelte` | probe-backed list, empty state |
| `album`, `playlist`, `artist`, `settings` | unchanged | |
| ~~`search`~~, ~~`library`~~ | removed | Ctrl+F / focus shortcut targets `explore` |

`routeForHomeItem` becomes `routeForPageItem`; it maps `ALBUM` → album and `PLAYLIST`/
`EDITORIAL` → playlist, and returns `null` for every other type. The shared renderer
branches first on `item_type === "MIX"` (invoking its `onPlayMix` callback) and otherwise
calls `routeForPageItem`, rendering inert cards when it returns `null`. Queue visibility
stays orthogonal to routing.

## 7. Sidebar anatomy

```
[OxiTide] [collapse]
[back] [forward]
Home
Explore
Feed
──────────────
My Collection            v
  Playlists
  Albums
  Tracks
  Show more              v
    Mixes & Radio
    Videos
    Profiles
    Purchases
──────────────
Playlists                +
  [folder] MISC
    playlist rows (nested, lazy)
  playlist rows
────── spacer ──────
Settings
Log out
```

- The `+` opens the existing playlist-creation flow (`playlistActions.openCreate()`).
- Playlist rows show mosaic covers (`PlaylistCover`) in full mode and cover thumbnails in
  rail mode; folder rows show a folder icon and item count.
- Context menus on tree rows reuse/extend `libraryPlaylistMenuItems`: open, rename,
  delete (owned), move to folder; folders offer rename/remove.
- ARIA: section toggles use `aria-expanded`; active rows use `aria-current="page"`.
- Re-fetch: the tree store exposes `refresh()`; the sidebar also refreshes when
  `playlistActions.revision` changes (playlist created from the picker anywhere).

## 8. Views

### 8.1 Shared section renderer

`lib/components/PageSections.svelte` takes `sections: PageSection[]` and callbacks
(`nav`, `onPlayMix`). It renders the existing card grid; items route via
`routeForPageItem`, except mixes which call `onPlayMix(id)`. Home adopts it unchanged in
appearance (SC-D15).

### 8.2 Explore

Sticky search bar on top; empty query renders explore sections, non-empty renders the
existing debounced search results (4 groups). Loading/error/empty states are reused from
`Search.svelte`. `Ctrl+F`/focus shortcut navigates to explore and focuses the input.

### 8.3 Feed

Section cards from the probe chain, or an empty state. Item routing matches Home.

### 8.4 Playlists & folder pages

Header "All playlists" with: `+` new playlist, "New folder" (modal prompt), sort toggle
(Name / Date added). Rows:
- folder: folder icon, name, "N playlists" (`data.totalNumberOfItems`), navigate to
  `folder; id`; context menu rename/remove/move.
- playlist: mosaic cover, title, "N items" (`numberOfTracks`), navigate to `playlist; id`;
  context menu open/rename/delete (owned)/move to folder.

Folder drill-in reuses the component with `folderId`; a breadcrumb row returns to
`playlists`. "Move to folder…" opens a picker listing the flattened folder tree plus
"Root". Mutations call the tree store's `refresh()` and show success/error toasts.

### 8.5 Albums / Tracks / Profiles

Carry over existing rendering: albums and artists as card grids navigating to their
detail routes; Tracks exactly the Library favorite-tracks block including pager,
unfavorite-drop effect, and play/shuffle.

### 8.6 Mixes & Radio

Card grid of favorite v2 mixes (artwork from mix images). Click plays the mix
(SC-D10); an empty mix shows an informational toast. An error fetching the page renders
`ErrorState` with retry.

### 8.7 Videos

List of favorite videos with 16:9 artwork, title, artists, duration; rows are not
clickable and a muted note explains video playback is unsupported.

### 8.8 Purchases

List of purchased items; entries that resolve to an album navigate to it; otherwise rows
are inert. Unavailable endpoint → empty state.

## 9. Rust client changes

### 9.1 `pages.rs` (from `home.rs`)

- `page_sections(path, extra_params) -> Result<Vec<PageSection>>` with the existing
  rows/modules parsing; `home_page`, `explore_page`, `feed_page`, `mixes_page` wrap it.
- `mix_page(id) -> MixDetail { id, title, tracks: Vec<Track> }`: call `pages/mix`,
  flatten all categories, keep only track-typed items (videos dropped).
- DTO rename: `HomeItem` → `PageItem`, `HomeSection` → `PageSection`; `PageItem` keeps
  `id/title/subtitle/item_type/artwork`.

### 9.2 `folders.rs`

- `FolderEntry { kind: Folder|Playlist, trn, name/id, total_number_of_items, playlist:
  Option<Playlist> }` from the v2 tree endpoint, looping `cursor` until absent,
  unchanged, or empty (safety cap: 20 pages).
- `collection_items(folder_id)` (read), `create_folder(name, parent_id)`,
  `rename_folder(trn, name)`, `remove_entry(trns)`, `move_playlists(trns, folder_id)`
  (writes, all `PUT` with query params, per §4).
- `trn:playlist:<uuid>` / `trn:folder:<id>` prefixes applied in Rust so callers pass raw
  ids; `folderId=root` is the default parent.

### 9.3 `library.rs` additions

- `favorite_videos(limit, offset)` → `ItemPage<Video>`.
- `favorite_mixes(limit, offset)` → permissive parse accepting `{data:[…]}` or `{items:[…]}`
  with each entry bare or wrapped in `{item}`/`{data}`.
- New DTOs in `models.rs`: `Video { id, title, duration, artists, image }`,
  `Mix { id, title, sub_title, mix_type, image }` (image resolved from the SMALL image URL).

### 9.4 `purchases.rs`

- `purchases(limit, offset)` per §4; permissive item parse. If the probe shows a different
  path/shape, the plan updates only this module and its tests.

### 9.5 Commands (`commands.rs` + `generate_handler!` in `main.rs`)

`get_explore`, `get_feed`, `get_favorite_mixes`, `get_mix_tracks`, `get_favorite_videos`,
`get_collection_items` (folder id, default `root`), `create_folder`, `rename_folder`,
`remove_collection_entry`, `move_playlists_to_folder`, `get_purchases`.

## 10. Frontend plumbing

- `api.ts`: new invoke wrappers (camelCase args) + DTO interfaces (`PageItem`,
  `PageSection`, `Video`, `Mix`, `FolderEntry`, `MixDetail`); keep interfaces in sync by
  hand per AGENTS.md.
- New `lib/collection.svelte.ts` store: root entries, per-folder children cache,
  `expanded` set, `ensureRoot()`, `toggleFolder(id)`, `refresh()`; playlists are marked
  owned with the same `assume_owned` rules as today.
- `nav.svelte.ts`: route union per §6; `routeForPageItem`; `focus-search` shortcut path
  updated; nav tests extended.
- Menus: `lib/menus.ts` gains folder/move items; existing playlist menu items reused.
- `App.svelte`: mount new views, pass `collection` + `playlistActions` to `Sidebar`,
  `Explore` replaces `Search`, `Library` removed.

## 11. Error handling & edge cases

| Case | Behavior |
|---|---|
| Feed probe 404s or parses empty | Empty state; never an error banner. |
| Purchases 404/shape mismatch | Empty state after probe confirmation; error toast only for network failures. |
| Folder cursor repeats or never ends | Stop after 20 pages; keep what was loaded. |
| Move/rename/remove fails | Toast error; local state untouched; tree refreshed only on success. |
| Playlist deleted from a detail page | Tree refresh on next sidebar revision; no cross-view event bus needed. |
| Mix has zero tracks (all videos) | Informational toast "This mix has no playable tracks." |
| Explore search fails | Existing `ErrorState` with retry. |
| Folder becomes empty after moves | Renders "No playlists" empty row inside the folder. |
| Videos artwork missing | Cover placeholder via existing `Cover` component. |

## 12. Testing

- **Rust** (`cargo test --workspace`, wiremock only): `pages.rs` explore/feed
  (first-endpoint-404 fallback, both-404 → empty), mix track/video filtering; `folders.rs`
  cursor paging, both entry kinds, trn prefixing, create/rename/remove/move query strings;
  `library.rs` video and mix envelope variants; purchases permissive parse; command-level
  tests through `TidalClient`.
- **UI** (`npm --prefix ui test`): `nav.test.ts` route mapping incl. mix/video; collection
  store (root load, lazy child load, refresh, error); explore behavior (query → results,
  cleared → sections) where extractable as pure logic.
- **Gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui run check`, `npm --prefix ui test`.
- **Manual GUI checklist** (`docs/superpowers/plans/notes/`, real account): confirm feed
  and purchases endpoints; verify tree with existing folders; create/rename/remove folder;
  move a playlist in/out; play a mix; videos inert; rail at narrow width; all pages
  render with real data.

## 13. Open items for the plan

- Feed probe mechanics: whether to add a temporary debug command or rely on the app's
  error surface while stepping candidate paths manually.
- Exact `favorites/mixes` and `pages/mix` envelopes to pin the permissive DTOs; the plan's
  probe task records observed JSON in the notes file before finalizing tests.
- Whether the "Move to folder" picker should list every nested folder or only one level
  (spec leans flattened, with a loading state).
- Sort direction default on the Playlists page (spec assumes Name ascending for folders,
  Date added descending for playlists, matching the official client's default ordering).
