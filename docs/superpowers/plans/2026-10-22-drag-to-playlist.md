# Drag to Playlist Implementation Plan

> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** drag tracks, track selections, albums, playlists and mixes onto playlists (in
the sidebar tree, the Rack tree, or the Library → Playlists grid) and into the queue
rail. Track tables gain multi-select, and owned-playlist reorder moves onto the same
drag gesture.

**Architecture:**
- **Drag controller:** one pointer-driven controller (`lib/dragDrop.svelte.ts`), with
  `use:dragSource` / `use:dropZone` actions and a single `DragGhost` overlay.
- **Payloads:** collections resolve their tracks on drop (`lib/dragPayload.ts`).
- **Multi-select:** an identity-keyed `Selection` (`lib/selection.svelte.ts`) gives
  TrackTable multi-select.
- **Rust:** one new sync command, `queue_insert`, for slot drops on the queue.

**Spec:** `docs/superpowers/specs/2026-10-22-oxitide-drag-to-playlist-design.md`

**Status (2026-10-22):**
- Tasks 2–11 are implemented.
- Gates are green: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui run check`, and `npm --prefix ui test`.
- Still open:
  - **Task 1** (the multi-row move spike) needs the owner's account. Until it lands,
    multi-row reorder is refused.
  - **The owner smoke run:** `notes/2026-10-22-drag-to-playlist-smoke.md`.
- Deviations from the spec are listed in spec §11.
- `--selection-bg` lives once in the semantic layer of `tokens.css`, derived from
  `--accent`, so every mode (Rack included) follows its own accent without per-mode
  copies.

## Global constraints

- **Do not commit.** The owner commits.
- **No new dependencies.**
- **Styling:** semantic tokens only, no `data-mode` branching. Add any new token to
  every mode in `modes.css` (and the Rack in `rack.css`).
- **Width-driven layout stays CSS** (AR-9). The controller never measures layout for
  the queue glide.
- **IPC:** camelCase args from the UI, snake_case Rust params. Keep `api.ts` in sync by
  hand.
- **Command registration:** add `queue_insert` to `generate_handler![]` (`main.rs`), to
  `SYNC_COMMANDS` (commands.rs tests, next to `queue_add`, ~L2779), and to remote
  `NOT_EXPOSED` (`remote/rpc.rs` ~L496).
- **Idle at rest:** no window listeners, rAF loops or timers unless a press or drag is
  in progress.
- **Gates, each task:**
  - Rust tasks: `cargo test --workspace` and `cargo clippy --workspace -- -D warnings`.
  - UI tasks: `npm --prefix ui run check` and `npm --prefix ui test`.

## Review focus

Each item is pinned to a test in the task that owns it.

1. **No listeners or frames at rest.** The controller adds window listeners on
   `pointerdown` and removes all of them on drop, cancel, or press-without-drag
   (Task 5).
2. **A drag must not click.** The release after a drag never opens a card, plays a row,
   or navigates a tree row (Task 5; Task 8 card case).
3. **Selection order is display order**, and duplicate track ids in a playlist stay
   distinct rows (Task 2; Task 7).
4. **Refusals never call the API**: not-owned playlists, a self-drop, and cancelled
   drags (Task 9; Task 4 resolve-on-drop).
5. **`queue_insert` never restarts or skips the playing track**, including inserts
   before `current` (Task 3).
6. **Reorder keeps working** for single rows in owned playlists after the HTML5 path is
   removed (Task 10).

---

### Task 1: Spike — TIDAL multi-index move semantics (owner-assisted)

**Files:** create `docs/superpowers/plans/notes/2026-10-22-playlist-multi-move-spike.md`.

**Steps:**
- [ ] On a scratch playlist `A B C D E F` (owner's account), call
  `move_playlist_items(id, [1,3], to)` for `to ∈ {0, 2, 5, 6}`. Use a throwaway
  `#[ignore]`d test or a temporary dev-only button. Neither ships.
- [ ] Record the resulting orders. Decide:
  - (a) is `toIndex` counted in the original list or in the list with the moved rows
    removed?
  - (b) does the moved block keep its relative order?
  - (c) is it reliable with non-contiguous indices?
