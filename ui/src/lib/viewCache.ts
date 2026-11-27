/// Session cache for page data: stale-while-revalidate.
///
/// Pages unmount when you leave them, so revisiting one used to refetch
/// everything. Through this cache a revisit renders at once from the last
/// response and, once that response is older than the source's `freshMs`,
/// quietly refetches and applies the result only if it changed. Past
/// `STALE_MS` an entry is not served at all (the page loads as new).
///
/// Bounded (LRU, `MAX_ENTRIES`), in memory only, cleared on sign-in/out.
/// Concurrent requests for one key share a single fetch, so a hover prefetch
/// and the click that follows it never double up. A fetch is shared only for
/// `JOIN_MS`; after that a visit starts its own, so one that never settles
/// cannot hold every revisit on "Loading…" until a restart (issue #10).

export interface Source<T> {
  key: string;
  fetch: () => Promise<T>;
  /// How long a response is served without asking again. 0 = always
  /// revalidate in the background (data the user edits, e.g. playlists).
  freshMs?: number;
}

export const DEFAULT_FRESH_MS = 60_000;
export const STALE_MS = 30 * 60_000;
export const MAX_ENTRIES = 80;
/// How long a running fetch is joined by later requests for its key. Longer
/// than a large list takes to load; a fetch still running past it is more
/// likely stuck than slow, and a new one costs only a duplicate request.
export const JOIN_MS = 60_000;

interface Entry {
  value: unknown;
  at: number;
}

interface Running {
  request: Promise<unknown>;
  at: number;
}

const entries = new Map<string, Entry>();
const inflight = new Map<string, Running>();
/// Bumped by `clearViewCache`/`invalidate`: a fetch started before it must not
/// write its (possibly pre-logout or pre-edit) result back.
let generation = 0;

const now = () => Date.now();

function servable(key: string): Entry | undefined {
  const entry = entries.get(key);
  if (!entry) return undefined;
  if (now() - entry.at > STALE_MS) {
    entries.delete(key);
    return undefined;
  }
  // Refresh LRU position.
  entries.delete(key);
  entries.set(key, entry);
  return entry;
}

function store(key: string, value: unknown) {
  entries.delete(key);
  entries.set(key, { value, at: now() });
  while (entries.size > MAX_ENTRIES) {
    const oldest = entries.keys().next().value as string;
    entries.delete(oldest);
  }
}

/// The running fetch for `key`, while it is young enough to join.
function joinable(key: string): Promise<unknown> | undefined {
  const running = inflight.get(key);
  return running && now() - running.at < JOIN_MS ? running.request : undefined;
}

/// Fetch through the cache: one request per key at a time (see `JOIN_MS`);
/// the result is stored.
export function fetchShared<T>(source: Source<T>): Promise<T> {
  const running = joinable(source.key);
  if (running) return running as Promise<T>;
  const started = generation;
  const request: Promise<T> = source
    .fetch()
    .then((value) => {
      if (started === generation) store(source.key, value);
      return value;
    })
    .finally(() => {
      if (inflight.get(source.key)?.request === request) inflight.delete(source.key);
    });
  inflight.set(source.key, { request, at: now() });
  return request;
}

/// Current data without repeating a request the page just made: joins a
/// running fetch, else serves a value stored under `maxAgeMs` ago, else
/// fetches. The Playlists filter walks folders this way, and the grid's
/// folder covers have usually just loaded the top level.
export function fetchRecent<T>(source: Source<T>, maxAgeMs: number): Promise<T> {
  const running = joinable(source.key);
  if (running) return running as Promise<T>;
  const entry = servable(source.key);
  if (entry && now() - entry.at < maxAgeMs) return Promise.resolve(entry.value as T);
  return fetchShared(source);
}

/// The cached value when one may be shown (synchronous; for initial state).
export function peek<T>(source: Source<T>): T | undefined {
  return servable(source.key)?.value as T | undefined;
}

function isFresh(source: Source<unknown>, entry: Entry): boolean {
  return now() - entry.at < (source.freshMs ?? DEFAULT_FRESH_MS);
}

/// Load a page's data: a cached value is applied synchronously, then, if it is
/// not fresh, the source is refetched in the background and the new value
/// applied only when it differs. Without a cached value this awaits the fetch
/// and rethrows its error (the page shows its error state); a failed
/// background refresh keeps the cached value.
export async function load<T>(source: Source<T>, apply: (value: T) => void): Promise<void> {
  const entry = servable(source.key);
  if (!entry) {
    apply(await fetchShared(source));
    return;
  }
  apply(entry.value as T);
  if (isFresh(source, entry)) return;
  try {
    const next = await fetchShared(source);
    if (!sameValue(entry.value, next)) apply(next);
  } catch {
    // Keep showing what we have.
  }
}

/// Warm a source ahead of a likely visit (card hover). No-op when fresh or
/// already loading; errors are swallowed (the visit will retry).
export function prefetch(source: Source<unknown>) {
  const entry = servable(source.key);
  if (entry && isFresh(source, entry)) return;
  if (joinable(source.key)) return;
  fetchShared(source).catch(() => {});
}

/// Drop one key, or every key starting with `prefix` + ":".
export function invalidate(keyOrPrefix: string) {
  generation += 1;
  for (const key of [...entries.keys()]) {
    if (key === keyOrPrefix || key.startsWith(`${keyOrPrefix}:`)) entries.delete(key);
  }
}

/// Forget everything; called when the signed-in account changes.
export function clearViewCache() {
  generation += 1;
  entries.clear();
  inflight.clear();
}

function sameValue(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  try {
    return JSON.stringify(a) === JSON.stringify(b);
  } catch {
    return false;
  }
}
