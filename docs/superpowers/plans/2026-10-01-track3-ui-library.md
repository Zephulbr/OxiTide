# Track 3 Phase C — UI Smoothness & Large-Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the UI's per-event whole-state invalidation, make filter/sort/source-index cheap at 10k rows, remove per-window array copies, parallelize the explicit `ensureAll` walk, and add the S6 synthetic 10k fixture plus the missing budget rows (filter/sort interaction times, app-owned pre-paint span).

**Architecture:** All fixes are internal and behavior-preserving (T3-D5): player events mutate the existing state object so Svelte invalidates only changed properties; `trackView` memoizes lower-cased search parts and sort keys per track and keeps a source-index map; `pager` appends/removes/moves in place and reuses one dedupe set, with `ensureAll` switching from a sequential page walk to the existing concurrency-capped window burst above `prefetchMaxTotal`; a perf-session-only bridge loads a deterministic synthetic 10k library into the Tracks view and times real input→first-frame interactions, reported as `ui_metrics.interactions` and gated by `check.mjs`.

**Tech Stack:** Svelte 5 runes (no component-test harness; `.svelte.ts` logic is vitest-tested), `Intl.Collator`, Node 20 `node:test` for the perf gate, PowerShell runner (unchanged), no Rust changes.

**Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track3-performance-design.md` (§6 Phase C; §4.2 budgets)
**Baseline:** `docs/superpowers/plans/notes/2026-09-30-track3-baseline.md` (S5–S7 rows and the deferred 10k filter/sort + pre-paint-span gates)
**Phase B record:** `docs/superpowers/plans/notes/2026-10-01-track3-phase-b.md`

## Global Constraints

- **Behavior-preserving (T3-D5):** no visible UI/animation change. The filter stays synchronous (no debounce): precomputed keys must meet the 50 ms budget without changing typing feel. A debounce is an owner decision only if the measurement breaches, and is not part of this plan.
- **Budgets are absolute and unchanged** (`scripts/perf/budgets.json` is the standing gate): S5-instrument frame p95 ≤16.7 / p99 ≤33; S5-atmosphere p95 ≤20 / p99 ≤33; long tasks ≤100; memory ceilings stay Phase D FAILs. New rows added by this plan: S6 `filter_max_ms` ≤50, S6 `sort_max_ms` ≤150, S1/S2 `startup_step_max_ms` ≤100. No existing budget may be weakened.
- **Commands:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`; `npm --prefix ui run check`; `npm --prefix ui test`; `node --test scripts/perf/check.test.mjs`. HTTP tests use wiremock only; never log token or credential values.
- **Perf evidence:** raw runs live in `.superpowers/perf/runs/` (gitignored); phase summaries commit under `docs/superpowers/plans/notes/`. Quarantine, never delete, artifacts.
- **Perf-session-only harness code:** the synthetic fixture and interaction timers exist only while `perf.active`; normal runs must be byte-for-byte unchanged in behavior.
- **No new runtime dependencies.** `Intl.Collator` is built in.
- Windows-first: commands are PowerShell; the runner is unchanged.

## Review Focus

- **A lazy page append landing mid-filter:** a window that arrives after `setFilter` engaged must extend the filtered view consistently — no duplicates, no dropped rows, `sourceIndex` still maps each visible row. Pinned by Task 3's `keeps a filtered view consistent while a lazy page appends`.
- **A playlist that repeats a track id, then moves a row:** both occurrences survive the in-place splice and the session cache rehydrates the moved order exactly. Pinned by Task 3's `reorders duplicate rows in place without corrupting the cache`.
- **Unfavorite then re-favorite at scale:** the persistent dedupe set must not block a removed id from returning in a later page. Pinned by Task 3's `allows a removed row to come back from a later page`.
- **Interleaved `position` / `state` events:** fine-grained mutation must land every field (queue from the state event, position from the tick) with no stale overwrite. Pinned by Task 1's `keeps queue and position updates when events interleave`.
- **The synthetic S6 fixture must never leak into normal use:** the bridge method only exists during a perf session and the hook registration is removed on unmount; verified by the Task 6 smoke checklist (component lifecycle, owner-run) plus the code's effect cleanup.

---

## File Structure

- `ui/src/lib/player.svelte.ts` — add in-place `applyEvent`; keep pure `reduce` for callers/tests; use `applyEvent` in the listener and the snapshot/restore/retry paths.
- `ui/src/lib/player.test.ts` — new `applyEvent` tests.
- `ui/src/lib/trackView.svelte.ts` — cached search parts and title keys, shared `Intl.Collator`, source-index map.
- `ui/src/lib/trackView.test.ts` — mechanism tests (no per-comparison `localeCompare`, no `indexOf` in `sourceIndex`), accent/case order guard, append-while-filtered integration test.
- `ui/src/lib/pager.svelte.ts` — in-place `push`/`splice`, persistent `loadedIds`, aliased cache publish, parallel `ensureAll`.
- `ui/src/lib/pager.test.ts` — identity, duplicate reorder, remove/re-add, parallel ensureAll, burst-error tests.
- `ui/src/lib/perfFixture.ts` (new) — deterministic synthetic track list.
- `ui/src/lib/perfFixture.test.ts` (new) — determinism and filter-spread tests.
- `ui/src/lib/perf.ts` — `InteractionLog` pure helper.
- `ui/src/lib/perf.test.ts` — `InteractionLog` tests.
- `ui/src/lib/perfHooks.ts` (new) — perf-session view hook registry.
- `ui/src/lib/perf.svelte.ts` — record interactions, include them in `ui_metrics` submissions, flush on record.
- `ui/src/App.svelte` — bridge gains `loadSyntheticTracks`, `measureFilter`, `measureSort`.
- `ui/src/views/CollectionTracks.svelte` — synthetic source + hook registration during perf sessions.
- `scripts/perf/check.mjs` — derive `filter_max_ms` / `sort_max_ms` from `ui_metrics.interactions`, derive `startup_step_max_ms`.
- `scripts/perf/check.test.mjs` — tests for both derivations.
- `scripts/perf/budgets.json` — S6 `filter_max_ms`/`sort_max_ms`; S1/S2 `startup_step_max_ms`.
- `scripts/perf/drive.mjs` — S6 loads the synthetic 10k fixture and times filter/sort interactions.
- `docs/superpowers/plans/notes/<execution-date>-track3-phase-c.md` and `-smoke.md` (new) — Task 6 records.

