import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  restoreSession: vi.fn(),
  getPlaybackState: vi.fn(),
}));

vi.mock("./api", () => ({ api: mocks }));

import { createPlayer, emptyState } from "./player.svelte";

const queue = [
  { track_id: 1, title: "A", artist: "X", album: "", duration_secs: 10, cover: null },
  { track_id: 2, title: "B", artist: "Y", album: "", duration_secs: 10, cover: null },
];

beforeEach(() => {
  mocks.restoreSession.mockReset().mockResolvedValue(undefined);
  mocks.getPlaybackState.mockReset().mockResolvedValue({
    ...emptyState(),
    current: queue[1],
    queue,
    current_index: 1,
  });
});

describe("player.restore", () => {
  it("restores the saved session paused, with shuffle and repeat", async () => {
    const player = createPlayer();
    await player.restore(queue, 1, 1500, true, "all");

    expect(mocks.restoreSession).toHaveBeenCalledWith(queue, 1, 1500, true, "all");
    expect(mocks.getPlaybackState).toHaveBeenCalled();
  });

  it("does nothing for an empty queue", async () => {
    const player = createPlayer();
    await player.restore([], 0, 0, false, "off");

    expect(mocks.restoreSession).not.toHaveBeenCalled();
    expect(mocks.getPlaybackState).not.toHaveBeenCalled();
  });
});
