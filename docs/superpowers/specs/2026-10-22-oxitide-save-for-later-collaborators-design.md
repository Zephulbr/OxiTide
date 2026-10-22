# OxiTide — Save for Later + playlist Collaborators

- **Status:** Approved by owner (2026-10-22)
- **Date:** 2026-10-22
- **Owner:** V
- **References:** TIDAL API reference (`tidal-music.github.io/tidal-api-reference`, spec
  1.10.197: `userCollectionSaveForLaters`, `shares`, `savedShares`,
  `playlists/{id}/relationships/collaborators`) · the official TIDAL app's "Save for Later"
  page and context-menu entry, and its playlist **Collaborators** dialog (owner screenshots,
  2026-10-22) · `2026-10-22-oxitide-drag-to-playlist-design.md` (menus, playlist ownership)
- **Process note:** the API was checked live on the owner's account on 2026-10-22, and
  every change was undone (§3). The invite-link URL format came from a link the owner
  copied from the official app.

---

## 1. Purpose & Outcome

TIDAL added two features that OxiTide doesn't have yet:

1. **Save for Later.** A list kept on TIDAL's side, separate from Favorites, for things
   to come back to. It holds tracks, albums, artists, playlists, and videos. It is synced,
   so items saved in OxiTide show up in the official apps and the other way round.
2. **Collaborators.** The owner of a playlist copies an **invitation link**. Anyone who
   opens it becomes a collaborator who can add tracks. The dialog lists the people on the
   playlist.

**Success:**
- Track rows, the player bar, album/playlist cards, the album, playlist and artist pages,
  and artist cards offer **Save for later** or **Remove from Save for later**. The label
  matches the item's current state.
- A **Save for Later** page in My Collection lists saved items newest first, with
  All / Albums / Tracks / Artists / Playlists / Mixes & Radio chips, like the official
  app.
- On an owned playlist, **Collaborators…** opens a dialog. Its **Copy invitation link**
  copies a working `tidal.com` invite link, and the dialog lists the owner and any
  collaborators.
- Nothing here runs at rest. The saved-id list loads once per session, the first time it
  is needed.

## 2. Scope

**In scope:**
- `crates/oxitide-tidal/src/later.rs`: list, add, and remove Save for Later items.
- `crates/oxitide-tidal/src/shares.rs`: create and delete a share; list a playlist's
  collaborators.
- JSON:API write helpers on the OpenAPI base in `client.rs`.
- Commands, DTOs, `api.ts` mirrors, and remote-RPC classification.
- UI:
  - `lib/later.svelte.ts` store;
  - menu entries;
  - the `later` route and view, plus a sidebar entry;
  - `CollaboratorsDialog.svelte`.
- Design-doc notes, tests, and a smoke checklist.

**Out of scope (follow-ups, §11):**
- editing a playlist as a collaborator (§7);
- accepting an invite inside OxiTide (`savedShares`);
- removing a collaborator, which TIDAL's public API can't do yet (§3);
- videos (OxiTide has no video playback); a saved video is hidden, as noted in §5.3;
- Save for Later on the phone remote;
- drag-to-Save-for-Later.

## 3. API findings (live check, 2026-10-22)

The checks used the stored PKCE login (`r_usr w_usr w_sub`, the scopes OxiTide already
requests) against `https://openapi.tidal.com/v2/`. They were the first writes OxiTide
made on that host, and they were accepted. No tokens or codes were logged.

| Call | Result |
|---|---|
| `GET userCollectionSaveForLaters/me` | 200: `{id, attributes: {numberOfItems, lastModifiedAt}}`. `me` works as the id. |
| `GET userCollectionSaveForLaters/me/relationships/items?include=items` | 200: `data: [{type, id, meta: {addedAt}}]`, newest first. `included` items carry JSON:API attributes (`title`, `duration`, `explicit`, …) but **no artists, album or cover** unless further includes are requested. |
| `POST …/me/relationships/items` with `{"data":[{"type":"tracks","id":"…"}]}` (≤ 20 entries) | 200: echoes the added items, and `meta.skipped` lists items already present (`ALREADY_PRESENT`) or unknown (`NOT_FOUND`). A 409 means `TOO_MANY_ITEMS_IN_COLLECTION`. |
| `DELETE …/me/relationships/items` with the same body | 200. The item was gone on the next read. |
| `POST shares` with `{"data":{"type":"shares","attributes":{"role":"COLLABORATOR"},"relationships":{"sharedResources":{"data":[{"type":"playlists","id":"<uuid>"}]}}}}` | 201: `attributes: {code (36-char UUID), role, createdAt, expiresAt = created + 7 days}`. **`externalLinks` is empty.** It worked on a playlist that had never been made public. |
| `GET shares?filter[code]=…` / `GET collaborationInvites?filter[code]=…` | 200: the same share through both resources (`collaborationInvites` is the deprecated view of it). |
| `DELETE shares/{id}?revokeGrants=true` | **400**: "Revoking grants issued through a share is not yet supported". |
| `DELETE shares/{id}` | 200. Afterwards `GET shares/{id}` returns 404. People who already joined stay collaborators (spec text). |
| `GET playlists/{id}/relationships/collaborators?include=collaborators` | 200: `data` lists `users` resources. The `users` attributes are `username`, plus private fields that only apply to `me`. **They have no picture.** |
| `GET api.tidal.com/v2/user-playlists/{uuid}` (internal) | 200: `{followInfo, playlist: {…, collaborators: [], creator: {id, name, type}, curators, sharingLevel: "PUBLIC"}, profile}`. |

