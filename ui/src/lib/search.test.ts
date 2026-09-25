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
