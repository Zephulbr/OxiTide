# Plan 9 — Data Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every track list sortable and filterable, search live as you type, playlists drag-reorderable with optimistic moves, and the queue/library lists virtualized.

**Architecture:** Four new pure modules carry all logic (`virtual.ts` windowing math, `trackView.svelte.ts` sort/filter + load-all lifecycle, `search.svelte.ts` debounced latest-wins search, `drag.ts` drop math + container action), each unit-tested in the existing vitest node setup. `TrackTable` takes a required `TrackView` instead of a raw array, so sort/filter/playlist edits share one visible order. `pager` gains `moveItem` for optimistic reorders; a new `VirtualList.svelte` windows the queue and library lists. No Rust changes.

**Tech Stack:** Svelte 5 runes (`$state`, `$derived`, props, actions), TypeScript, vitest (node environment, fake timers). No new crates, no new npm packages, no DOM test environment.

**Spec:** `docs/superpowers/specs/2026-09-25-oxitide-plan9-data-interaction-design.md`

## Global Constraints

- Svelte 5 runes only: no Svelte 4 stores, no `export let`.
- No new crates and no new npm packages, including no DOM test environment (P8-D6); component behavior is covered by `svelte-check` and the smoke checklist.
- `verbatimModuleSyntax` is on: type-only imports must use `import type` (or inline `type` specifiers).
- Version stays 0.2.0; no packaging or config changes. No Rust files change.
- Row heights are load-bearing: TrackTable/Album-library 36 px, QueueRail 40 px, Library playlists 44 px.
- Moves (↑/↓, menu Move, drag) are available only in original order, while `pager.done`, and not busy; Remove/Favorite stay available under sort/filter via `view.sourceIndex`.
- Playback follows the visible order (`view.items`); engaging sort/filter calls `pager.ensureAll()` first and never shows a partial order.
- Logic-only testing: every new behavior lives in a pure module with a vitest suite; components are covered by `svelte-check` + `plan9-smoke.md`.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **Engaging sort/filter before pages finish loading:** the table must show "Loading all tracks…" and then a complete, correctly ordered list — never a partially sorted window. Task 3 tests the lifecycle; Task 11 smoke checks it on a 2k playlist.
- **Remove while sorted/filtered on a playlist with duplicate tracks:** the right occurrence must leave the playlist. Task 3 tests `sourceIndex` with duplicates; Task 11 smoke removes a filtered row.
- **Fast typing in Search:** an older response must never replace a newer query's results. Task 5 tests the out-of-order resolution directly.
- **Dropping a playlist row onto itself or just below the last row:** must be a no-op with no API call or refresh. Task 7 tests `normalizeDrop`; Task 8 guards `from === to`; Task 11 smoke checks the last-row drop.
- **A filtered view that matches nothing vs an empty source list:** distinct copy ("No tracks match …" vs "No tracks"). Task 11 smoke verifies both.

---

### Task 1: Windowed list math

**Files:**
- Create: `ui/src/lib/virtual.ts`
- Create: `ui/src/lib/virtual.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `WindowRange { first: number; last: number; padTop: number; padBottom: number }`; `windowRange(total: number, scrollTop: number, viewport: number, rowHeight: number, overscan?: number): WindowRange` (overscan default 8).

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/virtual.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { windowRange } from "./virtual";

describe("windowRange", () => {
  it("starts at the top with a full window", () => {
    const range = windowRange(1000, 0, 360, 36);
    expect(range).toEqual({
      first: 0,
      last: 26,
      padTop: 0,
      padBottom: (1000 - 26) * 36,
    });
  });

  it("scrolls the window with overscan", () => {
    const range = windowRange(1000, 3600, 360, 36);
    expect(range.first).toBe(92);
    expect(range.last).toBe(118);
    expect(range.padTop).toBe(92 * 36);
    expect(range.padBottom).toBe((1000 - 118) * 36);
  });

  it("clamps to the end so a short list keeps its rows visible", () => {
    const range = windowRange(8, 10_000, 360, 36);
    expect(range.first).toBe(0);
    expect(range.last).toBe(8);
    expect(range.padBottom).toBe(0);
  });

  it("clamps a shrunk list back into view", () => {
    const range = windowRange(30, 3600, 360, 36);
    expect(range.first).toBe(4);
    expect(range.last).toBe(30);
  });

  it("handles an empty list", () => {
    expect(windowRange(0, 0, 360, 36)).toEqual({
      first: 0,
      last: 0,
      padTop: 0,
      padBottom: 0,
    });
  });

  it("renders at least one row before the viewport is measured", () => {
    expect(windowRange(100, 0, 0, 36).last).toBeGreaterThan(0);
  });

  it("keeps spacers plus visible rows equal to the content height", () => {
    const total = 1000;
    const rowHeight = 36;
    const range = windowRange(total, 1234, 360, rowHeight);
    expect(
      range.padTop + (range.last - range.first) * rowHeight + range.padBottom,
    ).toBe(total * rowHeight);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/virtual.test.ts`
Expected: FAIL — cannot resolve `./virtual`.

- [ ] **Step 3: Write the minimal implementation**

Create `ui/src/lib/virtual.ts`:

```ts
export interface WindowRange {
  first: number;
  last: number;
  padTop: number;
  padBottom: number;
}

/// Slice of a fixed-row-height list to render, with spacer heights for the
/// rows omitted above and below. `first` is clamped so a list that shrinks
/// (refresh, removal) cannot leave the window pointing past the end.
export function windowRange(
  total: number,
  scrollTop: number,
  viewport: number,
  rowHeight: number,
  overscan = 8,
): WindowRange {
  const visible = Math.max(1, Math.ceil(viewport / rowHeight) + overscan * 2);
  const first = Math.min(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan),
    Math.max(0, total - visible),
  );
  const last = Math.min(total, first + visible);
  return {
    first,
    last,
    padTop: first * rowHeight,
    padBottom: Math.max(0, (total - last) * rowHeight),
  };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/virtual.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/virtual.ts ui/src/lib/virtual.test.ts
git commit -m "feat(ui): add the windowed list math helper"
```

---

### Task 2: Optimistic `pager.moveItem`

**Files:**
- Modify: `ui/src/lib/pager.svelte.ts` (interface after `removeItem`; implementation after `applyRemoval`; return object)
- Test: `ui/src/lib/pager.test.ts` (append a new `describe` before the final closing of the file; the file ends with `});` for `describe("track pager", ...)`)

**Interfaces:**
- Consumes: the existing `items` state, `publish()`, cache helpers in `pager.svelte.ts`.
- Produces: `Pager.moveItem(from: number, to: number): void`.

- [ ] **Step 1: Write the failing tests**

Append to `ui/src/lib/pager.test.ts` (new top-level describe, after the existing one):

```ts
describe("pager moveItem", () => {
  it("moves a row down to its final index", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3), track(4)], offset, limit, 4),
    );
    const pager = createTrackPager("move-down", load, { pageSize: 4, prefetch: false });
    await pager.loadMore();

    pager.moveItem(0, 2);

    expect(ids(pager.items)).toEqual([2, 3, 1, 4]);
    expect(pager.total).toBe(4);
    expect(pager.done).toBe(true);
  });

  it("moves a row up and publishes the cache", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3)], offset, limit, 3),
    );
    const pager = createTrackPager("move-up", load, { pageSize: 3, prefetch: false });
    await pager.loadMore();

    pager.moveItem(2, 0);

    expect(ids(pager.items)).toEqual([3, 1, 2]);

    const rehydrated = createTrackPager("move-up", vi.fn(), {
      pageSize: 3,
      prefetch: false,
    });
    expect(ids(rehydrated.items)).toEqual([3, 1, 2]);
  });

  it("ignores equal or out-of-range indices", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("move-noop", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    pager.moveItem(1, 1);
    pager.moveItem(-1, 0);
    pager.moveItem(0, 2);
    pager.moveItem(2, 0);

    expect(ids(pager.items)).toEqual([1, 2]);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: FAIL — `pager.moveItem is not a function`.

- [ ] **Step 3: Add `moveItem` to the Pager interface**

In `ui/src/lib/pager.svelte.ts`, add to the `Pager` interface after `removeItem`:

```ts
  /// Reorder one loaded row (drag/arrow move). `to` is the final index within
  /// the loaded rows; equal or out-of-range indices are a no-op. Totals and
  /// offsets are unchanged, so this is safe while a page fetch is in flight.
  moveItem(from: number, to: number): void;
