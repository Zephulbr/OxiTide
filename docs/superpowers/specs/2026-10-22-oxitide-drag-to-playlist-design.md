# OxiTide — Drag to playlist (drag & drop + track multi-select)

- **Status:** Approved by owner (2026-10-22)
- **Date:** 2026-10-22
- **Owner:** V
- **References:** official TIDAL desktop behavior (drag tracks/collections onto sidebar
  playlists, drag within a playlist or the play queue to reorder) ·
  `2026-09-25-oxitide-plan8-ux-foundations-design.md` (context menus, picker) ·
  Tauri 2 `dragDropEnabled` (already `false` on the main window, `tauri.conf.json`)
- **Process note:** scope picked by the owner (2026-10-22): multi-select is in, and the
  drop targets are the playlist tree (sidebar + Rack), the queue rail, and the
  Library → Playlists grid. The "+ New playlist" drop target was not picked.

---

## 1. Purpose & Outcome

Adding music to a playlist takes four clicks: right-click, **Add to playlist**, find the
playlist in a modal, click it. The official TIDAL app also lets you **drag** a track, a
selection of tracks, an album, or a playlist onto a playlist in the sidebar. OxiTide
doesn't have that yet. This feature adds it, plus the two things that make it useful:

1. **Multi-select in track tables** (Ctrl/Shift-click, Shift+arrows, Ctrl+A), with the
   context menu acting on the whole selection.
2. **One app-wide drag gesture.** The same drag can:
   - add to a playlist (sidebar tree, Rack tree, Playlists grid);
   - insert into the queue at a chosen slot (queue rail);
   - reorder inside an owned playlist's table, which replaces today's HTML5 reorder.

**Success:**
- Dragging a track row, a selection, an album/playlist/mix card, a detail-page cover, or
  the player-bar cover onto an owned playlist adds those tracks. The existing
  duplicates flow runs when needed, a toast confirms the add, and the sidebar count
  updates.
- A drop on the queue rail inserts at the marked slot.
- A drop on a playlist you don't own, or on the playlist the tracks came from, is
  refused, and the target shows it before you let go.
- Escape or releasing anywhere else cancels with no side effects.
- No work runs while nothing is being dragged.

## 2. Scope

**In scope:**
- `lib/selection.svelte.ts`: identity-keyed row selection (pure logic + runes).
- TrackTable multi-select: mouse, keyboard, `aria-selected`, styling, and a selection
  context menu (Play next / Add to queue / Add to playlist… / Remove from playlist).
- `lib/dragDrop.svelte.ts`: one pointer-driven drag controller, with `use:dragSource`
  and `use:dropZone` actions and a `DragGhost.svelte` overlay mounted once per shell.
- `lib/dragPayload.ts`: the payload union, labels, and lazy track resolution (cards are
  only fetched on drop).
- **Drag sources:**
  - TrackTable rows (every table);
  - `MediaCard` (album / playlist / mix / track cards);
  - the Album / Playlist / Mix detail-header covers;
  - the player-bar cover (the playing track).
- **Drop targets:**
  - `PlaylistTree` playlist rows, in the sidebar (full and rail) and in the Rack. Folders
    spring open on hover, and the collapsed rail peeks open on hover.
  - Library → Playlists grid playlist cards.
  - Queue rail: insert at a slot.
  - The source table: reorder in an owned playlist.
- `playlistActions.addTracks(playlist, tracks)`: the picker's add-and-duplicates logic,
  callable without opening the modal first.
- A Rust `queue_insert { index, items }` command (`Queue::insert_at`,
  `EngineCmd::EnqueueAt`) for slot drops on the queue.
- Migrating TrackTable's playlist reorder from HTML5 DnD (`trackDrag`) to the new
  controller, including multi-row moves (gated by a TIDAL API spike, §6.4).
- Design-doc updates (`ui/design/*/DESIGN.md`), tests, and a smoke checklist.

**Out of scope (follow-ups, §11):**
- dragging the queue rail's own rows out to a playlist (the rail keeps its pointer
  reorder);
