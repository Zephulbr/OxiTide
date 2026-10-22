import { beforeEach, describe, expect, it, vi } from "vitest";
import { api, type Track } from "./api";
import {
  payloadCount,
  payloadCovers,
  payloadLabel,
  payloadSourcePlaylist,
  payloadSourceTable,
  resolveTracks,
  type DragPayload,
} from "./dragPayload";

vi.mock("./api", () => ({
  api: {
    getTrack: vi.fn(),
    getAlbumTracks: vi.fn(),
    getPlaylistTracks: vi.fn(),
    getMix: vi.fn(),
  },
}));

function track(id: number, cover: string | null = null): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
    album: cover === null ? undefined : { id: 1, title: "A", cover },
  } as Track;
}

beforeEach(() => vi.clearAllMocks());

describe("payloadLabel", () => {
  it("names one track, counts several", () => {
    expect(payloadLabel({ kind: "tracks", tracks: [track(1)] })).toBe("T1");
    expect(payloadLabel({ kind: "tracks", tracks: [track(1), track(2), track(3)] })).toBe("3 tracks");
    expect(payloadLabel({ kind: "track-id", id: 4, title: "Song" })).toBe("Song");
  });

  it("names a collection, falling back to its kind", () => {
    expect(payloadLabel({ kind: "collection", target: { kind: "album", id: 1, title: "LP" } })).toBe("LP");
    expect(payloadLabel({ kind: "collection", target: { kind: "album", id: 1 } })).toBe("Album");
    expect(payloadLabel({ kind: "collection", target: { kind: "playlist", id: "p" } })).toBe("Playlist");
    expect(payloadLabel({ kind: "collection", target: { kind: "mix", id: "m", title: "" } })).toBe("Mix");
  });
});

describe("payload helpers", () => {
  it("covers stack at most two", () => {
    const p: DragPayload = { kind: "tracks", tracks: [track(1, "a"), track(2), track(3, "c")] };
    expect(payloadCovers(p)).toEqual(["a", null]);
    expect(payloadCovers({ kind: "track-id", id: 1, title: "x", cover: "z" })).toEqual(["z"]);
  });

  it("counts known track totals only", () => {
    expect(payloadCount({ kind: "tracks", tracks: [track(1), track(2)] })).toBe(2);
    expect(payloadCount({ kind: "track-id", id: 1, title: "x" })).toBe(1);
    expect(payloadCount({ kind: "collection", target: { kind: "album", id: 1 } })).toBeNull();
  });

  it("knows the source playlist of rows and of a dragged playlist", () => {
    expect(
      payloadSourcePlaylist({ kind: "tracks", tracks: [], source: { tableId: "t", playlistId: "p1" } }),
    ).toBe("p1");
    expect(payloadSourcePlaylist({ kind: "collection", target: { kind: "playlist", id: "p2" } })).toBe("p2");
    expect(payloadSourcePlaylist({ kind: "collection", target: { kind: "album", id: 3 } })).toBeNull();
    expect(payloadSourcePlaylist({ kind: "track-id", id: 1, title: "x" })).toBeNull();
  });

  it("knows the source table of rows only", () => {
    expect(payloadSourceTable({ kind: "tracks", tracks: [], source: { tableId: "t1" } })).toBe("t1");
    expect(payloadSourceTable({ kind: "tracks", tracks: [] })).toBeNull();
    expect(payloadSourceTable({ kind: "track-id", id: 1, title: "x" })).toBeNull();
  });
});

describe("resolveTracks", () => {
  it("returns carried tracks without a request", async () => {
    const tracks = [track(1)];
    expect(await resolveTracks({ kind: "tracks", tracks })).toBe(tracks);
    expect(api.getTrack).not.toHaveBeenCalled();
  });

  it("fetches a lone track id", async () => {
    vi.mocked(api.getTrack).mockResolvedValue(track(9));
    expect(await resolveTracks({ kind: "track-id", id: 9, title: "T9" })).toEqual([track(9)]);
    expect(api.getTrack).toHaveBeenCalledWith(9);
  });

  it("loads each collection kind", async () => {
    vi.mocked(api.getAlbumTracks).mockResolvedValue([track(1)]);
    vi.mocked(api.getPlaylistTracks).mockResolvedValue([track(2)]);
    vi.mocked(api.getMix).mockResolvedValue({ id: "m", title: "M", tracks: [track(3)] });
    expect(await resolveTracks({ kind: "collection", target: { kind: "album", id: 5 } })).toEqual([track(1)]);
    expect(api.getAlbumTracks).toHaveBeenCalledWith(5);
    expect(await resolveTracks({ kind: "collection", target: { kind: "playlist", id: "p" } })).toEqual([track(2)]);
    expect(api.getPlaylistTracks).toHaveBeenCalledWith("p");
    expect(await resolveTracks({ kind: "collection", target: { kind: "mix", id: "m" } })).toEqual([track(3)]);
    expect(api.getMix).toHaveBeenCalledWith("m");
  });
});
