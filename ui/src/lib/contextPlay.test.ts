import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Track } from "./api";

vi.mock("./api", () => ({
  api: {
    playTracks: vi.fn(async () => 12),
    queueExtendContext: vi.fn(async () => true),
  },
  trackToQueueItem: (t: Track) => ({ track_id: t.id }),
}));
vi.mock("./trackFlags.svelte", () => ({
  trackFlags: {
    filterPlayable: vi.fn(async (items: Track[], start: number) => ({ items, start })),
  },
}));
vi.mock("./playSource.svelte", () => {
  const source = {
    from: null as unknown,
    set: vi.fn((from: unknown) => {
      source.from = from;
    }),
  };
  return { playSource: source };
});

import { api } from "./api";
import { claimPlay, createEarlyPlays, extendPlay, restAfter, startPlay, startsEarly } from "./contextPlay";
import type { Pager } from "./pager.svelte";
import { playSource } from "./playSource.svelte";
import { trackFlags } from "./trackFlags.svelte";

const track = (id: number) => ({ id, title: `T${id}` }) as Track;
const tracks = (...ids: number[]) => ids.map(track);
const idsOf = (list: readonly Track[]) => list.map((t) => t.id);
const FROM = { kind: "favorites", name: "Tracks", route: { name: "tracks" } } as const;
const OTHER = { kind: "album", name: "LP", route: { name: "album", id: 3 } } as const;

beforeEach(() => {
  vi.clearAllMocks();
  (playSource as { from: unknown }).from = null;
});

describe("claimPlay", () => {
  it("a newer claim retires the older one", () => {
    const first = claimPlay();
    expect(first()).toBe(true);
    const second = claimPlay();
    expect(first()).toBe(false);
    expect(second()).toBe(true);
  });
});

describe("restAfter", () => {
  it("is the rest of the list while it starts with the head, repeats included", () => {
    expect(idsOf(restAfter(tracks(1, 2), tracks(1, 2, 2, 3)))).toEqual([2, 3]);
  });

  it("falls back to the tracks not queued when the list changed", () => {
    expect(idsOf(restAfter(tracks(9, 1), tracks(1, 2, 9, 3)))).toEqual([2, 3]);
  });
});

describe("startPlay", () => {
  it("plays the filtered list and returns the context", async () => {
    const started = await startPlay(FROM, tracks(1, 2, 3), 1);
    expect(started).toEqual({ context: 12, ids: [1, 2, 3] });
    expect(api.playTracks).toHaveBeenCalledWith([{ track_id: 1 }, { track_id: 2 }, { track_id: 3 }], 1, false);
    expect(playSource.set).toHaveBeenCalledWith(FROM, [1, 2, 3]);
  });

  it("a shuffle starts the shuffled context at the top", async () => {
    await startPlay(FROM, tracks(1, 2, 3), 2, { shuffle: true });
    expect(vi.mocked(api.playTracks).mock.calls[0].slice(1)).toEqual([0, true]);
  });

  it("is hidden when the filters hide everything", async () => {
    vi.mocked(trackFlags.filterPlayable).mockResolvedValueOnce(null);
    expect(await startPlay(FROM, tracks(1), 0)).toBe("hidden");
    expect(await startPlay(FROM, [], 0)).toBe("hidden");
    expect(api.playTracks).not.toHaveBeenCalled();
  });

  it("gives way to a play claimed while the filters were checked", async () => {
    const isLatest = claimPlay();
    vi.mocked(trackFlags.filterPlayable).mockImplementationOnce((async (items: Track[], start: number) => {
      claimPlay();
      return { items, start };
    }) as never);
    expect(await startPlay(FROM, tracks(1), 0, { isLatest })).toBe("superseded");
    expect(api.playTracks).not.toHaveBeenCalled();
    expect(playSource.set).not.toHaveBeenCalled();
  });
});

