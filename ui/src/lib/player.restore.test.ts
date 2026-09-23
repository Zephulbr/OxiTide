import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  playTracks: vi.fn(),
  seek: vi.fn(),
  playPause: vi.fn(),
  getPlaybackState: vi.fn(),
}));

vi.mock("./api", () => ({ api: mocks }));

import { createPlayer, emptyState } from "./player.svelte";

const queue = [
  { track_id: 1, title: "A", artist: "X", album: "", duration_secs: 10, cover: null },
  { track_id: 2, title: "B", artist: "Y", album: "", duration_secs: 10, cover: null },
];

function order(...calls: { mock: { invocationCallOrder: number[] } }[]): number[] {
  return calls.map((fn) => fn.mock.invocationCallOrder[0]);
}

beforeEach(() => {
  mocks.playTracks.mockReset().mockResolvedValue(undefined);
  mocks.seek.mockReset().mockResolvedValue(undefined);
  mocks.playPause.mockReset().mockResolvedValue(undefined);
  mocks.getPlaybackState.mockReset().mockResolvedValue({
    ...emptyState(),
    playing: true,
    current: queue[1],
    queue,
  });
});

describe("player.restore", () => {
  it("resumes a playing session in order", async () => {
    const player = createPlayer();
    await player.restore(queue, 1, 1500, true);

    expect(mocks.playTracks).toHaveBeenCalledWith(queue, 1);
    expect(mocks.seek).toHaveBeenCalledWith(1500);
    expect(mocks.playPause).not.toHaveBeenCalled();
    const calls = order(mocks.playTracks, mocks.seek);
    expect(calls[0]).toBeLessThan(calls[1]);
    expect(mocks.getPlaybackState).toHaveBeenCalled();
  });

  it("re-pauses a paused session", async () => {
    const player = createPlayer();
    await player.restore(queue, 0, 900, false);

    expect(mocks.playPause).toHaveBeenCalledTimes(1);
    const calls = order(mocks.playTracks, mocks.seek, mocks.playPause);
    expect(calls).toEqual([...calls].sort((a, b) => a - b));
  });

  it("does nothing for an empty queue", async () => {
    const player = createPlayer();
    await player.restore([], 0, 0, true);

    expect(mocks.playTracks).not.toHaveBeenCalled();
    expect(mocks.seek).not.toHaveBeenCalled();
    expect(mocks.playPause).not.toHaveBeenCalled();
  });
});