- [ ] Write `normalizeMultiDrop(from: number[], to: number): number` to match. If the
  semantics are unusable, record **"multi-row reorder off"** in the note. Task 10 then
  refuses multi-row reorder (spec §6.4).

### Task 2: Selection model

**Files:** create `ui/src/lib/selection.svelte.ts`, `ui/src/lib/selection.test.ts`.

**Interfaces:** `createSelection<T>(): Selection<T>` (spec §5.1), plus the pure
`rangeBetween<T>(items, a, b): T[]`.

**Steps:**
- [x] Tests (node):
  - plain click selects one;
  - Ctrl toggles;
  - Shift ranges from the anchor in both directions;
  - Ctrl+Shift adds a range;
  - `extend` moves the range end;
  - `selectAll`, `clear`;
  - `ordered` follows the `items` order and drops missing rows;
  - `indices` with two equal-id but distinct `Track` objects returns both indices;
  - the anchor resets when its item leaves `items`.
- [x] Implement with a `SvelteSet<T>` and an `anchor`. Clicks never read the DOM.

### Task 3: `queue_insert` (Rust)

**Files:**
- `crates/oxitide-audio/src/queue.rs`
- `crates/oxitide-audio/src/engine.rs`
- `crates/oxitide-app/src/commands.rs`
- `crates/oxitide-app/src/main.rs`
- `crates/oxitide-app/src/remote/rpc.rs`
- `ui/src/lib/api.ts`

**Interfaces:**
- `Queue::insert_at(&mut self, index: usize, items: Vec<QueueItem>)`;
- `EngineCmd::EnqueueAt { index: usize, items: Vec<QueueItem> }`;
- `#[tauri::command] pub fn queue_insert(state, index: usize, items: Vec<QueueItem>)`;
- `api.queueInsert(index, items)`.

**Steps:**
- [x] `queue.rs` tests:
  - insert before `current` shifts `current` by `items.len()`;
  - insert at `current` keeps the playing item;
  - insert after `current`;
  - an index past the end appends;
  - empty queue → items appended, `current` stays `None`;
  - empty `items` is a no-op;
  - the revision bumps.
- [x] Engine test mirroring `EnqueueNext`'s "must not restart the session" test (output
  `opens()` unchanged, `current_index` adjusted, still playing).
- [x] Check whether `insert_next`'s caller re-evaluates the prefetch when the next item
  changes. Give `EnqueueAt` the same treatment, and add a test if the next track
  changes (insert at `current + 1`).
- [x] Register the command:
  - `generate_handler!`;
  - `SYNC_COMMANDS` with the reason "queue order: arrival order";
  - `NOT_EXPOSED` in `remote/rpc.rs`.
- [x] Add `queueInsert` to `api.ts`.

### Task 4: Payloads + shared track loading

**Files:**
- create `ui/src/lib/collectionTracks.ts`, `ui/src/lib/dragPayload.ts`,
  `ui/src/lib/dragPayload.test.ts`;
- edit `ui/src/lib/cardActions.ts`.

**Interfaces:**
- move `loadTracks(target: CardTarget)` from `cardActions.ts` to
  `collectionTracks.ts`, and have `cardActions` import it;
- `DragPayload`, `DragSourceRef`, `resolveTracks`, `payloadLabel`, `payloadCover`,
  `payloadSourcePlaylist` (spec §6.2).

**Steps:**
- [x] Tests (node, `api` mocked):
  - labels: one track → its title; N → "N tracks"; a collection → its title, falling
    back to "Album" / "Playlist" / "Mix";
  - `resolveTracks` makes no call for `tracks` payloads, `getTrack` for `track-id`, and
    `loadTracks` for collections;
  - `payloadSourcePlaylist` covers the playlist-card case.
- [x] Implement. `cardActions` behavior is unchanged: `menus.test.ts` and the card tests
  stay green.

### Task 5: Drag controller + ghost

**Files:**
- create `ui/src/lib/dragDrop.svelte.ts`, `ui/src/lib/dragDrop.test.ts`
  (`// @vitest-environment jsdom`), `ui/src/lib/components/DragGhost.svelte`;
- edit `ui/src/App.svelte` (mount the ghost once, outside the shell branch so both
  shells get it), `ui/src/app.css` (`body.dragging`).

