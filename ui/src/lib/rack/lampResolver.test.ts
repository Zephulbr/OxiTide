import { describe, expect, it, vi } from "vitest";
import type { CoverPalette } from "../api";
import { LAMP_PRESETS } from "./lamp";
import { createLampResolver } from "./lamp.svelte";

vi.mock("../api", () => ({ api: {} }));
vi.mock("../appearance.svelte", () => ({ appearance: {} }));

const blue: CoverPalette = {
  primary: "#3f7fb5",
  on_primary: "#000000",
  secondary: "#202020",
  tertiary: "#303030",
  backdrop: "#101010",
  ceiling_l: 0.1,
};

describe("createLampResolver", () => {
  it("applies presets without fetching", async () => {
    const apply = vi.fn();
    const fetcher = vi.fn();
    const r = createLampResolver(apply, fetcher);
    await r.update("green", "cover-1");
    expect(apply).toHaveBeenCalledWith(LAMP_PRESETS.green);
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("fetches only for cover, and the latest cover wins", async () => {
    const apply = vi.fn();
    let resolveFirst: (p: CoverPalette) => void = () => {};
    const fetcher = vi
      .fn()
      .mockImplementationOnce(() => new Promise<CoverPalette>((res) => (resolveFirst = res)))
      .mockResolvedValueOnce(blue);
    const r = createLampResolver(apply, fetcher);
    const first = r.update("cover", "a");
    await r.update("cover", "b");
    resolveFirst({ ...blue, primary: "#ff0000" });
    await first;
    expect(apply).toHaveBeenCalledTimes(1);
    expect(apply.mock.calls[0][0].h).toBeGreaterThan(230);
  });

  it("a failed fetch falls back to amber; no artwork keeps the current lamp", async () => {
    const apply = vi.fn();
    const r = createLampResolver(apply, vi.fn().mockRejectedValue(new Error("x")));
    await r.update("cover", null);
    expect(apply).not.toHaveBeenCalled();
    await r.update("cover", "c");
    expect(apply).toHaveBeenCalledWith(LAMP_PRESETS.amber);
  });
});
