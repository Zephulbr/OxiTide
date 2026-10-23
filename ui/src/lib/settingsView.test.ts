import { describe, expect, it } from "vitest";
import { SETTINGS_TABS } from "./settingsTabs";
import { TAB_META, formatSize, outputDeviceOptions, usageFraction } from "./settingsView";
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

describe("outputDeviceOptions", () => {
  const dac = { id: "hw:CARD=Pro,DEV=0", name: "MOONDROP Dawn Pro", is_default: false };
  const sys = { id: "default", name: "Default", is_default: true };

  it("lists the system default first, then the devices", () => {
    expect(outputDeviceOptions([sys, dac], "").map((o) => o.value)).toEqual([
      "",
      "default",
      "hw:CARD=Pro,DEV=0",
    ]);
    expect(outputDeviceOptions([sys], "")[1].label).toBe("Default (default)");
  });

  it("keeps a stored device that is not connected", () => {
    const options = outputDeviceOptions([sys], "hw:CARD=Pro,DEV=0");
    expect(options[options.length - 1]).toEqual({
      value: "hw:CARD=Pro,DEV=0",
      label: "hw:CARD=Pro,DEV=0 (not connected)",
    });
    expect(outputDeviceOptions([sys, dac], dac.id)).toHaveLength(3);
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