**Interfaces:**
- `dragDrop` singleton: `active`, `zoneKey`, `verdict`, `pending` (a `SvelteSet`),
  `cancel()`, `bindGhost(el)`;
- `dragSource(node, getPayload: () => DragPayload | null)` action;
- `dropZone(node, zone: DropZone)` action, where
  `DropZone = { key, accepts(p) → Verdict, hover?(p, pt), leave?(), drop(p, pt) → Promise<void> | void, scroller?: () => HTMLElement | null }`;
- `Verdict = { ok: true; hint: string } | { ok: false; reason: string }`;
- reuse `edgeScroll` and `DRAG_THRESHOLD` from `lib/queueDrag.ts` (already shared and
  tested).

**Steps:**
- [x] Tests:
  - a press that moves less than 5 px never activates and removes its listeners on
    `pointerup`;
  - after activation, `elementFromPoint` (stubbed) → zone → `accepts` is called once per
    zone change;
  - `drop` is called with the payload on release over an accepting zone, and not on a
    refusing zone;
  - Escape / `pointercancel` / window `blur` cancel;
  - the `click` after a real drag is swallowed exactly once;
  - a touch pointer is ignored;
  - a native `dragstart` on the source is prevented;
  - no window listeners at rest (spy, before and after);
  - an rAF-batched move updates the ghost `transform` without touching `$state`
    (spy on the ghost element style).
- [x] Implement:
  - `requestAnimationFrame` tick only while active;
  - zones in a module `WeakMap`;
  - `data-drop-zone` attribute;
  - `pending` holds zone keys while `drop()` awaits.
- [x] `DragGhost.svelte`:
  - `position: fixed; pointer-events: none; z-index` above popovers;
  - a thumb (or two stacked thumbs + count badge), the label, and the hint/reason line;
  - semantic tokens only;
  - no scale-in under reduced motion.
- [x] Cancel the drag on route change (`nav.route` effect in App) and on sign-out.

### Task 6: `playlistActions.addTracks`

**Files:** `ui/src/lib/playlists.svelte.ts`, `ui/src/lib/playlists.test.ts`,
`ui/src/App.svelte`.

**Interfaces:** `addTracks(playlist: Pick<Playlist, "uuid" | "title">, tracks: Track[]): Promise<void>`.
It shares the phase-1 body with `addTo`; extract `addPhaseOne(playlist, tracks)`.

**Steps:**
- [x] Tests:
  - all added → success toast with an **Open** action, `revision` bumps, the modal
    stays closed;
  - some skipped → `open === true`, `step === "duplicates"`, and `confirmDuplicates`
    works against that target;
  - API error → error toast, modal closed;
  - empty `tracks` → no call;
  - a concurrent call while `busy` is ignored.
- [x] Refactor `addTo` onto the shared helper. The existing picker tests stay green.
- [x] Move the `revision → collection.refresh()` effect from `Sidebar.svelte` to
  `App.svelte`, so the Rack shell's tree also refreshes (the Sidebar isn't mounted in
  the Rack).
- [x] The Playlist view watches a new `playlistActions.changed` (`{ uuid, n }` bumped per
  add) and reloads its pager when the open playlist received tracks.

### Task 7: TrackTable multi-select + selection menu

**Files:**
- `ui/src/lib/components/TrackTable.svelte`;
- `ui/src/lib/menus.ts`, `ui/src/lib/menus.test.ts`;
- create `ui/src/lib/components/TrackTable.selection.test.ts`;
- `ui/src/styles/tokens.css` / `modes.css` (`--selection-bg`);
- views that pass `playlistActions` to TrackTable (`Playlist.svelte`).

**Interfaces:**
- new prop `onAddManyToPlaylist?: (tracks: Track[]) => void`. Views pass
  `(ts) => playlistActions.openPicker(ts)`.
- `playlistActions` prop gains `onRemoveMany?(indices: number[])`.
- `selectionMenuItems({ count, playNext, addToQueue, addToPlaylist?, remove? })` in
  `menus.ts`.

**Steps:**
- [x] Tests (jsdom):
  - a row click selects;
  - Ctrl+click toggles;
  - Shift+click ranges;
  - clicks on the cover button or a name link don't change the selection;
  - Ctrl+A / Escape / Space / Shift+↓;
  - the selection clears when `view.levels` or the filter changes and survives an
    appended page;
  - right-click inside a 2+ selection shows `selectionMenuItems`, and right-click
    outside selects that row and shows the single menu;
  - double-click still plays.
