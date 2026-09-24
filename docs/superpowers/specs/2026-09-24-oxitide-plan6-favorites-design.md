# OxiTide — Plan 6 (Favorites) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-24
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 1, first slice)
- **Predecessor:** `2026-09-23-oxitide-plan5-polish-packaging-design.md`
- **Scope note:** Track 1 is split into two plans (owner decision, 2026-09-24):
  **Plan 6 = favorites** (this document) and **Plan 7 = playlist editing** (add-to-playlist
  picker, create/rename/delete, add/remove/reorder tracks).

---

## 1. Purpose & Outcome

Plan 6 delivers the first library-editing capability: favorite/unfavorite **tracks, albums,
and artists** from anywhere in the app. It adds the client's first **write verbs** (POST/DELETE
reusing the single-flight refresh-on-401 path, plus 429/`Retry-After` handling) and **hydrated
favorite state**, so a heart reflects the account's real state in every view — not just inside
the Favorites list.

Success = clicking a heart anywhere toggles the favorite on TIDAL, the UI reflects it
immediately, failures revert cleanly with a visible message, and the Library's Favorites list
and its count stay consistent after an unfavorite from another view.

## 2. Scope

### In scope (Plan 6)
- **Write verbs** in `TidalClient`: form-encoded POST and DELETE, refresh-on-401, one bounded
  429/`Retry-After` retry.
- **Favorites API**: `set_favorite_track/album/artist(id, favorite)` and a `favorite_ids()`
  hydration walk.
- **Commands**: three toggle commands plus `get_favorite_ids`; `fail_tidal` mapping unchanged.
- **Favorites store** (`ui/src/lib/favorites.svelte.ts`): reactive id sets, background hydration
  after login, optimistic toggles with per-item in-flight guard and revert-on-failure.
- **Heart affordances**: `TrackTable` rows (Album, Playlist, Library, Search), Album and Artist
  headers, `PlayerBar` now-playing.
- **Library consistency**: unfavoriting removes the row and count locally, and the session pager
  cache is updated so a remount does not resurrect the row.
- **Failure notice**: a minimal app-level transient strip for mutation errors.
- **Tests**: wiremock for the write verbs and favorites endpoints; vitest for the store and the
  pager addition; a manual smoke checklist.

### Explicitly deferred
- All playlist editing: create, rename, delete, add/remove/reorder tracks, the add-to-playlist
  picker (Plan 7).
- Right-click context menus and the toast system (Track 2a; the notice strip here is a
  placeholder that 2a restyles/replaces).
- Favoriting playlists and videos; batch favorite; favorite state for unavailable tracks.
- Hearts on the Library album/playlist lists (2a context menus will cover them).
- Cross-session (on-disk) favorite caching; hydration runs once per app session.

## 3. Tech Stack Additions

None. `reqwest` already supports form bodies; `SvelteSet` ships with the existing Svelte 5.

## 4. Repository Layout Changes

```
crates/oxitide-tidal/src/
  client.rs     # send_mutation, post_form, delete, 429/Retry-After retry
  library.rs    # set_favorite_track/album/artist, favorite_ids(), FavoriteIds
crates/oxitide-app/src/
  commands.rs   # set_favorite_track/album/artist, get_favorite_ids
  main.rs       # generate_handler! registration
ui/src/
  lib/favorites.svelte.ts        # NEW: favorites store
  lib/favorites.test.ts          # NEW
  lib/pager.svelte.ts            # removeItem() + exported removePagerItem()
  lib/pager.test.ts              # removeItem cases
  lib/api.ts                     # invoke wrappers + FavoriteIds type
  lib/components/TrackTable.svelte    # favorites prop + heart button
  lib/components/PlayerBar.svelte     # heart for the current track
  views/Album.svelte, Artist.svelte   # header heart
  views/Library.svelte                # reacts to unfavorite events
  views/Search.svelte, Playlist.svelte # pass the favorites prop
  App.svelte                          # store wiring + transient notice strip
docs/superpowers/plans/notes/
  plan6-smoke.md # NEW: manual checklist
```

## 5. Architecture

### 5.1 Write verbs (`crates/oxitide-tidal/src/client.rs`)

- Private `send_mutation(method, path, form, token) -> Result<reqwest::Response>` mirrors
  `send_get`: prepends `countryCode`, builds `{base_url}/{path}`, applies bearer auth, sends the
  form as `application/x-www-form-urlencoded`.
- Public API for this plan:
  - `post_form(path, form) -> Result<()>`
  - `delete(path) -> Result<()>`
  Both: send → on 401 `try_refresh(stale)` and resend once (same single-flight path as `get`) →
  map the status to `Error`. Any 2xx is success; the response body is ignored.
