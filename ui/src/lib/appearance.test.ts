import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import {
  appearance,
  clampLampBrightness,
  clampQueueWidth,
  createAppearance,
  defaultRackSettings,
  fontStack,
  resolveDynamicColor,
  resolveTypeface,
} from "./appearance.svelte";

vi.mock("./api", () => ({
  api: {
    getConfig: vi.fn(),
    setOneko: vi.fn(),
    setTheme: vi.fn(),
    setUiMode: vi.fn(),
    setRackSettings: vi.fn(),
  },
}));

describe("rack mode", () => {
  beforeEach(() => {
    vi.stubGlobal("window", {
      matchMedia: () => ({ matches: false, addEventListener: () => {} }),
    });
    vi.stubGlobal("document", {
      documentElement: { dataset: {}, style: { setProperty: () => {} } },
    });
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
    vi.mocked(api.getConfig).mockResolvedValue({
      ui: { mode: "rack", theme: "dark", dynamic_color: "full", typeface: "auto" },
      rack: { deck_face: "cd", meters: "led", lamp: "blue", eq_unit: false, lamp_brightness: 70 },
    } as never);
    vi.mocked(api.setUiMode).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it("init reads [rack], sets data-mode and the rack flags", async () => {
    const a = createAppearance();
    await a.init();
    expect(document.documentElement.dataset.mode).toBe("rack");
    expect(a.rackShell).toBe(true);
    expect(a.rack).toEqual({ deck_face: "cd", meters: "led", lamp: "blue", eq_unit: false, lamp_brightness: 70 });
    // Rack draws its own materials: the Atmosphere effects stay off.
    expect(a.backdropArt).toBe(false);
    expect(a.glassPanels).toBe(false);
    expect(a.discSpin).toBe(false);
    expect(a.waveSeek).toBe(false);
    // Lamp colour replaces dynamic colour (RK-D9), even when set to full.
    expect(a.dynamicColor).toBe("off");
  });

  it("setRack is local-first, stores the returned value, and reverts on failure", async () => {
    const a = createAppearance();
    await a.init();
    vi.mocked(api.setRackSettings).mockImplementationOnce(async (s) => ({ ...s, lamp_brightness: 40 }));
    await a.setRack({ meters: "bars", lamp_brightness: 10 });
    expect(api.setRackSettings).toHaveBeenCalledWith(expect.objectContaining({ meters: "bars", lamp_brightness: 40 }));
    expect(a.rack.meters).toBe("bars");
    vi.mocked(api.setRackSettings).mockRejectedValueOnce(new Error("disk full"));
    await expect(a.setRack({ deck_face: "transport" })).rejects.toThrow("disk full");
    expect(a.rack.deck_face).toBe("cd");
  });

  it("leaving rack drops the shell flag", async () => {
    const a = createAppearance();
    await a.init();
    await a.setMode("atmosphere");
    expect(a.rackShell).toBe(false);
    expect(api.setUiMode).toHaveBeenCalledWith("atmosphere");
  });

  it("defaults and clamps", () => {
    expect(defaultRackSettings()).toEqual({ deck_face: "display", meters: "needles", lamp: "amber", eq_unit: true, lamp_brightness: 100 });
    expect(clampLampBrightness(10)).toBe(40);
    expect(clampLampBrightness(150)).toBe(100);
    expect(clampLampBrightness(Number.NaN)).toBe(100);
  });
});

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
    expect(resolveTypeface("auto", "rack")).toBe("manrope");
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
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
    vi.mocked(api.getConfig).mockResolvedValue({ ui: { oneko: true, theme: "dark" } } as never);
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

  it("setOneko rolls back when persistence fails", async () => {
    vi.mocked(api.setOneko).mockRejectedValueOnce(new Error("disk full"));
    await expect(appearance.setOneko(true)).rejects.toThrow("disk full");
    expect(appearance.oneko).toBe(false);
  });
});

describe("theme", () => {
  let themeMediaListener: ((e: { matches: boolean }) => void) | undefined;

  beforeEach(() => {
    themeMediaListener = undefined;
    vi.stubGlobal("window", {
      matchMedia: (query: string) => ({
        matches: false,
        addEventListener: (_event: string, listener: (e: { matches: boolean }) => void) => {
          if (query === "(prefers-color-scheme: dark)") themeMediaListener = listener;
        },
      }),
    });
    vi.stubGlobal("document", {
      documentElement: { dataset: {}, style: { setProperty: () => {} } },
    });
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
    vi.mocked(api.getConfig).mockResolvedValue({ ui: { oneko: false, theme: "system" } } as never);
    vi.mocked(api.setTheme).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it("init resolves system, applies data-theme, and mirrors the setting", async () => {
    const appearance = createAppearance();
    await appearance.init();
    expect(appearance.theme).toBe("system");
    expect(appearance.resolvedTheme).toBe("light"); // matchMedia matches: false
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.style.colorScheme).toBe("light");
    expect(localStorage.setItem).toHaveBeenCalledWith("oxitide.theme", "system");
  });

  it("setTheme applies and persists", async () => {
    const appearance = createAppearance();
    await appearance.init();
    await appearance.setTheme("dark");
    expect(appearance.resolvedTheme).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(api.setTheme).toHaveBeenCalledWith("dark");
  });

  /// An embedder may throw on the localStorage property getter itself, before
  /// theme.ts's item-level guards can run; boot must not reject for that.
  it("init survives an embedder whose localStorage getter throws", async () => {
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get() {
        throw new Error("denied");
      },
    });
    const appearance = createAppearance();
    await expect(appearance.init()).resolves.toBeUndefined();
    expect(appearance.theme).toBe("system");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("system follows an OS preference change", async () => {
    const appearance = createAppearance();
    await appearance.init();
    expect(appearance.resolvedTheme).toBe("light");
    expect(themeMediaListener).toBeDefined();
    themeMediaListener?.({ matches: true });
    expect(appearance.resolvedTheme).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
  });
});