- [x] Implement:
  - row `onclick` (ignore `button, a, input, [role='link']` targets);
  - `class:selected`, `aria-selected`, `aria-multiselectable`;
  - keyboard additions in `onRowKeydown` (keep the existing keys intact).
- [x] Selection-menu behavior:
  - queue actions filter blocked rows (`trackFlags.filterPlayable`) and toast the
    count;
  - **Remove N** calls `onRemoveMany(selection.indices(rows))`. `Playlist.svelte`
    implements it with `api.removePlaylistItems(id, indices)`, then updates its local
    list the way `removeAt` does.
- [x] Add `--selection-bg` to the semantic layer and each mode. The Rack sets its own in
  `rack.css`.

### Task 8: Drag sources

**Files:**
- `TrackTable.svelte`;
- `MediaCard.svelte`;
- `DetailHeader.svelte` callers (`views/Album.svelte`, `views/Playlist.svelte`,
  `views/Mix.svelte`) or `DetailHeader`'s cover slot;
- `PlayerBar.svelte`;
- tests next to each.

**Steps:**
- [x] **TrackTable:** `use:dragSource` on the scroll container. The getter finds the row
  under the press (`data-row-index`, already present):
  - a selected row → `{ kind: "tracks", tracks: selection.ordered(rows), source: { tableId, playlistId } }`;
  - otherwise that row only.

  `tableId` is a per-instance id. `playlistId` comes from a new optional `sourcePlaylist`
  prop that `Playlist.svelte` passes. Remove `draggable` / `data-drag-index` from rows.
- [x] **MediaCard:**
  - collection cards (`cardTarget(item)`) → `collection` payload;
  - track cards (`isPlayableTrack`) → `track-id`;
  - artists / page links / radios → `null`.

  Test: a drag past the threshold doesn't open the page (the click is swallowed), and a
  plain click still does.
- [x] **Detail covers:** the Album / Playlist / Mix header cover becomes a source for
  that collection.
- [x] **Player bar cover:** the playing item → `track-id` (`QueueItem.track_id`). The
  CatJam overlay stays `pointer-events: none` over it.
- [x] `QueueRail` is not a source (spec §11). Its own `noNativeDrag` stays.

### Task 9: Drop targets — playlist tree + Playlists grid

**Files:** `PlaylistTree.svelte`, `Sidebar.svelte`, `views/Playlists.svelte`, and tests
(`PlaylistTree.test.ts` jsdom; extend the Playlists view test if one exists, else create
one).

**Steps:**
- [x] **Playlist rows** (tree) and **playlist cards** (grid) get
  `use:dropZone={{ key: "pl:" + uuid, accepts, drop }}`:
  - `accepts` refuses `!entry.owned` ("You can't edit this playlist") and
    `payloadSourcePlaylist(p) === uuid` ("Already in this playlist");
  - otherwise `{ ok: true, hint: "Add to " + title }`;
  - `drop` → `resolveTracks` → `playlistActions.addTracks(entry, tracks)`.

  To get `playlistActions` into `PlaylistTree`, add a prop and pass it from Sidebar and
  RackShell; the grid already has it.
- [x] Styles:
  - `.drop-target` / `.drop-refused` classes from `dragDrop.zoneKey` + `verdict`;
  - a spinner on the row while `dragDrop.pending.has(key)`.
- [x] **Folder spring:** in the tree, a zone with `accepts → { ok: false, reason: "" }`
  plus `hover` starts a 600 ms timer → `collection.toggleFolder(id)` if it's collapsed;
  `leave` clears it. Fake-timer test.
- [x] **Rail drag-peek:** while `dragDrop.active` and the pointer sits over the collapsed
  rail for 400 ms, open the peek (`peekIntent`), whatever `hoverExpand` is set to.
  It closes when the drag ends. Test with fake timers.
- [x] Auto-scroll: Sidebar `.scroll` and the Rack tree scroller are passed as the zone
  `scroller`.
- [x] Tests:
  - not-owned refused, with no `addTracks` call;
  - self-drop refused;
  - an owned row calls `addTracks` with the resolved tracks;
  - a cancelled drag makes no API call.