- **429 handling (mutations only):** read `Retry-After` (integer seconds; an HTTP-date value is
  treated as the default), sleep `min(value, 5s)` (default 1s when absent), resend once. A second
  429 maps to `Error::RateLimited`. Reads keep today's behavior (429 maps directly), so no read
  path gains hidden latency.

### 5.2 Favorites endpoints (`crates/oxitide-tidal/src/library.rs`)

- `set_favorite_track(id: u64, favorite: bool) -> Result<()>` — and the album/artist twins:
  - add: `POST users/{user_id}/favorites/{tracks|albums|artists}` form
    `{trackId|albumId|artistId}={id}`
  - remove: `DELETE users/{user_id}/favorites/{tracks|albums|artists}/{id}`
  The ids come from `self.session().user_id`.
- `favorite_ids() -> Result<FavoriteIds>`:
  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct FavoriteIds {
      pub tracks: Vec<u64>,
      pub albums: Vec<u64>,
      pub artists: Vec<u64>,
  }
  ```
  Walks the three favorites lists **in parallel** (`tokio::join!`) using the existing
  `get_all_pages` with a **1000-row window** (the size the favorites endpoint is known to honor),
  then maps `Favorited<T>` items to ids. Only ids cross IPC.
- **Known limitation:** TIDAL's favorites list endpoints omit unavailable tracks, so their ids
  are absent from the sets; a heart on an unavailable track can read hollow. Accepted and
  documented in the smoke checklist.

### 5.3 Commands and IPC (`crates/oxitide-app/src/commands.rs`, `main.rs`)

- `set_favorite_track(state, track_id: u64, favorite: bool) -> Result<(), String>` (and album/artist).
- `get_favorite_ids(state) -> Result<FavoriteIds, String>`.
- All map errors through `state.fail_tidal(e)`, so `Unauthorized` still emits `session_expired`.
- Registered in `generate_handler![...]`; `invoke` sends camelCase (`trackId`, `albumId`,
  `artistId`, `favorite`) while the Rust parameters stay snake_case.

### 5.4 Favorites store (`ui/src/lib/favorites.svelte.ts`)

Factory `createFavorites()`, following the `createNav`/`createPlayer` pattern (closure `$state`,
returned getters and methods):

- **State:** `SvelteSet<number>` for `tracks`, `albums`, `artists`; `hydrated`; `hydrating`;
  a per-item `pending` set; `error: string | null`; and for list sync
  `trackRevision: number` + `lastUnfavoritedTrackId: number | null`.
- **`hydrate()`** — in-flight deduped, idempotent; one `api.getFavoriteIds()` invoke fills the
  three sets. On failure, `hydrated` stays false, the error is surfaced, and hearts stay hollow
  (toggles still work: adding is idempotent); a later `hydrate()` call or the next launch retries.
- **`toggleTrack/Album/Artist(id)`** — optimistic: flip the set immediately, invoke, and on
  failure revert and set `error`. Per-item `pending` guard ignores repeat clicks while a mutation
  is in flight. Resolves to the resulting state, or `null` when reverted.
  - A successful **track unfavorite** additionally calls `removePagerItem("favorites", id)` and
    bumps `trackRevision` / records `lastUnfavoritedTrackId`, so any mounted Library view drops
    the row (see 5.6) and an unmounted one reads a corrected cache on remount.
  - A successful **track favorite** calls `invalidatePager("favorites")` so a later Library mount
    refetches instead of showing the stale list.
- **`reset()`** — clears sets, flags, and error; called on logout and session expiry.
- **`dismissError()`** — clears the error (the notice strip also auto-dismisses).

### 5.5 App wiring (`ui/src/App.svelte`)

- Instantiate the store once next to `nav`/`player` and pass it down as a prop (the existing
  pattern for `player`).
- Hydrate when `loggedIn` becomes true (fire-and-forget; boot is not blocked). `reset()` in
  `onLogout` and in the `session_expired` handler, alongside the existing `clearPagerCache()`.
- Render `favorites.error` as a minimal transient strip (message + dismiss, auto-dismiss ~6s).
  Track 2a replaces this with the real toast system.

### 5.6 Heart affordances

- **`TrackTable.svelte`** gains a required `favorites` prop and a heart button in the row action
  cell: `♥` when favorited, `♡` when not (text glyphs, matching the existing `▶`/`＋` buttons),
  with `aria-label` "Favorite"/"Unfavorite". Every call site (Album, Playlist, Library, Search)
  passes the store, so hearts appear everywhere tracks are listed.
- **Album.svelte / Artist.svelte** headers gain a heart button for the album/artist itself.
- **PlayerBar.svelte** gains a heart for `player.state.current.track_id` (not rendered when
  nothing is playing).
- **Library.svelte** watches `favorites.trackRevision`; when `lastUnfavoritedTrackId` is still in
  `pager.items`, it calls `pager.removeItem(id)`. This covers unfavorites made from the Library
  itself, the PlayerBar, or any other view.
- The Library's album/playlist lists get no hearts in this plan (2a context menus).

### 5.7 Pager additions (`ui/src/lib/pager.svelte.ts`)

- `Pager.removeItem(id: number): void` — no-op when the id is absent; otherwise removes the row,
  decrements `total` (when > 0) and `nextOffset` (floored at 0), and publishes to the session
  cache. `unavailable` stays correct because both `total` and `items.length` drop by one.
- Exported `removePagerItem(key: string, id: number): void` — applies the same correction to a
  cache entry directly, for when no pager instance is mounted.
- Exported `invalidatePager(key: string): void` — deletes a cache entry so the next mount refetches
  (used when a favorite is added, which the loaded list cannot patch locally).
- **Race note (plan-level):** if a page fetch is in flight when a row is removed, offsets computed
  before the removal can drift. The plan must settle the in-flight task first (await it) or fall
  back to `refresh()` for that case.

## 6. IPC Surface Changes

| Command / event | Change |
|---|---|
| `set_favorite_track` | NEW — `{ trackId: number, favorite: boolean } -> void` |
| `set_favorite_album` | NEW — `{ albumId: number, favorite: boolean } -> void` |
| `set_favorite_artist` | NEW — `{ artistId: number, favorite: boolean } -> void` |
| `get_favorite_ids` | NEW — `-> { tracks: number[], albums: number[], artists: number[] }` |
| `session_expired` (event) | unchanged — still emitted on `Unauthorized`, including writes |

Existing commands and events are unchanged.

## 7. UI Changes

- **App.svelte**: favorites store instance, hydrate/reset wiring, transient error strip.
- **TrackTable**: heart button + `favorites` prop.
- **Album / Artist**: header heart.
- **PlayerBar**: now-playing heart.
- **Library**: reacts to unfavorite events via `pager.removeItem`.
- **api.ts**: four wrappers and the `FavoriteIds` interface.
- **pager.svelte.ts**: `removeItem` + `removePagerItem`.

## 8. Error Handling

| Situation | Behaviour |
|---|---|
| Toggle fails (network) | Heart reverts to its previous state; transient notice strip shows the error |
| Toggle hits 429 | Client retries once after `Retry-After` (≤5s); if still limited, notice says TIDAL is rate-limiting |
| Session expires during a toggle | Existing `session_expired` flow; store resets with the pager cache |
| Hydration fails | Hearts render hollow; toggles still work; retried on the next `hydrate()` / launch |
| Repeat clicks while a toggle is pending | Ignored by the per-item guard (no duplicate requests, no flicker) |
| Unfavorite while the pager is mid-fetch | Plan settles the in-flight page or refreshes (5.7) |
| Unavailable favorited track | Not in the hydrated set; heart may read hollow (documented) |

## 9. Testing Strategy

- **Rust (wiremock, `oxitide-tidal`):**
  - `post_form` / `delete`: method, path, `authorization` header, form body, and 2xx → `Ok`.
  - 401 → refresh → single retry on writes; no refresh token → `Unauthorized`.
  - 429 with `Retry-After: 0` retried once → success; 429 twice → `RateLimited`.
  - `set_favorite_*`: add/remove path and form key per type, user id from the session.
  - `favorite_ids`: three endpoints walked in parallel with `limit=1000`, ids extracted.
- **UI (vitest):**
  - Store: hydrate fills sets; hydrate failure leaves hollow + error; optimistic toggle and
    revert on failure; pending guard suppresses a second call; unfavorite triggers
    `removePagerItem` and bumps `trackRevision`; `reset` clears everything.
  - Pager: `removeItem` updates items/total/nextOffset/unavailable and the cache; no-op for
    unknown ids; `removePagerItem` corrects an unmounted cache entry.
- **Manual smoke (`docs/superpowers/plans/notes/plan6-smoke.md`):** heart state correct after
  restart; toggle from Album/Playlist/Search/PlayerBar; unfavorite from Album removes the Library
  row and updates the count; network off → revert + notice; session expiry → re-login → hearts
  rehydrate; unavailable-track caveat noted.
- **Gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui test`, `npm --prefix ui run check`.