---

### Task 1: Fine-grained player state updates

**Files:**
- Modify: `ui/src/lib/player.svelte.ts`
- Modify: `ui/src/lib/player.test.ts`

**Interfaces:**
- Consumes: `EngineEvent`, `PlayerState` (unchanged).
- Produces: `export function applyEvent(state: PlayerState, event: EngineEvent): PlayerState` — mutates `state` in place and returns the same reference. `reduce` keeps its pure signature and is re-expressed through `applyEvent`.

- [ ] **Step 1: Write the failing tests**

Append to `ui/src/lib/player.test.ts` (keep the existing imports; add `applyEvent` to the import list):

```ts
describe("player event application", () => {
  const positioned: PlayerState = { ...emptyState(), queue: [
    { track_id: 1, title: "T", artist: "A", album: "", duration_secs: 10, cover: null },
  ] };

  it("mutates the existing state for a position event", () => {
    const state = { ...positioned };
    const result = applyEvent(state, { type: "position", position_ms: 1000, duration_ms: 10000 });
    expect(result).toBe(state);
    expect(state.position_ms).toBe(1000);
    expect(state.duration_ms).toBe(10000);
    expect(state.queue).toHaveLength(1);
  });

  it("applies a state event field by field", () => {
    const state = { ...positioned, error: "stale" };
    const result = applyEvent(state, {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.5, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      queue: [],
      stream: null,
    });
    expect(result).toBe(state);
    expect(state.playing).toBe(true);
    expect(state.volume).toBe(0.5);
    expect(state.queue).toEqual([]);
    expect(state.error).toBe("stale");
  });

  it("clears the error on now_playing and captures it on error", () => {
    const state = { ...positioned, error: "old" };
    applyEvent(state, { type: "now_playing", item: null });
    expect(state.error).toBe("");
    applyEvent(state, { type: "error", code: "auth", message: "nope" });
    expect(state.error).toBe("nope");
  });

  it("keeps queue and position updates when events interleave", () => {
    const state = emptyState();
    applyEvent(state, { type: "position", position_ms: 1000, duration_ms: 10000 });
    applyEvent(state, {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 1000, duration_ms: 10000, volume: 1, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      queue: [{ track_id: 1, title: "T", artist: "A", album: "", duration_secs: 10, cover: null }],
      stream: null,
    });
    applyEvent(state, { type: "position", position_ms: 2000, duration_ms: 10000 });
    expect(state.queue).toHaveLength(1);
    expect(state.position_ms).toBe(2000);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/player.test.ts`
Expected: FAIL — `applyEvent` is not exported (compile error / undefined).

- [ ] **Step 3: Implement `applyEvent` and route every update through it**

In `ui/src/lib/player.svelte.ts`, replace `reduce` with:

```ts
/// Apply an engine event to `state` in place. The returned object is the same
/// reference, so Svelte invalidates only the properties that changed: position
/// events arrive every 500 ms and a whole-state replacement re-ran every reader.
export function applyEvent(state: PlayerState, event: EngineEvent): PlayerState {
  switch (event.type) {
    case "state": {
      const { type, ...rest } = event;
      Object.assign(state, rest);
      return state;
    }
    case "position":
      state.position_ms = event.position_ms;
      state.duration_ms = event.duration_ms;
      return state;
    case "now_playing":
      state.error = "";
      return state;
    case "error":
      state.error = event.message;
      return state;
    default:
      return state;
  }
}

export function reduce(state: PlayerState, event: EngineEvent): PlayerState {
  return applyEvent({ ...state }, event);
}
```

In `createPlayer`, change the three whole-object writes to in-place ones:

```ts
  async function init() {
    await listen<EngineEvent>("playback_event", (event) => {
      applyEvent(state, event.payload);
      pendingSeek = reconcileSeek(pendingSeek, event.payload);
    });
    try {
      const snapshot = await api.getPlaybackState();
      applyEvent(state, { type: "state", ...snapshot });
      if (snapshot.volume > 0) lastVolume = snapshot.volume;
    } catch {
      /* not logged in yet */
    }
  }
```

```ts
  async function retry() {
    await api.retry();
    // The engine emits a State event, which does not clear the mirrored
    // error; clear it here once the retry command was accepted.
    state.error = "";
  }
```

In `restore`, replace `state = reduce(state, { type: "state", ...snapshot });` with:

```ts
    applyEvent(state, { type: "state", ...snapshot });
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/player.test.ts`
Expected: PASS — the new `applyEvent` tests and all existing `reduce` tests.

- [ ] **Step 5: Type-check and commit**

Run: `npm --prefix ui run check`

```bash
git add ui/src/lib/player.svelte.ts ui/src/lib/player.test.ts
git commit -m "perf(ui): apply player events in place so position ticks stop invalidating readers"
```

---

### Task 2: Fast track-view filter, sort, and source index

**Files:**
- Modify: `ui/src/lib/trackView.svelte.ts`
- Modify: `ui/src/lib/trackView.test.ts`

**Interfaces:**
- Consumes: the existing `createTrackView(source, ensureAll?)` and `Track`.
- Produces: the same public `TrackView` API and the same rendered order; internally lower-cased search parts and title keys are memoized per track object, sorting uses one shared `Intl.Collator` instead of `localeCompare` per comparison, and `sourceIndex` reads a precomputed identity map instead of scanning the source.

- [ ] **Step 1: Write the failing tests**

Append to `ui/src/lib/trackView.test.ts`:

```ts
describe("track view sort keys", () => {
  it("does not call localeCompare per comparison", () => {
    const spy = vi.spyOn(String.prototype, "localeCompare");
    const view = createTrackView(() => SOURCE);
    view.toggleSort("title");
    const computed = ids(view.items);
    const calls = spy.mock.calls.length;
    spy.mockRestore();

    expect(computed).toEqual([2, 3, 1]);
    expect(calls).toBe(0);
  });

  it("keeps locale order for accent and case variants with stable ties", () => {
    const list = [track(1, "éclair"), track(2, "Echo"), track(3, "ÉCLAIR")];
    const view = createTrackView(() => list);

    view.toggleSort("title");

    expect(ids(view.items)).toEqual([2, 1, 3]);
  });

  it("maps sourceIndex without scanning the source", () => {
    const view = createTrackView(() => SOURCE);
    view.toggleSort("duration");
    const spy = vi.spyOn(Array.prototype, "indexOf");
    const mapped = view.sourceIndex(0);
    const calls = spy.mock.calls.length;
    spy.mockRestore();

    expect(mapped).toBe(1);
    expect(calls).toBe(0);
  });
});

describe("track view live updates", () => {
  it("keeps a filtered view consistent while a lazy page appends", async () => {
    const pager = createTrackPager(
      "filter-append",
      async (offset, limit) =>
        offset === 0
          ? { items: [track(1, "alpha"), track(2, "beta")], offset, limit, total: 4 }
          : { items: [track(3, "alphabet"), track(4, "gamma")], offset, limit, total: 4 },
      { pageSize: 2, prefetch: false },
    );
    await pager.loadMore();
    const view = createTrackView(() => pager.items, async () => {});

    view.setFilter("alp");
    await vi.waitFor(() => expect(view.preparing).toBe(false));
    expect(ids(view.items)).toEqual([1]);

    await pager.loadMore();

    expect(ids(view.items)).toEqual([1, 3]);
    expect(view.sourceIndex(1)).toBe(2);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/trackView.test.ts`
Expected: FAIL — `localeCompare` is called per comparison (`calls > 0`) and `sourceIndex` calls `indexOf`; the other new tests pass.

- [ ] **Step 3: Memoize keys and index, share one collator**

In `ui/src/lib/trackView.svelte.ts`, replace `matches` and `applyView` with:

```ts
const collator = new Intl.Collator(undefined, { sensitivity: "base" });

/// Per-track lower-cased title/artist parts and title collation keys. Track
/// objects are stable while a list is mounted, so a keystroke or sort reuses
/// them instead of lower-casing every row again.
const searchParts = new WeakMap<Track, string[]>();
const titleKeys = new WeakMap<Track, string>();

function searchable(track: Track): string[] {
  let parts = searchParts.get(track);
  if (parts === undefined) {
    parts = [track.title, ...track.artists.map((artist) => artist.name)].map((value) =>
      value.toLowerCase(),
    );
    searchParts.set(track, parts);
  }
  return parts;
}

function titleKey(track: Track): string {
  let key = titleKeys.get(track);
  if (key === undefined) {
    key = track.title.toLocaleLowerCase();
    titleKeys.set(track, key);
  }
  return key;
}

function matches(track: Track, needle: string): boolean {
  return searchable(track).some((part) => part.includes(needle));
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
    .map((track, index) => ({ track, index, key: sort === "title" ? titleKey(track) : "" }))
    .sort((a, b) => {
      const cmp =
        sort === "title"
          ? collator.compare(a.key, b.key)
          : a.track.duration - b.track.duration;
      return cmp !== 0 ? cmp * factor : a.index - b.index;
    })
    .map((entry) => entry.track);
}
```

In `createTrackView`, add the index map right after the `items` derived and rewrite `sourceIndex`:

```ts
  /// Identity map for `sourceIndex`: O(1) per call instead of an O(n) scan;
  /// the first occurrence wins, matching `indexOf` for repeated references.
  const sourceIndexMap = $derived.by(() => {
    const map = new Map<Track, number>();
    source().forEach((track, index) => {
      if (!map.has(track)) map.set(track, index);
    });
    return map;
  });
```

```ts
  function sourceIndex(visibleIndex: number): number {
    const track = items[visibleIndex];
    if (!track) return -1;
    return sourceIndexMap.get(track) ?? -1;
  }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/trackView.test.ts`
Expected: PASS — new mechanism tests, the `visibleIndexOf`/`sourceIndex` suites, and the pager-integration retry test.

- [ ] **Step 5: Type-check and commit**

Run: `npm --prefix ui run check`

```bash
git add ui/src/lib/trackView.svelte.ts ui/src/lib/trackView.test.ts
git commit -m "perf(ui): precompute track-view filter and sort keys with an index map"
```

---

### Task 3: Pager in-place updates and parallel `ensureAll`

**Files:**
- Modify: `ui/src/lib/pager.svelte.ts`
- Modify: `ui/src/lib/pager.test.ts`

**Interfaces:**
- Consumes: the existing `createTrackPager(key, load, options?)` and its public `Pager` API (unchanged signatures).
- Produces: `items` keeps one array identity per pager (windows are pushed, removals/moves splice in place); the dedupe set persists across windows and is cleared for removed/refreshed ids; `ensureAll` uses the existing concurrency-capped `runBurst` for prefetched pagers above `prefetchMaxTotal`, surfaces burst failures as `error` (never rejects), and keeps the progressive first page.

- [ ] **Step 1: Write the failing tests**

Append to `ui/src/lib/pager.test.ts`:

```ts
describe("pager in-place updates", () => {
  it("appends windows in place so the rendered array identity is stable", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(offset + 1), track(offset + 2)], offset, limit, 4),
    );
    const pager = createTrackPager("in-place-append", load, { pageSize: 2, prefetch: false });
    const first = pager.items;

    await pager.loadMore();
    await pager.loadMore();

    expect(pager.items).toBe(first);
    expect(ids(pager.items)).toEqual([1, 2, 3, 4]);
  });

  it("removes and moves rows in place", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3)], offset, limit, 3),
    );
    const pager = createTrackPager("in-place-edit", load, { pageSize: 3, prefetch: false });
    await pager.loadMore();
    const first = pager.items;

    pager.moveItem(0, 2);
    expect(pager.items).toBe(first);
    pager.removeItem(1);
    expect(pager.items).toBe(first);

    expect(ids(pager.items)).toEqual([2, 3]);
  });

  it("allows a removed row to come back from a later page", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2)], offset, limit, 4)
        : page([track(2), track(3)], offset, limit, 4),
    );
    const pager = createTrackPager("remove-then-readd", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    pager.removeItem(2);
    await pager.loadMore();

    expect(ids(pager.items)).toEqual([1, 2, 3]);
  });

  it("reorders duplicate rows in place without corrupting the cache", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(1)], offset, limit, 3),
    );
    const pager = createTrackPager("dup-move", load, {
      pageSize: 3,
      prefetch: false,
      dedupe: false,
    });
    await pager.loadMore();
    const first = pager.items;

    pager.moveItem(0, 2);

    expect(pager.items).toBe(first);
    expect(ids(pager.items)).toEqual([2, 1, 1]);
    const rehydrated = createTrackPager("dup-move", load, {
      pageSize: 3,
      prefetch: false,
      dedupe: false,
    });
    expect(ids(rehydrated.items)).toEqual([2, 1, 1]);
  });

  it("fetches the remainder in parallel above the burst cap on ensureAll", async () => {
    const total = 20_100;
    let active = 0;
    let maxActive = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset > 0) {
        active += 1;
        maxActive = Math.max(maxActive, active);
        await delay(5);
        active -= 1;
      }
      const to = Math.min(offset + limit, total);
      return page(range(offset + 1, to).map(track), offset, limit, total);
    });
    const pager = createTrackPager("ensure-parallel", load, { prefetchMaxTotal: 10_000 });

    await pager.ensureAll();

    expect(pager.done).toBe(true);
    expect(pager.items.length).toBe(total);
    expect(maxActive).toBe(4);
  });

  it("surfaces a burst failure from ensureAll as error, then resumes on retry", async () => {
    const total = 20_100;
    let failOnce = true;
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset > 100 && failOnce) {
        failOnce = false;
        throw new Error("boom");
      }
      const to = Math.min(offset + limit, total);
      return page(range(offset + 1, to).map(track), offset, limit, total);
    });
    const pager = createTrackPager("ensure-parallel-retry", load, {
      prefetchMaxTotal: 10_000,
    });

    await expect(pager.ensureAll()).resolves.toBeUndefined();
    expect(pager.error).toContain("boom");

    await pager.ensureAll();

    expect(pager.error).toBe("");
    expect(pager.done).toBe(true);
    expect(pager.items.length).toBe(total);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: FAIL — `loadMore`/`removeItem`/`moveItem` replace the array (identity assertions fail) and the lazy `ensureAll` walks sequentially (`maxActive` is 1).

- [ ] **Step 3: Rework the pager internals**

In `ui/src/lib/pager.svelte.ts`, after the cache hydration, add the persistent dedupe set:

```ts
  const cached = cache.get(key);
  let items = $state<Track[]>(cached ? [...cached.items] : []);
  let total = $state(cached?.total ?? 0);
  let done = $state(cached?.done ?? false);
  let loading = $state(false);
  let error = $state("");
  /// Ids already appended while `dedupe` is on; maintained across windows so a
  /// page append does not rebuild a Set over the whole list every time.
  const loadedIds = new Set<number>(cached ? cached.items.map((t) => t.id) : []);
```

Replace `publish`, `applyWindow`, and `applyRemoval`:

```ts
  function publish() {
    // The cache entry deliberately shares the live array: every mutation
    // publishes immediately, and a rehydrating pager copies on mount.
    cache.set(key, { items, total, nextOffset, done });
  }

  /// Append a fetched window in place and advance past it.
  function applyWindow(page: TrackPage, offset: number, limit: number) {
    const incoming = dedupe
      ? page.items.filter((t) => !loadedIds.has(t.id))
      : page.items;
    if (incoming.length > 0) {
      items.push(...incoming);
      if (dedupe) for (const t of incoming) loadedIds.add(t.id);
    }
    if (page.total > 0) total = page.total;
    const received = page.items.length;
    nextOffset = offset + limit;
    done =
      received === 0 ||
      (total > 0 ? nextOffset >= total : received < limit);
  }

  function applyRemoval(id: number) {
    let removed = false;
    for (let i = items.length - 1; i >= 0; i--) {
      if (items[i].id === id) {
        items.splice(i, 1);
        removed = true;
      }
    }
    if (!removed) return;
    loadedIds.delete(id);
    if (total > 0) total -= 1;
    nextOffset = Math.max(0, nextOffset - 1);
    publish();
  }
```

Replace `moveItem` and `ensureAll`, and reset the dedupe set in `refresh`:

```ts
  function moveItem(from: number, to: number) {
    if (from === to) return;
    if (from < 0 || from >= items.length) return;
    if (to < 0 || to >= items.length) return;
    const [moved] = items.splice(from, 1);
    items.splice(to, 0, moved);
    publish();
  }
```

```ts
  async function ensureAll() {
    // A prior failure leaves `error` set; clear it so a later call (the
    // header Retry) drives the failed page instead of returning at once.
    if (error !== "") error = "";
    // Keep the progressive first page: it paints fast and seeds `total`.
    if (items.length === 0 && nextOffset === 0 && !done) await loadMore();
    while (!done && error === "") {
      // Prefetched pagers already burst; when the caller explicitly asks for
      // the whole list (sort/filter/play-all) do the same above
      // `prefetchMaxTotal` instead of walking one small page at a time.
      try {
        if (prefetch && total > 0) await runBurst();
        else await loadMore();
      } catch (e) {
        error = String(e);
        publish();
      }
    }
  }
