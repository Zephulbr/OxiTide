# OxiTide — Plan 7 (Playlist Editing) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-24
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 1, second slice)
- **Predecessor:** `2026-09-24-oxitide-plan6-favorites-design.md`
- **Scope note:** Track 1 is split into two plans (owner decision, 2026-09-24):
  Plan 6 = favorites (shipped) and **Plan 7 = playlist editing** (this document).

---

## 1. Purpose & Outcome

Plan 7 completes Track 1: the user can create, rename, and delete their playlists and edit
their contents (add tracks, remove rows, reorder) from anywhere in OxiTide. It adds the
client's first **PUT** verb, a **v2 base URL**, a **JSON-returning mutation**, and the
playlist domain is consolidated into one module.

Success = a playlist can be created from the add-to-playlist picker or Library, tracks can be
added from any track table and from the Album header, rows can be removed and moved up/down
in the Playlist view, duplicates are surfaced for a decision instead of being silently
dropped or doubled, and every write is verifiable in the official TIDAL app after a refetch.

## 2. Scope

### In scope (Plan 7)
- **Mutation plumbing** in `TidalClient`: shared 401-refresh / 429-`Retry-After` flow for all
  HTTP methods, a v2 base URL, and a JSON-decoding variant for mutations.
- **Playlist domain module** (`crates/oxitide-tidal/src/playlists.rs`): playlist reads moved
  from `catalog.rs` / `library.rs`, plus `create_playlist`, `rename_playlist`,
  `delete_playlist`, `add_playlist_tracks`, `remove_playlist_items`, `move_playlist_items`.
- **Commands**: six new `#[tauri::command]`s, registered in `generate_handler!`.
- **Playlist actions store** (`ui/src/lib/playlists.svelte.ts`): picker/step state, the
  two-phase duplicate flow, playlist creation, busy flag, and reset-on-logout; rename and
  delete are direct API calls from the views with notice-on-failure.
- **UI**: `Modal`, `PlaylistPicker` (list → create → duplicates), `ConfirmDialog`; add-to-
  playlist icons in `TrackTable` rows; Album header "Add to playlist"; Library New/Rename/
  Delete with inline rename; Playlist view Rename/Delete plus row `↑` `↓` `✕`.
- **Notice store** (`ui/src/lib/notice.svelte.ts`) rendered in App's existing strip markup.
- **Tests**: wiremock for the client and playlist endpoints; vitest for the store's flow
  logic; a manual smoke checklist in `docs/superpowers/plans/notes/plan7-smoke.md`.

### Explicitly deferred
- Drag-and-drop reordering (Track 2a UX pack designs the interaction; Plan 7 ships up/down
  move buttons).
- A reusable context-menu / kebab menu (2a).
- Playlist covers, visibility (`public`/`private`), folders, and reordering playlists in the
  Library list.
- Favoriting playlists (the API exists; the roadmap limits favorites to tracks/albums/
  artists).
- `If-None-Match`/ETag concurrency guards (single-writer assumption; revisit if a conflict
  is ever observed).
- Adding videos to playlists; playlist track search/filter; multi-select editing.
- On-disk playlist caching; the session pager cache is unchanged.

## 3. Tech Stack Additions

None. `reqwest` already supports PUT and query params; Svelte 5 provides `SvelteMap`; no new
crates, no new npm packages. Version stays 0.2.0; no packaging or config changes.

## 4. Repository Layout Changes

```
crates/oxitide-tidal/src/
  client.rs        (mutate refactor; v2 base; put_json; post_form_json)
  playlists.rs     (NEW: playlist reads moved here + write methods + tests)
  catalog.rs       (playlist reads removed; album/artist/track readers stay)
  library.rs       (user_playlists/all_user_playlists removed; favorites stay)
  lib.rs           (pub mod playlists;)
crates/oxitide-app/src/
  commands.rs      (six new commands + DTO tests)
  main.rs          (generate_handler! entries)
ui/src/lib/
  api.ts                    (six wrappers)
  playlists.svelte.ts       (NEW: createPlaylistActions store)
  notice.svelte.ts          (NEW: createNotice store)
  playlists.test.ts         (NEW: store tests)
  notice.test.ts            (NEW: notice store tests)
  components/Modal.svelte          (NEW: overlay/panel wrapper)
  components/PlaylistPicker.svelte (NEW: list/create/duplicates steps)
  components/ConfirmDialog.svelte  (NEW: confirm/cancel)
  components/TrackTable.svelte     (add icon; optional playlistActions prop)
ui/src/views/
  Album.svelte     ("Add to playlist" header button)
  Playlist.svelte  (rename/delete, row actions, refetch after writes)
  Library.svelte   (New playlist, inline rename, delete)
ui/src/App.svelte  (store wiring, three modals, notice strip)
```

