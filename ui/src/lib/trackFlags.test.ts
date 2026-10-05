import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { createTrackFlags, remapStart, type TrackLike } from "./trackFlags.svelte";

vi.mock("./api", () => ({
  api: {
    getConfig: vi.fn(),
    getTrackFlags: vi.fn(),
  },
}));

function track(id: number, explicit = false): TrackLike {
  return { id, explicit };
}

beforeEach(() => {
  vi.mocked(api.getConfig).mockResolvedValue({
    content: { block_explicit: false, block_ai: false },
  } as never);
  vi.mocked(api.getTrackFlags).mockReset();
});

describe("remapStart", () => {
  it("follows the original track or the next kept one", () => {
    expect(remapStart([1, 2, 3], [1, 2, 3], 2)).toBe(2);
    expect(remapStart([1, 2, 3], [2, 3], 0)).toBe(0);
    expect(remapStart([1, 2, 3], [1, 3], 1)).toBe(1);
    expect(remapStart([1, 2, 3], [1, 2], 2)).toBe(1);
    expect(remapStart([1, 2], [1], 99)).toBe(0);
  });
});

describe("filterPlayable", () => {
  it("keeps everything and skips the network when filters are off", async () => {
    const store = createTrackFlags();
    const result = await store.filterPlayable([track(1, true), track(2)], 1);
    expect(result?.items.map((t) => t.id)).toEqual([1, 2]);
    expect(result?.start).toBe(1);
    expect(api.getTrackFlags).not.toHaveBeenCalled();
  });

  it("drops explicit tracks when only that filter is on", async () => {
    const store = createTrackFlags();
    store.setContentFlags(true, false);
    const result = await store.filterPlayable([track(1, true), track(2)], 1);
    expect(result?.items.map((t) => t.id)).toEqual([2]);
    expect(result?.start).toBe(0);
    expect(api.getTrackFlags).not.toHaveBeenCalled();
  });

  it("enriches and drops AI tracks when that filter is on", async () => {
    vi.mocked(api.getTrackFlags).mockResolvedValue([
      { track_id: 1, explicit: false, ai: true },
      { track_id: 2, explicit: false, ai: false },
    ]);
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    const result = await store.filterPlayable([track(1), track(2)], 0);
    expect(api.getTrackFlags).toHaveBeenCalledTimes(1);
    expect(vi.mocked(api.getTrackFlags).mock.calls[0][0].sort()).toEqual([1, 2]);
    expect(result?.items.map((t) => t.id)).toEqual([2]);
  });

  it("returns null when everything is filtered", async () => {
    const store = createTrackFlags();
    store.setContentFlags(true, false);
    expect(await store.filterPlayable([track(1, true)], 0)).toBeNull();
  });

  it("fails open when enrichment rejects", async () => {
    vi.mocked(api.getTrackFlags).mockRejectedValue(new Error("offline"));
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    const result = await store.filterPlayable([track(1), track(2)], 0);
    expect(result?.items.map((t) => t.id)).toEqual([1, 2]);
  });
});

describe("ensure", () => {
  it("shares one request between same-tick callers", async () => {
    vi.mocked(api.getTrackFlags).mockResolvedValue([
      { track_id: 1, explicit: false, ai: false },
      { track_id: 2, explicit: false, ai: false },
    ]);
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    await Promise.all([store.ensure([1]), store.ensure([2])]);
    expect(api.getTrackFlags).toHaveBeenCalledTimes(1);
    const batch = vi.mocked(api.getTrackFlags).mock.calls[0][0].sort();
    expect(batch).toEqual([1, 2]);
  });

  it("does nothing when the AI filter is off", async () => {
    const store = createTrackFlags();
    await store.ensure([1, 2]);
    expect(api.getTrackFlags).not.toHaveBeenCalled();
  });

  it("does not re-request ids whose flags came back unknown", async () => {
    vi.mocked(api.getTrackFlags).mockResolvedValue([{ track_id: 1, explicit: null, ai: null }]);
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    await store.ensure([1]);
    await store.ensure([1]);
    expect(api.getTrackFlags).toHaveBeenCalledTimes(1);
  });

  it("still fetches after recordTracks seeds only explicit", async () => {
    vi.mocked(api.getTrackFlags).mockResolvedValue([{ track_id: 1, explicit: true, ai: false }]);
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    store.recordTracks([track(1, true)]);
    await store.ensure([1]);
    expect(api.getTrackFlags).toHaveBeenCalledTimes(1);
    expect(store.isBlocked(track(1, true))).toBe(false);
  });
});

describe("init and reset", () => {
  it("keeps the config toggles across reset and clears the flag cache", async () => {
    vi.mocked(api.getConfig).mockResolvedValue({
      content: { block_explicit: true, block_ai: true },
    } as never);
    const store = createTrackFlags();
    await store.init();
    expect(store.blockExplicit).toBe(true);
    expect(store.blockAi).toBe(true);
    store.reset();
    expect(store.blockExplicit).toBe(true);
    expect(store.blockAi).toBe(true);
    expect(store.isBlocked({ id: 1, explicit: true })).toBe(false);
  });
});