**Invitation link format** (from the official app):
`https://tidal.com/playlist/{playlistUuid}/invite?code={shareCode}`.

**Not verified yet:**
- the shape of a *non-empty* `collaborators` array on the internal v2 playlist;
- whether a collaborator can write to the playlist through OxiTide's v1 calls.

Both need a second account to join (plan Task 1, §7).

## 4. Decisions

| # | Decision | Rationale |
|---|---|---|
| SL-D1 | **Save for Later goes through the public OpenAPI** (`userCollectionSaveForLaters/me`), via new `post_openapi_json` / `delete_openapi_json` helpers. They share `mutate`'s single-flight refresh on 401 and its 429 handling, send `Content-Type: application/vnd.api+json`, and add an `Idempotency-Key` (a random UUID per logical write, the same across its retries). | It's the only documented endpoint for this list. The internal v2 API exposes nothing like it. |
| SL-D2 | **Items are hydrated into the existing v1 models.** The list call returns `(type, id, addedAt)`, and each page is filled in through the existing single fetches (`track`, `album`, `artist`, `playlist`) at `PAGE_CONCURRENCY`. Items that fail to load or are unavailable are dropped from the page, not shown as errors. | The rows and cards need artist refs, covers, album ids, and name links, and the v1 models already carry all of them. Mapping JSON:API includes would duplicate model code for a list that is usually short. If long lists become common, the follow-up is `include=items.artists,items.albums.coverArt`. |
| SL-D3 | **A session store, `later.svelte.ts`, holds the saved ids** (one `SvelteSet` per kind) and mirrors `favorites.svelte.ts`: hydrated once on first use (the first context menu that needs a label, or a visit to the page), optimistic toggles that revert with an error toast, and a reset on sign-out. Hydration walks the id-only relationship, capped at 50 pages. | Menus must show Save or Remove without a request per open. It's the same pattern as Favorites, so it behaves the same. |
| SL-D4 | **The menu label switches between "Save for later" and "Remove from Save for later"** (icons `bookmark` / `bookmark-minus`, added as Lucide paths). The entry sits in the library group next to Favorite. Mix cards get it too (§11). | Matches the official app's wording. |
| SL-D5 | **A `{ name: "later"; kind?: LaterKind }` route** holds the chips in the route, restored like `history`'s tab. The sidebar's collection group adds **Save for Later** under Tracks. The view keeps TIDAL's newest-first order and renders: **Tracks** chip → a `TrackTable`; **Albums / Artists / Playlists** chips → card grids; **All** → a single grid of cards, with a type badge only on track cards (as in the screenshot). | Matches the official page, and reuses OxiTide's existing table and card components. |
| SL-D6 | **The page loads through a `later` source in `viewCache` with `freshMs: 0`.** Toggles patch the cached list in place. Adds made in another app appear on the next visit. | The list is edited by the user, so the cache rule for user-edited data applies. |
| SL-D7 | **A save that TIDAL skips (already present) counts as success.** A 409 `TOO_MANY_ITEMS_IN_COLLECTION` shows "Save for Later is full". | Either way the item ends up saved, or the user gets a clear reason it wasn't. |
| CO-D1 | **Collaborators… is offered only on owned playlists** (`Playlist.owned`): in the playlist page's More menu and in the collection playlist menu (sidebar tree, Rack tree, Playlists page). | Only the owner can create a COLLABORATOR share. |
| CO-D2 | **The invitation link is built by OxiTide**: `https://tidal.com/playlist/{uuid}/invite?code={code}`. If TIDAL ever fills `externalLinks` (`TIDAL_SHARING`), that link takes precedence. | The share response has no link today (§3), and this format comes from the official app. |
| CO-D3 | **One share per playlist per session.** OxiTide remembers `uuid → {id, code, expiresAt}` in memory and reuses it until less than one hour of its 7 days is left, then creates a new one. Nothing is persisted. | Links are reusable, so a new share on every click would leave a trail of live invites. An expired link can't be renewed, so near the end a fresh one is better. |
| CO-D4 | **No "Remove collaborator" button and no "revoke" promise.** The dialog says that links expire after 7 days and that people who joined keep access. **Reset link** deletes the remembered share and makes a new one, so the old link stops working. | TIDAL can't remove grants yet (§3, 400), and the UI must not claim it can. |
| CO-D5 | **Members come from the internal v2 playlist** (`user-playlists/{uuid}`): the owner from `creator` + `profile`, then `collaborators`, parsed leniently (`id`, `name`, an optional picture). If that fails, the dialog falls back to the OpenAPI `collaborators` relationship (`username`, initials avatar). | One call that may include avatars, with a documented fallback. The non-empty shape is checked in plan Task 1. |
| CO-D6 | **The new commands are `async`** (network only, no ordering needs), and all of them are `NOT_EXPOSED` to the phone remote. | AGENTS.md: blocking work that doesn't depend on order is async. The phone has no UI for these. |

