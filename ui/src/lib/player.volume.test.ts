import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  setVolume: vi.fn(),
}));

vi.mock("./api", () => ({ api: mocks }));

import { createPlayer } from "./player.svelte";

describe("player volume while fixed (ASIO)", () => {
  it("sends nothing to the backend, mute included", async () => {
    mocks.setVolume.mockReset().mockResolvedValue(undefined);
    const player = createPlayer();
    player.state.volume = 1;
    player.state.volume_fixed = true;

    await player.setVolume(0.3, true);
    await player.setVolume(0.2, true, true);
    await player.toggleMute();

    expect(mocks.setVolume).not.toHaveBeenCalled();
  });

  it("persists normally once the device has a volume again", async () => {
    mocks.setVolume.mockReset().mockResolvedValue(undefined);
    const player = createPlayer();
    player.state.volume_fixed = false;

    await player.setVolume(0.3, true);

    expect(mocks.setVolume).toHaveBeenCalledWith(0.3, true);
  });
});
