import { untrack } from "svelte";
import type { Track } from "./api";
import { sortData as appSortData, type SortData } from "./sortData.svelte";
import { sortMemory, type SortMemory } from "./sortMemory.svelte";
import {
  addLevel,
  flipLevel,
  moveLevel,
  needsData,
  removeLevel,
  setPrimary,
  sortTracks,
  type SortKey,
  type SortLevel,
} from "./trackSort";

export interface TrackView {
  readonly filter: string;
  /// The sort the controls show, primary first; empty = source order.
  readonly levels: readonly SortLevel[];
  /// levels non-empty or filter non-empty
  readonly engaged: boolean;
  /// ensureAll (or sort data) in flight; items keep their previous order
  readonly preparing: boolean;
  /// engage failed; items stay in source order until retry()
  readonly error: string;
  /// visible rows: source order until ready, then sorted/filtered
  readonly items: Track[];
  setFilter(query: string): void;
  /// Header click: only this key (cycles default → flipped → off), or flip
  /// it in place when it is one of several levels (MS-D6).
  toggleSort(key: SortKey): void;
  /// "+" / Shift+click: append as the next level (MS-D5/D6).
  addSort(key: SortKey): void;
  flipSort(index: number): void;
  removeSort(index: number): void;
  moveSort(from: number, to: number): void;
  /// Back to original order for the sort only; the filter is untouched.
  clearSort(): void;
  retry(): void;
  /// Index of items[visibleIndex] within the source list, by identity;
  /// -1 when absent.
  sourceIndex(visibleIndex: number): number;
}

export interface TrackViewOptions {
  /// Page key for the remembered sort (MS-D13); omit to never remember.
  memoryKey?: string;
  memory?: SortMemory | null;
  data?: SortData;
}

/// Per-track lower-cased title/artist parts. Track objects are stable while a
/// list is mounted, so a keystroke reuses them instead of lower-casing every
/// row again.
const searchParts = new WeakMap<Track, string[]>();

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

function matches(track: Track, needle: string): boolean {
  return searchable(track).some((part) => part.includes(needle));
}

/// Fill the per-track search cache in small macrotask chunks so the first
/// filter over a large list does not pay the whole build on the keystroke
/// (measured ~80 ms cold for 10k rows). Cached rows are skipped, so repeated
/// calls after pager appends are cheap. Returns a cancel function and a
/// promise that resolves when the walk finishes or is cancelled.
export function warmTrackSearch(
  tracks: Track[],
  chunkSize = 500,
): { cancel: () => void; done: Promise<void> } {
  let cancel = () => {};
  const done = new Promise<void>((resolve) => {
    let index = 0;
    let cancelled = false;
    cancel = () => {
      cancelled = true;
      resolve();
    };
    const step = () => {
      if (cancelled) return;
      const end = Math.min(index + chunkSize, tracks.length);
      for (; index < end; index++) searchable(tracks[index]);
      if (index < tracks.length) setTimeout(step, 0);
      else resolve();
    };
    setTimeout(step, 0);
  });
  return { cancel, done };
}

/// Resolve the queue start index for a row captured before an async load;
/// -1 when the captured track is no longer in the visible list.
export function visibleIndexOf(
  items: Track[],
  captured: Track | undefined,
  fallback: number,
): number {
  if (!captured) return fallback;
  return items.indexOf(captured);
}

/// Sort/filter state over a track source. Engaging a sort (or a filter) first
/// awaits `ensureAll` so the operation covers the complete list, then any data
/// the sort keys need (plays, release dates); until then the view reports
/// `preparing` and keeps serving its previous order.
export function createTrackView(
  source: () => Track[],
  ensureAll?: () => Promise<void>,
  options: TrackViewOptions = {},
): TrackView {
  const data = options.data ?? appSortData;
  const memoryKey = options.memoryKey;
  const memory = memoryKey ? untrack(() => options.memory ?? sortMemory()) : null;
  const remembered = memoryKey && memory ? untrack(() => memory.get(memoryKey)) : undefined;

  let filter = $state("");
  let levels = $state.raw<SortLevel[]>(remembered ?? []);
  /// What `items` sorts by: trails `levels` while their data loads.
  let applied = $state.raw<SortLevel[]>([]);
  let preparing = $state(false);
  let error = $state("");
  /// The complete list is loaded (ensureAll resolved) since engaging.
  let full = $state(false);
  let engageToken = 0;

  const engaged = $derived(filter.trim() !== "" || levels.length > 0);

  const items = $derived.by(() => {
    const list = source();
    if (!engaged || !full) return list;
    const needle = filter.trim().toLowerCase();
    const filtered = needle === "" ? list : list.filter((track) => matches(track, needle));
    return applied.length === 0 ? filtered : sortTracks(filtered, applied, data.context);
  });

  /// Identity map for `sourceIndex`: O(1) per call instead of an O(n) scan;
  /// the first occurrence wins, matching `indexOf` for repeated references.
  const sourceIndexMap = $derived.by(() => {
    const map = new Map<Track, number>();
    source().forEach((track, index) => {
      if (!map.has(track)) map.set(track, index);
    });
    return map;
  });

  async function prepare() {
    const token = ++engageToken;
    preparing = true;
    error = "";
    try {
      if (!full && ensureAll) await ensureAll();
      if (token !== engageToken) return;
      full = true;
      const target = levels;
      if (needsData(target)) {
        await data.ensure(
          target.map((level) => level.key),
          source(),
          true,
        );
        if (token !== engageToken) return;
      }
      applied = levels;
    } catch (e) {
      if (token !== engageToken) return;
      error = String(e);
    } finally {
      if (token === engageToken) preparing = false;
    }
  }

  function disengage() {
    engageToken += 1;
    full = false;
    applied = [];
    preparing = false;
    error = "";
  }

  /// Apply the current levels/filter: synchronously when the list is complete
  /// and no data is missing, else through `prepare`.
  function engage() {
    if (!engaged) {
      disengage();
      return;
    }
    if (!ensureAll) full = true;
    if (full && !needsData(levels)) {
      if (preparing) {
        engageToken += 1;
        preparing = false;
      }
      applied = levels;
      return;
    }
    void prepare();
  }

  function commit(next: SortLevel[]) {
    levels = next;
    if (memoryKey) memory?.set(memoryKey, next);
    engage();
  }

  if (remembered && remembered.length > 0) {
    // A remembered sort engages after construction: views may be built inside
    // a `$derived` (Playlist), where state writes are not allowed.
    queueMicrotask(() => {
      if (levels.length > 0 && applied.length === 0 && !preparing) engage();
    });
  }

  /// The filter applies through `items` once the list is complete; only the
  /// first engagement has to load it.
  function setFilter(query: string) {
    filter = query;
    if (!engaged) disengage();
    else if (!ensureAll) full = true;
    else if (!full && !preparing) void prepare();
  }

  function retry() {
    full = false;
    error = "";
    void prepare();
  }

  function sourceIndex(visibleIndex: number): number {
    const track = items[visibleIndex];
    if (!track) return -1;
    return sourceIndexMap.get(track) ?? -1;
  }

  return {
    get filter() {
      return filter;
    },
    get levels() {
      return levels;
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
    toggleSort: (key) => commit(setPrimary(levels, key)),
    addSort: (key) => commit(addLevel(levels, key)),
    flipSort: (index) => commit(flipLevel(levels, index)),
    removeSort: (index) => commit(removeLevel(levels, index)),
    moveSort: (from, to) => commit(moveLevel(levels, from, to)),
    clearSort: () => commit([]),
    retry,
    sourceIndex,
  };
}