```

- [ ] **Step 4: Implement `moveItem`**

In `ui/src/lib/pager.svelte.ts`, add after `applyRemoval`:

```ts
  function moveItem(from: number, to: number) {
    if (from === to) return;
    if (from < 0 || from >= items.length) return;
    if (to < 0 || to >= items.length) return;
    const next = [...items];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    items = next;
    publish();
  }
```

Add `moveItem` to the returned object after `removeItem`:

```ts
    removeItem,
    moveItem,
  };
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: PASS (all existing + 3 new).

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/pager.svelte.ts ui/src/lib/pager.test.ts
git commit -m "feat(ui): add optimistic moveItem to the track pager"
```

---

### Task 3: TrackView store

**Files:**
- Create: `ui/src/lib/trackView.svelte.ts`
- Create: `ui/src/lib/trackView.test.ts`

**Interfaces:**
- Consumes: `Track` from `./api`.
- Produces: `TrackSort = "original" | "title" | "duration"`; `SortDirection = "asc" | "desc"`; `TrackView` (filter, sort, direction, engaged, preparing, error, items, setFilter, toggleSort, clearSort, retry, sourceIndex); `createTrackView(source: () => Track[], ensureAll?: () => Promise<void>): TrackView`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/trackView.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import type { Track } from "./api";
import { createTrackView } from "./trackView.svelte";

function track(id: number, title: string, duration = 100, artist = "A"): Track {
  return {
    id,
    title,
    duration,
    artists: [{ id: 1, name: artist }],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

const SOURCE = [
  track(1, "Zebra", 300, "Muse"),
  track(2, "alpha", 100, "Radiohead"),
  track(3, "Beta", 200, "muse"),
];

function ids(tracks: Track[]): number[] {
  return tracks.map((t) => t.id);
}

describe("track view sort", () => {
  it("stays in source order until engaged", () => {
    const view = createTrackView(() => SOURCE);

    expect(view.sort).toBe("original");
    expect(view.engaged).toBe(false);
    expect(ids(view.items)).toEqual([1, 2, 3]);
  });

  it("cycles a column asc, desc, then back to original", () => {
    const view = createTrackView(() => SOURCE);

    view.toggleSort("title");
    expect(ids(view.items)).toEqual([2, 3, 1]);

    view.toggleSort("title");
    expect(ids(view.items)).toEqual([1, 3, 2]);

    view.toggleSort("title");
    expect(view.sort).toBe("original");
    expect(ids(view.items)).toEqual([1, 2, 3]);
  });

  it("switches columns at ascending and keeps a stable tie order", () => {
    const tied = [track(1, "Same", 200), track(2, "Same", 100), track(3, "Same", 300)];
    const view = createTrackView(() => tied);

    view.toggleSort("title");
    expect(ids(view.items)).toEqual([1, 2, 3]);

    view.toggleSort("duration");
    expect(ids(view.items)).toEqual([2, 1, 3]);
  });

  it("sorts durations numerically", () => {
    const view = createTrackView(() => SOURCE);

    view.toggleSort("duration");

    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("clearSort restores the original order without touching the filter", () => {
    const view = createTrackView(() => SOURCE);
    view.setFilter("a");
    view.toggleSort("title");

    view.clearSort();

    expect(view.sort).toBe("original");
    expect(view.filter).toBe("a");
  });
});

describe("track view filter", () => {
  it("matches title and artist names case-insensitively", () => {
    const view = createTrackView(() => SOURCE);

    view.setFilter("mus");

    expect(ids(view.items)).toEqual([1, 3]);
  });

  it("matches the title too", () => {
    const view = createTrackView(() => SOURCE);

    view.setFilter("alp");

    expect(ids(view.items)).toEqual([2]);
  });

  it("trims the filter and ignores an empty one", () => {
    const view = createTrackView(() => SOURCE);

    view.setFilter("  ");
    expect(view.engaged).toBe(false);

    view.setFilter("  beta ");
    expect(ids(view.items)).toEqual([3]);
  });

  it("sorts a filtered subset", () => {
    const view = createTrackView(() => SOURCE);
    view.setFilter("muse");
    view.toggleSort("title");

    expect(ids(view.items)).toEqual([3, 1]);
  });
});

describe("track view loading lifecycle", () => {
  it("calls ensureAll once when engagement begins", async () => {
    const ensureAll = vi.fn(async () => {});
    const view = createTrackView(() => SOURCE, ensureAll);

    view.setFilter("a");
    await Promise.resolve();
    view.setFilter("ab");

    expect(ensureAll).toHaveBeenCalledTimes(1);
    expect(view.preparing).toBe(false);
  });

  it("serves source order while preparing, then the sorted order", async () => {
    let release: () => void = () => {};
    const ensureAll = vi.fn(() => new Promise<void>((resolve) => (release = resolve)));
    const view = createTrackView(() => SOURCE, ensureAll);

    view.toggleSort("title");
    expect(view.preparing).toBe(true);
    expect(ids(view.items)).toEqual([1, 2, 3]);

    release();
    await Promise.resolve();
    await Promise.resolve();
    expect(view.preparing).toBe(false);
    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("reports an ensureAll failure and retries on demand", async () => {
    const ensureAll = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce(undefined);
    const view = createTrackView(() => SOURCE, ensureAll);

    view.toggleSort("title");
    await Promise.resolve();
    await Promise.resolve();
    expect(view.error).toBe("Error: offline");
    expect(ids(view.items)).toEqual([1, 2, 3]);

    view.retry();
    await Promise.resolve();
    await Promise.resolve();
    expect(view.error).toBe("");
    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("is ready immediately when no loader is supplied", () => {
    const view = createTrackView(() => SOURCE);

    view.toggleSort("title");

    expect(view.preparing).toBe(false);
    expect(ids(view.items)).toEqual([2, 3, 1]);
  });

  it("clears preparing and error when the view disengages", async () => {
    const ensureAll = vi.fn(() => new Promise<void>(() => {}));
    const view = createTrackView(() => SOURCE, ensureAll);

    view.setFilter("a");
    expect(view.preparing).toBe(true);

    view.setFilter("");

    expect(view.engaged).toBe(false);
    expect(view.preparing).toBe(false);
    expect(view.error).toBe("");
  });
});

describe("track view sourceIndex", () => {
  it("maps a visible row back to its source position", () => {
    const view = createTrackView(() => SOURCE);
    view.toggleSort("duration");

    expect(ids(view.items)).toEqual([2, 3, 1]);
    expect(view.sourceIndex(0)).toBe(1);
    expect(view.sourceIndex(2)).toBe(0);
  });

  it("keeps duplicate tracks addressable by identity", () => {
    const source = [track(7, "Dup"), track(7, "Dup"), track(8, "Other")];
    const view = createTrackView(() => source);
    view.setFilter("dup");

    expect(view.items).toHaveLength(2);
    expect(view.sourceIndex(0)).toBe(0);
    expect(view.sourceIndex(1)).toBe(1);
  });

  it("returns -1 for an index outside the visible rows", () => {
    const view = createTrackView(() => SOURCE);

    expect(view.sourceIndex(9)).toBe(-1);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/trackView.test.ts`
Expected: FAIL — cannot resolve `./trackView.svelte`.

- [ ] **Step 3: Write the implementation**

Create `ui/src/lib/trackView.svelte.ts`:

