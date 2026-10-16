import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearViewCache,
  DEFAULT_FRESH_MS,
  fetchShared,
  invalidate,
  load,
  MAX_ENTRIES,
  peek,
  prefetch,
  STALE_MS,
  type Source,
} from "./viewCache";

function source<T>(key: string, values: T[], freshMs?: number) {
  const fetch = vi.fn(async () => {
    const next = values.length > 1 ? values.shift()! : values[0];
    return next;
  });
  return { key, fetch, freshMs } satisfies Source<T>;
}

beforeEach(() => {
  vi.useFakeTimers();
  clearViewCache();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("viewCache", () => {
  it("fetches on a miss and serves the next visit from cache without a request", async () => {
    const src = source("a", [1]);
    const seen: number[] = [];
    await load(src, (v) => seen.push(v));
    await load(src, (v) => seen.push(v));
    expect(seen).toEqual([1, 1]);
    expect(src.fetch).toHaveBeenCalledTimes(1);
  });

  it("applies a cached value synchronously", async () => {
    const src = source("a", [1]);
    await load(src, () => {});
    let applied = false;
    void load(src, () => (applied = true));
    expect(applied).toBe(true);
  });

  it("revalidates a stale-but-servable entry and applies only a changed value", async () => {
    const src = source("a", [{ n: 1 }, { n: 1 }, { n: 2 }]);
    await load(src, () => {});
    vi.advanceTimersByTime(DEFAULT_FRESH_MS + 1);
    const first: unknown[] = [];
    await load(src, (v) => first.push(v));
    // Cached value, then an identical refetch: applied once.
    expect(first).toEqual([{ n: 1 }]);
    vi.advanceTimersByTime(DEFAULT_FRESH_MS + 1);
    const second: unknown[] = [];
    await load(src, (v) => second.push(v));
    expect(second).toEqual([{ n: 1 }, { n: 2 }]);
  });

  it("always revalidates a freshMs: 0 source", async () => {
    const src = source("p", [1, 2], 0);
    await load(src, () => {});
    const seen: number[] = [];
    await load(src, (v) => seen.push(v));
    expect(seen).toEqual([1, 2]);
  });

  it("stops serving entries past STALE_MS", async () => {
    const src = source("a", [1]);
    await load(src, () => {});
    vi.advanceTimersByTime(STALE_MS + 1);
    expect(peek(src)).toBeUndefined();
  });

  it("keeps the cached value when a background refresh fails", async () => {
    const src: Source<number> = {
      key: "a",
      fetch: vi.fn<() => Promise<number>>().mockResolvedValueOnce(1).mockRejectedValue(new Error("x")),
    };
    await load(src, () => {});
    vi.advanceTimersByTime(DEFAULT_FRESH_MS + 1);
    const seen: number[] = [];
    await expect(load(src, (v) => seen.push(v))).resolves.toBeUndefined();
    expect(seen).toEqual([1]);
    expect(peek(src)).toBe(1);
  });

  it("rethrows a failed first load", async () => {
    const src = { key: "a", fetch: vi.fn().mockRejectedValue(new Error("down")) };
    await expect(load(src, () => {})).rejects.toThrow("down");
    expect(peek(src)).toBeUndefined();
  });

  it("shares one request between a prefetch and the visit that follows", async () => {
    let resolve!: (v: number) => void;
    const src = { key: "a", fetch: vi.fn(() => new Promise<number>((r) => (resolve = r))) };
    prefetch(src);
    const visit = load(src, () => {});
    resolve(7);
    await visit;
    expect(src.fetch).toHaveBeenCalledTimes(1);
    expect(peek(src)).toBe(7);
  });

  it("prefetch skips fresh entries", async () => {
    const src = source("a", [1]);
    await load(src, () => {});
    prefetch(src);
    expect(src.fetch).toHaveBeenCalledTimes(1);
  });

  it("invalidates a key and its sub-keys, and drops a fetch started before", async () => {
    await load(source("artist:1", [1]), () => {});
    await load(source("artist:1:top", [1]), () => {});
    await load(source("artist:10", [1]), () => {});
    let resolve!: (v: number) => void;
    const slow = { key: "album:5", fetch: () => new Promise<number>((r) => (resolve = r)) };
    const pending = fetchShared(slow);
    invalidate("artist:1");
    resolve(3);
    await pending;
    expect(peek({ key: "artist:1", fetch: async () => 0 })).toBeUndefined();
    expect(peek({ key: "artist:1:top", fetch: async () => 0 })).toBeUndefined();
    expect(peek({ key: "artist:10", fetch: async () => 0 })).toBe(1);
    expect(peek(slow)).toBeUndefined();
  });

  it("evicts the least recently used entry past MAX_ENTRIES", async () => {
    for (let i = 0; i < MAX_ENTRIES; i++) await load(source(`k${i}`, [i]), () => {});
    // Touch k0 so k1 becomes the oldest.
    expect(peek({ key: "k0", fetch: async () => 0 })).toBe(0);
    await load(source("extra", [1]), () => {});
    expect(peek({ key: "k0", fetch: async () => 0 })).toBe(0);
    expect(peek({ key: "k1", fetch: async () => 0 })).toBeUndefined();
  });

  it("clearViewCache forgets everything", async () => {
    const src = source("a", [1]);
    await load(src, () => {});
    clearViewCache();
    expect(peek(src)).toBeUndefined();
  });
});
