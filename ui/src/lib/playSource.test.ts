import { describe, it, expect, vi } from "vitest";
import { createPlaySource, playlistRowState } from "./playSource.svelte";

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
    source.set("p1");
    expect(source.playlistId).toBe("p1");
    source.set(null);
    expect(source.playlistId).toBe(null);
  });

  it("reports each playlist that starts playback", () => {
    const onPlay = vi.fn();
    const source = createPlaySource(onPlay);
    source.set("p1");
    source.set("p2");
    expect(onPlay.mock.calls).toEqual([["p1"], ["p2"]]);
  });

  it("does not report clearing the source", () => {
    const onPlay = vi.fn();
    const source = createPlaySource(onPlay);
    source.set(null);
    expect(onPlay).not.toHaveBeenCalled();
  });

  /// Review Focus: restoring must not count as a new play in the recency log.
  it("restore re-applies the source without reporting a play", () => {
    const onPlay = vi.fn();
    const source = createPlaySource(onPlay);
    source.restore("p9");
    expect(source.playlistId).toBe("p9");
    expect(onPlay).not.toHaveBeenCalled();
  });

  it("persists every source change, including clearing", () => {
    const persist = vi.fn();
    const source = createPlaySource(undefined, persist);
    source.set("p1");
    source.set(null);
    expect(persist.mock.calls).toEqual([["p1"], [null]]);
  });
});
