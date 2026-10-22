# Save for Later + Collaborators Implementation Plan

> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- **Save for Later:** a TIDAL-synced list of tracks, albums, artists and playlists, with
  a menu entry on every surface that shows one and a page in My Collection.
- **Collaborators:** a dialog on owned playlists that copies an invitation link and
  lists the members.

**Architecture:**
- **OpenAPI writes:** `client.rs` gains JSON:API writes on the OpenAPI base (a `Body`
  enum on `mutate`).
- **Tidal modules:** `later.rs` lists, hydrates and writes Save for Later; `shares.rs`
  creates and deletes invites and lists members.
- **Commands:** five async commands, all `NOT_EXPOSED` to the phone.
- **UI:**
  - `lib/later.svelte.ts` mirrors `favorites`;
  - `views/SaveForLater.svelte` is the page;
  - `CollaboratorsDialog.svelte` is the dialog.

**Spec:** `docs/superpowers/specs/2026-10-22-oxitide-save-for-later-collaborators-design.md`

**Status (2026-10-23):**
- Tasks 2–10 are implemented; gates green (`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`).
- Still open: **Task 1** (needs a second account) and the owner smoke run, `notes/2026-10-23-save-for-later-collab-smoke.md`.
- Deviations from the spec are listed in spec §11.

## Global constraints

- **Do not commit.** The owner commits.
- **No new dependencies.** The idempotency key comes from `getrandom` (already a dep).
- **Styling:** semantic tokens only, no `data-mode` branching.
- **IPC:** camelCase args from the UI, snake_case Rust params. Keep `api.ts` in sync by
  hand.
- **Command registration:** each new command goes in `generate_handler![]` (`main.rs`)
  and in remote `NOT_EXPOSED`. None of them are sync.
- **Tests:** HTTP tests use wiremock only; no live calls.
- **Gates:**
  - `cargo test --workspace` and `cargo clippy --workspace -- -D warnings`;
  - `npm --prefix ui run check` and `npm --prefix ui test`.

## Tasks

- [ ] **Task 1: collaborator spike (owner + a second account).**
  - A second account joins an owned playlist through a copied link.
  - Record in `notes/` the shape of the internal v2 `playlist.collaborators` and of
    OpenAPI `collaborators?include=collaborators`.
  - Check whether v1 `POST playlists/{uuid}/items` works for the collaborator.
  - This settles CO-D5's parsing and the deferred §7 work.
- [x] **Task 2: client JSON:API writes.**
  - `mutate` takes `Body::{Empty, Form, JsonApi}`.
  - `post_openapi<T>` and `delete_openapi(path, params, doc)` send
    `application/vnd.api+json` and the same `Idempotency-Key` on every resend.
  - Tests: content type, key present and stable across a 401 refresh, the DELETE body.
- [x] **Task 3: `later.rs`.**
  - Types: `LaterKind`, `LaterRef`, `LaterPage`, `LaterIds`, `LaterEntry`/`LaterItem`.
  - `later_page` (cursor from `links.meta.nextCursor`, else `links.next`), `later_ids`
    (50-page cap, repeated-cursor stop), `later_add` (chunks of 20, `meta.skipped`),
    `later_remove`, and `later_entries` (hydrate at `PAGE_CONCURRENCY`, keep order, drop
    permanent failures, propagate transient ones).
  - Wiremock tests for each.
- [x] **Task 4: `shares.rs`.**
  - `invite_url`, `create_playlist_invite` (`externalLinks` take precedence),
    `delete_share`, and `playlist_members` (internal v2 first, OpenAPI fallback, lenient).
  - Wiremock tests.
- [x] **Task 5: commands.**
  - The five commands: `get_save_for_later_ids`, `get_save_for_later_page`,
    `set_save_for_later`, `playlist_invite`, `playlist_members`.
  - The `AppState` invite cache: reuse while more than 1 h is left, `renew` deletes
    first, cleared on sign-out.
  - Register them in `main.rs` and remote `NOT_EXPOSED`.
  - Unit test for the cache-reuse rule.
- [x] **Task 6: UI API and store.**
  - `api.ts` types and calls; `lib/later.svelte.ts` (hydrate, has, toggle with
    optimistic revert and a toast, reset).
  - Created and reset next to `favorites`.
  - Node tests.
- [x] **Task 7: menus.**
  - `later` in `TrackMenuContext`, `nowPlayingMenuItems` and `collectionCardMenuItems`
    (album and playlist targets only).
  - The artist page/cards and the album/playlist More menus.
  - Icons `bookmark` and `bookmark-minus`.
  - Menu tests: the label switches. (Mixes were later found saveable, so they get the
    entry too; spec §11.)
- [x] **Task 8: the Save for Later page.**
  - Route `{ name: "later"; kind? }`, restore parsing, title, and the sidebar and Rack
    entries.
  - `views/SaveForLater.svelte`: cursor walk, chips, empty state, video footnote, and
    removals reflected at once.
  - jsdom test.
- [x] **Task 9: the Collaborators dialog.**
  - `CollaboratorsDialog.svelte`: copy link, members, Reset link with a confirm.
  - Opened from the playlist page's More menu and from `collectionPlaylistMenuItems`
    (owned only).
  - jsdom test.
- [x] **Task 10: docs.**
  - `ui/design/DESIGN.md` notes, the AGENTS.md architecture line, and the smoke
    checklist in `notes/`.