- dragging playlists into folders (collection organisation);
- the "+ New playlist" drop target;
- undoing an add;
- dragging out to other apps or the OS (text/URL export);
- dropping files from the OS (`dragDropEnabled` stays `false`);
- touch drag, the mini player (no targets), and the phone remote.

## 3. Decisions

| # | Decision | Rationale |
|---|---|---|
| DD-D1 | **Pointer-driven drag, not HTML5 DnD.** One controller tracks `pointerdown` → threshold → window `pointermove`/`pointerup`, draws its own ghost, and hit-tests targets with `document.elementFromPoint`. | HTML5 DnD swallows pointer events and `:hover` while dragging. That breaks the rail's hover-peek, spring-loaded folders, and edge auto-scroll. `setDragImage` support in WebView2/WebKitGTK is unverified, and the queue rail already uses pointer drag for these same reasons (native image drags cancel pointers). A single model also lets one gesture reorder *or* add. |
| DD-D2 | **One gesture, many targets.** A drag from an owned playlist's table can reorder in place or land on another playlist or the queue. The target under the pointer decides. | Matches TIDAL, and removes the "is this a move or an add?" mode. |
| DD-D3 | **Collections resolve on drop.** Card/cover payloads carry a `CardTarget` (or track id). Tracks are fetched (`loadTracks`, now shared from `cardActions.ts`) only when dropped on an accepting target. | A drag that is cancelled or refused costs no request. |
| DD-D4 | **Selection is keyed by row identity (`Track` object), not index or track id.** Order comes from the table's current `view.items`. | Playlists repeat track ids. Indices shift on sort/filter, but row objects stay stable across pager appends and re-sorts. |
| DD-D5 | **Dragging a selected row carries the selection, in display order. Dragging an unselected row carries that row only** and leaves the selection alone. | Explorer/Finder/TIDAL convention. |
| DD-D6 | **Only owned playlists accept drops** (`Playlist.owned`). Other targets are refused and show a not-allowed state and a reason tooltip. Dropping onto the playlist the payload came from is also refused. | You can't add to someone else's playlist (the API 403s). A self-drop on the tree is almost always a mis-aimed reorder. |
| DD-D7 | **Adding reuses the picker's semantics**: phase 1 adds with `allowDuplicates=false`. If anything was skipped, the existing modal opens at the duplicates step. Content-filtered tracks are not filtered out (same as the picker). | One add path and one duplicate UX. The backend already chunks by 100 and handles ETags. |
| DD-D8 | **Queue drops filter blocked tracks** (`trackFlags.filterPlayable`), like card "Add to queue". A drop between rows inserts there via the new `queue_insert`. A drop below the last row appends. | The queue only holds playable tracks. Today there is no insert-at-index command. |
| DD-D9 | **Spring-loading:** hovering a collapsed folder for 600 ms expands it. Hovering the collapsed sidebar rail for 400 ms peeks it open, even with `hover_panels` off, and it closes when the drag ends. Sidebar, tree and queue scrollers auto-scroll near their edges (the queue's `edgeScroll`). | Without these, nested and off-screen playlists can't be reached mid-drag. |
| DD-D10 | **Paused is idle (Track 13).** The controller attaches window listeners only between `pointerdown` and the end of the drag. Hit-testing and ghost moves run at most once per animation frame. The ghost position is written straight to the DOM, not to `$state`. | No per-move reactive churn and no listeners at rest. |
| DD-D11 | **The ghost** is a compact pill: cover thumb (or stacked thumbs for more than one track), a label ("Song title", "12 tracks", album/playlist name), and a target hint ("Add to Road Trip", "Insert in queue", "Move", "Can't add here"). It uses semantic tokens only, with no `data-mode` branching. Rack gets its skin through `rack.css` like any shared component. | Visible intent before release. Follows AGENTS.md's styling rules. |
| DD-D12 | **Keyboard parity is via the selection, not keyboard drag.** Everything a drag does can also be done from the keyboard: select with Shift+arrows / Ctrl+A, then use the context-menu key → Add to playlist / Add to queue; reorder stays Alt+↑/↓ in the queue. Escape cancels a drag. | Drag is a pointer shortcut. The menu path stays the accessible one. |
| DD-D13 | **Remove the HTML5 `trackDrag` action** once TrackTable reorder runs on the controller. The pure `dropTarget`/`normalizeDrop` math in `drag.ts` stays and gets a multi-row variant. | Two drag systems on one row would fight (`dragstart` cancels pointers). |
| DD-D14 | **`queue_insert` is sync (arrival order) and `NOT_EXPOSED` to the phone remote.** | Queue commands are sync per AGENTS.md. The phone has no drag UI, and its RPC surface stays minimal. |
| DD-D15 | **A drop that would add more than one distinct track to a playlist asks first** (owner request, 2026-10-22). The tracks are fetched, then the playlist dialog's confirm step reads "Add N tracks [from X] to Y?" with Cancel / Add. A single track adds at once; queue drops and the menu's picker never ask. | A missed drop of a whole album or playlist is costly to undo (no undo yet). The menu's picker is already a deliberate choice. |

## 4. Interaction model

### 4.1 Starting a drag
- Primary button, `pointerType !== "touch"`, and the pointer moves at least **5 px**
  from where it went down. Before that the press is an ordinary click, so a single
  click still selects a row and double-click still plays.
- A press on a text input, a slider, or `[data-no-drag]` never starts a drag. Presses on
  buttons and links *can* start a drag (you can grab a row by its cover button or
  title link, as in TIDAL). If the drag starts, the `click` that follows the release is
  swallowed once, in the capture phase (the queue rail's `swallowClick` pattern).
- Native `dragstart` is prevented on drag sources, because `<img>` covers would
  otherwise start an OS drag.
- While dragging: `body.dragging` sets `user-select: none` and `cursor: grabbing` (or
  `no-drop` over a refusing target).
- **Cancel:** Escape (capture listener), `pointercancel`, window `blur`, a route change,
  or sign-out.

### 4.2 Targets
Each zone registers `{ accepts(payload) → Verdict, hover?(payload, point), drop(payload, point) }`.
A `Verdict` is `{ ok: true, hint } | { ok: false, reason }`. The controller finds the
innermost `[data-drop-zone]` under the pointer, calls `accepts` when the zone changes
(not on every move), and calls `hover` once per frame for slot-aware zones (queue, the
source table).

| Target | Accepts | Feedback | Drop |
|---|---|---|---|
| Playlist row (tree) / Playlists-grid card | `owned`, and not the payload's source playlist | accent ring + tint; refusal: muted + `no-drop` + reason in the ghost | resolve tracks → `playlistActions.addTracks` with a row spinner until done |
| Folder row (tree, collapsed) | never (spring-open only) | after 600 ms → `collection.toggleFolder` | — |
| Collapsed rail | — | after 400 ms → drag-peek | — |
| Queue rail list | any payload | insertion line at the slot (`dragTarget` math); below the last row → "append" | resolve → filter blocked → `queueInsert(slot)` or `queueAdd` |
| Source TrackTable (owned playlist, reorder allowed) | payload `source.tableId` is this table | today's drop line | `onDrop(fromIndices, to)` |

### 4.3 After a drop
- **Playlist add:** toast `Added 12 tracks to Road Trip` (singular: `Added "Song" to
  Road Trip`) with an **Open** action that navigates to the playlist. If tracks were
  skipped, the duplicates modal opens instead, as the picker does now. The tree count
  updates through `collection.refresh()` (the Sidebar already calls it when
  `playlistActions.revision` bumps, so `addTracks` bumps `revision` after an add), and
  `invalidatePlaylistCovers(uuid)` drops the collage and
  `playlist:<id>` cache. An add to the playlist open in the main view refreshes its
  pager.
- **Queue:** toast `Added 12 tracks to the queue`. The rail updates from the state
  event (`queue_rev`).
- **Errors:** an error toast. The drag state is already cleared.

## 5. Multi-select

### 5.1 Model (`lib/selection.svelte.ts`)
```ts
export interface Selection<T> {
  readonly size: number;
  has(item: T): boolean;
  /// Plain click: select only `item`. Ctrl/Meta: toggle. Shift: range anchor→item
  /// over `items` (replacing, or adding with Ctrl+Shift).
  click(item: T, items: readonly T[], mods: { ctrl: boolean; shift: boolean }): void;
  extend(item: T, items: readonly T[]): void; // Shift+Arrow
  selectAll(items: readonly T[]): void;
  clear(): void;
  /// Selected items in `items` order (drops any no longer present).
  ordered(items: readonly T[]): T[];
  /// Source indices of the selection, for remove/move.
  indices(items: readonly T[]): number[];
}
```
- The state is a `SvelteSet<T>` plus an `anchor: T | null`.
- The pure range math (`rangeBetween(items, a, b)`) is unit-tested on node.

### 5.2 TrackTable wiring
- **Row click** on the row background, number, or text (not a button or link): plain →
  select one; Ctrl → toggle; Shift → range. Today a single click does nothing, so no
  existing behavior changes.
- **Keyboard** on a focused row:
  - Shift+↑/↓/PgUp/PgDn/Home/End extend the selection;
  - Ctrl+A selects all loaded rows;
  - Space toggles the focused row;
  - Escape clears.
  The existing arrow / Enter / roving tab stop is unchanged.
- **Clears** when sort levels or the filter query change (row order or membership
  changed), and on unmount. It survives pager appends.
- **Visuals:** `.selected` row tint (`--selection-bg`, a new semantic token in each
  mode). `aria-multiselectable="true"` on the grid and `aria-selected` on rows (the
  table already has grid/row roles when not split).
- **Context menu:** right-clicking a row that's part of a 2+ selection opens
  `selectionMenuItems`:
  - Play next (N);
  - Add N to queue;
  - Add N to playlist… (the picker with the tracks);
  - Remove N from playlist (owned only, via `api.removePlaylistItems(id, indices)`;
    the parent passes `onRemoveMany`).

  Right-clicking an unselected row selects it alone and shows the usual single-track
  menu.
- Blocked (content-filtered) rows can be selected. Queue actions filter them, and
  playlist adds keep them (DD-D7).

## 6. Architecture

### 6.1 Files
```
ui/src/lib/dragPayload.ts          payload union, label/cover helpers, resolveTracks()
ui/src/lib/dragDrop.svelte.ts      controller singleton + dragSource / dropZone actions
ui/src/lib/components/DragGhost.svelte   overlay (mounted in App for both shells)
ui/src/lib/selection.svelte.ts     multi-select model
ui/src/lib/collectionTracks.ts     loadTracks(target) moved out of cardActions.ts
ui/src/lib/drag.ts                 keep dropTarget/normalizeDrop (+ normalizeMultiDrop); drop trackDrag
crates/oxitide-audio/src/queue.rs  Queue::insert_at
crates/oxitide-audio/src/engine.rs EngineCmd::EnqueueAt
crates/oxitide-app/src/commands.rs queue_insert (+ main.rs, SYNC_COMMANDS, remote NOT_EXPOSED)
```

### 6.2 Payload
```ts
export type DragPayload =
  | { kind: "tracks"; tracks: Track[]; source?: DragSourceRef }
  | { kind: "track-id"; id: number; title: string; cover?: string | null }  // PageItem tracks
  | { kind: "collection"; target: CardTarget; cover?: string | null };
export interface DragSourceRef { tableId?: string; playlistId?: string }
export function resolveTracks(p: DragPayload): Promise<Track[]>;
export function payloadLabel(p: DragPayload): string;   // "Song", "12 tracks", album title
```
- A `collection` whose target is a playlist sets `source.playlistId` implicitly, so the
  self-drop rule applies to dragging a playlist card onto its own tree row.
- `resolveTracks` for a `collection` reuses `loadTracks`. For `track-id` it uses
  `api.getTrack`.

### 6.3 Controller (`dragDrop`)
- **State the UI reads** (runes):
  - `active: DragPayload | null`;
  - `zone: ZoneId | null`;
  - `verdict`;
  - `pendingTargets: SvelteSet<string>` (rows showing a spinner while their drop
    resolves).
- **`use:dragSource={() => payload | null}`.** The getter runs at drag start, so
  TrackTable computes "selection or this row" lazily. `null` means "not draggable now".
- **`use:dropZone={zone}`** stores the zone in a module `WeakMap<Element, Zone>` and sets
  `data-drop-zone`. `zone.key` names the zone, for `pendingTargets` and spring timers.
- **Ghost:** `DragGhost.svelte` binds its element to the controller. The controller
  writes `transform: translate(x, y)` in its rAF tick, and the ghost re-renders only
  when the verdict or label changes.
- **Auto-scroll:** zones may expose `scroller`. The controller runs `edgeScroll`
  (`lib/queueDrag.ts`) on whichever scroller is under the pointer, once per frame.

### 6.4 Playlist reorder via the controller
- TrackTable registers itself as a drop zone, with `accepts` → `source.tableId === id &&
  canDrag`. The slot comes from `dropTarget(...)` as today, and the drop line renders
  from `dropAt`.
- **Single-row** moves keep today's `movePlaylistItems(id, [from], normalizeDrop(from,
  to))`.
- **Multi-row** moves need TIDAL's `toIndex` semantics for a list of indices
  (`POST playlists/{id}/items/{i,j,k}`). Is the target counted before or after the moved
  rows are removed? Does the block keep its relative order? A **spike** (plan Task 1)
  checks this on a scratch playlist and records the answer in
  `docs/superpowers/plans/notes/`. Until it passes, a multi-row reorder drag shows
  "Can't move several rows yet". Adding several rows to *other* targets is unaffected.

### 6.5 `queue_insert`
- `Queue::insert_at(index, items)` splices at `min(index, len)`. If `index <= current`,
  `current += items.len()`, so the playing track keeps playing. The queue revision
  bumps (copy-on-write `items_mut`).
- Shuffle: inserted items land at that position in the current play order, matching
  `insert_next`.
- `EngineCmd::EnqueueAt { index, items }` mirrors `EnqueueNext`: same session, no
  restart, and the prefetch is re-evaluated the same way `insert_next` triggers it (the
  plan task checks that `insert_next`'s path covers "the next track changed").

## 7. Visual design
- **Drop-target row:** `outline: 2px solid var(--accent)` inset, with background
  `color-mix(var(--accent) 14%, transparent)`.
- **Refused:** `opacity: .55` on the row, the `no-drop` cursor, and the ghost hint in
  `--text-muted`.
- **Queue slot:** the queue rail's existing drag gap, opened at the slot (no new
  visuals).
- **Ghost:** `--surface-3` background, `--shadow-panel`, `--radius-control`, 36 px tall,
  max 280 px wide. More than one track stacks two offset thumbs with a count badge
  (`--accent` / `--on-accent`).
- Reduced motion: no ghost scale-in, and the springs still work (they're timing, not
  animation).
- Record the ghost, target and refused states in `ui/design/DESIGN.md` (shared) with
  per-mode notes. The Rack shows them in its skin through the existing `pane` and
  tree selectors in `rack.css`.

## 8. Accessibility
- Selection is fully keyboard-operable (§5.2). `aria-selected` and
  `aria-multiselectable` are exposed.
- Toasts are already announced (live region). The drop results reuse them.
- Drag is mouse-only. Every drag outcome is reachable from the menu path (DD-D12).

## 9. Testing
- **Node:**
  - `selection` range/toggle/ordered/indices;
  - `dragPayload` labels and `resolveTracks` (mocked api);
  - the `normalizeMultiDrop` math;
  - verdict rules (owned, self-drop, source table).
- **jsdom:**
  - controller: threshold, cancel on Escape/blur, click swallowed after a drag, no
    window listeners at rest (spy on `addEventListener`/`removeEventListener`), a zone
    swap calls `accepts` once;
  - TrackTable: click/Ctrl/Shift selection, Ctrl+A, Escape, clear on sort, selection
    menu shown for a 2+ selection, a drag from a selected row carries the ordered
    selection;
  - PlaylistTree: a refused row on not-owned, `addTracks` called on drop, and the
    folder spring timer (fake timers);
  - `playlistActions.addTracks`: success toast, duplicates → modal opens at the
    `duplicates` step, error toast.
- **Rust:** `Queue::insert_at` (before / at / after current, past the end, empty queue,
  shuffle); an engine test that `EnqueueAt` doesn't restart the session (mirrors the
  `EnqueueNext` test). The `SYNC_COMMANDS` and remote classification tests cover the
  new command.
- **Smoke (owner, real account):** `docs/superpowers/plans/notes/2026-10-xx-drag-to-playlist-smoke.md`.
  Check: every source × target pair, the duplicates prompt, a refused not-owned
  playlist, folder spring and rail peek, queue slot insert while playing, owned
  playlist reorder (single + multi if the spike passed), the Rack shell, and Linux
  deferred with the batched bare-metal run.

## 10. Risks
- **elementFromPoint under the ghost:** the ghost is `pointer-events: none`. The
  rail-peek overlay and the floating player bar sit above the page, so the hit test
  respects real stacking. Intended.
- **The queue dock glide (AR-9)** animating mid-drag: the queue zone refuses drops
  while a glide runs. `queueTransition.ts` exposes no running flag today, so the plan
  adds a `glideActive()` getter next to `startPendingGlide`/`releasePageHold`.
- **Virtualized rows** unmount during auto-scroll. Zones attach to the scroll
  container, never to rows (like today's `trackDrag`), and rows are found through
  `data-row-index`.
- **Large payloads** (dragging a 3 000-track playlist): fetching it takes a while. The
  target row's spinner shows it's working, and the backend chunks the add.

## 11. Implementation notes (2026-10-22)

Where the build differs from the text above:
- **Space keeps playing the row** (an existing binding with tests behind it).
  Ctrl+Space toggles a row's selection instead (Explorer convention).
- **No `aria-selected` / `aria-multiselectable`.** The track list is an ARIA `table`, not a
  `grid`, and those attributes are unsupported on table rows. A polite live region announces
  "N tracks selected" instead.
- **Queue slot feedback is a 2 px accent insertion line**, not the rail's row gap. The gaps
  around the current track close while a drag is over the rail, so slots sit at
  `index × rowH`.
- `playlistActions.addTracks` takes a track list **or a loader**, so a failed fetch of a
  dropped album is reported like a failed add. A second drop while an add runs shows an
  info toast instead of being ignored silently.
- `collection.refresh()` keeps an open folder's rows on screen while it reloads, so the
  refresh after each drop doesn't blink the tree.
- The HTML5 `trackDrag` action and `VirtualList`'s unused `drag` prop are removed;
  `drag.ts` keeps only `dropTarget` / `normalizeDrop`.
- Single-row reorder uses the controller now. Multi-row reorder is refused until the
  Task 1 spike settles TIDAL's semantics.

Owner feedback round (2026-10-22):
- **Sidebar / Rack tree playlist rows are drag sources** (a `collection` payload, like a
  Playlists-page card), so a playlist drops onto another playlist or the queue. Folder rows
  are not draggable. The Playlists-page cards were drop targets only; they now drag their
  playlist too, and their titles are `NameLink`s (hover underline, like Home's cards).
- **The player bar's title block drags the playing track** as well as its cover.
- **DD-D15's confirm step** lives in `PlaylistPicker` (`step: "confirm"`,
  `playlistActions.confirming` / `confirmAdd`). The modal's focus trap leaves focus on Close,
  so Enter never confirms by accident.
- **A left press outside a table's rows clears its selection** (after the owner smoke run).
  Presses inside a context menu or dialog keep it, and the window listener exists only
  while something is selected.
- **Confirm and duplicates dialogs restyled** (second round): From / To card, clamped long
  names, per-row Keep one / Add both switches, an "All" switch, and a footer of "Skip
  duplicates" + "Add N copies". The duplicates toast reads "Added 2 copies, kept one of 3"
  instead of "2 added, 3 kept one of".

## 12. Follow-ups
- Drag queue-rail rows out to playlists (hand off from the rail's pointer drag to the
  controller once the pointer leaves the rail).
- Drag playlists into folders, in the tree and in the grid.
- A "+ New playlist" drop target (create with the dragged tracks).
- Undo for adds (remove the appended index range, ETag-guarded).
- Multi-select in the queue rail.
