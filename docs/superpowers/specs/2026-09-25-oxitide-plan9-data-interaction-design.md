# OxiTide — Plan 9 (Data Interaction) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-25
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 2a, second slice)
- **Predecessor:** `2026-09-25-oxitide-plan8-ux-foundations-design.md`
- **Scope note:** Track 2a's second slice, as fixed by the Plan 8 spec (§15):
  sortable/filterable tables, search-as-you-type, playlist drag-and-drop reorder,
  and virtualization beyond `TrackTable`. The planning decisions were confirmed
  with the owner on 2026-09-25 (see §13).

---

## 1. Purpose & Outcome

Plan 8 gave OxiTide its interaction foundations (toasts, menus, shortcuts,
history, states). Plan 9 makes the data itself workable: every track list can be
sorted and filtered, search answers as you type, playlists reorder by dragging,
and the two remaining unbounded lists (queue, library) stop building thousands of
DOM nodes.

Success = finding a track in a 2,000-row favorites list takes a filter keystroke
and a sorted glance, not a scroll hunt; typing in Search shows results without
pressing Enter, and fast typing never flashes stale results; dragging a playlist
row moves it once, with no full-list refetch flash; and the queue stays smooth
when it holds an entire album or playlist.

## 2. Scope

### In scope (Plan 9)

- **Sort/filter** on every `TrackTable` (Search, Album, Playlist, Library
  favorites): clickable Title/Time headers cycling asc → desc → original, a `#`
  header resetting to original, and a header filter field matching title +
  artist names. Engaging sort/filter calls `pager.ensureAll()` first so the
  operation applies to the complete list; the table shows a progress state until
  it is ready.
- **Search-as-you-type**: 300 ms debounce at ≥ 2 characters, Enter searches
  immediately, a request-sequence guard drops stale responses, previous results
  stay visible while a new query is in flight, and clearing the box clears
  results.
- **Playlist drag-and-drop reorder**: native HTML5 drag on rows with a handle,
  a container-computed drop index that works over virtualized rows, and an
  insertion indicator. Drops ride an optimistic `pager.moveItem` with an
  API call and refresh-based rollback; the existing ↑/↓ buttons and context
  menu Move items move onto the same optimistic path.
- **Virtualization**: extract the windowing math; virtualize `QueueRail` and the
  Library playlist/album lists via a shared `VirtualList` component.
- **Tests**: vitest for every new pure module and the `pager`/`menus`
  additions; `plan9-smoke.md` human checklist.

### Explicitly deferred (2b / Track 3 / later)

- Visual identity, layout/IA, density, expanded now-playing (2b).
- Search pagination and rich suggestions (no Rust search command change here);
  the 20-per-group cap stays.
- Command palette, customizable shortcuts, toast history/undo.
- Performance harness and budgets (Track 3); the >10k engage caveat in §11 is
  revisited there.
- Sort/filter persistence across sessions or navigation (state is per-mount).
- Queue/library virtualization beyond the lists named in §5.7.

## 3. Tech Stack Additions

None. No new crates, no new npm packages, no DOM test environment (P8-D6
continues). Version stays 0.2.0; no packaging or config changes. **This slice is
UI-only: no Rust files change.**

## 4. Repository Layout Changes

```
ui/src/lib/
  virtual.ts              (NEW: windowRange pure math)
  trackView.svelte.ts     (NEW: createTrackView sort/filter/ensureAll lifecycle)
  search.svelte.ts        (NEW: createSearch debounced live-search store)
  drag.ts                 (NEW: dropTarget + normalizeDrop + trackDrag action)
  virtual.test.ts         (NEW)
  trackView.test.ts       (NEW)
  search.test.ts          (NEW)
  drag.test.ts            (NEW)
  pager.svelte.ts         (add moveItem)
  pager.test.ts           (moveItem cases)
  menus.ts                (playlist context gains canMove)
  menus.test.ts           (canMove cases)
ui/src/lib/components/
  VirtualList.svelte      (NEW: spacer/slice renderer for <ul> lists)
  TrackTable.svelte       (view prop replaces tracks + player; sort/filter
                           headers; drag wiring; windowRange)
  QueueRail.svelte        (VirtualList; shared trackDrag action)
ui/src/views/
  Playlist.svelte         (view; optimistic move; drag drop handler)
  Library.svelte          (view; virtualized playlist/album lists)
  Album.svelte            (view)
  Search.svelte           (createSearch store; view for results)
docs/superpowers/plans/notes/
  plan9-smoke.md          (NEW)
```