## 10. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Task | Exit criteria |
|---|---|---|
| P6-1 | Write verbs + 429 handling | `post_form`/`delete` with refresh-on-401 and bounded `Retry-After` retry; tests green |
| P6-2 | Favorites API + hydration | `set_favorite_*` and `favorite_ids()` with wiremock tests |
| P6-3 | Tauri commands | Four commands registered; invoke round-trips |
| P6-4 | Favorites store + App wiring | Hydration after login, reset on logout/expiry, notice strip |
| P6-5 | Heart affordances | Hearts in TrackTable, Album/Artist headers, PlayerBar |
| P6-6 | Library list consistency | Pager `removeItem`/`removePagerItem`; row disappears with correct count |
| P6-7 | Verification | Gates green + smoke checklist recorded |

## 11. Performance Impact (PM-D1)

- **Hydration:** one invoke after login; three parallel list walks with 1000-row windows
  (typically 1–3 requests for the tracks list, 1 each for albums/artists); only ids cross IPC
  (~10–100 KB). Fire-and-forget, so time-to-first-paint is unchanged.
- **Hearts:** O(1) `SvelteSet` lookups per row; no extra network traffic per view; only the
  affected heart cell re-renders.
- **Toggles:** one request each; no list refetch — the Library row is removed locally.

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| v1 write endpoints unverified against the live API | First implementation task includes a live add/remove probe on the owner's account; the researched v2 equivalents are the fallback (see §14) |
| `Retry-After` sent as an HTTP-date | Treated as the 1s default; still one bounded retry |
| Local row removal races an in-flight page fetch | Settle or refresh (5.7); plan-level task |
| Very large libraries make hydration heavy | 1000-row windows keep it to one request per 1000 favorites; hydration is fire-and-forget and a failure leaves it unhydrated instead of stalling |
| Unavailable favorites absent from the id sets | Documented limitation; hearts on unavailable rows may read hollow |
| Notice strip duplicates future toasts | Explicitly a placeholder; Track 2a replaces it |

