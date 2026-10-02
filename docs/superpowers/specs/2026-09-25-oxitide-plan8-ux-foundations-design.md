# OxiTide — Plan 8 (UX Foundations) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-25
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 2a, first slice)
- **Predecessor:** `2026-09-24-oxitide-plan7-playlists-design.md`
- **Scope note:** Track 2 is split into 2a (behavior) and 2b (visual/IA); 2a is
  split again (owner decision, 2026-09-25): **Plan 8 = interaction foundations**
  (this document) and **Plan 9 = data interaction** (sort/filter, search-as-you-type,
  playlist drag-and-drop, remaining virtualization).

---

## 1. Purpose & Outcome

Plan 8 gives OxiTide its interaction layer: one toast system instead of two
strips, right-click context menus, keyboard shortcuts, back/forward navigation
(including mouse side buttons), and consistent loading/empty/error presentation,
plus the carryover polish Plan 5 and Plan 7 deferred.

Success = every action outcome surfaces in one place with the right severity and
without vanishing errors; every common action is reachable by right-click;
playback and navigation are keyboard-drivable without breaking typing or
accessibility; history behaves like a desktop app; and no view shows an ad-hoc
"Loading…" or bare red error anymore.

## 2. Scope

### In scope (Plan 8)
- **Toast system** (`ui/src/lib/toasts.svelte.ts` + `ToastHost.svelte`): stacked,
  severity-aware, persistent errors, optional action button, ARIA live region.
  Replaces the `notice` store and both App strips; `createFavorites` and
  `createPlaylistActions` take a `Toasts` dependency.
- **Context menus** (`ui/src/lib/menu.svelte.ts`, `ContextMenu.svelte`,
  pure item builders in `ui/src/lib/menus.ts`): additive right-click menus on
  track rows, queue items, and Library playlist rows.
- **Keyboard shortcuts** (`ui/src/lib/shortcuts.ts` + App host): playback,
  seek, volume, search focus, history, and Escape; mouse side buttons for
  back/forward; a read-only shortcuts list in Settings.
- **Nav history** (`ui/src/lib/nav.svelte.ts`): back/forward stack with
  `canBack`/`canForward`, Sidebar buttons, no persistence.
- **Consistent states**: shared `Loading.svelte` / `EmptyState.svelte`, filled
  missing empty states, load errors always via `ErrorState`, action errors via
  toasts through a pure `errorMessage(e)` mapper.
- **Carryover polish**: pager count spinner (Plan 5), `Modal` focus/ARIA,
  picker autofocus + Enter, clickable Home cards, QueueRail clear confirmation,
  Settings shortcuts list, `forbidden` message fix.
- **Tests**: vitest for every new pure module; svelte-check; `plan8-smoke.md`.

### Explicitly deferred (Plan 9 / 2b / later)
- Sortable/filterable tables; search-as-you-type with debounce and stale-response
  guards; playlist drag-and-drop reorder; virtualization beyond `TrackTable`.
- Visual identity, layout/IA, expanded now-playing (2b).
- Command palette, customizable shortcuts, shortcut discoverability overlay.
- Toast history/undo actions; navigation persistence across restarts.
- Global media keys / SMTC (Track 6); performance harness (Track 3).

## 3. Tech Stack Additions

None. No new crates, no new npm packages (including no DOM test environment:
component behavior is covered by `svelte-check` and the smoke checklist, per the
owner decision). Version stays 0.2.0; no packaging or config changes.

## 4. Repository Layout Changes

