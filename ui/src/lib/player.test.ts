import { describe, expect, it } from "vitest";
import { applyEvent, reduce, emptyState, reconcileSeek, muteTarget, volumeIcon, type PlayerState } from "./player.svelte";

describe("player reducer", () => {
  it("applies a state event", () => {
    const next = reduce(emptyState(), {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.5, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      dsp_active: false, dsp_stages: [], crossfade_ms: 0,
      queue_rev: 0, queue: [],
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
      dsp_active: false, dsp_stages: [], crossfade_ms: 0,
      queue_rev: 0, queue: [],
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

describe("mute target", () => {
  it("silences when the current volume is audible", () => {
    expect(muteTarget(0.7, 0.7)).toBe(0);
  });

  it("restores the last audible volume when muted", () => {
    expect(muteTarget(0, 0.7)).toBe(0.7);
  });

  it("falls back to half volume when there is no audible history", () => {
    expect(muteTarget(0, 0)).toBe(0.5);
  });
});

describe("volume icon", () => {
  it("shows the crossed speaker when muted", () => {
    expect(volumeIcon(0)).toBe("volume-x");
  });

  it("shows one wave below the split", () => {
    expect(volumeIcon(0.01)).toBe("volume-1");
    expect(volumeIcon(0.49)).toBe("volume-1");
  });

  it("shows two waves at and above the split", () => {
    expect(volumeIcon(0.5)).toBe("volume-2");
    expect(volumeIcon(1)).toBe("volume-2");
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
        dsp_active: false, dsp_stages: [], crossfade_ms: 0,
        queue_rev: 0, queue: [], stream: null,
      }),
    ).toBe(1000);
  });
});

describe("player event application", () => {
  const positioned: PlayerState = { ...emptyState(), queue: [
    { track_id: 1, title: "T", artist: "A", album: "", duration_secs: 10, cover: null },
  ] };

  it("mutates the existing state for a position event", () => {
    const state = { ...positioned };
    const result = applyEvent(state, { type: "position", position_ms: 1000, duration_ms: 10000 });
    expect(result).toBe(state);
    expect(state.position_ms).toBe(1000);
    expect(state.duration_ms).toBe(10000);
    expect(state.queue).toHaveLength(1);
  });

  it("applies a state event field by field", () => {
    const state = { ...positioned, error: "stale" };
    const result = applyEvent(state, {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.5, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      dsp_active: false, dsp_stages: [], crossfade_ms: 0,
      queue_rev: 0, queue: [],
      stream: null,
    });
    expect(result).toBe(state);
    expect(state.playing).toBe(true);
    expect(state.volume).toBe(0.5);
    expect(state.queue).toEqual([]);
    expect(state.error).toBe("stale");
  });

  it("keeps its queue when a state event omits it", () => {
    const state = { ...positioned, queue_rev: 3 };
    const queue = state.queue;
    applyEvent(state, {
      type: "state", playing: false, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.2, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, dsp_active: false, dsp_stages: [],
      crossfade_ms: 0, queue_rev: 3,
    });
    expect(state.queue).toBe(queue);
    expect(state.volume).toBe(0.2);
    applyEvent(state, {
      type: "state", playing: false, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.2, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, dsp_active: false, dsp_stages: [],
      crossfade_ms: 0, queue_rev: 4, queue: [],
    });
    expect(state.queue).toEqual([]);
    expect(state.queue_rev).toBe(4);
  });

  it("clears the error on now_playing and captures it on error", () => {
    const state = { ...positioned, error: "old", error_code: "output" };
    applyEvent(state, { type: "now_playing", item: null });
    expect(state.error).toBe("");
    expect(state.error_code).toBe("");
    applyEvent(state, { type: "error", code: "auth", message: "nope" });
    expect(state.error).toBe("nope");
    expect(state.error_code).toBe("auth");
  });

  it("keeps queue and position updates when events interleave", () => {
    const state = emptyState();
    applyEvent(state, { type: "position", position_ms: 1000, duration_ms: 10000 });
    applyEvent(state, {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 1000, duration_ms: 10000, volume: 1, shuffle: false,
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      dsp_active: false, dsp_stages: [], crossfade_ms: 0,
      queue_rev: 0, queue: [{ track_id: 1, title: "T", artist: "A", album: "", duration_secs: 10, cover: null }],
      stream: null,
    });
    applyEvent(state, { type: "position", position_ms: 2000, duration_ms: 10000 });
    expect(state.queue).toHaveLength(1);
    expect(state.position_ms).toBe(2000);
  });
});