## 5. Architecture

### 5.1 Windowed list math (`virtual.ts`, `VirtualList.svelte`)

```ts
export interface WindowRange {
  first: number;
  last: number;
  padTop: number;
  padBottom: number;
}
export function windowRange(
  total: number,
  scrollTop: number,
  viewport: number,
  rowHeight: number,
  overscan?: number, // default 8
): WindowRange;
```

- `visible = max(1, ceil(viewport / rowHeight) + overscan * 2)`; `first` clamps
  to `max(0, total - visible)` so a shrunk list cannot leave a blank window;
  `last = min(total, first + visible)`; pads are `first * rowHeight` and
  `(total - last) * rowHeight`. This is `TrackTable`'s current math, extracted
  unchanged (`TrackTable.svelte:50-62`).

```
VirtualList.svelte props:
  total: number; rowHeight: number; maxHeight?: string; overscan?: number;
  children: Snippet<[number]>; // one row, given its index
```

`VirtualList` owns `scrollTop` and `viewport` (`bind:clientHeight`), renders a
scroll container (`max-height`, `overflow:auto`), a top spacer, the visible
indexes, and a bottom spacer. Consumers render one row per index.

### 5.2 TrackView (`trackView.svelte.ts`)

```ts
export type TrackSort = "original" | "title" | "duration";
export type SortDirection = "asc" | "desc";

export interface TrackView {
  readonly filter: string;
  readonly sort: TrackSort;
  readonly direction: SortDirection;
  /// sort != "original" or filter non-empty
  readonly engaged: boolean;
  /// ensureAll in flight; items still show source order
  readonly preparing: boolean;
  /// engage failed; items stay in source order until retry()
  readonly error: string;
  /// visible rows: source order until ready, then sorted/filtered
  readonly items: Track[];
  setFilter(query: string): void;
  toggleSort(key: Exclude<TrackSort, "original">): void;
  /// Back to original order for the sort only; the filter is untouched
  /// (its own ✕ clears it).
  clearSort(): void;
  retry(): void;
  /// Index of items[visibleIndex] within the source list, by identity;
  /// -1 when absent (callers treat that as "no row to act on").
  sourceIndex(visibleIndex: number): number;
}

export function createTrackView(
  source: () => Track[],
  ensureAll?: () => Promise<void>,
): TrackView;
```

- **Engage:** `setFilter`/`toggleSort` set state; if the view becomes `engaged`
  and not already ready, it calls `ensureAll()` once and sets `preparing`.
  On success `items` becomes `sort(filter(source()))`; on failure `error` is set
  and `items` stays in source order. `retry()` clears the error and reloads.
  When `ensureAll` is undefined (Album, Search) the view is ready immediately.
  Becoming disengaged (filter cleared and sort back to original) clears
  `preparing`/`error`.
- **Filter:** trimmed, case-insensitive substring match against the title and
  the joined artist names. Empty string clears it.
- **Sort:** stable — source index is the tie-break. `toggleSort` cycles
  asc → desc → original for the clicked key; switching keys starts at asc.
  Title uses `localeCompare` with `sensitivity: "base"`; duration is numeric.
  `direction` is ignored while `sort === "original"`.
- **`sourceIndex`** exists for playlist edits under `engaged` views:
  `source().indexOf(items[visibleIndex])`. Rows are distinct objects (a pager
  page yields fresh objects, duplicates included), so identity is reliable.
