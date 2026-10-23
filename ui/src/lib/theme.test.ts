import { describe, expect, it, vi } from "vitest";
import {
  readMirror,
  readThemeMirror,
  resolveTheme,
  THEME_STORAGE_KEY,
  TONE_STORAGE_KEY,
  writeThemeMirror,
} from "./theme";

describe("resolveTheme", () => {
  it("returns explicit settings unchanged", () => {
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
  });
  it("system follows the OS preference", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });
});

describe("theme mirror", () => {
  /// Review Focus: a missing or corrupt mirror must fall back to dark without
  /// throwing (cleared storage, privacy mode, hand-edited value).
  it("reads valid settings, defaults anything else to dark", () => {
    const get = (value: string | null) => ({ getItem: () => value, setItem: () => {} });
    expect(readThemeMirror(get("light"))).toBe("light");
    expect(readThemeMirror(get("system"))).toBe("system");
    expect(readThemeMirror(get(null))).toBe("dark");
    expect(readThemeMirror(get("evil"))).toBe("dark");
  });

  it("survives a throwing storage", () => {
    const storage = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(readThemeMirror(storage)).toBe("dark");
    expect(() => writeThemeMirror(storage, "light")).not.toThrow();
    expect(readThemeMirror(undefined)).toBe("dark");
  });

  it("readMirror accepts only the allowed values", () => {
    const get = (value: string | null) => ({ getItem: () => value, setItem: () => {} });
    const tones = ["warm", "neutral", "black"] as const;
    expect(readMirror(get("black"), TONE_STORAGE_KEY, tones, "warm")).toBe("black");
    expect(readMirror(get("plaid"), TONE_STORAGE_KEY, tones, "warm")).toBe("warm");
    expect(readMirror(get(null), TONE_STORAGE_KEY, tones, "warm")).toBe("warm");
  });

  it("writes under the documented key", () => {
    const setItem = vi.fn();
    writeThemeMirror({ getItem: () => null, setItem }, "system");
    expect(setItem).toHaveBeenCalledWith(THEME_STORAGE_KEY, "system");
  });
});