```ts
import type { Track } from "./api";

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
  /// Back to original order for the sort only; the filter is untouched.
  clearSort(): void;
  retry(): void;
  /// Index of items[visibleIndex] within the source list, by identity;
  /// -1 when absent.
  sourceIndex(visibleIndex: number): number;
}

function matches(track: Track, needle: string): boolean {
  if (track.title.toLowerCase().includes(needle)) return true;
  return track.artists.some((artist) => artist.name.toLowerCase().includes(needle));
}

function applyView(
  list: Track[],
  filter: string,
  sort: TrackSort,
  direction: SortDirection,
): Track[] {
  const needle = filter.trim().toLowerCase();
  const filtered =
    needle === "" ? list : list.filter((track) => matches(track, needle));
  if (sort === "original") return filtered;
  const factor = direction === "asc" ? 1 : -1;
  return filtered
    .map((track, index) => ({ track, index }))
    .sort((a, b) => {
      const cmp =
        sort === "title"
          ? a.track.title.localeCompare(b.track.title, undefined, { sensitivity: "base" })
          : a.track.duration - b.track.duration;
      return cmp !== 0 ? cmp * factor : a.index - b.index;
    })
    .map((entry) => entry.track);
}

/// Sort/filter state over a track source. Engaging a non-original order (or a
/// filter) first awaits `ensureAll` so the operation covers the complete list;
/// until it resolves the view reports `preparing` and serves source order.
export function createTrackView(
  source: () => Track[],
  ensureAll?: () => Promise<void>,
): TrackView {
  let filter = $state("");
  let sort = $state<TrackSort>("original");
  let direction = $state<SortDirection>("asc");
  let preparing = $state(false);
  let error = $state("");
  let ready = $state(false);
  let engageToken = 0;

  const engaged = $derived(filter.trim() !== "" || sort !== "original");

  const items = $derived.by(() => {
    const list = source();
    if (!engaged || !ready) return list;
    return applyView(list, filter, sort, direction);
  });

  async function prepare() {
    if (ready) return;
    if (!ensureAll) {
      ready = true;
      return;
    }
    const token = ++engageToken;
    preparing = true;
    error = "";
    try {
      await ensureAll();
      if (token !== engageToken) return;
      ready = true;
    } catch (e) {
      if (token !== engageToken) return;
      error = String(e);
    } finally {
      if (token === engageToken) preparing = false;
    }
  }

  function engage() {
    if (engaged) void prepare();
  }

  function disengageIfIdle() {
    if (filter.trim() === "" && sort === "original") {
      engageToken += 1;
      ready = false;
      preparing = false;
      error = "";
    }
  }

  function setFilter(query: string) {
    filter = query;
    if (filter.trim() === "" && sort === "original") {
      disengageIfIdle();
      return;
    }
    engage();
  }

  function toggleSort(key: Exclude<TrackSort, "original">) {
    if (sort !== key) {
      sort = key;
      direction = "asc";
    } else if (direction === "asc") {
      direction = "desc";
    } else {
      sort = "original";
      direction = "asc";
    }
    if (sort === "original" && filter.trim() === "") {
      disengageIfIdle();
      return;
    }
    engage();
  }

  function clearSort() {
    sort = "original";
    direction = "asc";
    if (filter.trim() === "") disengageIfIdle();
  }

  function retry() {
    ready = false;
    error = "";
    void prepare();
  }

  function sourceIndex(visibleIndex: number): number {
    const track = items[visibleIndex];
    if (!track) return -1;
    return source().indexOf(track);
  }

  return {
    get filter() {
      return filter;
    },
    get sort() {
      return sort;
    },
    get direction() {
      return direction;
    },
    get engaged() {
      return engaged;
    },
    get preparing() {
      return preparing;
    },
    get error() {
      return error;
    },
    get items() {
      return items;
    },
    setFilter,
    toggleSort,
    clearSort,
    retry,
    sourceIndex,
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/trackView.test.ts`
Expected: PASS (17 tests).

