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
/// and the click that follows it never double up.

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

interface Entry {
  value: unknown;
  at: number;
}

const entries = new Map<string, Entry>();
const inflight = new Map<string, Promise<unknown>>();
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

/// Fetch through the cache: one request per key at a time; the result is stored.
export function fetchShared<T>(source: Source<T>): Promise<T> {
  const running = inflight.get(source.key);
  if (running) return running as Promise<T>;
  const started = generation;
  const request = source
    .fetch()
    .then((value) => {
      if (started === generation) store(source.key, value);
      return value;
    })
    .finally(() => {
      if (inflight.get(source.key) === request) inflight.delete(source.key);
    });
  inflight.set(source.key, request);
  return request;
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
  if (inflight.has(source.key)) return;
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