```
ui/src/lib/
  toasts.svelte.ts        (NEW: createToasts store)
  menu.svelte.ts          (NEW: createMenu store + menuFor action)
  menus.ts                (NEW: pure MenuItem builders)
  shortcuts.ts            (NEW: matchShortcut + SHORTCUTS table)
  errors.ts               (NEW: errorMessage mapper)
  nav.svelte.ts           (history stack, back/forward, canBack/canForward)
  notice.svelte.ts        (DELETED; replaced by toasts)
  favorites.svelte.ts     (takes Toasts; error/dismissError removed)
  playlists.svelte.ts     (takes Toasts instead of Notice)
  toasts.test.ts          (NEW)
  menu.test.ts            (NEW)
  menus.test.ts           (NEW)
  shortcuts.test.ts       (NEW)
  errors.test.ts          (NEW)
  nav.test.ts             (NEW)
  notice.test.ts          (DELETED)
  favorites.test.ts       (fake Toasts instead of error state)
  playlists.test.ts       (fake Toasts instead of Notice)
ui/src/lib/components/
  ToastHost.svelte        (NEW)
  ContextMenu.svelte      (NEW)
  Loading.svelte          (NEW)
  EmptyState.svelte       (NEW)
  Sidebar.svelte          (back/forward buttons)
  Modal.svelte            (focus management + aria-modal)
  TrackTable.svelte       (row context menu)
  QueueRail.svelte        (item context menu, clear confirm)
  PlaylistPicker.svelte   (autofocus + Enter submit)
ui/src/views/
  Home.svelte             (Loading/EmptyState, clickable cards)
  Search.svelte           (Loading/EmptyState, no-results state)
  Album.svelte            (Loading/EmptyState)
  Artist.svelte           (Loading/EmptyState)
  Playlist.svelte         (Loading + count spinner, errorMessage)
  Library.svelte          (Loading + count spinner, errorMessage)
  Settings.svelte         (shortcuts list, ErrorState with retry)
ui/src/App.svelte         (toast host, menu host, shortcut host, history wiring)
```

## 5. Architecture

### 5.1 Toasts (`ui/src/lib/toasts.svelte.ts`, `ToastHost.svelte`)

```ts
export type ToastSeverity = "success" | "info" | "error";
export interface ToastAction { label: string; run(): void }
export interface Toast { id: number; message: string; severity: ToastSeverity; action?: ToastAction }
export interface Toasts {
  readonly toasts: readonly Toast[];
  show(message: string, options?: { severity?: ToastSeverity; action?: ToastAction }): number;
  dismiss(id: number): void;
}
export function createToasts(): Toasts
```

- Default severity `info`. Success/info auto-dismiss after 6 s; **errors persist**
  until dismissed. Timers are per toast and cleared on dismiss.
- Cap: 3 visible. When full, evict the oldest auto-dismissing toast first, then
  the oldest error, before adding the new one.
- `ToastHost.svelte` renders nothing when empty; fixed top-right stack; container
  `aria-live="polite"`; error toasts add `role="alert"`; each toast has a dismiss
  button and renders its optional action button.
- **Migration:** `notice.svelte.ts` is deleted; `createPlaylistActions(toasts)`
  and `createFavorites(toasts)` take the store. `createFavorites` loses
  `error`/`dismissError`; hydration and toggle failures call
  `toasts.show(errorMessage(e), { severity: "error" })`. App drops both strips and
  the favorites auto-dismiss effect. Success outcomes (playlist added/created/
  removed, renamed/deleted) become success toasts; the raw `String(e)` calls in
  views/stores go through `errorMessage`.

### 5.2 Context menus (`ui/src/lib/menu.svelte.ts`, `ContextMenu.svelte`, `menus.ts`)

```ts
export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  danger?: boolean;
  run(): void;
}
export interface MenuState { x: number; y: number; items: MenuItem[] }
export interface Menu {
  readonly open: MenuState | null;
  show(x: number, y: number, items: MenuItem[]): void;
  close(): void;
}
export function createMenu(): Menu;
/// Svelte action: wires `contextmenu` -> preventDefault + show at the pointer.
export function menuFor(items: () => MenuItem[]): (node: HTMLElement) => { destroy(): void };
```

- `ContextMenu.svelte` renders the open menu at the click point, clamped to the
  viewport (measured after mount), with ↑/↓ active-item movement, Enter to run,
  Escape to close, and click-away/scroll/resize closing it. The active item
  wraps; disabled items are skipped by arrow movement.