- **Lifetime:** created per view mount; sort/filter state is not persisted and
  resets when the view is left. The pager's session cache still hydrates rows.

### 5.3 TrackTable changes

- Props: `view: TrackView` replaces `tracks: Track[]`; the unused `player`
  prop (`TrackTable.svelte:11`) is dropped. `maxHeight`, `favorites`,
  `onPlay`/`onPlayAll`/`onEndReached`/`onAddToPlaylist`/`playlistActions`
  keep their names and now index into `view.items`.
- Windowed rendering uses `windowRange(view.items.length, scrollTop, viewport,
  ROW_HEIGHT, OVERSCAN)`; `ROW_HEIGHT = 36` and the row style stay unchanged.
- Header:
  - `Play all` (unchanged) then `#`, `Title`, `Time`. Title and Time are
    buttons toggling via `view.toggleSort`; the active one shows ▲/▼; `#`
    calls `view.clearSort()` (the filter stays). Sorting controls are
    disabled only while `view.preparing`.
  - A filter field row under the columns: input bound to `view.filter` with a
    ✕ clear button when non-empty; placeholder `Filter tracks`.
  - While `view.preparing`, an inline `Loading label="Loading all tracks…"`
    replaces the sort indicators’ row space; while `view.error`, an inline
    message + Retry (`view.retry()`) appears.
  - Filtered-to-empty body reads `No tracks match “<filter>”`; otherwise the
    existing `No tracks`.
- Rows gain `draggable` + `data-drag-index` only when drag is enabled, plus the
  ⠿ handle; the scroller gets `use:trackDrag` (§5.5). Menu context:
  `playlist: playlistActions && { ...playlistActions, canMove: !view.engaged }`,
  so Move items drop out while sorted/filtered but Remove/Favorite stay
  (`menus.ts` addition below).
- The ↑/↓ inline buttons are hidden while `view.engaged`; Remove stays and
  still receives the visible index — the parent maps it back through
  `view.sourceIndex(index)` before calling the API (§5.6).
- `onPlay(index)` / `onPlayAll()` continue to receive visible indices; parents
  build queues from `view.items` (§7).

`menus.ts` context change:

```ts
playlist?: {
  onRemove(index: number): void;
  onMove(index: number, delta: -1 | 1): void;
  busy: boolean;
  disabled: boolean;
  canMove?: boolean; // default true; false omits move-up/move-down only
};
```

The builder still omits the whole block while `busy`/`disabled`; `canMove ===
false` keeps Remove and omits the two move items.

### 5.4 Search store (`search.svelte.ts`)

```ts
export interface SearchStore {
  readonly query: string;
  readonly results: SearchResults | null;
  readonly loading: boolean;
  readonly error: string;
  /// query whose results are on screen (for the no-results state)
  readonly settledQuery: string;
  setQuery(query: string): void; // debounced; lives at >= 2 chars
  runNow(): void;                // Enter: cancel timer, fire immediately
  retry(): void;                 // re-run the settled query immediately
  reset(): void;                 // clear query, results, and any timer
}
export function createSearch(
  fetch: (query: string) => Promise<SearchResults>,
  delayMs?: number, // default 300
): SearchStore;
```

- `setQuery` trims for length checks but keeps the raw value in `query`.
  Below 2 characters: cancel any timer, clear `results`/`settledQuery`/`error`,
  and set `loading = false`. At ≥ 2 characters: restart the debounce timer.
- `runNow` cancels the timer and searches immediately at ≥ 1 character (the
  previous Enter behavior); below that it clears.
- Each request takes a monotonically increasing sequence id; a response applies
  only if it is still the latest (`loading` ends and results/error update), so
  fast typing cannot flash out-of-order results. Old results and `settledQuery`
  stay until the newest response lands.
- Errors set `error` and clear `results` (matching Plan 8's `ErrorState`
  pattern); `retry()` re-fires `settledQuery || query`.