```

```ts
  async function refresh() {
    // Let an in-flight page finish first so it cannot republish into the
    // cache after the reset.
    if (inflight) await inflight;
    cache.delete(key);
    items = [];
    loadedIds.clear();
    total = 0;
    nextOffset = 0;
    done = false;
    error = "";
    await loadMore();
  }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: PASS — all existing pager tests (sequential `ensureAll` for `prefetch: false`, burst, error, refresh, remove/move) plus the six new ones.

- [ ] **Step 5: Run the whole UI suite, type-check, and commit**

Run: `npm --prefix ui test; if ($?) { npm --prefix ui run check }`
Expected: all green.

```bash
git add ui/src/lib/pager.svelte.ts ui/src/lib/pager.test.ts
git commit -m "perf(ui): update pager rows in place and parallelize ensureAll above the burst cap"
```

---

### Task 4: Synthetic 10k fixture and S6 interaction timing

**Files:**
- Create: `ui/src/lib/perfFixture.ts`, `ui/src/lib/perfFixture.test.ts`, `ui/src/lib/perfHooks.ts`
- Modify: `ui/src/lib/perf.ts`, `ui/src/lib/perf.test.ts`, `ui/src/lib/perf.svelte.ts`, `ui/src/App.svelte`, `ui/src/views/CollectionTracks.svelte`

**Interfaces:**
- Consumes: `Track`; `perf.active`; the existing `__perf` bridge and `perf_submit_ui_metrics`.
- Produces: `makeSyntheticTracks(count: number): Track[]` (deterministic); `InteractionLog` with `record(kind, ms)` / `reset()` / `summary()` where `summary() = { filter_count, filter_max_ms, sort_count, sort_max_ms }`; `perf.recordInteraction(kind, ms)`; `perfHooks.loadSyntheticTracks` / `perfHooks.toggleViewSort`; bridge methods `loadSyntheticTracks(count)`, `measureFilter(text)`, `measureSort(key)` returning `false` when the view is absent.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/perfFixture.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { makeSyntheticTracks } from "./perfFixture";