If a lifecycle test races (microtask counts differ), replace the two `await Promise.resolve()` lines with `await vi.waitFor(() => expect(view.preparing).toBe(false))` for the success cases and `await vi.waitFor(() => expect(view.error).not.toBe(""))` for the failure case; keep the assertions unchanged.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/trackView.svelte.ts ui/src/lib/trackView.test.ts
git commit -m "feat(ui): add the track view store for sorting and filtering"
```

---

### Task 4: Sortable/filterable TrackTable and view migration

**Files:**
- Modify: `ui/src/lib/menus.ts` (playlist context gains `canMove`)
- Modify: `ui/src/lib/menus.test.ts` (canMove cases)
- Modify: `ui/src/lib/components/TrackTable.svelte` (full replacement below)
- Modify: `ui/src/views/Album.svelte`, `ui/src/views/Search.svelte`, `ui/src/views/Library.svelte`, `ui/src/views/Playlist.svelte`

**Interfaces:**
- Consumes: `TrackView`, `createTrackView` (Task 3), `windowRange` (Task 1), `Menu` items.
- Produces: `TrackTable` props `{ view: TrackView; favorites; onPlay?; onPlayAll?; onEndReached?; onAddToPlaylist?; playlistActions?; maxHeight? }` (no `tracks`, no `player`); `TrackMenuContext.playlist` gains `canMove?: boolean`.

- [ ] **Step 1: Add the failing menu tests**

In `ui/src/lib/menus.test.ts`, extend `playlistCtx` to accept `canMove`:

```ts
function playlistCtx(overrides: { busy?: boolean; disabled?: boolean; canMove?: boolean } = {}) {
  return {
    onRemove: vi.fn(),
    onMove: vi.fn(),
    busy: false,
    disabled: false,
    ...overrides,
  };
}
```

Append a test inside `describe("trackMenuItems")` after the "omits editing items while busy or disabled" test:

```ts
  it("keeps remove and favorite but omits the moves while canMove is false", () => {
    const items = trackMenuItems(
      baseCtx({ index: 1, trackCount: 3, playlist: playlistCtx({ canMove: false }) }),
    );

    expect(items.map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
      "remove",
    ]);
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: FAIL — the item list still contains `move-up`/`move-down`.

- [ ] **Step 3: Implement `canMove`**

In `ui/src/lib/menus.ts`, change the playlist context type:

```ts
  playlist?: {
    onRemove(index: number): void;
    onMove(index: number, delta: -1 | 1): void;
    busy: boolean;
    disabled: boolean;
    /// false omits move-up/move-down but keeps Remove (sorted/filtered views).
    canMove?: boolean;
  };
```

Change the playlist block in `trackMenuItems`:

```ts
  const playlist = ctx.playlist;
  if (playlist && !playlist.busy && !playlist.disabled) {
    if (playlist.canMove !== false) {
      items.push(
        {
          id: "move-up",
          label: "Move up",
          disabled: ctx.index === 0,
          run: () => playlist.onMove(ctx.index, -1),
        },
        {
          id: "move-down",
          label: "Move down",
          disabled: ctx.index === ctx.trackCount - 1,
          run: () => playlist.onMove(ctx.index, 1),
        },
      );
    }
    items.push({ id: "remove", label: "Remove", run: () => playlist.onRemove(ctx.index) });
  }
  return items;
```

- [ ] **Step 4: Run the menu tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: PASS.

- [ ] **Step 5: Replace TrackTable**

Replace all of `ui/src/lib/components/TrackTable.svelte` with:

```svelte
<script lang="ts">
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import { formatDuration } from "../format";
  import { menuFor } from "../menu.svelte";
  import { trackMenuItems } from "../menus";
  import { windowRange } from "../virtual";
  import type { Favorites } from "../favorites.svelte";
  import type { TrackView } from "../trackView.svelte";
  import Loading from "./Loading.svelte";
  let {
    view,
    favorites,
    onPlay,
    onPlayAll,
    onEndReached,
    onAddToPlaylist,
    playlistActions,
    maxHeight = "min(60vh, 560px)",
  }: {
    view: TrackView;
    favorites: Favorites;
    onPlay?: (index: number) => void | Promise<void>;
    onPlayAll?: () => void | Promise<void>;
    /// Called when the list is scrolled near its end; used for lazy paging.
    onEndReached?: () => void;
    /// Renders the "add to playlist" row action. Uses ⊕ to stay distinct
    /// from the "＋" queue action.
    onAddToPlaylist?: (track: Track) => void;
    /// Playlist-view row editing (move up/down, remove). `disabled` is set by
    /// the parent until the pager has the complete list; `onDrop` (Task 8)
    /// receives absolute source-order indices from drag-and-drop.
    playlistActions?: {
      onRemove(index: number): void;
      onMove(index: number, delta: -1 | 1): void;
      busy: boolean;
      disabled: boolean;
    };
    maxHeight?: string;
  } = $props();

  /// Row height in px; keep in sync with the `.row` style below.
  const ROW_HEIGHT = 36;
  const OVERSCAN = 8;
  const END_THRESHOLD = 400;

  let scrollTop = $state(0);
  let viewport = $state(0);
  let loadingAll = $state(false);

  const rows = $derived(view.items);
  const range = $derived(windowRange(rows.length, scrollTop, viewport, ROW_HEIGHT, OVERSCAN));
  const slice = $derived(rows.slice(range.first, range.last));

  function indicator(key: "title" | "duration"): string {
    if (view.sort !== key) return "";
    return view.direction === "asc" ? "▲" : "▼";
  }

  async function play(index: number) {
    if (onPlay) {
      loadingAll = true;
      try {
        await onPlay(index);
      } finally {
        loadingAll = false;
      }
      return;
    }
    api.playTracks(rows.map(trackToQueueItem), index);
  }

  async function playAll() {
    if (onPlayAll) {
      loadingAll = true;
      try {
        await onPlayAll();
      } finally {
        loadingAll = false;
      }
      return;
    }
    play(0);
  }

  function onScroll(event: Event) {
    const element = event.currentTarget as HTMLElement;
    scrollTop = element.scrollTop;
    if (element.scrollHeight - element.scrollTop - element.clientHeight < END_THRESHOLD) {
      onEndReached?.();
    }
  }
</script>

<div class="table-tools">
  <input
    class="filter"
    placeholder="Filter tracks"
    value={view.filter}
    oninput={(event) => view.setFilter(event.currentTarget.value)}
  />
  {#if view.filter.trim() !== ""}
    <button onclick={() => view.setFilter("")} aria-label="Clear filter">✕</button>
  {/if}
  {#if view.preparing}
    <Loading label="Loading all tracks…" />
  {/if}
  {#if view.error}
    <span class="muted">{view.error}</span>
    <button onclick={() => view.retry()}>Retry</button>
  {/if}
</div>

<div
  class="table-scroll"
  style="max-height:{maxHeight}"
  bind:clientHeight={viewport}
  onscroll={onScroll}
>
  <table>
    <thead>
      <tr>
        <th style="text-align:left"><button onclick={playAll} disabled={loadingAll}>{loadingAll ? "Loading all…" : "Play all"}</button></th>
        <th style="text-align:left">
          <button class="sort" title="Original order" onclick={() => view.clearSort()}>#</button>
        </th>
        <th style="text-align:left">
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("title")}>
            Title {indicator("title")}
          </button>
        </th>
        <th style="text-align:right">
          <button class="sort" disabled={view.preparing} onclick={() => view.toggleSort("duration")}>
            Time {indicator("duration")}
          </button>
        </th>
      </tr>
    </thead>
    <tbody>
      {#if range.padTop > 0}
        <tr aria-hidden="true"><td colspan="4" style="height:{range.padTop}px;padding:0;border:0"></td></tr>
      {/if}
      {#each slice as t, i (`${t.id}-${range.first + i}`)}
        {@const index = range.first + i}
        <tr
          class="row"
          use:menuFor={() =>
            trackMenuItems({
              track: t,
              index,
              trackCount: rows.length,
              favorites,
              play: () => play(index),
              addToQueue: () => api.queueAdd([trackToQueueItem(t)]),
              addToPlaylist: () => onAddToPlaylist?.(t),
              playlist: playlistActions && { ...playlistActions, canMove: !view.engaged },
            })}
        >
          <td>
            <button onclick={() => play(index)} disabled={loadingAll} aria-label="Play">▶</button>
            <button
              onclick={() => api.queueAdd([trackToQueueItem(t)])}
              aria-label="Add to queue"
              title="Add to queue"
            >＋</button>
            {#if onAddToPlaylist}
              <button
                onclick={() => onAddToPlaylist(t)}
                aria-label="Add to playlist"
                title="Add to playlist"
              >⊕</button>
            {/if}
            <button
              class:on={favorites.hasTrack(t.id)}
              onclick={() => favorites.toggleTrack(t.id)}
              aria-label={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
              title={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
            >{favorites.hasTrack(t.id) ? "♥" : "♡"}</button>
            {#if playlistActions}
              {#if !view.engaged}
                <button
                  onclick={() => playlistActions.onMove(index, -1)}
                  disabled={playlistActions.busy || playlistActions.disabled || index === 0}
                  aria-label="Move up"
                  title="Move up"
                >↑</button>
                <button
                  onclick={() => playlistActions.onMove(index, 1)}
                  disabled={playlistActions.busy || playlistActions.disabled || index === rows.length - 1}
                  aria-label="Move down"
                  title="Move down"
                >↓</button>
              {/if}
              <button
                onclick={() => playlistActions.onRemove(index)}
                disabled={playlistActions.busy || playlistActions.disabled}
                aria-label="Remove from playlist"
                title="Remove from playlist"
              >✕</button>
            {/if}
          </td>
          <td class="muted">{index + 1}</td>
          <td class="title-cell"><div class="title-text">{t.title}<span class="muted"> · {t.artists.map((a) => a.name).join(", ")}</span></div></td>
          <td style="text-align:right" class="muted">{formatDuration(t.duration)}</td>
        </tr>
      {/each}
      {#if range.padBottom > 0}
        <tr aria-hidden="true"><td colspan="4" style="height:{range.padBottom}px;padding:0;border:0"></td></tr>
      {/if}
      {#if rows.length === 0}
        <tr><td colspan="4" class="muted">{view.filter.trim() ? `No tracks match “${view.filter.trim()}”` : "No tracks"}</td></tr>
      {/if}
    </tbody>
  </table>
</div>

<style>
  .table-tools {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .table-tools .filter {
    flex: 0 1 260px;
  }
  .table-scroll {
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .sort {
    background: transparent;
    border: 0;
    color: inherit;
    padding: 0;
    cursor: pointer;
  }
  .row {
    height: 36px;
    border-bottom: 1px solid var(--line);
  }
  .title-cell {
    max-width: 0;
    width: 100%;
  }
  .title-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .on {
    color: var(--accent);
  }
</style>
```

- [ ] **Step 6: Migrate Album.svelte**

In `ui/src/views/Album.svelte`:

Add the import:

```ts
  import { createTrackView } from "../lib/trackView.svelte";
```

Add the view after the `error` state declaration:

```ts
  const view = createTrackView(() => tracks);
```

Replace the `TrackTable` invocation:

```svelte
  <TrackTable
    {view}
    {favorites}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
```

- [ ] **Step 7: Migrate Search.svelte**

In `ui/src/views/Search.svelte`:

Add the import:

```ts
  import { createTrackView } from "../lib/trackView.svelte";
```

Add the view after the `searchedQuery` state declaration:

```ts
  const resultsView = createTrackView(() => results?.tracks.items ?? []);
```

Replace the `TrackTable` invocation:

```svelte
    <TrackTable
      view={resultsView}
      {favorites}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    />
```

- [ ] **Step 8: Migrate Playlist.svelte**

In `ui/src/views/Playlist.svelte`:

Add the imports:

```ts
  import { createTrackView } from "../lib/trackView.svelte";
```

After the `pager` derived declaration, add:

```ts
  async function ensureAllTracks() {
    await pager.ensureAll();
    if (pager.error) throw new Error(pager.error);
  }

  const view = $derived(createTrackView(() => pager.items, ensureAllTracks));
```

Change `removeAt` to map the visible row and add `moveTrack`:

```ts
  async function removeAt(index: number) {
    if (busy) return;
    const sourceIndex = view.sourceIndex(index);
    if (sourceIndex < 0) return;
    busy = true;
    try {
      await api.removePlaylistItems(id, [sourceIndex]);
      await pager.refresh();
      toasts.show("Removed from playlist", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function moveTrack(from: number, to: number) {
    if (busy || from === to) return;
    busy = true;
    pager.moveItem(from, to);
    try {
      await api.movePlaylistItems(id, [from], to);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      await pager.refresh();
    } finally {
      busy = false;
    }
  }

  function moveAt(index: number, delta: -1 | 1) {
    return moveTrack(index, index + delta);
  }
```

Change `playFrom`/`playAll` to build the queue from the visible order:

```ts
  async function playFrom(index: number) {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), 0);
  }
```

Replace the `TrackTable` invocation:

```svelte
  <TrackTable
    {view}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void pager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    playlistActions={{ onRemove: removeAt, onMove: moveAt, busy, disabled: !pager.done }}
  />
```

- [ ] **Step 9: Migrate Library.svelte**

In `ui/src/views/Library.svelte`:

Add the import:

```ts
  import { createTrackView } from "../lib/trackView.svelte";
```

After the `favoritesPager` declaration, add:

```ts
  async function ensureAllFavorites() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) throw new Error(favoritesPager.error);
  }

  const view = createTrackView(() => favoritesPager.items, ensureAllFavorites);
```

Change `playFrom`/`playAll` to use the visible order:

```ts
  async function playFrom(index: number) {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), 0);
  }
```

Replace the `TrackTable` invocation:

```svelte
  <TrackTable
    {view}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void favoritesPager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
```

- [ ] **Step 10: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all suites pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 11: Commit**

```bash
git add ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/lib/components/TrackTable.svelte ui/src/views/Album.svelte ui/src/views/Search.svelte ui/src/views/Playlist.svelte ui/src/views/Library.svelte
git commit -m "feat(ui): add sortable and filterable track tables"
```

---

### Task 5: Debounced live-search store

**Files:**
- Create: `ui/src/lib/search.svelte.ts`
- Create: `ui/src/lib/search.test.ts`

**Interfaces:**
- Consumes: `SearchResults` from `./api`.
- Produces: `SearchStore` (query, results, loading, error, settledQuery, setQuery, runNow, retry, reset); `createSearch(fetch: (query: string) => Promise<SearchResults>, delayMs?: number): SearchStore` (delay default 300).

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/search.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchResults } from "./api";
import { createSearch } from "./search.svelte";