### Task 10: Drop targets — queue rail + playlist reorder

**Files:** `QueueRail.svelte`, `TrackTable.svelte`, `lib/drag.ts` (+ `drag.test.ts`),
`lib/queueTransition.ts`, `views/Playlist.svelte`.

**Steps:**
- [x] **Queue rail:** a `dropZone` on the rail scroller:
  - `hover` computes the slot with the rail's `dragTarget` math and opens the existing
    row gap at it;
  - below the last row → "Add to end of queue";
  - `drop` → `resolveTracks` → `filterPlayable` → `api.queueInsert(slot, items)`, or
    `queueAdd` at the end, then a toast;
  - refuse while `glideActive()` (add this getter to `queueTransition.ts`).
- [x] **Reorder:** TrackTable registers its scroller as a zone that accepts only its own
  `tableId` while `canDrag`:
  - `hover` → `dropAt = dropTarget(...)`;
  - `drop` → `playlistActions.onDrop(fromIndices, to)`.

  Then:
  - single row: `normalizeDrop` as today;
  - multi-row: `normalizeMultiDrop` from Task 1, or refuse with "Can't move several rows
    yet" if the spike said off;
  - change `Playlist.svelte`'s `moveTrack` to take `number[]`.
- [x] Delete `trackDrag` and its `DragParams` from `drag.ts`, and drop the row
  `[draggable='true']` reliance. Check `titlebar.ts`'s exclusion list still makes sense
  (keep the selector; it's harmless).
- [x] Tests:
  - `normalizeMultiDrop` cases;
  - single-row reorder end to end in jsdom (stubbed `elementFromPoint`);
  - a queue drop between rows calls `queueInsert` with the right slot;
  - a drop after the last row calls `queueAdd`;
  - blocked tracks are filtered;
  - refused during a glide.

### Task 11: Design docs, AGENTS.md, smoke checklist

**Files:**
- `ui/design/DESIGN.md` (+ `atmosphere`, `instrument`, `rack` notes);
- `AGENTS.md` (one bullet under Architecture);
- create `docs/superpowers/plans/notes/2026-10-22-drag-to-playlist-smoke.md`.

**Steps:**
- [x] DESIGN.md: the ghost, drop-target, refused, pending and selection states;
  `--selection-bg` per mode.
- [x] AGENTS.md bullet:
  - drag & drop is pointer-driven through `lib/dragDrop.svelte.ts` (never HTML5 DnD;
    `dragDropEnabled` stays false);
  - zones attach to containers;
  - payloads resolve on drop;
  - TrackTable selection is identity-keyed.
- [x] Smoke checklist (owner, real account, Windows; Linux added to the batched
  bare-metal run):
  - [ ] row → owned sidebar playlist: added, toast, count updates
  - [ ] Ctrl/Shift selection of 5 rows → playlist; order kept
  - [ ] album card, playlist card, mix card, track card (Home) → playlist
  - [ ] album / playlist / mix header cover → playlist
  - [ ] player-bar cover → playlist
  - [ ] drop on a not-owned (favorited) playlist is refused with a reason
  - [ ] drop that hits duplicates → duplicates modal, both choices
  - [ ] collapsed folder springs open; nested playlist accepts
  - [ ] collapsed rail peeks open during drag (hover panels off and on)
  - [ ] Library → Playlists grid card accepts
  - [ ] queue rail: insert between rows while playing (no restart), append at end
  - [ ] owned playlist: single-row reorder; multi-row reorder (if spike passed)
  - [ ] Escape mid-drag cancels; releasing over empty space does nothing
  - [ ] a drag never opens the card/row it started on
  - [ ] Rack shell: tree drops work, ghost readable in the skin
  - [ ] light mode + instrument mode legibility
  - [ ] Remove N from playlist via the selection menu

## Order & checkpoints

1. Tasks 2–4 are independent and can run in parallel. Task 1 needs the owner and can
   run any time before Task 10.
2. Task 5, then Task 6.
3. Task 7, then Task 8, then Task 9: the first usable slice (sidebar drops).
   **Checkpoint:** an owner try-out before the queue and reorder work.
4. Task 10, then Task 11.
