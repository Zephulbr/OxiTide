import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Track } from "./api";

vi.mock("./api", () => ({
  api: {
    getArtistTopTracksPage: vi.fn(),
    playTracks: vi.fn(async () => 41),
    queueExtendContext: vi.fn(async () => {}),
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

import { api, type TrackPage } from "./api";
import { FIRST_PAGE, artistTracksPager, firstThenArtist, playArtistTracks } from "./artistTracks";
import { clearPagerCache } from "./pager.svelte";
import { playSource } from "./playSource.svelte";

const track = (id: number): Track => ({
  id,
  title: `T${id}`,
  duration: 1,
  artists: [{ id: 7, name: "Band" }],
  trackNumber: 1,
  volumeNumber: 1,
  explicit: false,
});

beforeEach(() => {
  vi.clearAllMocks();
  clearPagerCache();
});

describe("firstThenArtist", () => {
  it("puts the picked track first without repeating it", () => {
    const all = [track(1), track(2), track(3)];
    expect(firstThenArtist(track(2), all).map((t) => t.id)).toEqual([2, 1, 3]);
  });

  it("keeps a picked track the artist list lacks", () => {
    expect(firstThenArtist(track(9), [track(1)]).map((t) => t.id)).toEqual([9, 1]);
  });

  it("is the artist list without a pick", () => {
    expect(firstThenArtist(undefined, [track(1), track(2)]).map((t) => t.id)).toEqual([1, 2]);
  });
});

const BAND = { id: 7, name: "Band" };
const FROM = { kind: "artist", name: "Band", route: { name: "artist", id: 7 } };

/// A server holding `count` tracks with ids 1..count, served in windows.
function serve(count: number) {
  vi.mocked(api.getArtistTopTracksPage).mockImplementation(async (_id, offset, limit) => {
    const ids = Array.from({ length: Math.max(0, Math.min(limit, count - offset)) }, (_, i) => offset + i + 1);
    return { items: ids.map(track), total: count, offset, limit } satisfies TrackPage;
  });
}

const queued = (call: unknown[]) => (call[0] as { track_id: number }[]).map((item) => item.track_id);

describe("playArtistTracks (issue #3)", () => {
  it("plays a small artist's whole list at once", async () => {
    serve(3);

    expect(await playArtistTracks(BAND, track(3))).toBe(true);

    expect(api.getArtistTopTracksPage).toHaveBeenCalledTimes(1);
    expect(api.getArtistTopTracksPage).toHaveBeenCalledWith(7, 0, FIRST_PAGE);
    expect(queued(vi.mocked(api.playTracks).mock.calls[0])).toEqual([3, 1, 2]);
    expect(api.queueExtendContext).not.toHaveBeenCalled();
    expect(playSource.set).toHaveBeenCalledWith(FROM, [3, 1, 2]);
  });

  it("starts on the first page, then adds the rest at the play's revision", async () => {
    serve(250);

    await playArtistTracks(BAND, track(180));

    const head = queued(vi.mocked(api.playTracks).mock.calls[0]);
    expect(head[0]).toBe(180);
    expect(head).toHaveLength(FIRST_PAGE + 1);
    await vi.waitFor(() => expect(api.queueExtendContext).toHaveBeenCalled());
    const [items, rev] = vi.mocked(api.queueExtendContext).mock.calls[0];
    expect(rev).toBe(41);
    const rest = items.map((item) => item.track_id);
    expect(rest).not.toContain(180);
    expect(new Set([...head, ...rest]).size).toBe(250);
    expect(head.length + rest.length).toBe(250);
    await vi.waitFor(() =>
      expect(vi.mocked(playSource.set).mock.lastCall?.[1]).toHaveLength(250),
    );
  });

  it("plays a cached list whole without fetching", async () => {
    serve(250);
    await artistTracksPager(7).ensureAll();
    vi.mocked(api.getArtistTopTracksPage).mockClear();

    await playArtistTracks(BAND, track(5));

    expect(api.getArtistTopTracksPage).not.toHaveBeenCalled();
    expect(queued(vi.mocked(api.playTracks).mock.calls[0])).toHaveLength(250);
    expect(api.queueExtendContext).not.toHaveBeenCalled();
  });

  it("throws when the first page fails to load", async () => {
    vi.mocked(api.getArtistTopTracksPage).mockRejectedValue(new Error("offline"));
    await expect(playArtistTracks(BAND, track(3))).rejects.toThrow("offline");
    expect(api.playTracks).not.toHaveBeenCalled();
  });
});