## 5. Architecture

### 5.1 Mutation plumbing (`crates/oxitide-tidal/src/client.rs`)

- Extract the current 401-refresh / 429-`Retry-After` logic from `post_form`/`delete` into a
  private `mutate(method, base, path, query, form) -> Result<reqwest::Response>`;
  the existing public methods become thin wrappers with unchanged behavior and tests.
- Add a second base URL, derived once in `new`: trim the trailing `/`; if it ends with `/v1`,
  replace that with `/v2`, otherwise append `/v2`. Production (`https://api.tidal.com/v1/`)
  yields `https://api.tidal.com/v2/`; tests that pass the bare mock server URI get
  `{server}/v2`, so their v2 mocks mount under `/v2/…`.
  v2 requests do **not** send the `countryCode` query param; v1 requests keep it.
- New public verbs:
  - `put_json<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)]) -> Result<T>`
    — v2 base, query params, JSON response.
  - `post_form_json<T: DeserializeOwned>(&self, path: &str, form: &[(&str, String)]) -> Result<T>`
    — v1 base, form body, JSON response.
- All verbs share: refresh once on 401, retry once on 429 honoring `Retry-After` (≤5 s), then
  the existing status→error mapping (`Unauthorized`/`Forbidden`/`NotFound`/`RateLimited`/`Api`).

### 5.2 Playlist domain (`crates/oxitide-tidal/src/playlists.rs`)

Reads moved here unchanged: `playlist`, `playlist_tracks_page`, `playlist_tracks_all` (from
`catalog.rs`), `user_playlists`, `all_user_playlists` (from `library.rs`), with their tests.
New writes (all verified against community `tidalapi`; see §14):

| Method | Request |
|---|---|
| `create_playlist(name, description) -> Playlist` | `PUT {v2} my-collection/playlists/folders/create-playlist?name=&description=&folderId=root`; parse `{data: …}` into `Playlist` |
| `rename_playlist(id, title, description)` | `POST playlists/{uuid}` form `title`, `description` |
| `delete_playlist(id)` | `DELETE playlists/{uuid}` |
| `add_playlist_tracks(id, track_ids, allow_duplicates) -> Vec<u64>` | `GET playlists/{uuid}` first (fresh `numberOfTracks`), then sequential `POST playlists/{uuid}/items?limit=N` form `trackIds` (comma-joined chunk), `toIndex = initial_total + added_so_far`, `onDupes = SKIP\|ADD`, `onArtifactNotFound = SKIP`; accumulate `addedItemIds` |
| `remove_playlist_items(id, indices)` | `DELETE playlists/{uuid}/items/{i,j,…}` |
| `move_playlist_items(id, indices, to_index)` | `POST playlists/{uuid}/items/{i,j,…}` form `toIndex` |

Implementation notes:
- **Chunking:** adds are chunked at 100 (preserving request order). `limit` matches the chunk
  length. The UI sends one remove/move index at a time; the methods still accept slices.
- **Requested ids are deduped** (first occurrence wins) before adding, so a duplicate inside
  one request cannot be mistaken for a server-side skip.
- **`addedItemIds` parsing:** accept numbers or numeric strings, ignore extra fields. When the
  field is absent, assume every requested id was added (return the deduped request) so the UI
  never shows a false duplicate dialog.
- **Create parsing:** a private `V2Envelope<T> { data: T }`; the `Playlist` DTO's defaults
  cover missing v2 fields (`numberOfVideos`, `image`, …) — `uuid` and `title` must be present.
- `lib.rs` gains `pub mod playlists;`.

### 5.3 Commands and IPC (`crates/oxitide-app/src/commands.rs`, `main.rs`)

Six commands following the existing `require_client` / `state.fail_tidal` pattern, with
snake_case params (camelCase args from `invoke`) and registration in `generate_handler!`:

```rust
create_playlist(name: String, description: String) -> Playlist
rename_playlist(id: String, title: String, description: String) -> ()
delete_playlist(id: String) -> ()
add_playlist_tracks(id: String, track_ids: Vec<u64>, allow_duplicates: bool) -> Vec<u64>
remove_playlist_items(id: String, indices: Vec<u32>) -> ()
move_playlist_items(id: String, indices: Vec<u32>, to_index: u32) -> ()
```

### 5.4 Playlist actions store (`ui/src/lib/playlists.svelte.ts`)

`createPlaylistActions(notice: Notice): PlaylistActions`, matching the
`createNav`/`createFavorites` factory pattern; App creates the shared `Notice` (§5.5) and
injects it so picker outcomes and view-level rename/delete failures land in one strip. All
flow logic lives here so vitest covers it; components are thin.

```ts
export interface PlaylistActions {
  readonly open: boolean;
  readonly step: "list" | "create" | "duplicates";
  /// True when opened from a track/album surface (create navigates); false from Library.
  readonly fromPicker: boolean;
  readonly pendingTrackIds: readonly number[];
  readonly duplicates: readonly Track[];      // phase-1 skips awaiting a decision
  readonly choices: SvelteMap<number, boolean>; // true = "Add both", default false
  readonly busy: boolean;
  /// Bumped after a successful create so mounted lists (Library) refresh.
  readonly revision: number;
  openPicker(tracks: Track[]): void;           // steps to "list"
  openCreate(): void;                          // Library entry: steps to "create", fromPicker=false
  startCreate(): void;                         // picker's "New playlist…" button
  close(): void;                               // resets steps/choices
  reset(): void;                               // close (logout/expiry)
  addTo(playlist: Playlist): Promise<void>;    // phase 1; steps to "duplicates" or notices
  createAndAdd(name: string, description: string): Promise<Playlist | null>;
  setChoice(trackId: number, addBoth: boolean): void;
  setAllChoices(addBoth: boolean): void;
  confirmDuplicates(): Promise<void>;          // phase 2; notices the outcome
}
```

Behavior:
- **`addTo` (phase 1):** call `api.addPlaylistTracks(id, pending, false)`; `skipped =
  requested − added` (request order). No skips → close modal, notice `"N added"`. Skips →
  `duplicates = skipped tracks`, `step = "duplicates"`.
- **`confirmDuplicates` (phase 2):** ids whose choice is `true` are sent with
  `allow_duplicates: true`; the rest stay unwritten. Then close and notice
  `"added X, kept one of Y"` counts. Closing the modal without confirming keeps one of each
  (no phase 2).