- `menuFor` is a Svelte action that wires `contextmenu` → `preventDefault` +
  `menu.show(clientX, clientY, items())`.
- **Pure builders** in `menus.ts` (unit-tested, no DOM). `TrackTable` supplies
  the callbacks (`play`/`addToQueue`/`addToPlaylist` encapsulate its existing
  fallbacks), so the builder never reaches for `api`:

```ts
export interface TrackMenuContext {
  track: Track;
  index: number;
  trackCount: number;
  favorites: Favorites;
  play(): void | Promise<void>;
  addToQueue(): void;
  addToPlaylist(): void;
  playlist?: {
    onRemove(index: number): void;
    onMove(index: number, delta: -1 | 1): void;
    busy: boolean;
    disabled: boolean;
  };
}
export function trackMenuItems(ctx: TrackMenuContext): MenuItem[];
export function queueMenuItems(ctx: {
  index: number;
  playNow(): void;
  remove(): void;
}): MenuItem[];
export function libraryPlaylistMenuItems(ctx: {
  playlist: Playlist;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[];
```

| Builder | Items |
|---|---|
| `trackMenuItems` | Play, Add to queue, Add to playlist, Favorite/Unfavorite; plus Move up, Move down, Remove when the playlist context is present and not `busy`/`disabled` (edges disabled: Move up at index 0, Move down at the last row) |
| `queueMenuItems` | Play now, Remove |
| `libraryPlaylistMenuItems` | Open, Rename, Delete (Delete `danger: true`) |

- Surfaces: `TrackTable` rows (all views), `QueueRail` items, Library playlist
  rows. Inline buttons stay unchanged — menus are additive.

### 5.3 Keyboard shortcuts and mouse side buttons (`ui/src/lib/shortcuts.ts`)

```ts
export type ShortcutAction =
  | "play-pause" | "seek-back" | "seek-forward"
  | "prev-track" | "next-track"
  | "volume-up" | "volume-down"
  | "focus-search" | "history-back" | "history-forward" | "escape";
export function isTypingTarget(target: EventTarget | null): boolean;
export function matchShortcut(event: KeyboardEvent, opts: { overlayOpen: boolean }): ShortcutAction | null;
export interface ShortcutHint { keys: string; description: string }
export const SHORTCUTS: ShortcutHint[];
```

| Keys | Action |
|---|---|
| `Space` | Play/pause |
| `←` / `→` | Seek −5 s / +5 s |
| `Shift+←` / `Shift+→` | Previous / next track |
| `↑` / `↓` | Volume +5% / −5% (clamped 0–100) |
| `Ctrl+F` or `/` | Focus Search (navigates to Search if needed) |
| `Alt+←` / `Alt+→` | History back / forward |
| `Esc` | Close context menu → else close the queue rail → else leave dialogs to their own handler |

- Typing guard: all shortcuts are ignored while `isTypingTarget(event.target)`
  (input, textarea, select, contenteditable), except `Esc` and `Ctrl+F`.
- `matchShortcut` returns null for key-repeat where harmful? Repeats are allowed
  for seek/volume/arrows; play-pause ignores `event.repeat`.