describe("extendPlay", () => {
  const started = { context: 12, ids: [1, 2] };

  it("adds the rest to the context and credits it to the source", async () => {
    (playSource as { from: unknown }).from = { ...FROM, route: { ...FROM.route } };
    await extendPlay(FROM, started, tracks(3, 4));
    expect(api.queueExtendContext).toHaveBeenCalledWith([{ track_id: 3 }, { track_id: 4 }], 12);
    expect(playSource.set).toHaveBeenCalledWith(FROM, [1, 2, 3, 4]);
  });

  it("leaves another source's credit alone", async () => {
    (playSource as { from: unknown }).from = OTHER;
    await extendPlay(FROM, started, tracks(3));
    expect(api.queueExtendContext).toHaveBeenCalled();
    expect(playSource.set).not.toHaveBeenCalled();
  });

  /// The engine decides, not the claims: a play clicked later that never
  /// started (its load failed) must not cut this queue short.
  it("sends the rest even after a newer claim, and credits it only if it joined", async () => {
    (playSource as { from: unknown }).from = FROM;
    claimPlay();
    vi.mocked(api.queueExtendContext).mockResolvedValueOnce(false);
    await extendPlay(FROM, started, tracks(3));
    expect(api.queueExtendContext).toHaveBeenCalledWith([{ track_id: 3 }], 12);
    expect(playSource.set).not.toHaveBeenCalled();
  });

  it("is best effort", async () => {
    vi.mocked(api.queueExtendContext).mockRejectedValueOnce(new Error("offline"));
    await expect(extendPlay(FROM, started, tracks(3))).resolves.toBeUndefined();
  });
});

/// A pager holding `loaded` rows of `all`; `finishLoad` completes or fails
/// the pending load.
function fakePager(all: Track[], loaded: number) {
  let release: (ok: boolean) => void = () => {};
  const pager = {
    items: all.slice(0, loaded),
    done: false,
    error: "",
    ensureAll: vi.fn(async () => {
      pager.error = "";
      const ok = await new Promise<boolean>((resolve) => (release = resolve));
      if (ok) {
        pager.items = [...all];
        pager.done = true;
      } else {
        pager.error = "offline";
      }
    }),
    loadMore: vi.fn(async () => {}),
  };
  return { pager, finishLoad: (ok: boolean) => release(ok), asPager: pager as unknown as Pager };
}

const flush = () => new Promise((r) => setTimeout(r));

describe("createEarlyPlays", () => {
  it("starts early only while the list loads in its own order", () => {
    const { pager, asPager } = fakePager(tracks(1, 2), 1);
    expect(startsEarly(asPager, { engaged: false })).toBe(true);
    expect(startsEarly(asPager, { engaged: true })).toBe(false);
    pager.done = true;
    expect(startsEarly(asPager, { engaged: false })).toBe(false);
  });

  it("plays the loaded rows now and adds the rest once loaded", async () => {
    const { pager, finishLoad, asPager } = fakePager(tracks(1, 2, 3, 4), 2);
    const early = createEarlyPlays();

    const result = await early.play(asPager, FROM, pager.items, 1, claimPlay());

    expect(result).toEqual({ context: 12, ids: [1, 2] });
    expect(vi.mocked(api.playTracks).mock.calls[0].slice(0, 2)).toEqual([[{ track_id: 1 }, { track_id: 2 }], 1]);
    finishLoad(true);
    await vi.waitFor(() =>
      expect(api.queueExtendContext).toHaveBeenCalledWith([{ track_id: 3 }, { track_id: 4 }], 12),
    );
  });

  it("finishes the queue on Retry after a failed load", async () => {
    const { pager, finishLoad, asPager } = fakePager(tracks(1, 2, 3), 1);
    const early = createEarlyPlays();
    await early.play(asPager, FROM, pager.items, 0, claimPlay());
    finishLoad(false);
    await flush();
    expect(api.queueExtendContext).not.toHaveBeenCalled();

    early.retry(asPager);
    expect(pager.loadMore).not.toHaveBeenCalled();
    finishLoad(true);
    await vi.waitFor(() => expect(api.queueExtendContext).toHaveBeenCalledWith([{ track_id: 2 }, { track_id: 3 }], 12));
  });

  it("a Retry on another list just loads it", async () => {
    const first = fakePager(tracks(1, 2), 1);
    const other = fakePager(tracks(5, 6), 1);
    const early = createEarlyPlays();
    await early.play(first.asPager, FROM, first.pager.items, 0, claimPlay());
    first.finishLoad(false);
    await flush();

    early.retry(other.asPager);
    expect(other.pager.loadMore).toHaveBeenCalled();
    expect(first.pager.ensureAll).toHaveBeenCalledTimes(1);
  });

  it("does not start a play that a newer one superseded", async () => {
    const { pager, asPager } = fakePager(tracks(1, 2), 1);
    const isLatest = claimPlay();
    claimPlay();
    expect(await createEarlyPlays().play(asPager, FROM, pager.items, 0, isLatest)).toBe("superseded");
    expect(api.playTracks).not.toHaveBeenCalled();
    expect(pager.ensureAll).not.toHaveBeenCalled();
  });
});
