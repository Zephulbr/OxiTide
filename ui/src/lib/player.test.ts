import { describe, expect, it } from "vitest";
import { reduce, emptyState, type PlayerState } from "./player.svelte";

describe("player reducer", () => {
  it("applies a state event", () => {
    const next = reduce(emptyState(), {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.5, shuffle: false,
      repeat: "off", preferred_quality: "lossless", queue: [],
    });
    expect(next.playing).toBe(true);
    expect(next.volume).toBe(0.5);
  });

  it("applies a position event without clobbering the queue", () => {
    const start: PlayerState = { ...emptyState(), queue: [
      { track_id: 1, title: "T", artist: "A", album: "", duration_secs: 10, cover: null },
    ] };
    const next = reduce(start, { type: "position", position_ms: 1000, duration_ms: 10000 });
    expect(next.position_ms).toBe(1000);
    expect(next.queue).toHaveLength(1);
  });

  it("captures errors", () => {
    const next = reduce(emptyState(), { type: "error", code: "auth", message: "nope" });
    expect(next.error).toBe("nope");
  });
});
