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
      settledQuery = trimmed;
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
