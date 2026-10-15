import { describe, expect, it } from "vitest";
import { SETTINGS_TABS } from "./settingsTabs";
import { TAB_META, formatSize, usageFraction } from "./settingsView";
import { formatDbShort } from "./dsp";

describe("TAB_META", () => {
  it("describes every settings tab", () => {
    for (const id of SETTINGS_TABS) {
      expect(TAB_META[id].label.length).toBeGreaterThan(0);
      expect(TAB_META[id].icon.length).toBeGreaterThan(0);
    }
  });
});

describe("formatSize", () => {
  it("uses megabytes below a gigabyte", () => {
    expect(formatSize(0)).toBe("0.0 MB");
    expect(formatSize(5 * 1024 * 1024)).toBe("5.0 MB");
  });
  it("switches to gigabytes", () => {
    expect(formatSize(1536 * 1024 * 1024)).toBe("1.5 GB");
  });
});

describe("usageFraction", () => {
  it("is the used share of the limit", () => {
    expect(usageFraction(25, 100)).toBe(0.25);
  });
  it("caps at full and ignores a missing limit", () => {
    expect(usageFraction(300, 100)).toBe(1);
    expect(usageFraction(10, 0)).toBe(0);
    expect(usageFraction(0, 100)).toBe(0);
  });
});

describe("formatDbShort", () => {
  it("signs boosts and cuts", () => {
    expect(formatDbShort(0)).toBe("0");
    expect(formatDbShort(3)).toBe("+3");
    expect(formatDbShort(-2.5)).toBe("−2.5");
  });
});