- `Search.svelte` becomes a thin renderer: `#search-input` keeps its id
  (App's `Ctrl+F` focus), `oninput` → `store.setQuery`, Enter → `store.runNow`.
  Full `Loading` renders only when there is no previous result; with results on
  screen a small inline spinner appears next to the input. Empty state keeps
  `No results for “<settledQuery>”`.

### 5.5 Drag reorder (`drag.ts`)

```ts
/// Insertion slot (0..count) under the pointer. `offset` is content above the
/// first row (e.g. the sticky thead). Rounding puts the slot at the closer edge.
export function dropTarget(
  clientY: number,
  scrollerTop: number,
  scrollTop: number,
  rowHeight: number,
  count: number,
  offset?: number, // default 0
): number;

/// Removing the dragged row first shifts a downward target up by one.
export function normalizeDrop(from: number, to: number): number;

export interface DragParams {
  enabled: () => boolean;
  count: () => number;
  rowHeight: () => number;
  offset?: () => number; // default () => 0
  onChange: (from: number | null, at: number | null) => void;
  onDrop: (from: number, to: number) => void;
}

/// Svelte action on the scroll container. Rows carry `draggable` and
/// `data-drag-index`; listeners are per container, not per row.
export function trackDrag(node: HTMLElement, params: DragParams): {
  update(next: DragParams): void;
  destroy(): void;
};
```

- `dragstart` (delegated): resolve the row via `closest("[data-drag-index]")`;
  if `enabled()`, record `from`, set `dataTransfer.effectAllowed = "move"`.
- `dragover`: while dragging and enabled, `preventDefault()` (allows the drop),
  compute `at = dropTarget(...)`, report via `onChange`.
- `drop`: `preventDefault()`, call `onDrop(from, normalizeDrop(from, at))`,
  clear state. `dragend`/`dragleave` (leaving the container bounds) clear state.
- Chromium's native auto-scroll scrolls the container while dragging near an
  edge; the pointer math plus `onChange` keeps the indicator correct as rows
  mount/unmount. If a virtualized drag source unmounts during auto-scroll, the
  browser keeps the drag operation alive; the drop slot is computed from the
  container, not the row. The smoke checklist verifies a long drag.
- Consumers own the visual indicator: they store `from`/`at` from `onChange`
  and render a line at `offset + at * rowHeight` inside the scroller, or a row
  highlight for the source.

### 5.6 Optimistic moves (`pager.moveItem`, playlist paths)

```ts
// Pager addition
/// Reorder within the loaded rows; no total/offset change. No-op when either
/// index is out of range or equal.
moveItem(from: number, to: number): void;
```

- Implementation mirrors the engine queue semantics used by
  `Queue::reorder`: remove at `from`, insert at `to` (final index), then
  `publish()`. Reordering does not shift `nextOffset`, so it is safe while a
  page fetch is in flight.
- `Playlist.svelte`:
  - `moveAt(index, delta)` and the DnD drop handler both go through
    `moveTrack(from, to)`: `pager.moveItem(from, to)` first (instant), then
    `api.movePlaylistItems(id, [from], to)`; on failure
    `toasts.show(errorMessage(e))` + `await pager.refresh()` to roll back.
    `busy` still guards concurrent moves, and `disabled: !pager.done` still
    gates them until the full list is loaded.
  - `removeAt(index)` maps the visible row with `view.sourceIndex(index)` (a
    -1 no-ops) and keeps today's route: API call then `pager.refresh()` (server
    indices shift, so a local splice would not model it).
  - Drop is only enabled when `playlistActions` exists, `!view.engaged`,
    `pager.done`, and `!busy`.
- Playback (`playFrom`, `playAll`) awaits `pager.ensureAll()` and then builds
  the queue from `view.items` — so Play follows the sorted/filtered order, and
  an index captured before all pages loaded still names the clicked row.

### 5.7 QueueRail and Library virtualization

