import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Track } from "./api";
import { clearSuggestCache } from "./homeSuggest";
import { albumRoute, goToTargets, primaryArtistRoute } from "./queueLinks";

const resolved: Track = {
  id: 1,
  title: "T",
  duration: 1,
  artists: [{ id: 30, name: "Primary" }, { id: 31, name: "Feat" }],
  album: { id: 40, title: "LP" },
  trackNumber: 1,
  volumeNumber: 1,
  explicit: false,
};

beforeEach(() => clearSuggestCache());

describe("queue link routes", () => {
  it("uses the item's own ids without a lookup", async () => {
    const getTrack = vi.fn();
    const item = { track_id: 1, artist: "A", artist_refs: [{ id: 5, name: "A" }], album_id: 6 };
    expect(await albumRoute(item, getTrack)).toEqual({ name: "album", id: 6 });
    expect(await primaryArtistRoute(item, getTrack)).toEqual({ name: "artist", id: 5 });
    expect(getTrack).not.toHaveBeenCalled();
  });

  it("resolves an id-less item once, through the session cache", async () => {
    const getTrack = vi.fn(async () => resolved);
    const item = { track_id: 1, artist: "Primary, Feat" };
    expect(await albumRoute(item, getTrack)).toEqual({ name: "album", id: 40 });
    expect(await primaryArtistRoute(item, getTrack)).toEqual({ name: "artist", id: 30 });
    expect(getTrack).toHaveBeenCalledTimes(1);
  });

  it("gives null when the lookup fails", async () => {
    const getTrack = vi.fn(async () => {
      throw new Error("offline");
    });
    expect(await albumRoute({ track_id: 1, artist: "A" }, getTrack)).toBeNull();
  });
});

describe("goToTargets", () => {
  it("offers one entry per known artist and the album", () => {
    const go = vi.fn();
    const targets = goToTargets(
      { track_id: 1, artist: "A, B", artist_refs: [{ id: 1, name: "A" }, { id: 2, name: "B" }], album_id: 3 },
      go,
      vi.fn(),
    );
    expect(targets.goToArtists.map((a) => a.name)).toEqual(["A", "B"]);
    targets.goToArtists[1].open();
    expect(go).toHaveBeenCalledWith({ name: "artist", id: 2 });
  });

  it("offers a resolving artist entry and no album for a bare item without one", async () => {
    const go = vi.fn();
    const targets = goToTargets({ track_id: 1, artist: "Primary, Feat", album: "" }, go, vi.fn(async () => resolved));
    expect(targets.goToAlbum).toBeUndefined();
    expect(targets.goToArtists.map((a) => a.name)).toEqual(["Primary, Feat"]);
    targets.goToArtists[0].open();
    await vi.waitFor(() => expect(go).toHaveBeenCalledWith({ name: "artist", id: 30 }));
  });
});
