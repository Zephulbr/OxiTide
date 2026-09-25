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