- **QueueRail:** the item list becomes a bounded scroller (`max-height:
  calc(100vh - 200px)`, `overflow: auto`) rendered by `VirtualList` with a
  fixed 40 px row. Rows keep the current-track highlight, jump/remove buttons,
  context menu, and ⠿ handle; the per-row dragover/drop handlers are replaced
  by one `use:trackDrag` on the scroll container with `onDrop =
  (from, to) => player.reorder(from, to)`. Keys become index-based (window
  shifts remount rows; identity has no visual effect here).
- **Library:** the playlists list (fixed 44 px rows — tall enough for the
  inline rename input) and the albums list (36 px) render through
  `VirtualList`; `editingUuid`/`editTitle` state stays in the parent, so
  scrolling an edited row out of view loses nothing. Empty rows (`No
  playlists` / `No albums`) and the menu/rename/delete flows are unchanged.
- **TrackTable** switches to `windowRange` but keeps its table-spacer approach
  (top/bottom `colspan="4"` rows) because `<tr>` slicing cannot use
  `VirtualList`'s div spacers.

## 6. Interaction Contracts

- **Playback follows the visible order.** Row play and Play all build the queue
  from `view.items`; under a filter, only visible tracks are queued.
- **Moves require original order.** ↑/↓, menu Move items, and drag-and-drop are
  unavailable while `view.engaged`; Remove and Favorite remain and map through
  `view.sourceIndex`.
- **Engage is all-or-nothing.** Sort/filter never shows a partially sorted
  list: until `ensureAll` completes (or when it fails), the table shows source
  order with a clear progress/error affordance.
- **Search never shows stale answers.** A response only applies when its
  sequence id is the latest; the previous answer stays until then.
- **Drag is one gesture = one move.** Drop calls the same API as the buttons;
  nothing is re-fetched on success, and a failure rolls back to server truth.
- **Fixed row heights are load-bearing** for windowing and drop math:
  TrackTable 36 px, QueueRail 40 px, Library playlists 44 px, Library albums
  36 px. Changing one requires updating its constant and the smoke check.

## 7. UI Changes

- **TrackTable**: sort/filter header + filter field, progress/error rows,
  filtered-empty copy, drag handle/indicator, `view` contract.
- **Album / Search / Library / Playlist**: each constructs a `TrackView` over
  its source and passes it; play handlers read `view.items`.
- **Search**: store-driven input/results; inline spinner beside the input while
  results exist.
- **QueueRail**: virtualized list; shared drag action.
- **Library**: virtualized playlists/albums lists.

## 8. Error Handling

| Situation | Behaviour |
|---|---|
| `ensureAll` fails on engage | Inline header error + Retry; table stays in source order; sort/filter state kept |
| Filter matches nothing | `No tracks match “<filter>”` row |
| Search request fails | Inline `ErrorState` + Retry (Plan 8 pattern); old result cleared |
| Stale search response | Discarded by sequence id; no state change |
| Playlist move API fails | Error toast + `pager.refresh()` rollback (list returns to server order) |
| Reorder attempted before `pager.done` | Controls disabled (existing `disabled` gate) |
| Drop at the dragged row's own slot | `normalizeDrop` yields `from`; `moveItem` no-ops |
| Virtualized list shrinks under the pointer | `windowRange` clamps `first`; no blank table |
| Drag source unmounts mid-scroll | Container-level drop math still resolves; smoke covers it |

## 9. Testing Strategy

- **vitest (pure modules):**
  - `virtual.test.ts`: window size with overscan, `first`/`last` bounds at
    top/middle/end, shrink clamp, pad sums equal the unrendered heights.
  - `trackView.test.ts`: toggle cycles (asc → desc → original) per key and key
    switches; stable ties; title/duration orders; filter by title and artist
    (case-insensitive, trimmed); engage calls `ensureAll` once and sets
    `preparing`; error + `retry`; `clearSort` keeps the filter; `sourceIndex`
    mapping (including -1); disengage clears `preparing`/`error`.
  - `search.test.ts` (fake timers): 300 ms debounce; 2-char minimum; Enter
    immediate at 1+ chars; stale responses discarded; old results retained
    while loading; clear on empty; error + retry.
  - `drag.test.ts`: `dropTarget` at row edges (rounding), scrolled offsets,
    `offset` for the sticky header, clamping to `0..count`; `normalizeDrop`
    down/up and self-drop.
  - `pager.test.ts` additions: `moveItem` reorders loaded items and publishes
    the cache; invalid/equal indices no-op; offsets/total unchanged.
  - `menus.test.ts` additions: `canMove: false` keeps Remove/Favorite and omits
    move items; default keeps them.