- The App host (`svelte:window`) executes actions: player calls are no-ops when
  nothing is playing; `focus-search` navigates to Search and focuses the input
  (Search's input gains `id="search-input"`); `escape` closes the menu, then the
  queue rail, and is skipped entirely when a `[role="dialog"]` is open (modals
  own their Escape).
- **Mouse side buttons:** the App host listens to `mousedown` and `auxclick`;
  `button === 3` calls `nav.back()`, `button === 4` calls `nav.forward()`, with
  `preventDefault`. OS/driver remapping to `Alt+←/→` is covered by the shortcut
  path. Both are smoke-verified.
- Settings gains a read-only "Keyboard shortcuts" section rendered from
  `SHORTCUTS`; no customization.

### 5.4 Nav history (`ui/src/lib/nav.svelte.ts`)

```ts
export interface Nav {
  readonly route: Route;
  readonly queueOpen: boolean;
  readonly canBack: boolean;
  readonly canForward: boolean;
  goto(next: Route): void;
  back(): void;
  forward(): void;
  toggleQueue(): void;
}
```

- `createNav` keeps `history: Route[]` and `index`; starts at `home`.
  `goto` is a no-op when the next route equals the current one (same `name` and
  `id`); otherwise it truncates any forward entries and pushes. `back`/`forward`
  move `index` (clamped). `toggleQueue` is not part of history. History lives for
  the session only.
- Sidebar renders `‹`/`›` buttons above the links, disabled per
  `canBack`/`canForward`, tooltips naming the shortcuts.
- `Alt+←/→` and mouse buttons 3/4 drive `back()`/`forward()`.

### 5.5 Consistent loading / empty / error states

- `Loading.svelte`: `{ label?: string }` default `"Loading…"`, small CSS spinner
  + text. `EmptyState.svelte`: `{ message: string; action?: { label: string; run(): void } }`.
- Views replace ad-hoc loading text and empty text with these components; labels
  may stay contextual (`"Searching…"`). Missing states are added: Search
  no-results (`No results for “<query>”`), Album/Artist empty branches, Home
  empty via `EmptyState`.
- Rule: **load failures render inline `ErrorState` with Retry; action outcomes
  render toasts**. Settings' post-load failures move from bare red text to
  `ErrorState`; its `apply()` failures use `toasts.error(errorMessage(e))`.
- `errors.ts`:

```ts
export function errorMessage(e: unknown): string
```

Maps exact backend strings: `"forbidden"` → `"You can only edit your own
playlists"`; `"rate limited"` → `"TIDAL is rate-limiting requests — try again in
a moment"`; anything else → `String(e)`.

### 5.6 Carryover polish

- **Pager count spinner:** Library and Playlist render a small `Loading` beside
  the `N tracks` label while `pager.loading` (the Plan 5 follow-up).
- **Modal:** `aria-modal="true"`, focus moves to the first focusable element on
  open and restores to the previously focused element on close; Escape and ✕
  unchanged; still no backdrop-click close.
- **PlaylistPicker:** autofocus the name field in the create step; Enter submits
  the form.
- **Home cards:** `routeForHomeItem(item)` (pure, exported from `nav.svelte.ts`,
  unit-tested) maps `item_type` containing `PLAYLIST` or `EDITORIAL` with a
  non-numeric id to the playlist route, and `item_type` containing `ALBUM` with a
  numeric id to the album route; everything else is inert (no route exists for
  tracks/mixes). Cards without a route render as non-interactive panels.
- **QueueRail:** Clear opens `ConfirmDialog` before clearing.
- **Settings:** shortcuts list from `SHORTCUTS`.
- **App-level login/logout errors:** logout failures use a toast; boot/login
  inline messaging stays.

## 6. Interaction Contracts

- Toast severity semantics: `success` = an action completed; `info` = neutral
  outcome ("3 added"); `error` = the action failed and nothing changed.
- Menus mirror existing inline actions; menu items never introduce behavior the
  inline controls do not already have.
- Shortcuts never fire while typing; `Space`/arrows call `preventDefault` only
  when handled; `Ctrl+F` always `preventDefault`s to suppress WebView find.
- History: forward entries are cleared by any new navigation; queue state is
  orthogonal.

## 7. UI Changes

- **App.svelte**: `createToasts()`, `createMenu()`, shortcut/mouse host,
  `ToastHost`, `ContextMenu`; favorites/playlist stores constructed with toasts;
  no strips.
- **Sidebar**: back/forward buttons.
- **TrackTable**: row `use:menuFor` with `trackMenuItems` context.
- **QueueRail**: item `menuFor` + clear confirm.
- **Home/Search/Album/Artist/Playlist/Library/Settings**: `Loading`/`EmptyState`
  adoption, errorMessage in toasts, pager spinner, shortcuts list.
- **Modal/PlaylistPicker**: focus and Enter polish.

## 8. Error Handling

| Situation | Behaviour |
|---|---|
| Favorite toggle/hydration fails | Error toast via `errorMessage` (persists until dismissed) |
| Playlist action fails | Error toast via `errorMessage`; no state change (unchanged) |
| View load fails | Inline `ErrorState` with Retry (unchanged, now universal) |
| Search returns nothing | `EmptyState` "No results for “<query>”" |
| Shortcut pressed while typing | Ignored (except Esc/Ctrl+F) |
| Esc with a dialog open | Dialog handles it; the shortcut host does not also close the rail |
| Mouse button 3/4 unmapped by WebView | Shortcut fallback (`Alt+←/→`) and smoke note |
| Toast over cap | Oldest auto-dismissing evicted first, then oldest error |

## 9. Testing Strategy

- **vitest (pure modules):**
  - `toasts.test.ts`: severity defaults; 6 s auto-dismiss with fake timers;
    errors persist; dismiss clears timer; cap eviction order; action button
    retained.
  - `menu.test.ts`: open/close; show replaces previous; disabled items present.
  - `menus.test.ts`: item lists per surface; playlist edges/busy disable moves
    and remove; danger flag on delete; callbacks invoked once.
  - `shortcuts.test.ts`: every key map; typing guard; repeat handling; Esc
    precedence incl. `overlayOpen`; `isTypingTarget`.
  - `errors.test.ts`: known mappings and fallback.
  - `nav.test.ts`: push/no-op dedupe, forward truncation, back/forward bounds,
    `canBack`/`canForward`, `routeForHomeItem` mapping.
  - Migrated: `favorites.test.ts` and `playlists.test.ts` assert against a fake
    `Toasts`; `notice.test.ts` deleted.
- **svelte-check:** 0 errors / 0 warnings.
- **Manual smoke (`docs/superpowers/plans/notes/plan8-smoke.md`):** shortcuts
  while typing and in dialogs; mouse side buttons; menu positioning near screen
  edges + keyboard menu nav; toast stack/severity/persistence; history across
  views and queue rail independence; Home card navigation; pager spinner;
  QueueRail clear confirm; modal focus behavior.
- **Gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui test`, `npm --prefix ui run check`.

## 10. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Task | Exit criteria |
|---|---|---|
| P8-1 | Toasts store + host + migration | `toasts.test.ts` green; favorites/playlists tests migrated; strips gone |
| P8-2 | Menu store + component + builders | `menu.test.ts`/`menus.test.ts` green; TrackTable/QueueRail/Library wired |
| P8-3 | Shortcuts + mouse buttons + Settings list | `shortcuts.test.ts` green; host executes all actions; list rendered |
| P8-4 | Nav history + Sidebar buttons | `nav.test.ts` green; buttons and shortcuts work; mouse buttons wired |
| P8-5 | Loading/Empty/Error consistency + polish | views adopted; `errors.test.ts` green; modal/picker/Home/Queue polish |
| P8-6 | Verification | Gates green + smoke checklist recorded |

## 11. Performance Impact (PM-D1)

- Window listeners: one `keydown`, one `mousedown`, one `auxclick` in App for the
  session; menu listeners attach only while a menu is open.
- Toast timers: at most 3 concurrent; cleared on dismiss.
- Hosts (`ToastHost`, `ContextMenu`) render nothing when closed; no polling, no
  new per-row reactive work beyond one contextmenu handler.
- History is an in-memory array of route objects; no persistence, no I/O.

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `Space`/arrows hijack focused buttons or scrolling | Typing/interactive-target guard; `preventDefault` only when handled; smoke covers dialog buttons |
| `Ctrl+F` opens the WebView find bar | `preventDefault` on the handled shortcut; smoke verifies |
| Mouse side buttons swallowed or OS-remapped | Handle `mousedown`/`auxclick` button 3/4; `Alt+←/→` fallback; smoke verifies both |
| Esc double-handling (modal + queue rail) | `overlayOpen` guard from a `[role="dialog"]` check; dialog owns Escape |
| Favorites API churn breaks Plan 6 tests | Migration is mechanical: inject a fake `Toasts`; assertions move to toast calls |
| Home item type guessing routes to the wrong view | Conservative mapping (only PLAYLIST/EDITORIAL and ALBUM with matching id shape); unknown types inert; smoke checks real feed |
| Toast stack obscures content | Top-right, max 3, dismiss per toast; errors intentionally sticky |
| Context menu clipped off-screen | Viewport clamp measured after mount; smoke checks edges |

## 13. Decisions Log (Plan 8)

| # | Decision | Rationale |
|---|---|---|
| P8-D1 | Track 2a splits: Plan 8 foundations, Plan 9 data interaction. | Foundations (toasts/menus/shortcuts/history) are reused by the data surfaces; each plan stays reviewable. |
| P8-D2 | Shortcuts are playback + app essentials, fixed map, documented in Settings, no customization. | Covers daily use without a binding system; the palette/configurability stays deferred. |
| P8-D3 | Context menus are additive; inline buttons stay. | No discoverability loss and no churn to what Plan 7 shipped; menus are the home for future actions. |
| P8-D4 | Stacked toasts (max 3); errors persist; optional action button. | One outcome surface with correct severity; errors should not vanish before being read. |
| P8-D5 | History is an in-session stack with Sidebar buttons, `Alt+←/→`, and mouse side buttons. | Desktop expectations. **Superseded 2026-10-02:** route history/index now persist to localStorage and restore on launch (`createNav(window.localStorage)`); logout clears them. Queue/session restore stays in Track 10. |
| P8-D6 | Logic-only testing; no DOM test environment or new packages. | Matches the repo convention; all new behavior lives in pure stores/modules; components are covered by svelte-check + smoke. |
| P8-D7 | `createFavorites` migrates to toasts and loses its own error state. | One toast surface; removes the second strip and the App timer effect. |
| P8-D8 | Load failures inline; action outcomes toasts. | Load errors need Retry in place; transient outcomes should not replace view content. |
| P8-D9 | Mouse side buttons handled via `mousedown`/`auxclick` button 3/4. | Chromium/WebView2 fire these; OS remapping is covered by the shortcut fallback. |
| P8-D10 | Home cards navigate only for playlist-ish and album items. | No routes exist for tracks/mixes; guessing would land on the wrong view. |

## 14. Pre-spec Research Notes (audit, 2026-09-25)

- Keyboard handling in the UI is limited to Enter-in-search (`Search.svelte:36`)
  and Escape-in-modal (`Modal.svelte:10-14`); no global handlers exist.
- No `contextmenu`, popover, dropdown, or menu components anywhere.
- `TrackTable` headers are static; no sort/filter (Plan 9).
- `nav.goto` is a plain assignment; no history (`nav.svelte.ts:23`).
- Two outcome strips exist (`App.svelte:146-151`, `:152-157`) with separate
  dismiss paths; favorites errors have their own timer effect (`App.svelte:77-81`).
- Loading/empty/error presentation diverges across views (Search "Searching…",
  Settings bare red text, missing Search/Artist empty states).
- Backend error strings (Display): `forbidden`, `rate limited`, `unauthorized`,
  `not found: {path}`, `api error {status}: {message}` (`error.rs:5-23`).
- Home items carry `item_type` from the module (`PLAYLIST_LIST`, `TRACK_LIST`,
  `EDITORIAL`, …) with numeric or uuid ids (`home.rs:88-109`); only
  playlist/editorial and album items have routes.
- QueueRail already uses HTML5 drag-and-drop (`QueueRail.svelte:22-26`), a
  precedent for Plan 9's playlist reorder.

## 15. Out of Scope (tracked for later)

- **Plan 9 (Track 2a, second slice):** sortable/filterable tables,
  search-as-you-type, playlist drag-and-drop, remaining virtualization.
- **Track 2b:** visual identity, layout/IA, expanded now-playing, density,
  responsive window fit.
- **Track 3:** performance harness and budgets; navigation/session persistence.
- **Later:** command palette, customizable shortcuts, toast history/undo,
  global media keys (Track 6).