## 13. Decisions Log (Plan 6)

| # | Decision | Rationale |
|---|---|---|
| P6-D1 | Track 1 splits into Plan 6 (favorites) and Plan 7 (playlist editing). | Playlist creation lives inside the official add-to-playlist flow, so it belongs with item editing; favorites stand alone. |
| P6-D2 | Frontend store + thin Rust commands. | Matches the existing factory-store pattern; one hydration invoke instead of per-row IPC. |
| P6-D3 | Hydrate all three favorite-id sets after login. | No membership endpoint exists; accurate hearts everywhere are worth one background walk. |
| P6-D4 | 429 `Retry-After` retry on mutations only, capped at 5s. | Writes are user-initiated and rare; reads keep their current latency profile. |
| P6-D5 | Optimistic toggles with revert + a transient notice. | Instant feedback; the toast system is 2a's job, this strip is a placeholder. |
| P6-D6 | Inline hearts now; context menus in 2a. | Actions must be reachable without pulling Track 2a forward. |
| P6-D7 | Write via the community v1 API (`api.tidal.com/v1`). | OxiTide's PKCE client is v1-based and its scope already includes `w_usr`; v2 paths are the researched fallback. |

## 14. Pre-spec Research Notes (endpoints)

Recorded for Plan 6 and Plan 7 (reference implementation: the community `tidalapi` client;
official v2 surface: `api.tidal.com/v2`):

- **Favorites:** v1 `POST users/{id}/favorites/{tracks|albums|artists}` (form id, batch =
  comma-separated) and `DELETE .../{id}`. Verified in tidalapi 0.8.x source.
- **Playlist create:** v2 `PUT my-collection/playlists/folders/create-playlist?name=&description=&folderId=root`
  (JSON:API `{data: ...}` response).
- **Playlist rename/description:** v1 `POST playlists/{uuid}` with form `title`/`description`.
- **Playlist delete:** v1 `DELETE playlists/{uuid}`.
- **Playlist items:** add `POST playlists/{uuid}/items` (form `trackIds`, `toIndex`, `onDupes`,
  `onArtifactNotFound`), remove `DELETE playlists/{uuid}/items/{indices}`, **reorder
  `POST playlists/{uuid}/items/{indices}` with `toIndex`** — the roadmap's open question is
  resolved: a dedicated reorder endpoint exists (v2 equivalent:
  `PATCH /playlists/{id}/relationships/items`). Mutations send `If-None-Match: <etag>` from the
  last playlist GET as a concurrency guard.
- **Playlist image:** no community endpoint sets a custom cover (the official
  `PATCH /playlists/{id}/relationships/coverArt` is internal-tier); the 2×2 first-four-covers
  cover is automatic server-side, so Plan 7 can defer image picking.
- **Scopes:** the PKCE flow already requests `r_usr w_usr w_sub`, so write operations are
  authorized.

## 15. Out of Scope (tracked for later)

- **Plan 7 (Track 1, second slice):** add-to-playlist picker (create/recent/show all), create
  modal (name, description, visibility), rename, delete, add/remove/reorder tracks.
- **Track 2a:** context menus, toast system, sortable/filterable tables.
- **Later tracks:** favorite playlists/videos, batch favorite, offline favorite queueing, and
  anything listed in the roadmap's Tracks 3–7.
