import { describe, expect, it, vi } from "vitest";
import type { RowTrack } from "./convert";
import { PagedTracks, TRACK_PAGE } from "./paged.svelte";

const row = (id: number): RowTrack => ({
  id,
  title: `Song ${id}`,
  artist: "Artist",
  album: "Album",
  cover: null,
  duration: 200,
  explicit: false,
  artistId: null,
  albumId: null,
});

/// Pages by offset: `ids` lists the rows each window brings back, `totals` what
/// it reports (default 0, no total).
function lister(windows: Record<number, { ids: number[]; total?: number }>) {
  return vi.fn(async (offset: number) => {
    const window = windows[offset] ?? { ids: [] };
    return { items: window.ids, total: window.total ?? 0 };
  });
}

async function readAll(paged: PagedTracks<number>) {
  await paged.first();
  while (!paged.done) await paged.more();
}

describe("PagedTracks", () => {
  it("reads to the known total past empty pages that report no total", async () => {
    const fetch = lister({
      0: { ids: [1, 2], total: 5 * TRACK_PAGE },
      [3 * TRACK_PAGE]: { ids: [3] },
    });
    const paged = new PagedTracks(fetch, row);
    await readAll(paged);
    expect(paged.rows.map((r) => r.id)).toEqual([1, 2, 3]);
    expect(paged.total).toBe(5 * TRACK_PAGE);
    expect(paged.left).toBe(0);
    expect(fetch.mock.calls.map(([offset]) => offset)).toEqual([0, 100, 200, 300, 400]);
  });

  it("ends at a short page when no total is ever reported", async () => {
    const fetch = lister({ 0: { ids: Array.from({ length: TRACK_PAGE }, (_, i) => i + 1) }, [TRACK_PAGE]: { ids: [101] } });
    const paged = new PagedTracks(fetch, row);
    await readAll(paged);
    expect(paged.rows).toHaveLength(101);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it("ends at once on an empty list", async () => {
    const fetch = lister({});
    const paged = new PagedTracks(fetch, row);
    await paged.first();
    expect(paged.done).toBe(true);
    expect(paged.total).toBe(0);
  });
});