- **svelte-check:** 0 errors / 0 warnings.
- **Manual smoke (`docs/superpowers/plans/notes/plan9-smoke.md`):** sort/filter
  on a 2k+ playlist (progress → complete sort; no partial order); remove under
  filter; move controls disabled while engaged; long drag with auto-scroll;
  drop indicator alignment; queue drag + jump/remove under virtualization;
  Library list scroll + rename row; search debounce, stale resistance, Enter,
  no-results; shortcuts (Space, Ctrl+F, Esc) unaffected while the filter field
  is focused; 36/40/44 px alignment.
- **Gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui test`, `npm --prefix ui run check`.

## 10. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Task | Exit criteria |
|---|---|---|
| P9-1 | `virtual.ts` + `pager.moveItem` | `virtual.test.ts` and `pager.test.ts` additions green |
| P9-2 | `trackView` + TrackTable sort/filter across the four views | `trackView.test.ts` green; headers/filter field work; menus `canMove` tested |
| P9-3 | `search.svelte.ts` + Search rewire | `search.test.ts` green; live search + stale guard in the view |
| P9-4 | `drag.ts` + playlist DnD + optimistic moves | `drag.test.ts` green; drop/drag path wired; move no longer refetches |
| P9-5 | `VirtualList` + QueueRail/Library virtualization | queue/library scroll smoothly; drag still reorders |
| P9-6 | Verification | Gates green + smoke checklist recorded |

## 11. Performance Impact (PM-D1)

- Sort/filter are `$derived`: sorting runs once per source/state change;
  filtering is a single O(n) pass per keystroke over the complete list.
- `ensureAll` reuses the existing pager burst (parallel windows, 10k cap);
  progress is visible, and repeated engages do not re-fetch.
- Search holds one timer and never issues concurrent fetches (the debounce
  guarantees a quiet period; late responses are dropped, not displayed).
- Drag listeners attach per container (one set per list), never per row.
- `VirtualList`/`windowRange` cap rendered rows regardless of collection size;
  Library and QueueRail stop growing the DOM with the collection.
- **Known caveat:** engaging sort/filter on a list above the 10k prefetch cap
  walks pages sequentially with per-page requests. Rare today; Track 3's
  harness should measure it and can add a cap/toast if needed.

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| HTML5 drag over virtualized rows misplaces the drop | Index derives from container pointer math, not row events; indicator from the same value; smoke covers long drags and edges |
| Auto-scroll during drag is inconsistent in some WebViews | Chromium container auto-scroll is the expected behavior; ↑/↓ buttons and menu Move items remain available as fallbacks; smoke verifies |
| Optimistic move diverges from the server | Every failure path refreshes from the server; `busy` serializes moves; smoke checks a forced failure |
| Engage on huge lists is slow | Progress row; pager burst already parallel under the cap; caveat documented for Track 3 |
| Fixed row heights drift when styles change | Constants named in §6; smoke checks alignment; `windowRange` unit-tested with exact values |
| Sort/filter + DnD index confusion | DnD disabled while `engaged`; Remove maps via `sourceIndex`; `canMove` omission unit-tested |
| Vitest runs `.svelte.ts` runes without a DOM | Same pattern as `pager.svelte.ts`/`toasts.svelte.ts` today; no component behavior is unit-tested (P8-D6) |

## 13. Decisions Log (Plan 9)

| # | Decision | Rationale |
|---|---|---|
| P9-D1 | Sort/filter engages `ensureAll` and applies to the complete list (owner choice). | Partial sorts are misleading; "Play all" already establishes the load-all precedent. |
| P9-D2 | Clickable column headers cycle asc/desc/original plus a header filter field; all TrackTables share the controls. | Compact, no extra chrome, one component change reaches every view. |
| P9-D3 | Moves are disabled under sort/filter; Remove/Favorite stay via `sourceIndex`. | "Up one visible row" and drop slots are ambiguous in a reordered view; removal by identity is not. |
| P9-D4 | Native HTML5 DnD with a container-computed drop index (owner choice). | Reuses the QueueRail precedent and works with virtualized rows; no new interaction subsystem. |
| P9-D5 | Optimistic `pager.moveItem` + API, refresh only on failure; buttons share the path. | Removes the full-refetch flash Plan 7 shipped; server truth still wins on error. |
| P9-D6 | Live search debounced 300 ms at ≥ 2 chars; Enter immediate; latest-wins sequence; 20-per-group cap kept. | Responsive typing without hammering the API; no Rust change this slice. |
| P9-D7 | Virtualize QueueRail and Library playlist/album lists through a shared `VirtualList`. | The queue is unbounded and the library is the largest user collection; one tested helper serves both. |
| P9-D8 | `TrackTable` takes a required `TrackView` (drops `tracks` and the unused `player` prop). | One source of truth for visible order/index mapping; removes a dead prop while touching all call sites. |
| P9-D9 | Logic-only testing (no DOM environment) continues, per P8-D6. | Repo convention; all Plan 9 logic lives in pure modules. |
| P9-D10 | Sort/filter state is per-mount, not persisted. | Matches the in-session history/cache posture; persistence belongs with Track 3 session work. |

## 14. Pre-spec Research Notes (audit, 2026-09-25)

- `TrackTable` is the only virtualized list: fixed 36 px rows, overscan 8,
  spacers at `TrackTable.svelte:50-62`, static headers at `:106-113`; row
  actions and menus consume indices from the passed array.
- `TrackTable.player` is accepted but never read (`TrackTable.svelte:11`).
- Pager: first page 100, background burst (favorites 1000-row windows, cap
  10k; playlists 100-row windows), `ensureAll` at `pager.svelte.ts:206-210`,
  `done` gates playlist edits (`Playlist.svelte:183`), session cache keyed by
  `"favorites"`/`"playlist:{uuid}"`.
- Playlist edits live in the view: `removeAt`/`moveAt` at
  `Playlist.svelte:104-129`, both awaiting `pager.refresh()`; no optimistic
  update today. `api.movePlaylistItems(id, indices, toIndex)` exists
  (`api.ts:133-134`).
- Search is submit-only with no debounce or stale guard
  (`Search.svelte:31-53`); Rust command hardcodes four types, limit 20,
  offset 0 (`commands.rs:296-301`); `TidalClient::search` supports
  limit/offset if pagination is wanted later.
- The only DnD precedent is QueueRail's per-row HTML5 handlers
  (`QueueRail.svelte:25-41`); no shared helper, no auto-scroll handling.
- No debounce helper or generic timer utility exists in `ui/src`; only the
  toast store uses timers.
- 13 vitest suites today, all pure `ui/src/lib/*.test.ts`; no DOM test
  environment; `@tauri-apps/api` + `plugin-opener` are the only runtime deps.
- Plan 8 shipped `Loading`/`EmptyState`/`ErrorState`, toasts, menus, shortcuts,
  and in-session history — all reused here.

## 15. Out of Scope (tracked for later)

- **Plan 10 (Track 2b):** visual identity, layout/IA, expanded now-playing,
  density, responsive window fit.
- **Track 3:** performance harness and budgets; session/navigation persistence;
  sort/filter persistence; >10k engage tuning.
- **Later:** search pagination/autocomplete, command palette, customizable
  shortcuts, toast history/undo, global media keys (Track 6).