- **`createAndAdd`:** create first, then add the pending tracks to the new uuid (which cannot
  collide); with no pending tracks (Library's New playlist) it only creates. Returns the
  created playlist so the caller can navigate (only when `fromPicker`) and bumps `revision`
  so a mounted Library list refetches. Add failure after create still returns the playlist
  with a notice (the playlist exists).
- **Busy:** every remote call sets `busy`; controls are disabled while it is true.
- **Errors:** any failure calls `notice.show(String(e))` and leaves state unchanged (no local
  list mutation); the view refetches nothing on failure.
- **`reset`:** closes the picker and clears choices; App calls it on logout and session
  expiry so a stale picker cannot outlive the signed-in account.

### 5.5 Notice store (`ui/src/lib/notice.svelte.ts`)

```ts
export interface Notice { readonly message: string | null; show(msg: string): void; dismiss(): void }
export function createNotice(): Notice
```

6 s auto-dismiss timer that resets on each `show`. App creates one instance, injects it into
`createPlaylistActions`, and renders `notice.message` in the existing strip markup next to the
favorites error strip. Library/Playlist views use the same instance for rename/delete
failures. Unification with `favorites.error` is out of scope.

### 5.6 Components

- **`Modal.svelte`:** overlay + panel + title + close button; closes on Escape and the ✕;
  renders nothing when closed. Backdrop clicks do not close (keeps the markup free of a11y
  warnings); no focus trap in this plan (personal app; 2a polish).
- **`PlaylistPicker.svelte`:** driven by the store.
  - *list step:* playlists from `api.getUserPlaylists()` fetched on open (title + track count),
    an "Add to playlist" heading, "New playlist…" button, empty state "No playlists yet".
    Choosing a playlist calls `addTo`.
  - *create step:* name (required, trimmed) + description (optional) + Create/Cancel; on
    create, calls `createAndAdd`; navigates to the new playlist when it came from a picker
    flow with pending tracks, otherwise just closes (Library passes no tracks and stays).
  - *duplicates step:* for each skipped track, two side-by-side cards built from the same
    `Track` metadata — "In playlist" (the existing occurrence) and "Adding" (the pending
    addition) — with a per-row Keep one / Add both toggle (default Keep one) and an
    Apply-to-all control; Confirm calls `confirmDuplicates`, Cancel/close is equivalent to
    keeping one of each.
- **`ConfirmDialog.svelte`:** message + Confirm/Cancel, used for playlist deletion.
- **`TrackTable.svelte`:**
  - `onAddToPlaylist?: (track: Track) => void` — renders a "＋" button (title/aria "Add to
    playlist") in every track table (Search, Album, Library, Playlist).
  - `playlistActions?: { onRemove(index: number): void; onMove(index: number, delta: -1 | 1):
    void; busy: boolean; disabled: boolean }` — renders `↑` `↓` `✕`; `↑` disabled at index 0,
    `↓` at the last row, all disabled while `busy` or `disabled` (the Playlist view passes
    `disabled: !pager.done` so index-based writes only run against the complete list).
- **Album view:** header "Add to playlist" button calls `openPicker(tracks.map(t => t.id))`.
- **Playlist view:** Rename and Delete buttons beside the title; Rename opens a small `Modal`
  with the current name/description prefilled and calls `api.renamePlaylist` (success updates
  the header and notice); Delete opens `ConfirmDialog`. Row remove calls the API then
  `await pager.refresh()` and notices "Removed"; row move calls the API then
  `await pager.refresh()` (the visible order is the feedback). On failure the notice shows the
  error and no refetch happens. Deleting the open playlist navigates to Library.
- **Library view:** "New playlist" button (`openCreate`), per-playlist inline rename (row
  becomes an input with Save/Cancel; failure restores display mode + notice), and Delete with
  `ConfirmDialog`; local list updates after each successful call.
- **App.svelte:** creates the `Notice` and the `playlistActions` store; renders
  `PlaylistPicker`, `ConfirmDialog` (delete), and the notice strip; calls
  `playlistActions.reset()` on logout and session expiry; passes callbacks to views/TrackTable
  as it does for `favorites`.

## 6. IPC Surface Changes

| Command | Change |
|---|---|
| `create_playlist` | NEW — `{ name: string, description: string } -> Playlist` |
| `rename_playlist` | NEW — `{ id: string, title: string, description: string } -> void` |
| `delete_playlist` | NEW — `{ id: string } -> void` |
| `add_playlist_tracks` | NEW — `{ id: string, trackIds: number[], allowDuplicates: boolean } -> number[]` (added ids) |
| `remove_playlist_items` | NEW — `{ id: string, indices: number[] } -> void` |
| `move_playlist_items` | NEW — `{ id: string, indices: number[], toIndex: number } -> void` |

Existing commands and events are unchanged.

## 7. UI Changes

- **App.svelte**: two new stores, three modals, notice strip.
- **TrackTable**: add-to-playlist icon; optional playlist row actions.
- **Album**: header add-all button.
- **Playlist**: rename/delete; row remove/move; refetch after writes.
- **Library**: New playlist; inline rename; delete.
- **api.ts**: six wrappers.
- **New**: `playlists.svelte.ts`, `notice.svelte.ts`, `Modal.svelte`, `PlaylistPicker.svelte`,
  `ConfirmDialog.svelte`.

## 8. Error Handling

| Situation | Behaviour |
|---|---|
| Any write fails (network) | Notice with the error; no local change; views keep the server state |
| Write hits 429 | Client retries once after `Retry-After` (≤5 s); if still limited, notice says TIDAL is rate-limiting |
| Session expires during a write | Existing `session_expired` flow (favorites/pager cache reset); `playlistActions.reset()` closes the picker |
| Editing a playlist owned by someone else | `Forbidden` → notice ("You can only edit your own playlists") |
| Playlist deleted elsewhere | `NotFound` → notice; the view refetches on the next mount |
| Add returns no `addedItemIds` | Treated as "all added"; no duplicate dialog (documented) |
| Duplicate dialog dismissed | Keep one of each; nothing further is sent |
| Remove/move while the pager is incomplete | Buttons disabled until `pager.done` (index-based writes need the full list) |
| Remove/move at the list edges | `↑`/`↓` disabled; callbacks are not invoked |

## 9. Testing Strategy

- **Rust (wiremock, `oxitide-tidal`):**
  - `mutate` refactor: every existing `post_form`/`delete` test stays green; new pins for the
    v2 base (path and host) and no `countryCode` on v2.
  - `put_json`: query params on create; 401 → refresh → single retry; 429 retry; JSON decode.
  - `post_form_json`: parses `addedItemIds` as numbers **and** numeric strings; missing field
    falls back to the requested ids.
  - `add_playlist_tracks`: initial `GET` for `numberOfTracks`; `onDupes=SKIP` vs `ADD`;
    101 ids → two POSTs with `toIndex` 0-based accounting (e.g., 5 then 105) and `limit`
    matching each chunk; accumulated ids in request order; dedupes repeated ids.
  - `remove_playlist_items` / `move_playlist_items`: `items/{i,j}` path, `toIndex` form,
    204/200 → `Ok`.
  - `create_playlist`: parses `{data: …}`; missing `uuid` → decode error.
  - `rename_playlist` / `delete_playlist`: path, form fields, 2xx → `Ok`; 403 → `Forbidden`.
- **UI (vitest, `playlists.test.ts`):** phase-1 skip math; empty-skip path notices and closes;
  default keep-one sends no phase 2; add-both sends phase 2 with only chosen ids and
  `allowDuplicates: true`; apply-to-all; dismiss = keep one; create-then-add ordering and
  create-then-add-failure notice; busy toggling; reset closes the picker and clears choices;
  failure paths call `notice.show` and change nothing. **`notice.test.ts`:** auto-dismiss
  after 6 s with fake timers, timer resets on a new message, manual dismiss.
- **Manual smoke (`docs/superpowers/plans/notes/plan7-smoke.md`):** create/rename/delete
  visible in Library and the official app; add single track and whole album from every
  surface; duplicate dialog keep-one vs add-both reflected in the official app; remove one of
  two identical rows removes the right one; move up/down and edge states; the unavailable-row
  index probe (edit rows after a hidden unavailable track and confirm the right track
  changed); >1k-track playlist refetch timing; network-off and session-expiry during writes;
  delete-while-open navigates to Library.
- **Gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui test`, `npm --prefix ui run check`.

## 10. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Task | Exit criteria |
|---|---|---|
| P7-1 | Mutation plumbing | `mutate` refactor + v2 base + `put_json`/`post_form_json`; existing tests green, new pins |
| P7-2 | Playlist domain module | Reads moved; six write methods with wiremock tests |
| P7-3 | Commands | Six commands registered; DTO tests |
| P7-4 | Actions store + notice | Two-phase duplicate flow, create, reset-on-logout, vitest green |
| P7-5 | Modals + TrackTable actions | Picker (list/create/duplicates), Confirm, row icons, Album add-all |
| P7-6 | View wiring | Playlist rename/delete + row actions + refetch; Library CRUD/inline rename |
| P7-7 | Verification | Gates green + smoke checklist recorded |

## 11. Performance Impact (PM-D1)

- **Picker:** one `getUserPlaylists` request per open; no polling; the modal renders nothing
  while closed.
- **Adds:** one `GET` (fresh count) + one `POST` per 100 tracks; phase 2 only for tracks the
  user chooses to duplicate.
- **After a write:** one playlist refetch (the normal mount path, already virtualized and
  prefetched); no full-library work, no extra caching.
- **Row actions:** callbacks only; no new per-row reactive work beyond a boolean.
- **Reorder/remove:** one request each; buttons are inert while busy.

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Index-based writes assume the loaded row index equals the server position; TIDAL omits unavailable rows while counting them | Writes disabled until `pager.done`; refetch after every write so drift cannot accumulate; smoke probe on a playlist with an unavailable row; fallback is a contained change that resolves indices by occurrence ordinal |
| `toIndex` boundary semantics for moves (clamp at end, same-position move) | Pinned by wiremock request shapes; verified in smoke against the official app |
| `addedItemIds` element type / presence is unverified against the live API | Parser accepts numbers and numeric strings; missing field falls back to "all added"; smoke verifies the duplicate dialog appears |
| No `If-None-Match`: a concurrent edit (official app) shifts indices between load and write | Accepted for a single-user client; refetch reveals the outcome; ETag work is deferred until a conflict is observed |
| Chunked album adds append out of order if positions are stale | Each add starts with a fresh `GET` and computes `toIndex` as `initial_total + added_so_far`, which tolerates skipped duplicates |
| Deleting a playlist that is currently playing | Queue is snapshot-based and unaffected; the view navigates to Library |
| Modal polish (focus, escape) is minimal | Documented as placeholder-quality; Track 2a owns overlays/toasts |

## 13. Decisions Log (Plan 7)

| # | Decision | Rationale |
|---|---|---|
| P7-D1 | Full Track 1 slice including reorder, via `↑`/`↓` buttons; drag-and-drop deferred to 2a. | Reorder is a Track 1 outcome; DnD is a UX-pack interaction that would pull 2a forward. |
| P7-D2 | Inline icons + a modal picker; no context-menu framework now. | Actions must be reachable without building the 2a menu system. |
| P7-D3 | Removing a duplicated track removes only the clicked occurrence. | Matches the table and the index-based API; `remove_by_id`-style bulk removal is a footgun. |
| P7-D4 | Track writes are server-authoritative: refetch after success; CRUD updates local lists. | Index-based writes and duplicates make optimistic rows risky; one refetch is cheap. |
| P7-D5 | Two-phase duplicate flow: add with `onDupes=SKIP`, then ask about the skips and send `onDupes=ADD` only for "Add both". | No full playlist pre-scan; warnings arrive only when a real collision exists; keep-one is the default and writes nothing. |
| P7-D6 | Create lives inside the picker; picker-created playlists navigate to themselves, Library-created ones stay. | Matches the "add to a new playlist" flow and avoids surprising navigation. |
| P7-D7 | Row edit buttons disabled until the pager is done. | Index-based writes need the complete list; prevents wrong-row edits on lazy lists. |
| P7-D8 | No `If-None-Match` in this plan; no ETag plumbing. | Single writer in practice; ETag handling needs a header-carrying GET and conflict UX that isn't justified yet. |
| P7-D9 | Playlist concerns consolidate into `playlists.rs`. | One clear unit for reads and writes; `library.rs` keeps favorites, `catalog.rs` keeps albums/artists/tracks. |
| P7-D10 | Only the user's own playlists are editable; pre-existing duplicates are never auto-deduped. | TIDAL enforces ownership (403); silently changing user data is out of scope. |

## 14. Pre-spec Research Notes (endpoints)

Reference implementation: community `tidalapi` master (`playlist.py`, `user.py`, `session.py`),
verified 2026-09-24. Community tokens use `api.tidal.com/v2` (not `openapi.tidal.com/v2`).

- **Create:** `PUT my-collection/playlists/folders/create-playlist` on the v2 base with query
  `name`, `description`, `folderId=root`; response is JSON:API `{data: {uuid, title,
  numberOfTracks, …, description, duration, type, image, squareImage}}`.
- **Rename:** `POST playlists/{uuid}` (v1) with form `title`, `description`.
- **Delete:** `DELETE playlists/{uuid}` (v1).
- **Add items:** `POST playlists/{uuid}/items` with form `trackIds` (comma-joined), `toIndex`,
  `onDupes` (`SKIP`/`ADD`), `onArtifactNotFound` (`SKIP`), and `limit`; response includes
  `addedItemIds`.
- **Remove items:** `DELETE playlists/{uuid}/items/{comma-joined indices}` (indices, not ids).
- **Move items:** `POST playlists/{uuid}/items/{comma-joined indices}` with `toIndex` — the
  roadmap's open question (dedicated endpoint vs delete+re-add) is resolved: a dedicated move
  endpoint exists with the same index addressing.
- **ETag:** every playlist `GET` returns an ETag; tidalapi sends `If-None-Match` on writes but
  refreshes it first, so it is a best-effort guard rather than conflict handling (deferred,
  P7-D8).
- **Scopes:** the PKCE flow already requests `r_usr w_usr w_sub`; playlist writes are
  authorized.

## 15. Out of Scope (tracked for later)

- **Track 2a:** drag-and-drop reorder, context menus/kebab, toast system, sort/filter.
- **Track 2b:** Library information architecture (playlist folders/tree).
- **Later tracks:** playlist covers, public/private toggle, favoriting playlists, multi-select
  editing, offline write queueing, and anything in roadmap Tracks 3–7.
- **Plan 7 prep commit:** correct the stale Plan 6 smoke counts (favorites 9 → 12, total 53 →
  56) and note the `lastUnfavoritedTrackId` → `pendingRemovalsToDrop` drift in the plan/spec.