## 5. Save for Later

### 5.1 Rust (`oxitide-tidal/src/later.rs`)
```rust
pub enum LaterKind { Track, Album, Artist, Playlist, Video }  // serde "tracks"… ↔ JSON:API type
pub struct LaterRef { pub kind: LaterKind, pub id: String, pub added_at: Option<String> }
pub struct LaterPage { pub items: Vec<LaterRef>, pub cursor: Option<String> }

impl TidalClient {
    pub async fn later_page(&self, cursor: Option<&str>) -> Result<LaterPage>;
    pub async fn later_ids(&self) -> Result<Vec<LaterRef>>;          // all pages, cap 50
    pub async fn later_add(&self, items: &[(LaterKind, String)]) -> Result<LaterWrite>;   // chunks of 20
    pub async fn later_remove(&self, items: &[(LaterKind, String)]) -> Result<()>;
}
pub struct LaterWrite { pub skipped: Vec<(LaterKind, String, String /* reason */)> }
```
- The cursor comes from `links.meta.nextCursor` (JSON:API convention). If the response
  has a `links.next` URL instead, the cursor is read from its `page[cursor]` query value.
  This is unverified: the live list had one page, and its response had only
  `links.self`. A repeated cursor stops the walk, as in `collection_items`.
- `countryCode` is passed explicitly, as on every `get_openapi` call.

### 5.2 Commands (`commands.rs`, async, `NOT_EXPOSED`)
- `get_save_for_later_ids() -> LaterIdsDto { tracks: u64[], albums: u64[], artists: u64[], playlists: String[] }`.
  Videos are dropped here.
- `get_save_for_later_page(cursor?: String, kind?: LaterKind) -> LaterPageDto { items: LaterItemDto[], cursor? }`,
  hydrated per SL-D2. `LaterItemDto` is tagged by kind and carries the existing
  `Track` / `Album` / `Artist` / `Playlist` DTO plus `added_at`. `kind` filters after
  hydration, client-side, because the API has no type filter. The view keeps paging
  until it has a screenful or the cursor runs out.
- `set_save_for_later(kind, id, saved: bool) -> ()`.

### 5.3 UI
- **`lib/later.svelte.ts`**: `hydrate()`, `has(kind, id)`, `toggle(kind, id)`, `reset()`,
  and a `revision` that the page uses to drop removed items. It is created next to
  `favorites` and reset with it on sign-out.
- **Menus (`lib/menus.ts`).** A `later?: { active: boolean; toggle(): void }` field is
  added to:
  - `TrackMenuContext` and `nowPlayingMenuItems`, after Favorite;
  - `collectionCardMenuItems`, for album, playlist and mix targets;
  - the artist page header menu and artist cards;
  - the album and playlist page More menus.

  A single-track selection gets it. The 2+ selection menu doesn't in v1 (follow-up).
- **The `later` view (`views/SaveForLater.svelte`).** The title is "Save for Later",
  with chips All / Albums / Tracks / Artists / Playlists / Mixes & Radio. A chip is hidden when the
  session's id set has none of that kind. Each chip is a filter of one cursor walk, and
  the walk is shared, so switching chips doesn't refetch.
  - **Empty state:** "Nothing saved yet. Use **Save for later** in any track, album,
    artist, playlist or mix menu."
  - **Saved videos:** they don't count toward any chip. If any exist, the page shows a
    footnote: "N videos saved in TIDAL aren't shown here."