function results(titles: string[]): SearchResults {
  return {
    tracks: {
      items: titles.map((title, index) => ({
        id: index + 1,
        title,
        duration: 1,
        artists: [],
        trackNumber: 1,
        volumeNumber: 1,
        explicit: false,
      })),
    },
    albums: { items: [] },
    artists: { items: [] },
    playlists: { items: [] },
  };
}

function firstTitle(search: { results: SearchResults | null }): string | undefined {
  return search.results?.tracks.items[0]?.title;
}

describe("search store", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("does not fetch below two characters and clears earlier results", async () => {
    const fetch = vi.fn(async (query: string) => results([query]));
    const search = createSearch(fetch);
    search.setQuery("aa");
    await vi.advanceTimersByTimeAsync(300);
    expect(firstTitle(search)).toBe("aa");

    search.setQuery("a");

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(search.results).toBeNull();
    expect(search.loading).toBe(false);
  });

  it("debounces keystrokes into one fetch with the latest text", async () => {
    const fetch = vi.fn(async (query: string) => results([query]));
    const search = createSearch(fetch);

    search.setQuery("ab");
    await vi.advanceTimersByTimeAsync(100);
    search.setQuery("abc");
    await vi.advanceTimersByTimeAsync(100);
    search.setQuery("abcd");
    await vi.advanceTimersByTimeAsync(300);

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch).toHaveBeenCalledWith("abcd");
  });

  it("searches immediately on Enter even for one character", async () => {
    const fetch = vi.fn(async (query: string) => results([query]));
    const search = createSearch(fetch);

    search.setQuery("a");
    search.runNow();
    await vi.advanceTimersByTimeAsync(0);

    expect(fetch).toHaveBeenCalledWith("a");
  });

  it("ignores a response that resolves after the query changed", async () => {
    let releaseFirst: (value: SearchResults) => void = () => {};
    const fetch = vi.fn((query: string) => {
      if (query === "aa") {
        return new Promise<SearchResults>((resolve) => (releaseFirst = resolve));
      }
      return Promise.resolve(results(["second"]));
    });
    const search = createSearch(fetch);

    search.setQuery("aa");
    await vi.advanceTimersByTimeAsync(300);
    search.setQuery("bb");
    await vi.advanceTimersByTimeAsync(300);
    expect(firstTitle(search)).toBe("second");

    releaseFirst(results(["first"]));
    await vi.advanceTimersByTimeAsync(0);

    expect(search.settledQuery).toBe("bb");
    expect(firstTitle(search)).toBe("second");
  });

  it("keeps the previous results visible while a new query loads", async () => {
    const fetch = vi.fn(async (query: string) => results([query]));
    const search = createSearch(fetch);
    search.setQuery("aa");
    await vi.advanceTimersByTimeAsync(300);
    expect(firstTitle(search)).toBe("aa");

    let release: (value: SearchResults) => void = () => {};
    fetch.mockImplementationOnce(() => new Promise((resolve) => (release = resolve)));
    search.setQuery("bb");
    await vi.advanceTimersByTimeAsync(300);

    expect(search.loading).toBe(true);
    expect(firstTitle(search)).toBe("aa");

    release(results(["bb"]));
    await vi.advanceTimersByTimeAsync(0);

    expect(search.loading).toBe(false);
    expect(firstTitle(search)).toBe("bb");
  });

  it("reports a failure and retries the settled query", async () => {
    const fetch = vi
      .fn<(query: string) => Promise<SearchResults>>()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce(results(["back"]));
    const search = createSearch(fetch);

    search.setQuery("aa");
    await vi.advanceTimersByTimeAsync(300);
    expect(search.error).toBe("Error: offline");
    expect(search.results).toBeNull();

    search.retry();
    await vi.advanceTimersByTimeAsync(0);

    expect(search.error).toBe("");
    expect(firstTitle(search)).toBe("back");
  });

  it("resets the query, results, and pending timer", async () => {
    const fetch = vi.fn(async (query: string) => results([query]));
    const search = createSearch(fetch);
    search.setQuery("aa");

    search.reset();
    await vi.advanceTimersByTimeAsync(300);

    expect(fetch).not.toHaveBeenCalled();
    expect(search.query).toBe("");
    expect(search.results).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/search.test.ts`
Expected: FAIL — cannot resolve `./search.svelte`.

- [ ] **Step 3: Write the implementation**

Create `ui/src/lib/search.svelte.ts`:

```ts
import type { SearchResults } from "./api";

export interface SearchStore {
  readonly query: string;
  readonly results: SearchResults | null;
  readonly loading: boolean;
  readonly error: string;
  /// Query whose results are on screen (for the no-results state).
  readonly settledQuery: string;
  /// Debounced; live searches fire at >= 2 characters.
  setQuery(query: string): void;
  /// Enter: cancels the debounce and searches immediately at >= 1 character.
  runNow(): void;
  /// Re-runs the settled query after a failure.
  retry(): void;
  /// Clears the query, results, and any pending timer.
  reset(): void;
}

const MIN_LIVE_CHARS = 2;

/// Debounced, latest-wins search state. A response only applies when its
/// request is still the newest; older results stay visible until it lands.
export function createSearch(
  fetch: (query: string) => Promise<SearchResults>,
  delayMs = 300,
): SearchStore {
  let query = $state("");
  let results = $state<SearchResults | null>(null);
  let loading = $state(false);
  let error = $state("");
  let settledQuery = $state("");
  let timer: ReturnType<typeof setTimeout> | null = null;
  let sequence = 0;

  function cancel() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  function clearResults() {
    cancel();
    sequence += 1;
    results = null;
    settledQuery = "";
    error = "";
    loading = false;
  }

  async function run(text: string) {
    const trimmed = text.trim();
    cancel();
    const token = ++sequence;
    loading = true;
    error = "";
    try {
      const next = await fetch(trimmed);
      if (token !== sequence) return;
      results = next;
      settledQuery = trimmed;
    } catch (e) {
      if (token !== sequence) return;
      results = null;
      error = String(e);
    } finally {
      if (token === sequence) loading = false;
    }
  }

  function setQuery(next: string) {
    query = next;
    if (next.trim().length < MIN_LIVE_CHARS) {
      clearResults();
      return;
    }
    cancel();
    // Invalidate any in-flight response for text the user has moved past.
    sequence += 1;
    timer = setTimeout(() => {
      timer = null;
      void run(query);
    }, delayMs);
  }

  function runNow() {
    if (query.trim().length === 0) {
      clearResults();
      return;
    }
    void run(query);
  }

  function retry() {
    void run(settledQuery || query);
  }

  function reset() {
    query = "";
    clearResults();
  }

  return {
    get query() {
      return query;
    },
    get results() {
      return results;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get settledQuery() {
      return settledQuery;
    },
    setQuery,
    runNow,
    retry,
    reset,
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/search.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/search.svelte.ts ui/src/lib/search.test.ts
git commit -m "feat(ui): add the debounced live-search store"
```

---

### Task 6: Rewire Search.svelte to the live store

**Files:**
- Modify: `ui/src/views/Search.svelte`

**Interfaces:**
- Consumes: `createSearch`, `SearchStore` (Task 5); `TrackView`/`createTrackView` (Task 3).
- Produces: a thin Search view; `#search-input` id stays (App's `Ctrl+F` focuses it).

- [ ] **Step 1: Replace the Search component**

Replace all of `ui/src/views/Search.svelte` with:

```svelte
<script lang="ts">
  import { api } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createSearch } from "../lib/search.svelte";
  import { createTrackView } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    player,
    favorites,
    playlistActions,
  }: { nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions } = $props();

  const search = createSearch(api.search);
  const resultsView = createTrackView(() => search.results?.tracks.items ?? []);

  const noResults = $derived(
    search.results !== null &&
      search.results.tracks.items.length === 0 &&
      search.results.albums.items.length === 0 &&
      search.results.artists.items.length === 0 &&
      search.results.playlists.items.length === 0,
  );
</script>

<h2>Search</h2>
<div class="search-bar">
  <input
    id="search-input"
    value={search.query}
    oninput={(event) => search.setQuery(event.currentTarget.value)}
    onkeydown={(event) => event.key === "Enter" && search.runNow()}
    placeholder="Search TIDAL"
  />
  {#if search.loading && search.results}
    <Loading label="Searching…" />
  {/if}
</div>
{#if search.loading && !search.results && !search.error}
  <Loading label="Searching…" />
{:else if search.error}
  <ErrorState message={search.error} onRetry={() => search.retry()} />
{:else if noResults}
  <EmptyState message={`No results for “${search.settledQuery}”`} />
{:else if search.results}
  <h3>Tracks</h3>
  <TrackTable
    view={resultsView}
    {favorites}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
  <h3>Albums</h3>
  <ul>
    {#each search.results.albums.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
  </ul>
  <h3>Artists</h3>
  <ul>
    {#each search.results.artists.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "artist", id: a.id })}>{a.name}</button></li>
    {/each}
  </ul>
  <h3>Playlists</h3>
  <ul>
    {#each search.results.playlists.items as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
  </ul>
{/if}

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
```

Note: Task 4 already added a `resultsView` const to `Search.svelte`; this replacement supersedes it (the view is recreated here alongside the store). Keep the `player` prop even though TrackTable no longer takes it; App still passes it.

- [ ] **Step 2: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all suites pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 3: Commit**

```bash
git add ui/src/views/Search.svelte
git commit -m "feat(ui): make search live with stale-response guards"
```

---

### Task 7: Drag-and-drop math and action

**Files:**
- Create: `ui/src/lib/drag.ts`
- Create: `ui/src/lib/drag.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `dropTarget(clientY, scrollerTop, scrollTop, rowHeight, count, offset?): number`; `normalizeDrop(from, to): number`; `DragParams { enabled(): boolean; count(): number; rowHeight(): number; offset?(): number; onChange(from: number | null, at: number | null): void; onDrop(from: number, to: number): void }`; `trackDrag(node, params): { update(next): void; destroy(): void }`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/drag.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { dropTarget, normalizeDrop } from "./drag";

describe("dropTarget", () => {
  it("maps the top of a row to the slot before it", () => {
    expect(dropTarget(0, 0, 0, 36, 10)).toBe(0);
  });

  it("rounds to the nearer edge", () => {
    expect(dropTarget(17, 0, 0, 36, 10)).toBe(0);
    expect(dropTarget(19, 0, 0, 36, 10)).toBe(1);
  });

  it("accounts for scroll and container position", () => {
    expect(dropTarget(100 + 720, 100, 720, 36, 100)).toBe(20);
  });

  it("accounts for a sticky header offset", () => {
    expect(dropTarget(50, 0, 0, 36, 10, 36)).toBe(0);
    expect(dropTarget(90, 0, 0, 36, 10, 36)).toBe(2);
  });

  it("clamps to the list bounds", () => {
    expect(dropTarget(-50, 0, 0, 36, 3)).toBe(0);
    expect(dropTarget(9999, 0, 0, 36, 3)).toBe(3);
  });

  it("returns zero for an empty list", () => {
    expect(dropTarget(200, 0, 0, 36, 0)).toBe(0);
  });
});

describe("normalizeDrop", () => {
  it("shifts a downward target up by one", () => {
    expect(normalizeDrop(0, 3)).toBe(2);
    expect(normalizeDrop(2, 5)).toBe(4);
  });

  it("leaves upward and self targets unchanged", () => {
    expect(normalizeDrop(5, 2)).toBe(2);
    expect(normalizeDrop(3, 3)).toBe(3);
    expect(normalizeDrop(3, 4)).toBe(3);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/drag.test.ts`
Expected: FAIL — cannot resolve `./drag`.

- [ ] **Step 3: Write the implementation**

Create `ui/src/lib/drag.ts`:

```ts
/// Insertion slot (0..count) under the pointer. `offset` is content above the
/// first row (e.g. a sticky table head). Rounding puts the slot at the closer
/// row edge.
export function dropTarget(
  clientY: number,
  scrollerTop: number,
  scrollTop: number,
  rowHeight: number,
  count: number,
  offset = 0,
): number {
  const y = clientY - scrollerTop + scrollTop - offset;
  const slot = Math.round(y / rowHeight);
  return Math.max(0, Math.min(count, slot));
}

/// Moving down removes the dragged row first, shifting the target up one.
export function normalizeDrop(from: number, to: number): number {
  return to > from ? to - 1 : to;
}

export interface DragParams {
  enabled: () => boolean;
  count: () => number;
  rowHeight: () => number;
  offset?: () => number;
  onChange: (from: number | null, at: number | null) => void;
  onDrop: (from: number, to: number) => void;
}

/// Svelte action for the scroll container. Rows carry `draggable` and
/// `data-drag-index`; listeners attach per container, never per row, so the
/// index math works even when virtualized rows mount and unmount mid-drag.
export function trackDrag(node: HTMLElement, initial: DragParams) {
  let params = initial;
  let from: number | null = null;

  function rowIndex(target: EventTarget | null): number | null {
    const element = target instanceof Element ? target.closest("[data-drag-index]") : null;
    if (!element) return null;
    const index = Number(element.getAttribute("data-drag-index"));
    return Number.isInteger(index) ? index : null;
  }

  function slotAt(clientY: number): number {
    return dropTarget(
      clientY,
      node.getBoundingClientRect().top,
      node.scrollTop,
      params.rowHeight(),
      params.count(),
      params.offset?.() ?? 0,
    );
  }

  function onDragStart(event: DragEvent) {
    if (!params.enabled()) return;
    const index = rowIndex(event.target);
    if (index === null) return;
    from = index;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", String(index));
    }
    params.onChange(from, null);
  }

  function onDragOver(event: DragEvent) {
    if (from === null || !params.enabled()) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    params.onChange(from, slotAt(event.clientY));
  }

  function onDrop(event: DragEvent) {
    if (from === null || !params.enabled()) return;
    event.preventDefault();
    const source = from;
    from = null;
    params.onChange(null, null);
    params.onDrop(source, normalizeDrop(source, slotAt(event.clientY)));
  }

  function clear() {
    if (from === null) return;
    from = null;
    params.onChange(null, null);
  }

  function onDragLeave(event: DragEvent) {
    if (event.relatedTarget && node.contains(event.relatedTarget as Node)) return;
    clear();
  }

  node.addEventListener("dragstart", onDragStart);
  node.addEventListener("dragover", onDragOver);
  node.addEventListener("drop", onDrop);
  node.addEventListener("dragend", clear);
  node.addEventListener("dragleave", onDragLeave);

  return {
    update(next: DragParams) {
      params = next;
    },
    destroy() {
      node.removeEventListener("dragstart", onDragStart);
      node.removeEventListener("dragover", onDragOver);
      node.removeEventListener("drop", onDrop);
      node.removeEventListener("dragend", clear);
      node.removeEventListener("dragleave", onDragLeave);
    },
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/drag.test.ts`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/drag.ts ui/src/lib/drag.test.ts
git commit -m "feat(ui): add the drag drop math and container action"
```

---

### Task 8: Playlist drag-and-drop with optimistic moves

**Files:**
- Modify: `ui/src/lib/components/TrackTable.svelte` (drag wiring on top of Task 4's version)
- Modify: `ui/src/views/Playlist.svelte` (pass `onDrop`; `moveTrack` already added in Task 4 — extend the `playlistActions` object here)

**Interfaces:**
- Consumes: `trackDrag`, `DragParams` (Task 7), `pager.moveItem` (Task 2), `playlistActions.onDrop` (added by this task).
- Produces: drag-reorder in the TrackTable playlist context; `Playlist.moveTrack` reachable from the drop path.

- [ ] **Step 1: Wire drag into TrackTable**

In `ui/src/lib/components/TrackTable.svelte`:

Add the imports:

```ts
  import { trackDrag, type DragParams } from "../drag";
```

Extend the `playlistActions` prop type with the drop callback:

```ts
    playlistActions?: {
      onRemove(index: number): void;
      onMove(index: number, delta: -1 | 1): void;
      busy: boolean;
      disabled: boolean;
      onDrop?(from: number, to: number): void;
    };
```

Add drag state and the derived action params after the `slice` derived:

```ts
  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);
  let theadHeight = $state(0);

  const canDrag = $derived(
    playlistActions !== undefined &&
      !playlistActions.busy &&
      !playlistActions.disabled &&
      !view.engaged,
  );

  const drag = $derived<DragParams>({
    enabled: () => canDrag,
    count: () => rows.length,
    rowHeight: () => ROW_HEIGHT,
    offset: () => theadHeight,
    onChange: (from, at) => {
      dragFrom = from;
      dropAt = at;
    },
    onDrop: (from, to) => playlistActions?.onDrop?.(from, to),
  });
```

Attach the action to the scroller:

```svelte
<div
  class="table-scroll"
  style="max-height:{maxHeight}"
  bind:clientHeight={viewport}
  onscroll={onScroll}
  use:trackDrag={drag}
>
```

Render the drop indicator inside the scroller, immediately before the closing `</div>` of `.table-scroll` (after `</table>`):

```svelte
  {#if dropAt !== null}
    <div class="drop-line" style="top:{theadHeight + dropAt * ROW_HEIGHT}px"></div>
  {/if}
```

Measure the header: change `<thead>` to `<thead bind:clientHeight={theadHeight}>`.

Make rows draggable, tracked, and highlighted while dragging:

```svelte
        <tr
          class="row"
          class:dragging={dragFrom === index}
          draggable={canDrag}
          data-drag-index={canDrag ? index : undefined}
          use:menuFor={() =>
```

Add the handle at the start of the action cell, before the play button:

```svelte
          <td>
            {#if canDrag}
              <span class="handle" title="Drag to reorder" aria-hidden="true">⠿</span>
            {/if}
            <button onclick={() => play(index)} disabled={loadingAll} aria-label="Play">▶</button>
```

Add the styles to the `<style>` block:

```css
  .table-scroll {
    overflow: auto;
    position: relative;
  }
  .handle {
    cursor: grab;
    color: var(--muted);
    padding-right: 4px;
  }
  .row.dragging {
    opacity: 0.5;
  }
  .drop-line {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--accent);
    pointer-events: none;
  }
```

Merge these with the existing `.table-scroll { overflow: auto; }` rule rather than duplicating it.

- [ ] **Step 2: Pass the drop handler from Playlist**

In `ui/src/views/Playlist.svelte`, extend the `playlistActions` object:

```svelte
    playlistActions={{ onRemove: removeAt, onMove: moveAt, busy, disabled: !pager.done, onDrop: moveTrack }}
```

`moveTrack` was added in Task 4 and already: no-ops on `from === to`/busy, applies `pager.moveItem(from, to)`, awaits `api.movePlaylistItems(id, [from], to)`, and on failure toasts + `pager.refresh()`.

- [ ] **Step 3: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all suites pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/TrackTable.svelte ui/src/views/Playlist.svelte
git commit -m "feat(ui): drag reorder playlist tracks optimistically"
```

---

### Task 9: VirtualList and the virtualized QueueRail

**Files:**
- Create: `ui/src/lib/components/VirtualList.svelte`
- Modify: `ui/src/lib/components/QueueRail.svelte`

**Interfaces:**
- Consumes: `windowRange` (Task 1), `trackDrag`/`DragParams` (Task 7).
- Produces: `VirtualList` props `{ total: number; rowHeight: number; maxHeight?: string; overscan?: number; drag?: DragParams; children: Snippet<[number]> }`.

- [ ] **Step 1: Create VirtualList**

Create `ui/src/lib/components/VirtualList.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  import { trackDrag, type DragParams } from "../drag";
  import { windowRange } from "../virtual";

  let {
    total,
    rowHeight,
    maxHeight = "none",
    overscan = 8,
    drag,
    children,
  }: {
    total: number;
    rowHeight: number;
    maxHeight?: string;
    overscan?: number;
    drag?: DragParams;
    children: Snippet<[number]>;
  } = $props();

  const NO_DRAG: DragParams = {
    enabled: () => false,
    count: () => 0,
    rowHeight: () => 1,
    onChange: () => {},
    onDrop: () => {},
  };

  let scrollTop = $state(0);
  let viewport = $state(0);
  const range = $derived(windowRange(total, scrollTop, viewport, rowHeight, overscan));
  const indexes = $derived(
    Array.from({ length: Math.max(0, range.last - range.first) }, (_, i) => range.first + i),
  );

  function onScroll(event: Event) {
    scrollTop = (event.currentTarget as HTMLElement).scrollTop;
  }
</script>

<div
  class="scroll"
  style="max-height:{maxHeight}"
  bind:clientHeight={viewport}
  onscroll={onScroll}
  use:trackDrag={drag ?? NO_DRAG}
>
  <div style="height:{range.padTop}px"></div>
  {#each indexes as index (index)}
    {@render children(index)}
  {/each}
  <div style="height:{range.padBottom}px"></div>
</div>

<style>
  .scroll {
    overflow: auto;
  }
</style>
```

- [ ] **Step 2: Virtualize QueueRail**

Replace all of `ui/src/lib/components/QueueRail.svelte` with:

```svelte
<script lang="ts">
  import { api } from "../api";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import VirtualList from "./VirtualList.svelte";
  import { menuFor } from "../menu.svelte";
  import { queueMenuItems } from "../menus";
  import type { DragParams } from "../drag";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();
  let confirmingClear = $state(false);

  /// Fixed row height; keep in sync with the row style below.
  const QUEUE_ROW_HEIGHT = 40;

  const queueDrag: DragParams = {
    enabled: () => true,
    count: () => player.state.queue.length,
    rowHeight: () => QUEUE_ROW_HEIGHT,
    onChange: () => {},
    onDrop: (from, to) => {
      if (from !== to) player.reorder(from, to);
    },
  };

  function jump(index: number) {
    const items = player.state.queue;
    if (index >= 0 && index < items.length) {
      api.playTracks(items, index);
    }
  }
</script>

<div style="padding:12px">
  <div style="display:flex;justify-content:space-between;align-items:center">
    <strong>Queue</strong>
    <button onclick={() => (confirmingClear = true)}>Clear</button>
  </div>
  <div role="list" style="margin:8px 0">
    <VirtualList
      total={player.state.queue.length}
      rowHeight={QUEUE_ROW_HEIGHT}
      maxHeight="calc(100vh - 220px)"
      drag={queueDrag}
    >
      {#snippet children(index)}
        {@const item = player.state.queue[index]}
        {#if item}
          <div
            role="listitem"
            draggable="true"
            data-drag-index={index}
            use:menuFor={() =>
              queueMenuItems({ index, playNow: () => jump(index), remove: () => player.remove(index) })}
            style="height:{QUEUE_ROW_HEIGHT}px;box-sizing:border-box;display:flex;align-items:center;padding:4px;border-bottom:1px solid var(--line);{player.state.current?.track_id === item.track_id ? 'background:var(--panel);' : ''}"
          >
            <span class="muted" style="cursor:grab;padding-right:6px" title="Drag to reorder" aria-hidden="true">⠿</span>
            <button onclick={() => jump(index)} style="all:unset;cursor:pointer;flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
              {item.title} <span class="muted">{item.artist}</span>
            </button>
            <button onclick={() => player.remove(index)} aria-label="Remove">✕</button>
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  </div>
  {#if player.state.queue.length === 0}<p class="muted">Queue is empty</p>{/if}
  {#if confirmingClear}
    <ConfirmDialog
      title="Clear queue"
      message="Remove all tracks from the queue?"
      onconfirm={() => {
        confirmingClear = false;
        player.clear();
      }}
      oncancel={() => (confirmingClear = false)}
    />
  {/if}
</div>
```

- [ ] **Step 3: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all suites pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/VirtualList.svelte ui/src/lib/components/QueueRail.svelte
git commit -m "feat(ui): virtualize the queue rail"
```

---

### Task 10: Virtualize the Library lists

**Files:**
- Modify: `ui/src/views/Library.svelte`

**Interfaces:**
- Consumes: `VirtualList` (Task 9).
- Produces: windowed playlists (44 px rows) and albums (36 px rows) lists; empty states and menu/rename/delete flows unchanged.

- [ ] **Step 1: Replace the playlists and albums lists**

In `ui/src/views/Library.svelte`, add the import:

```ts
  import VirtualList from "../lib/components/VirtualList.svelte";
```

Replace the playlists `<ul>` block:

```svelte
  <div role="list">
    <VirtualList total={playlists.length} rowHeight={44} maxHeight="min(40vh, 360px)">
      {#snippet children(index)}
        {@const p = playlists[index]}
        {#if p}
          <div
            role="listitem"
            style="height:44px;box-sizing:border-box;display:flex;align-items:center;gap:6px"
            use:menuFor={() =>
              libraryPlaylistMenuItems({
                playlist: p,
                open: () => nav.goto({ name: "playlist", id: p.uuid }),
                rename: () => startRename(p),
                remove: () => (deleting = p),
              })}
          >
            {#if editingUuid === p.uuid}
              <input bind:value={editTitle} aria-label="Playlist name" />
              <button onclick={() => void saveRename(p)}>Save</button>
              <button onclick={() => (editingUuid = null)}>Cancel</button>
            {:else}
              <button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button>
              <button onclick={() => startRename(p)}>Rename</button>
              <button onclick={() => (deleting = p)}>Delete</button>
            {/if}
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  </div>
  {#if playlists.length === 0}<p class="muted">No playlists</p>{/if}
```

Replace the albums `<ul>` block:

```svelte
  <div role="list">
    <VirtualList total={albums.length} rowHeight={36} maxHeight="min(40vh, 360px)">
      {#snippet children(index)}
        {@const a = albums[index]}
        {#if a}
          <div
            role="listitem"
            style="height:36px;box-sizing:border-box;display:flex;align-items:center"
          >
            <button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button>
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  </div>
  {#if albums.length === 0}<p class="muted">No albums</p>{/if}
```

- [ ] **Step 2: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all suites pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 3: Commit**

```bash
git add ui/src/views/Library.svelte
git commit -m "feat(ui): virtualize the library playlist and album lists"
```

---

### Task 11: Verification and smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/plan9-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: recorded gate output and a human smoke checklist.

- [ ] **Step 1: Run the full Rust and UI gates**

Run each and capture the summary lines:

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: `cargo test` all green (unchanged from main), clippy with no warnings, all vitest suites pass (13 existing + `virtual`, `trackView`, `search`, `drag`), svelte-check 0 errors / 0 warnings.

- [ ] **Step 2: Write the smoke checklist**

Create `docs/superpowers/plans/notes/plan9-smoke.md`:

```markdown
# Plan 9 — Manual Smoke Checklist

Run against a real account in `npm --prefix ui run app:dev`. Record pass/fail
and notes per item; a failed item blocks the plan until fixed or explicitly
accepted.

## Sort and filter

- [ ] Open a playlist with 2,000+ tracks. Type in the filter field: the header
      shows "Loading all tracks…", then the list filters over ALL tracks (not
      just the first page); the count line is unchanged.
- [ ] Click Title: ascending ▲, click again descending ▼, click again back to
      original order. Click # while a filter is active: order resets, filter
      stays.
- [ ] Sort by Time ascending/descending; ties keep original order.
- [ ] Filter by an artist name that only appears in the artist column; matching
      rows show.
- [ ] Filter with no matches shows `No tracks match “…”`; clear it with ✕.
- [ ] Remove a row while filtered: the correct track disappears after refresh;
      no moves are offered in the row menu (Remove/Favorite remain).
- [ ] Navigate away and back: sort/filter state is reset; rows hydrate from the
      session cache without a full refetch flash.
- [ ] Repeat on Library → Favorite tracks and on an Album (instant, no loading
      row); Search results table gets the same controls.

## Live search

- [ ] Type "ra" quickly into Search: results appear without Enter after ~300 ms.
- [ ] Type fast through several queries: the final result set matches the final
      text (no older result flashing in).
- [ ] Press Enter with one character: search runs immediately.
- [ ] Clear the box: results clear.
- [ ] A no-match query shows `No results for “…”`; a failed request shows the
      inline error with Retry.
- [ ] `Ctrl+F` still focuses the input; Space/arrows in the input edit text and
      do not control playback.

## Drag reorder

- [ ] Drag a playlist row by the ⠿ handle: a 2 px indicator line tracks the
      insertion slot; the source row dims.
- [ ] Drop mid-list: the row lands there instantly (no whole-table reload
      flash); the server agrees after navigating away and back.
- [ ] Drop a row onto itself or just below the last row: nothing moves.
- [ ] Scroll a long playlist, drag a row near the top/bottom edge: the list
      auto-scrolls and the drop lands where the indicator showed.
- [ ] While a filter/sort is active, the ⠿ handle and ↑/↓ buttons are absent;
      menu has no Move items.
- [ ] ↑/↓ buttons and menu Move still work in original order and are instant.
- [ ] Force a move failure (disconnect the network): an error toast appears and
      the list returns to server order.

## Virtualization

- [ ] Queue an entire large playlist, open the queue rail: scrolling is smooth,
      rows are 40 px aligned, and drag reorder + jump/remove/menu still work.
- [ ] Library playlists and albums scroll within their boxes; rename/edit/
      delete/menu still work; scrolling an edited row out of view and back
      keeps it editable.
- [ ] Rows are pixel-aligned at 36 px in TrackTable/albums, 40 px in the queue,
      44 px in Library playlists.
- [ ] Empty states show for an empty queue, no playlists, and no albums.
```

- [ ] **Step 3: Run the human smoke pass**

Hand the checklist to the owner. Record results inline in the file (pass/fail + notes) in a follow-up commit:

```bash
git add docs/superpowers/plans/notes/plan9-smoke.md
git commit -m "docs: record the plan 9 smoke checklist"
```

- [ ] **Step 4: Fix any failed item**

Any failure goes through superpowers:systematic-debugging, not a guess. Do not mark the plan complete with an unchecked or failed item.

---

## Self-Review Notes

- Spec coverage: §5.1→Tasks 1/9, §5.2→Task 3, §5.3→Tasks 4/8, §5.4→Tasks 5/6, §5.5→Tasks 7/8/9, §5.6→Tasks 2/4/8, §5.7→Tasks 9/10; §8 error rows, §9 test bullets, and the Review Focus items each map to a task above.
- Type consistency check: `TrackView.clearSort` (not `reset`), `pager.moveItem(from, to)`, `DragParams.onChange/onDrop`, `VirtualList.drag`, and `playlistActions.onDrop` are named identically everywhere they appear.
- Known deliberate deviation from the spec: `ensureAllTracks`/`ensureAllFavorites` wrappers throw when the pager reports an error, so `TrackView` can distinguish complete from partial loads (the spec's `() => Promise<void>` signature is preserved).
