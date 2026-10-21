import { describe, it, expect, vi } from "vitest";
import {
  createPlaySource,
  legacyPlaylistSource,
  parseSavedPlayFrom,
  playFromLabel,
  playlistRowState,
  type PlayFrom,
} from "./playSource.svelte";

const playlist = (id: string, name = "Road trip"): PlayFrom => ({
  kind: "playlist",
  name,
  route: { name: "playlist", id },
});
const album: PlayFrom = { kind: "album", name: "Blood Sugar Sex Magik", route: { name: "album", id: 7 } };

describe("playlistRowState", () => {
  it("keeps both the playing treatment and the open highlight when both apply", () => {
    expect(playlistRowState(true, true)).toBe("source-active");
  });
  it("marks the playback source when it is not open", () => {
    expect(playlistRowState(true, false)).toBe("source");
  });
  it("keeps the accent treatment for the open playlist when it is not the source", () => {
    expect(playlistRowState(false, true)).toBe("active");
  });
  it("is idle when neither", () => {
    expect(playlistRowState(false, false)).toBe("idle");
  });
});

describe("createPlaySource", () => {
  it("tracks the playlist that started playback and clears with null", () => {
    const source = createPlaySource();
    expect(source.playlistId).toBe(null);
    source.set(playlist("p1"), [1]);
    expect(source.playlistId).toBe("p1");
    source.set(null);
    expect(source.playlistId).toBe(null);
    expect(source.from).toBe(null);
  });

  it("marks no playlist for other sources", () => {
    const source = createPlaySource();
    source.set(album, [1]);
    expect(source.playlistId).toBe(null);
    expect(source.from).toEqual(album);
  });

  it("credits only the tracks the source queued", () => {
    const source = createPlaySource();
    source.set(album, [1, 2]);
    expect(source.fromFor(2)).toEqual(album);
    // Added later (Add to queue, Play next, the phone remote).
    expect(source.fromFor(9)).toBe(null);
  });

  it("reports each playlist that starts playback", () => {
    const onPlay = vi.fn();
    const source = createPlaySource(onPlay);
    source.set(playlist("p1"), []);
    source.set(album, []);
    source.set(playlist("p2"), []);
    expect(onPlay.mock.calls).toEqual([["p1"], ["p2"]]);
  });

  it("does not report clearing the source", () => {
    const onPlay = vi.fn();
    const source = createPlaySource(onPlay);
    source.set(null);
    expect(onPlay).not.toHaveBeenCalled();
  });

  /// Review Focus: restoring must not count as a new play in the recency log.
  it("restore re-applies the source and its tracks without reporting a play", () => {
    const onPlay = vi.fn();
    const source = createPlaySource(onPlay);
    source.restore({ ...playlist("p9"), track_ids: [4] });
    expect(source.playlistId).toBe("p9");
    expect(source.fromFor(4)).toEqual(playlist("p9"));
    expect(onPlay).not.toHaveBeenCalled();
  });

  it("persists a restored source without reporting a play", () => {
    const onPlay = vi.fn();
    const persist = vi.fn();
    const source = createPlaySource(onPlay, persist);
    source.restore({ ...playlist("p9"), track_ids: [4] });
    expect(persist).toHaveBeenCalledWith("p9", { ...playlist("p9"), track_ids: [4] });
    expect(onPlay).not.toHaveBeenCalled();
  });

  it("persists every source change, including clearing", () => {
    const persist = vi.fn();
    const source = createPlaySource(undefined, persist);
    source.set(playlist("p1"), [1, 2]);
    source.set(null);
    expect(persist.mock.calls).toEqual([
      ["p1", { ...playlist("p1"), track_ids: [1, 2] }],
      [null, null],
    ]);
  });
});

describe("parseSavedPlayFrom", () => {
  it("reads a saved source back", () => {
    const saved = { ...album, track_ids: [1, 2] };
    expect(parseSavedPlayFrom(JSON.parse(JSON.stringify(saved)))).toEqual(saved);
  });

  it("drops an unknown kind and a malformed route", () => {
    expect(parseSavedPlayFrom({ kind: "podcast", name: "x", route: null, track_ids: [] })).toBe(null);
    expect(parseSavedPlayFrom({ kind: "album", name: "x", route: { name: "nowhere" }, track_ids: [1] })).toEqual({
      kind: "album",
      name: "x",
      route: null,
      track_ids: [1],
    });
    expect(parseSavedPlayFrom(null)).toBe(null);
  });
});

describe("legacyPlaylistSource", () => {
  it("keeps an older session's playlist marker but credits no tracks", () => {
    const source = createPlaySource();
    source.restore(legacyPlaylistSource("p3"));
    expect(source.playlistId).toBe("p3");
    expect(source.fromFor(1)).toBe(null);
  });
});

describe("playFromLabel", () => {
  it("names the kind in the eyebrow", () => {
    expect(playFromLabel("album")).toBe("From album");
    expect(playFromLabel("radio")).toBe("From");
  });
});
