import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  seek: vi.fn(),
}));

vi.mock("./api", () => ({ api: mocks }));

import { createPlayer } from "./player.svelte";

describe("player.seek", () => {
  it("rounds a fractional position before invoking the backend", async () => {
    mocks.seek.mockReset().mockResolvedValue(undefined);
    const player = createPlayer();

    await player.seek(1234.7);

    expect(mocks.seek).toHaveBeenCalledWith(1235);
  });

  it("clamps a negative position to zero", async () => {
    mocks.seek.mockReset().mockResolvedValue(undefined);
    const player = createPlayer();

    await player.seek(-250.4);

    expect(mocks.seek).toHaveBeenCalledWith(0);
  });
});