- **`lib/titles.ts`** gets a title for the route, `nav.svelte.ts` the route plus restore
  parsing, and `Sidebar.svelte` plus the Rack tree's collection list get the entry.
- **Toasts:** "Saved for later" with an **Open** action that goes to the page, and
  "Removed from Save for later".

## 6. Collaborators (owner side)

### 6.1 Rust (`oxitide-tidal/src/shares.rs`)
```rust
pub struct PlaylistInvite { pub share_id: String, pub code: String, pub expires_at: String, pub url: String }
pub struct PlaylistMember { pub id: u64, pub name: String, pub picture: Option<String>, pub owner: bool }

impl TidalClient {
    pub async fn create_playlist_invite(&self, uuid: &str) -> Result<PlaylistInvite>;  // POST shares, COLLABORATOR
    pub async fn delete_share(&self, share_id: &str) -> Result<()>;                   // DELETE shares/{id}
    pub async fn playlist_members(&self, uuid: &str) -> Result<Vec<PlaylistMember>>;   // CO-D5
}
pub fn invite_url(uuid: &str, code: &str) -> String;  // CO-D2, unit-tested
```

### 6.2 Commands (async, `NOT_EXPOSED`)
- `playlist_invite(uuid, renew: bool) -> PlaylistInviteDto`. The CO-D3 cache lives on
  `AppState` (a `Mutex<HashMap>`), is cleared on sign-out, and `renew` deletes the cached
  share before creating a new one.
- `playlist_members(uuid) -> PlaylistMemberDto[]`.

### 6.3 Dialog (`lib/components/CollaboratorsDialog.svelte`)
- The dialog uses the `Modal` + `focusTrap` pattern, titled **Collaborators**.
- **Row 1:** a `copy` icon button, **Copy invitation link**, with the sub-line "Invite
  others by sharing the link". It calls `playlist_invite`, copies the link, and toasts
  "Invitation link copied". A spinner shows on the row while the call runs.
- **Member rows:** an avatar (picture or initials) and the name. The owner is marked
  "Owner".
- **Footer note:** "Links work for 7 days. People who join keep access." Next to it, a
  **Reset link** text button. It appears only once a link exists this session, and it
  confirms first ("The current link will stop working").
- The dialog is opened from the playlist page's More menu and from
  `collectionPlaylistMenuItems` when `playlist.owned`.

## 7. Collaborator side (deferred, needs a second account)

- Today `Playlist.owned` is `creator.id == me`, so a playlist you joined is read-only in
  OxiTide, the same as before this feature. That's safe: nothing breaks.
- Making joined playlists editable needs two answers from plan Task 1:
  1. **The `collaborators` shape:** what a non-empty internal v2 `collaborators` array
     looks like.
  2. **v1 writes as a collaborator:** whether `POST playlists/{uuid}/items` and the
     remove/move calls accept a collaborator. If they don't, adds go through
     `POST openapi playlists/{id}/relationships/items`.
- With those answers, a follow-up spec adds an `editable` flag, set to owner or
  collaborator. The drag targets, the picker, and remove/reorder switch from `owned` to
  `editable`. Delete and rename stay owner-only.
- Accepting an invite inside OxiTide (paste a link → `GET shares?filter[code]` →
  `POST savedShares`) is part of that follow-up.

## 8. Visual design
- The dialog follows the existing `Modal` styling (the semantic tokens `--surface-*`,
  `--text`, `--text-muted`). The copy-link row has a circular `--surface-3` icon well,
  like the screenshot, and member avatars are 40 px circles.
- The Save for Later page reuses the `MediaCard` grid and `TrackTable`. The track-card
  type badge reuses the existing card badge style if there is one; otherwise it gets a
  small `--surface-3` pill.
- Rack: both surfaces render through `RoutedPage` and `Modal`, so they get the skin
  through `pane` and the existing modal rules.
- Record both surfaces in `ui/design/DESIGN.md`, with per-mode notes where needed.

