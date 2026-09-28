import { describe, it, expect } from "vitest";
import { createPlaySource, playlistRowState } from "./playSource.svelte";

describe("playlistRowState", () => {
  it("lets the playback source win over the open playlist", () => {
    expect(playlistRowState(true, true)).toBe("source");
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
});
