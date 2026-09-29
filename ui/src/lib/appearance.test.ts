import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import {
  clampQueueWidth,
  createAppearance,
  fontStack,
  resolveDynamicColor,
  resolveTypeface,
} from "./appearance.svelte";

vi.mock("./api", () => ({
  api: {
    getConfig: vi.fn(),
    setOneko: vi.fn(),
  },
}));

describe("clampQueueWidth", () => {
  it("clamps to the 260–420 dock range", () => {
    expect(clampQueueWidth(100)).toBe(260);
    expect(clampQueueWidth(320)).toBe(320);
    expect(clampQueueWidth(999)).toBe(420);
    expect(clampQueueWidth(319.6)).toBe(320);
  });
});

describe("fontStack", () => {
  it("bundled and system picks", () => {
    expect(fontStack("manrope", "")).toBe('"Manrope", system-ui, sans-serif');
    expect(fontStack("inter", "")).toBe('"Inter", system-ui, sans-serif');
    expect(fontStack("system", "")).toBe("system-ui, sans-serif");
  });

  it("custom names get quoted and fall back to system-ui", () => {
    expect(fontStack("custom", "Commit Mono")).toBe('"Commit Mono", system-ui, sans-serif');
    expect(fontStack("custom", "  ")).toBe("system-ui, sans-serif");
  });

  /// Review Focus: quotes and backslashes in a custom name must not break
  /// out of the CSS value.
  it("strips css-breaking characters from custom names", () => {
    expect(fontStack("custom", 'Evil"; } * { display:none')).toBe(
      '"Evil; } * { display:none", system-ui, sans-serif',
    );
    expect(fontStack("custom", "a\\b")).toBe('"ab", system-ui, sans-serif');
  });
});

describe("resolveTypeface", () => {
  it("auto follows the mode default", () => {
    expect(resolveTypeface("auto", "atmosphere")).toBe("manrope");
    expect(resolveTypeface("auto", "instrument")).toBe("inter");
  });
  it("explicit picks always win", () => {
    expect(resolveTypeface("system", "atmosphere")).toBe("system");
    expect(resolveTypeface("custom", "instrument")).toBe("custom");
  });
});

describe("resolveDynamicColor", () => {
  it("auto is full in atmosphere and off in instrument", () => {
    expect(resolveDynamicColor("auto", "atmosphere")).toBe("full");
    expect(resolveDynamicColor("auto", "instrument")).toBe("off");
  });
  it("explicit picks always win, including off in atmosphere", () => {
    expect(resolveDynamicColor("accent", "atmosphere")).toBe("accent");
    expect(resolveDynamicColor("off", "atmosphere")).toBe("off");
    expect(resolveDynamicColor("full", "instrument")).toBe("full");
  });
});

describe("oneko setting", () => {
  beforeEach(() => {
    vi.stubGlobal("window", {
      matchMedia: () => ({ matches: false, addEventListener: () => {} }),
    });
    vi.stubGlobal("document", {
      documentElement: { dataset: {}, style: { setProperty: () => {} } },
    });
    vi.mocked(api.getConfig).mockResolvedValue({ ui: { oneko: true } } as never);
    vi.mocked(api.setOneko).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it("init reads ui.oneko and setOneko updates the singleton", async () => {
    const appearance = createAppearance();
    await appearance.init();
    expect(appearance.oneko).toBe(true);
    await appearance.setOneko(false);
    expect(appearance.oneko).toBe(false);
    expect(api.setOneko).toHaveBeenCalledWith(false);
  });
});