## 9. Testing
- **Rust (wiremock, never live):**
  - `later_page`: parses refs and `addedAt`, and follows and stops on the cursor;
  - `later_add`: sends a JSON:API body in chunks of 20 with `Content-Type:
    application/vnd.api+json` and an `Idempotency-Key`, and parses `meta.skipped`;
  - the 409 is mapped;
  - `later_remove`: the DELETE carries a body;
  - `create_playlist_invite`: builds the share body and the URL, and an
    `externalLinks` link takes precedence;
  - `playlist_members`: the v2 shape, the fallback to OpenAPI, and lenient parsing of
    collaborators;
  - commands: the `NOT_EXPOSED` classification and the async-command checks.
- **Node:** `invite_url` parity (if mirrored), the `later` store (optimistic toggle and
  revert, hydration not overriding toggles made during it), and the menu builders (label
  switch).
- **jsdom:** `SaveForLater.svelte` (chips hidden when a kind is empty, the empty state,
  the video footnote) and `CollaboratorsDialog` (copy calls the command once per session,
  Reset link confirms first).
- **Smoke (owner, real account):**
  `docs/superpowers/plans/notes/2026-10-xx-save-for-later-collab-smoke.md`. Check:
  - save from each surface, then confirm it appears in the official app, and the other
    way round;
  - copy an invitation link, then confirm it opens in the official app;
  - with a second account: join, then confirm the member appears in the dialog, and run
    the §7 write checks;
  - the Rack shell.

## 10. Risks
- **OpenAPI write limits:** this is OxiTide's first write traffic on that host. 429s go
  through `mutate`'s existing Retry-After handling.
- **Hydration cost** on a long list: twenty single fetches per page. SL-D2 names the
  include-based follow-up.
- **The member list shape:** only an empty `collaborators` has been seen (§3). CO-D5's
  fallback keeps the dialog working if the guess is wrong.
- **The API is new:** `collaborationInvites` was already deprecated in favour of
  `shares`. Keep the parsing lenient and the calls isolated in the two modules.

## 11. Implementation notes (2026-10-23)

Where the build differs from the text above:
- **Ids hydrate at sign-in**, next to Favorites (one id walk), not on the first menu.
  Otherwise the first menu opened would always say "Save for later".
- **`later` is a module singleton** (`lib/later.svelte.ts`, like `trackFlags`). App
  `connect`s it to toasts and navigation and resets it on sign-out, so no new props run
  through the shells. **The Collaborators dialog** is likewise one host in App
  (`CollaboratorsDialog.svelte`, opened through `lib/collaborators.svelte.ts`) in both
  shells.
- **Album and artist pages** carry a "Save" `DetailAction` next to Favorite. The
  playlist page has the entry, and **Collaborators…** (owned only), in its More menu.
- **Artist cards outside the page** still have no context menu. The artist header is
  their path. On the Save for Later page, track and artist cards get a small page menu
  (Play / Add to playlist / Go to album and artist / Remove for tracks; Open / Remove
  for artists). Collection cards keep their card menu, which now includes the entry.
- **The page doesn't go through `viewCache`.** Each visit walks the list fresh and draws
  as pages arrive (SL-D6's `freshMs: 0` in effect), and removals made anywhere drop off
  through the store's `revision`.
- **`get_save_for_later_page` returns each page's video count** for the footnote.
- **Reset link confirms in place** in the dialog footer, because a second `Modal` would
  close the first: Modal closes on any press outside its panel.
- **The `profile` object** (live, 2026-10-22) is `{userId, name, color: []}`, with no
  picture. Avatars use initials unless a picture field ever appears.
- **The Rack's Library tray** gains a "Save for Later" key.
- **Mixes & Radio can be saved** (owner report and live check, 2026-10-23). The original
  scope excluded mixes because the OpenAPI spec (1.10.199) lists only tracks, albums,
  artists, playlists and videos as write types. The official app saves a mix as a
  **`playlists` reference carrying the mix id**: 30 hex characters, no dashes. The
  OpenAPI playlist for that id reads back `playlistType: "MIX"`, and re-posting it as
  `playlists` returned `ALREADY_PRESENT`. So:
  - `LaterKind::Mix` serializes as `"mix"` and writes as `playlists`;
  - a `playlists` reference whose id isn't a dashed UUID is read back as a mix;
  - it is hydrated from the `pages/mix` header through `mix_info` (title, subtitle,
    artwork).

  In the UI:
  - mix cards get the menu entry;
  - the mix page gets a Save action;
  - the page gains a **Mixes & Radio** chip after Playlists.

## 12. Follow-ups
- Collaborator-side editing and accepting invites (§7).
- Removing a collaborator, once TIDAL supports revoking grants.
- Save for Later for a multi-row selection and as a drag target.
- Include-based hydration (SL-D2) if lists grow long.
- Save for Later on the phone remote.
