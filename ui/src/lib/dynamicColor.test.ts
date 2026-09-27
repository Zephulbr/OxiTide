import { describe, expect, it } from "vitest";
import { createPaletteApplier, paletteTokens, type CoverPalette } from "./dynamicColor.svelte";

const palette: CoverPalette = {
  primary: "#ff0000",
  on_primary: "#000000",
  secondary: "#222222",
  tertiary: "#00ff00",
  ceiling_l: 0.1,
};

describe("paletteTokens", () => {
  it("off applies nothing", () => {
    expect(paletteTokens(palette, "off")).toEqual({});
  });
  it("accent-only applies primary and on-primary (spec §5.4)", () => {
    expect(paletteTokens(palette, "accent")).toEqual({
      "--dyn-primary": "#ff0000",
      "--dyn-on-primary": "#000000",
    });
  });
  it("full applies all four roles", () => {
    expect(paletteTokens(palette, "full")).toEqual({
      "--dyn-primary": "#ff0000",
      "--dyn-on-primary": "#000000",
      "--dyn-secondary": "#222222",
      "--dyn-tertiary": "#00ff00",
    });
  });
});

describe("createPaletteApplier", () => {
  function deferred<T>() {
    let resolve!: (v: T) => void;
    return { promise: new Promise<T>((r) => (resolve = r)), resolve };
  }

  /// Review Focus: a stale response must never clobber a newer track.
  it("ignores an older response that resolves after a newer track", async () => {
    const applied: Record<string, string>[] = [];
    const slow = deferred<CoverPalette>();
    const fast = deferred<CoverPalette>();
    const fetcher = (uuid: string) => (uuid === "slow" ? slow.promise : fast.promise);
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);

    void applier.onTrack("slow", "full");
    void applier.onTrack("fast", "full");
    fast.resolve({ ...palette, primary: "#111111" });
    await Promise.resolve();
    slow.resolve({ ...palette, primary: "#999999" });
    await Promise.resolve();

    expect(applied).toEqual([expect.objectContaining({ "--dyn-primary": "#111111" })]);
  });

  it("off and missing covers clear the tokens synchronously", async () => {
    const applied: Record<string, string>[] = [];
    const applier = createPaletteApplier((t) => applied.push(t), () => Promise.resolve(palette));
    await applier.onTrack(null, "full");
    await applier.onTrack("uuid", "off");
    expect(applied).toEqual([{}, {}]);
  });

  /// A position tick re-runs the effect with the same cover/mode; it must not
  /// re-fetch or re-apply.
  it("dedupes identical consecutive (cover, mode) calls", async () => {
    const applied: Record<string, string>[] = [];
    let fetches = 0;
    const pending = deferred<CoverPalette>();
    const fetcher = () => {
      fetches += 1;
      return pending.promise;
    };
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);

    void applier.onTrack("uuid", "full");
    void applier.onTrack("uuid", "full");
    pending.resolve(palette);
    await Promise.resolve();
    await Promise.resolve();

    expect(fetches).toBe(1);
    expect(applied).toEqual([paletteTokens(palette, "full")]);
  });
});
