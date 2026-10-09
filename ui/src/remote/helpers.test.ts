import { describe, expect, it } from "vitest";
import type { PageItem, RemoteState } from "../lib/api";
import { fromPageItem, fromTrack, toPageInput, toQueueItem } from "./convert";
import { clock, nextRepeat, qualityBadges } from "./nowPlaying";
import { QUEUE_PAGE, QueueCache } from "./queueCache";
import { dropIndex, visibleRange } from "./queueDrag";
import { hrefFor, parseHash, routeForItem, tabFor, type RemoteRoute } from "./routes";

describe("queue cache", () => {
  it("drops pages when the revision or length changes", () => {
    const cache = new QueueCache();
    expect(cache.sync(0, 250)).toBe(true);
    expect(cache.missing(0, 250)).toEqual([0, 1, 2]);
    cache.set(1, [{ track_id: 9, title: "x", artist: "", album: "", duration_secs: 1 }], 0);
    expect(cache.get(QUEUE_PAGE)?.track_id).toBe(9);
    expect(cache.missing(90, 210)).toEqual([0, 2]);
    expect(cache.sync(0, 250)).toBe(false);
    expect(cache.sync(1, 250)).toBe(true);
    expect(cache.get(QUEUE_PAGE)).toBeUndefined();
    expect(cache.set(0, [], 0)).toBe(false);
    expect(cache.missing(0, 0)).toEqual([]);
  });
});

describe("queue drag and window", () => {
  it("drops on whole rows within the queue", () => {
    expect(dropIndex(5, 130, 64, 10)).toBe(7);
    expect(dropIndex(5, -500, 64, 10)).toBe(0);
    expect(dropIndex(5, 5000, 64, 10)).toBe(9);
    expect(dropIndex(5, 20, 64, 10)).toBe(5);
  });

  it("renders only rows near the viewport", () => {
    expect(visibleRange(0, 640, 64, 10_000, 4)).toEqual({ from: 0, to: 14 });
    expect(visibleRange(64_000, 640, 64, 10_000, 4)).toEqual({ from: 996, to: 1014 });
    expect(visibleRange(-300, 640, 64, 5, 4)).toEqual({ from: 0, to: 5 });
  });
});

describe("routes", () => {
  it("round-trips every route", () => {
    const routes: RemoteRoute[] = [
      { name: "now" },
      { name: "queue" },
      { name: "search" },
      { name: "home" },
      { name: "library" },
      { name: "favorites", kind: "albums" },
      { name: "playlists", folder: "root" },
      { name: "playlists", folder: "abc-123" },
      { name: "album", id: 42 },
      { name: "artist", id: 7 },
      { name: "playlist", id: "1f2e-3d" },
      { name: "mix", id: "0123abc" },
    ];
    for (const route of routes) expect(parseHash(hrefFor(route))).toEqual(route);
  });

  it("falls back to Now Playing on junk", () => {
    expect(parseHash("")).toEqual({ name: "now" });
    expect(parseHash("#/album/abc")).toEqual({ name: "now" });
    expect(parseHash("#/nope")).toEqual({ name: "now" });
    expect(parseHash("#p=token")).toEqual({ name: "now" });
  });

  it("maps feed items and tabs", () => {
    const item = (item_type: string, id: string): PageItem => ({ id, title: "t", item_type });
    expect(routeForItem(item("ALBUM", "12"))).toEqual({ name: "album", id: 12 });
    expect(routeForItem(item("PLAYLIST", "uuid-x"))).toEqual({ name: "playlist", id: "uuid-x" });
    expect(routeForItem(item("MIX", "m1"))).toEqual({ name: "mix", id: "m1" });
    expect(routeForItem(item("ARTIST", "3"))).toEqual({ name: "artist", id: 3 });
    expect(routeForItem(item("TRACK", "5"))).toBeNull();
    expect(tabFor({ name: "album", id: 1 })).toBe("library");
    expect(tabFor({ name: "explore" })).toBe("home");
  });
});

describe("conversions", () => {
  it("keeps complete metadata for instant playback", () => {
    const row = fromTrack({
      id: 1,
      title: "T",
      duration: 200,
      artists: [{ id: 5, name: "A" }],
      album: { id: 9, title: "B", cover: "c" },
      trackNumber: 1,
      volumeNumber: 1,
      explicit: true,
    });
    expect(row).toMatchObject({ artistId: 5, albumId: 9, cover: "c" });
    expect(toPageInput(row)).toEqual({
      id: 1,
      title: "T",
      artists: ["A"],
      album: "B",
      duration: 200,
      cover: "c",
      explicit: true,
    });
    expect(toQueueItem(row)).toEqual({
      track_id: 1,
      title: "T",
      artist: "A",
      album: "B",
      duration_secs: 200,
      cover: "c",
    });
  });

  it("plays feed tracks through their own payload", () => {
    const row = fromPageItem({ id: "7", title: "T", item_type: "TRACK", artists: ["X"], duration: 90 });
    expect(toPageInput(row)).toMatchObject({ id: 7, artists: ["X"], duration: 90 });
  });
});

describe("now playing helpers", () => {
  const base = { current: { audioQuality: "max" }, stream: { sampleRate: 96000, bitDepth: 24 } };
  it("labels quality", () => {
    expect(qualityBadges(base as unknown as RemoteState)).toEqual({ label: "MAX", detail: "24-bit / 96 kHz" });
    expect(
      qualityBadges({ current: { audioQuality: "high" }, stream: { sampleRate: 44100, bitDepth: 0 } } as unknown as RemoteState),
    ).toEqual({ label: "AAC", detail: "44.1 kHz" });
    expect(qualityBadges({ current: null, stream: null } as unknown as RemoteState)).toBeNull();
  });

  it("cycles repeat and formats time", () => {
    expect(nextRepeat("off")).toBe("all");
    expect(nextRepeat("all")).toBe("one");
    expect(nextRepeat("one")).toBe("off");
    expect(clock(61_900)).toBe("1:01");
  });
});
