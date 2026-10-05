import { describe, expect, it } from "vitest";
import { createPaletteApplier, paletteTokens, type CoverPalette } from "./dynamicColor.svelte";

const palette: CoverPalette = {
  primary: "#ff0000",
  on_primary: "#000000",
  secondary: "#222222",
  tertiary: "#00ff00",
  backdrop: "#333333",
  ceiling_l: 0.1,
};

describe("paletteTokens", () => {
  it("off applies nothing", () => {
    expect(paletteTokens(palette, "off")).toEqual({});
  });
  it("accent-only applies the primary tokens plus the backdrop wash (spec §5.4)", () => {
    expect(paletteTokens(palette, "accent")).toEqual({
      "--dyn-primary": "#ff0000",
      "--dyn-on-primary": "#000000",
      "--dyn-backdrop": "#333333",
    });
  });
  it("full applies every dynamic token", () => {
    expect(paletteTokens(palette, "full")).toEqual({
      "--dyn-primary": "#ff0000",
      "--dyn-on-primary": "#000000",
      "--dyn-secondary": "#222222",
      "--dyn-tertiary": "#00ff00",
      "--dyn-backdrop": "#333333",
    });
  });
});

describe("createPaletteApplier", () => {
  function deferred<T>() {
    let resolve!: (v: T) => void;
    let reject!: (e: unknown) => void;
    return {
      promise: new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
      }),
      resolve,
      reject,
    };
  }

  /// Review Focus: a stale response must never clobber a newer track.
  it("ignores an older response that resolves after a newer track", async () => {
    const applied: Record<string, string>[] = [];
    const slow = deferred<CoverPalette>();
    const fast = deferred<CoverPalette>();
    const fetcher = (uuid: string) => (uuid === "slow" ? slow.promise : fast.promise);
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);

    void applier.onTrack("slow", "full", "dark");
    void applier.onTrack("fast", "full", "dark");
    fast.resolve({ ...palette, primary: "#111111" });
    await Promise.resolve();
    slow.resolve({ ...palette, primary: "#999999" });
    await Promise.resolve();

    expect(applied).toEqual([expect.objectContaining({ "--dyn-primary": "#111111" })]);
  });

  it("off clears the tokens; a missing cover keeps the last color", async () => {
    const applied: Record<string, string>[] = [];
    const applier = createPaletteApplier((t) => applied.push(t), () => Promise.resolve(palette));
    await applier.onTrack("uuid", "full", "dark");
    await applier.onTrack(null, "full", "dark");
    await applier.onTrack("uuid", "off", "dark");
    expect(applied).toEqual([paletteTokens(palette, "full"), {}]);
  });

  it("a failed fetch keeps the current tokens", async () => {
    const applied: Record<string, string>[] = [];
    const applier = createPaletteApplier(
      (t) => applied.push(t),
      () => Promise.reject(new Error("offline")),
    );
    await applier.onTrack("uuid", "full", "dark");
    expect(applied).toEqual([]);
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

    void applier.onTrack("uuid", "full", "dark");
    void applier.onTrack("uuid", "full", "dark");
    pending.resolve(palette);
    await Promise.resolve();
    await Promise.resolve();

    expect(fetches).toBe(1);
    expect(applied).toEqual([paletteTokens(palette, "full")]);
  });

  /// A stale failure must not clear a palette a newer request already applied.
  it("ignores an older failed response after a newer track applied", async () => {
    const applied: Record<string, string>[] = [];
    const slow = deferred<CoverPalette>();
    const fast = deferred<CoverPalette>();
    const fetcher = (uuid: string) => (uuid === "slow" ? slow.promise : fast.promise);
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);

    const stale = applier.onTrack("slow", "full", "dark");
    void applier.onTrack("fast", "full", "light");
    fast.resolve({ ...palette, primary: "#111111" });
    await Promise.resolve();
    slow.reject(new Error("offline"));
    await stale;

    expect(applied).toEqual([expect.objectContaining({ "--dyn-primary": "#111111" })]);
  });

  /// Review Focus 4: a failed re-analysis after a theme switch must fall back
  /// to the room's static CSS instead of keeping the other room's inline palette.
  it("clears a stale palette when the re-fetch for the new theme fails", async () => {
    const applied: Record<string, string>[] = [];
    let fetches = 0;
    const fetcher = async () => {
      fetches += 1;
      if (fetches === 1) return palette;
      throw new Error("offline");
    };
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);
    await applier.onTrack("uuid", "full", "dark");
    await applier.onTrack("uuid", "full", "light");
    expect(applied).toEqual([paletteTokens(palette, "full"), {}]);
  });

  it("keeps the current palette when a same-theme re-fetch fails", async () => {
    const applied: Record<string, string>[] = [];
    let fetches = 0;
    const fetcher = async () => {
      fetches += 1;
      if (fetches === 1) return palette;
      throw new Error("offline");
    };
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);
    await applier.onTrack("uuid", "full", "dark");
    await applier.onTrack("other", "full", "dark");
    expect(applied).toEqual([paletteTokens(palette, "full")]);
  });

  it("clears a stale palette when the theme changes with no cover", async () => {
    const applied: Record<string, string>[] = [];
    const applier = createPaletteApplier((t) => applied.push(t), () => Promise.resolve(palette));
    await applier.onTrack("uuid", "full", "dark");
    await applier.onTrack(null, "full", "light");
    expect(applied).toEqual([paletteTokens(palette, "full"), {}]);
  });

  /// Review Focus 2: a theme change with the same cover/mode must re-analyze.
  it("re-fetches when only the theme changes", async () => {
    const applied: Record<string, string>[] = [];
    let fetches = 0;
    const fetcher = async () => {
      fetches += 1;
      return { ...palette, primary: fetches === 1 ? "#111111" : "#222222" };
    };
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);
    await applier.onTrack("uuid", "full", "dark");
    await applier.onTrack("uuid", "full", "light");
    expect(fetches).toBe(2);
    expect(applied[applied.length - 1]).toEqual(
      expect.objectContaining({ "--dyn-primary": "#222222" }),
    );
  });
});