describe("makeSyntheticTracks", () => {
  it("is deterministic and unique", () => {
    const a = makeSyntheticTracks(10_000);
    const b = makeSyntheticTracks(10_000);
    expect(a).toHaveLength(10_000);
    expect(new Set(a.map((t) => t.id)).size).toBe(10_000);
    expect(a[123].title).toBe(b[123].title);
    expect(a[123].artists[0].name).toBe(b[123].artists[0].name);
  });

  it("spreads titles so filter queries narrow the list", () => {
    const tracks = makeSyntheticTracks(10_000);
    const matches = tracks.filter((t) => t.title.toLowerCase().includes("the ")).length;
    expect(matches).toBeGreaterThan(0);
    expect(matches).toBeLessThan(tracks.length);
  });
});
```

Append to `ui/src/lib/perf.test.ts` (add `InteractionLog` to the import):

```ts
describe("InteractionLog", () => {
  it("tracks filter and sort maxima independently", () => {
    const log = new InteractionLog();
    log.record("filter", 18);
    log.record("filter", 27);
    log.record("sort", 40);
    expect(log.summary()).toEqual({
      filter_count: 2,
      filter_max_ms: 27,
      sort_count: 1,
      sort_max_ms: 40,
    });
    log.reset();
    expect(log.summary()).toEqual({
      filter_count: 0,
      filter_max_ms: 0,
      sort_count: 0,
      sort_max_ms: 0,
    });
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/perfFixture.test.ts src/lib/perf.test.ts`
Expected: FAIL — `./perfFixture` and `InteractionLog` do not exist.

- [ ] **Step 3: Add the fixture and the interaction log**

Create `ui/src/lib/perfFixture.ts`:

```ts
import type { Track } from "./api";

const FIRST = ["The", "A", "My", "Your", "Our", "Every", "Last", "First"];
const SECOND = ["Midnight", "Golden", "Silent", "Electric", "Broken", "Endless", "Hidden", "Neon"];
const THIRD = ["Dream", "River", "Fire", "City", "Echo", "Heart", "Signal", "Sky"];
const ARTISTS = ["Aurora", "Muse", "Kite", "Pulse", "Radiohead", "Mono", "Vega", "Nova"];

/// Deterministic synthetic tracks for the S6 filter/sort scenario: the same
/// count always yields the same rows so run-to-run numbers are comparable.
export function makeSyntheticTracks(count: number): Track[] {
  const tracks: Track[] = [];
  for (let i = 0; i < count; i++) {
    const title = `${FIRST[i % FIRST.length]} ${SECOND[(i * 7) % SECOND.length]} ${
      THIRD[(i * 13) % THIRD.length]
    } ${String(i).padStart(5, "0")}`;
    tracks.push({
      id: 900_000_000 + i,
      title,
      duration: 120 + (i % 240),
      artists: [{ id: 90_000 + (i % 50), name: ARTISTS[(i * 3) % ARTISTS.length] }],
      album: { id: 800_000_000 + i, title: `${THIRD[(i * 5) % THIRD.length]} Sessions`, cover: null },
      audioQuality: "LOSSLESS",
      trackNumber: (i % 12) + 1,
      volumeNumber: 1,
      explicit: false,
    });
  }
  return tracks;
}
```

In `ui/src/lib/perf.ts`, append:

```ts
export interface InteractionSummary {
  filter_count: number;
  filter_max_ms: number;
  sort_count: number;
  sort_max_ms: number;
}

/// Timed user interactions (S6 filter/sort) for the ui_metrics submission.
export class InteractionLog {
  private filterCount = 0;
  private filterMax = 0;
  private sortCount = 0;
  private sortMax = 0;

  record(kind: "filter" | "sort", ms: number) {
    if (kind === "filter") {
      this.filterCount += 1;
      this.filterMax = Math.max(this.filterMax, ms);
    } else {
      this.sortCount += 1;
      this.sortMax = Math.max(this.sortMax, ms);
    }
  }

  reset() {
    this.filterCount = 0;
    this.filterMax = 0;
    this.sortCount = 0;
    this.sortMax = 0;
  }

  summary(): InteractionSummary {
    return {
      filter_count: this.filterCount,
      filter_max_ms: this.filterMax,
      sort_count: this.sortCount,
      sort_max_ms: this.sortMax,
    };
  }
}
```

Create `ui/src/lib/perfHooks.ts`:

```ts
/// Perf-session-only hooks registered by the mounted Tracks view: the S6
/// synthetic fixture and the interaction timers. Empty outside a perf session
/// and cleared when the view unmounts, so normal runs cannot reach the fixture.
export interface PerfHooks {
  loadSyntheticTracks?: (count: number) => void;
  toggleViewSort?: (key: "title" | "duration") => void;
}

export const perfHooks: PerfHooks = {};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/perfFixture.test.ts src/lib/perf.test.ts`
Expected: PASS.

- [ ] **Step 5: Wire the perf session, bridge, and Tracks view**

In `ui/src/lib/perf.svelte.ts`, import and use the log:

```ts
import { FrameSampler, InteractionLog, LongTaskLog } from "./perf";
```

```ts
  const frames = new FrameSampler();
  const longTasks = new LongTaskLog();
  const interactions = new InteractionLog();
```

Add `interactions` to the submission and expose `recordInteraction`:

```ts
      await api.perfSubmitUiMetrics({
        scenario: snapshot?.scenario ?? "",
        mode: appearance.mode,
        frames: frames.summary(),
        long_tasks: longTasks.summary(),
        interactions: interactions.summary(),
      });
```

```ts
  /// Record a timed interaction and flush immediately: the run ends as soon as
  /// the driver's scenario does, so the periodic submit could lose the last one.
  function recordInteraction(kind: "filter" | "sort", ms: number) {
    interactions.record(kind, ms);
    void submit();
  }
```

Add `recordInteraction,` to the returned object (next to `mark`).

In `ui/src/App.svelte`, import the hook registry next to the perf import:

```ts
  import { perfHooks } from "./lib/perfHooks";
```

Extend the bridge object (inside the existing `if (!perf.active) return;` effect):

```ts
    const bridge = {
      goto: (name: string) => nav.goto({ name } as never),
      scrollTracks: (px: number) =>
        document.querySelector<HTMLElement>("[data-perf-scroll]")?.scrollBy({ top: px }),
      setFilter: (text: string) => {
        const input = document.querySelector<HTMLInputElement>("[data-perf-filter]");
        if (!input) return false;
        input.value = text;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        return true;
      },
      loadSyntheticTracks: (count: number) => {
        if (!perfHooks.loadSyntheticTracks) return false;
        perfHooks.loadSyntheticTracks(count);
        return true;
      },
      measureFilter: async (text: string) => {
        const input = document.querySelector<HTMLInputElement>("[data-perf-filter]");
        if (!input) return false;
        const started = performance.now();
        input.value = text;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        perf.recordInteraction("filter", performance.now() - started);
        return true;
      },
      measureSort: async (key: "title" | "duration") => {
        if (!perfHooks.toggleViewSort) return false;
        const started = performance.now();
        perfHooks.toggleViewSort(key);
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        perf.recordInteraction("sort", performance.now() - started);
        return true;
      },
      setMode: (mode: "instrument" | "atmosphere") => appearance.setMode(mode),
    };
```

In `ui/src/views/CollectionTracks.svelte`, add imports and the synthetic source:

```ts
  import { api, trackToQueueItem, type Track } from "../lib/api";
  import { perf } from "../lib/perf.svelte";
  import { makeSyntheticTracks } from "../lib/perfFixture";
  import { perfHooks } from "../lib/perfHooks";
```

```ts
  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

  /// S6-only: the perf bridge swaps in a synthetic 10k list so filter/sort
  /// budgets are measured at the spec's scale. Null in every normal run.
  let synthetic = $state<Track[] | null>(null);
  const rows = $derived(synthetic ?? favoritesPager.items);

  async function ensureAllFavorites() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) throw favoritesPager.error;
  }

  const view = createTrackView(() => rows, async () => {
    if (synthetic) return; // the synthetic fixture is already complete
    await ensureAllFavorites();
  });

  $effect(() => {
    if (!perf.active) return;
    perfHooks.loadSyntheticTracks = (count: number) => {
      synthetic = makeSyntheticTracks(count);
    };
    perfHooks.toggleViewSort = (key) => view.toggleSort(key);
    return () => {
      delete perfHooks.loadSyntheticTracks;
      delete perfHooks.toggleViewSort;
    };
  });
```

Change the count line in the template from `favoritesPager.items.length` to `rows.length` (both branches); the `unavailable` suffix still comes from the pager and stays 0 for the synthetic fixture.

- [ ] **Step 6: Run the suite, type-check, and commit**

Run: `npm --prefix ui test; if ($?) { npm --prefix ui run check }`
Expected: all green.

```bash
git add ui/src/lib/perfFixture.ts ui/src/lib/perfFixture.test.ts ui/src/lib/perfHooks.ts ui/src/lib/perf.ts ui/src/lib/perf.test.ts ui/src/lib/perf.svelte.ts ui/src/App.svelte ui/src/views/CollectionTracks.svelte
git commit -m "feat(perf): load a 10k synthetic library and time S6 interactions"
```

---

### Task 5: Gate S6 filter/sort and the app-owned pre-paint span

**Files:**
- Modify: `scripts/perf/check.mjs`, `scripts/perf/check.test.mjs`, `scripts/perf/budgets.json`, `scripts/perf/drive.mjs`

**Interfaces:**
- Consumes: `ui_metrics.interactions` (`{ filter_count, filter_max_ms, sort_count, sort_max_ms }`) and the startup marks (`logging_ready`, `config_loaded`, `cache_ready`, `services_ready`).
- Produces: metrics `filter_max_ms`, `sort_max_ms` (max across submissions), `startup_step_max_ms` (largest app-owned adjacent-mark delta); budget rows S6 `filter_max_ms ≤ 50`, S6 `sort_max_ms ≤ 150`, S1/S2 `startup_step_max_ms ≤ 100`; S6 driver using `loadSyntheticTracks(10000)`, `measureFilter`, `measureSort`.

- [ ] **Step 1: Write the failing tests**

Append to `scripts/perf/check.test.mjs`:

```js
test("ui interaction metrics aggregate S6 filter and sort maxima", () => {
  const file = runFile([
    { kind: "session", run_id: "S6-1", scenario: "S6" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: {
        interactions: { filter_count: 2, filter_max_ms: 18, sort_count: 1, sort_max_ms: 40 },
      },
    },
    {
      kind: "ui_metrics",
      t_ms: 2,
      metrics: {
        interactions: { filter_count: 1, filter_max_ms: 27, sort_count: 1, sort_max_ms: 33 },
      },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.filter_max_ms, 27);
  assert.equal(metrics.sort_max_ms, 40);
});

test("startup step metric measures the largest app-owned pre-paint step", () => {
  const file = runFile([
    { kind: "session", run_id: "S1-2", scenario: "S1" },
    { kind: "mark", name: "logging_ready", t_ms: 390 },
    { kind: "mark", name: "config_loaded", t_ms: 391 },
    { kind: "mark", name: "cache_ready", t_ms: 392 },
    { kind: "mark", name: "services_ready", t_ms: 441 },
    { kind: "mark", name: "window_shown", t_ms: 805 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.startup_step_max_ms, 49);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `node --test scripts/perf/check.test.mjs`
Expected: FAIL — `filter_max_ms` / `sort_max_ms` / `startup_step_max_ms` undefined.

- [ ] **Step 3: Implement the derivations**

In `scripts/perf/check.mjs`'s `metricsFor`, inside the `ui_metrics` branch after the long-tasks block:

```js
      const interactions = line.metrics?.interactions;
      if (interactions) {
        if (typeof interactions.filter_max_ms === "number") {
          metrics.filter_max_ms = Math.max(metrics.filter_max_ms ?? 0, interactions.filter_max_ms);
        }
        if (typeof interactions.sort_max_ms === "number") {
          metrics.sort_max_ms = Math.max(metrics.sort_max_ms ?? 0, interactions.sort_max_ms);
        }
      }
```

After the main loop, before the resolve/fetch derivations:

```js
  // The spec's pre-paint span rule: no application-owned startup step may
  // block the first frame for >100 ms. Runtime-owned spans (loader → first
  // mark, Tauri/WebView2 construction, dev-bundle render) are excluded.
  const appSteps = [
    ["logging_ready", "config_loaded"],
    ["config_loaded", "cache_ready"],
    ["cache_ready", "services_ready"],
  ]
    .filter(
      ([from, to]) =>
        metrics[`${from}_ms`] !== undefined && metrics[`${to}_ms`] !== undefined,
    )
    .map(([from, to]) => metrics[`${to}_ms`] - metrics[`${from}_ms`]);
  if (appSteps.length > 0) metrics.startup_step_max_ms = Math.max(...appSteps);
```

- [ ] **Step 4: Add the budget rows**

In `scripts/perf/budgets.json`:

- `S1`: after `{ "metric": "long_task_max_ms", "max": 100 }` add `{ "metric": "startup_step_max_ms", "max": 100 }`.
- `S2`: same addition.
- `S6`: add `{ "metric": "filter_max_ms", "max": 50 }` and `{ "metric": "sort_max_ms", "max": 150 }`.

- [ ] **Step 5: Drive S6 with the synthetic fixture**

In `scripts/perf/drive.mjs`, replace the `S6` branch with:

```js
    } else if (id.startsWith("S6")) {
      await awaitLoggedIn();
      await until("!!window.__perf", "__perf bridge");
      await applyScenarioMode();
      await evaluate('window.__perf.goto("tracks")');
      await until('!!document.querySelector("[data-perf-filter]")', "filter input");
      await until(
        'typeof window.__perf.loadSyntheticTracks === "function"',
        "synthetic fixture hook",
      );
      const loaded = await evaluate("window.__perf.loadSyntheticTracks(10000)");
      if (!loaded) throw new Error("S6: synthetic track fixture unavailable");
      await delay(2000); // let the 10k-row view render
      for (const query of ["a", "th", "the ", "the e", ""]) {
        await evaluate(`window.__perf.measureFilter(${JSON.stringify(query)})`);
        await delay(300);
      }
      for (const key of ["title", "duration", "title"]) {
        await evaluate(`window.__perf.measureSort(${JSON.stringify(key)})`);
        await delay(500);
      }
      await delay(3000);
    }
```

- [ ] **Step 6: Verify**

Run: `node --test scripts/perf/check.test.mjs; if ($?) { npm --prefix ui run check }`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add scripts/perf/check.mjs scripts/perf/check.test.mjs scripts/perf/budgets.json scripts/perf/drive.mjs
git commit -m "feat(perf): gate S6 filter/sort and the app-owned startup span"
```

---

### Task 6: Re-measure S5–S7 and close Phase C

**Files:**
- Create: `docs/superpowers/plans/notes/<execution-date>-track3-phase-c.md`
- Create: `docs/superpowers/plans/notes/<execution-date>-track3-phase-c-smoke.md`
- Modify: `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md`

**Interfaces:**
- Consumes: the runner (`scripts/perf/run-scenario.ps1`), `check.mjs`, the baseline and Phase B notes.
- Produces: the Phase C before/after record; the first S6 filter/sort and startup-span gate results.

- [ ] **Step 1: Quarantine the old corpus and re-run the Phase C scenarios (5 runs each)**

Move (never delete) the Phase A/B artifacts so each scenario's corpus is uniform:

```powershell
New-Item -ItemType Directory -Force .superpowers/perf/runs-quarantine/phase-b-ui | Out-Null
Move-Item -Path .superpowers/perf/runs/S5-*.jsonl,.superpowers/perf/runs/S6-*.jsonl,.superpowers/perf/runs/S7-*.jsonl -Destination .superpowers/perf/runs-quarantine/phase-b-ui/
```

Then, from the worktree:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-instrument -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S6 -Runs 5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S7 -Runs 5
```

Expected: S5 frame/long-task rows PASS, memory rows FAIL (Phase D); S6 frame rows PASS and the new `filter_max_ms` ≤ 50 / `sort_max_ms` ≤ 150 rows are measured; S7 memory row FAIL (Phase D).

- [ ] **Step 2: Read the results and decide (bounded)**

- If `filter_max_ms` breaches 50 ms, record the breach and its cause. Do **not** add a debounce silently: a debounce changes typing feel (T3-D5) and needs the owner's sign-off first; put it in the note as a proposed, undecided follow-up.
- If `sort_max_ms` breaches 150 ms, the same rule applies (do not weaken the budget; propose the fix).
- If `startup_step_max_ms` breaches 100 ms, identify which app-owned mark pair regressed and stop; the note records the offender.

- [ ] **Step 3: Write the Phase C note**

`docs/superpowers/plans/notes/<date>-track3-phase-c.md` with:
- Header (Date, Build = `track3-ui-library` debug + Vite dev, Machine, Conditions, run ids).
- Before/after table per scenario (baseline S5/S6/S7 rows → new p95), with PASS/FAIL.
- The new rows: S6 `filter_max_ms`, S6 `sort_max_ms`, S1/S2 `startup_step_max_ms`.
- Per-change attribution: player in-place updates, track-view keys/index, pager in-place/parallel, synthetic fixture (measurement only).
- Memory rows still FAIL (Phase D), budgets unchanged; any bounded-decision outcome from Step 2.

- [ ] **Step 4: Create the smoke checklist and hand it to the owner**

`docs/superpowers/plans/notes/<date>-track3-phase-c-smoke.md` (owner-run, real account/library):
- Tracks: type in the filter while the list is still paging; sort by title/duration asc/desc/original; play a row and play-all after filtering; unfavorite a row while sorted; confirm no visible layout or ordering change from before the phase.
- Playlists: open a playlist with a repeated track; drag-reorder a row; remove a row; re-open the playlist and confirm the order.
- S5/S6 modes: scroll and filter in both Instrument and Atmosphere; no mode-specific jank.
- Perf sessions only: start `run-scenario.ps1 -Scenario S6 -Runs 1` and confirm the synthetic 10k list appears during the run; then launch normally and confirm the real favorites show (the fixture never leaks).
Mark as pending until the owner closes it.

- [ ] **Step 5: Update the roadmap and commit**

Run the full local suite once before committing: `cargo test --workspace` + `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check` + `npm --prefix ui run test`, `node --test scripts/perf/check.test.mjs`.

Roadmap edits in `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md`:
- Add a `Revised:` line: `- **Revised:** <date> — Track 3 Phase C (UI + large library) complete; Phase D (memory/cache/stability) next.`
- Track 3 table row status: `Active — Phases A–C complete <date>; Phase D (memory, cache, stability) next`.
- In §3's Track 3 block, append a **Phase C complete** paragraph mirroring the Phase B one: filter/sort/player/pager fixes, the S6 synthetic gate, the startup-span gate, and the note/checklist paths.

```bash
git add docs/superpowers/plans/notes docs/superpowers/specs
git commit -m "test: record the Track 3 Phase C results"
```

---

## Self-Review Output

**Spec coverage:** §6 Phase C offender "whole-player.state replacement every 500 ms" → Task 1; "undebounced filter + localeCompare sort over the full list and O(n) sourceIndex" → Task 2 (no debounce: keys must meet the budget; a debounce is owner-gated per T3-D5); "O(n) array copies per pager window and in removeItem/moveItem" → Task 3; "sequential ensureAll walks" → Task 3; "non-virtualized lists only if measurements breach" → no task (S5/S6 frame rows pass in the baseline; re-checked in Task 6); "re-run S5–S7 including Atmosphere-overhead" → Task 6. Baseline-note deferred rows "10k filter/sort gate" → Tasks 4–5; "pre-paint span gate" → Task 5. Budgets unchanged/only added → Global Constraints.

**Placeholder scan:** no TBD/TODO; `<execution-date>`/`<date>` are intentional file-name and roadmap-text patterns resolved when the note is written.

**Type consistency:** `applyEvent` name/signature matches its tests and all call sites; `InteractionSummary` field names match `check.mjs` and `budgets.json` metric names (`filter_max_ms`, `sort_max_ms`); `makeSyntheticTracks`, `perfHooks.loadSyntheticTracks`, `perfHooks.toggleViewSort`, `perf.recordInteraction`, and the bridge methods `loadSyntheticTracks` / `measureFilter` / `measureSort` match across `perfFixture.ts`, `perfHooks.ts`, `perf.svelte.ts`, `App.svelte`, `CollectionTracks.svelte`, and `drive.mjs`; `startup_step_max_ms` matches between `check.mjs`, its test, and `budgets.json`.

**Review Focus tests:** lazy append mid-filter → Task 2 `keeps a filtered view consistent while a lazy page appends`; duplicate playlist reorder → Task 3 `reorders duplicate rows in place without corrupting the cache`; unfavorite/re-favorite → Task 3 `allows a removed row to come back from a later page`; interleaved events → Task 1 `keeps queue and position updates when events interleave`; synthetic leak → Task 4 effect cleanup + Task 6 smoke checklist (no component-test harness exists; owner-run per AGENTS.md).
