import { describe, expect, it } from "vitest";
import { reduce, emptyState, reconcileSeek, type PlayerState } from "./player.svelte";

describe("player reducer", () => {
  it("applies a state event", () => {
    const next = reduce(emptyState(), {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.5, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      queue: [],
      stream: null,
    });
    expect(next.playing).toBe(true);
    expect(next.volume).toBe(0.5);
  });

  it("mirrors the stream metadata", () => {
    const next = reduce(emptyState(), {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 1, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      queue: [],
      stream: { quality: "lossless", codec: "flac", sample_rate: 44100, bit_depth: 16, channels: 2, bitrate_kbps: 1000 },
    });
    expect(next.stream?.sample_rate).toBe(44100);
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

describe("seek reconciliation", () => {
  it("releases the pending position when the engine catches up", () => {
    expect(
      reconcileSeek(1000, { type: "position", position_ms: 100, duration_ms: 2000 }),
    ).toBe(1000);
    expect(
      reconcileSeek(1000, { type: "position", position_ms: 1100, duration_ms: 2000 }),
    ).toBeNull();
  });

  it("clears the pending position on errors and track changes", () => {
    expect(reconcileSeek(1000, { type: "error", code: "x", message: "y" })).toBeNull();
    expect(reconcileSeek(1000, { type: "now_playing", item: null })).toBeNull();
  });

  it("keeps the pin through the optimistic state event a seek emits", () => {
    expect(
      reconcileSeek(1000, {
        type: "state", playing: true, buffering: false, current: null,
        position_ms: 1000, duration_ms: 2000, volume: 1, shuffle: false,
        repeat: "off", preferred_quality: "lossless",
        output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
        queue: [], stream: null,
      }),
    ).toBe(1000);
  });
});
