import { describe, expect, it, vi } from "vitest";
import type { OutputDevice, OutputDeviceList } from "./api";
import {
  connectionLabel,
  deviceError,
  deviceIcon,
  displayName,
  formatRate,
  modeLabel,
  numberRuns,
  pickerView,
  pinnedDevices,
  supportsLine,
} from "./outputDevices";

function device(id: string, extra: Partial<OutputDevice> = {}, prefs: Partial<OutputDevice["prefs"]> = {}): OutputDevice {
  return {
    id,
    name: id.toUpperCase(),
    system_name: `${id} (system)`,
    group: "wasapi",
    kind: "speakers",
    transport: "internal",
    is_default: false,
    connected: true,
    wrapper: false,
    present: true,
    caps: null,
    ...extra,
    prefs: { nickname: "", icon: "", pinned: false, hidden: false, mode: "shared", ...prefs },
  };
}

function list(devices: OutputDevice[], extra: Partial<OutputDeviceList> = {}): OutputDeviceList {
  return {
    system_default: { now: null, mode: "shared" },
    devices,
    pinned: [],
    selected: "",
    icons: true,
    moved: [],
    ...extra,
  };
}

describe("displayName", () => {
  it("prefers the nickname and falls back to the device name", () => {
    expect(displayName(device("dac", {}, { nickname: "Desk DAC" }))).toBe("Desk DAC");
    expect(displayName(device("dac", {}, { nickname: "   " }))).toBe("DAC");
  });
});

describe("deviceIcon", () => {
  it("follows the type, a chosen icon, or none", () => {
    expect(deviceIcon(device("tv", { kind: "hdmi" }))).toBe("monitor");
    expect(deviceIcon(device("dac", { kind: "usb" }))).toBe("usb");
    expect(deviceIcon(device("out", { kind: "spdif" }))).toBe("cable");
    expect(deviceIcon(device("dac", { kind: "usb" }, { icon: "headphones" }))).toBe("headphones");
    expect(deviceIcon(device("dac", {}, { icon: "none" }))).toBeNull();
    expect(deviceIcon(device("gone", { connected: false }, { icon: "headphones" }))).toBe("unplug");
  });
});

describe("connectionLabel", () => {
  it("says how a device connects", () => {
    expect(connectionLabel(device("a", { transport: "usb" }))).toBe("USB");
    expect(connectionLabel(device("a", { transport: "hdmi" }))).toBe("HDMI");
    expect(connectionLabel(device("a", { transport: "internal", kind: "spdif" }))).toBe("Digital (S/PDIF)");
    expect(connectionLabel(device("a", { transport: "unknown", kind: "headphones" }))).toBe("Headphones");
    expect(connectionLabel(device("a", { group: "asio" }))).toBe("Always exclusive");
    expect(connectionLabel(device("a", { group: "asio", wrapper: true }))).toBe("Wrapper over Windows audio");
    expect(connectionLabel(device("a", { connected: false }))).toBe("Not connected");
  });
});

describe("capabilities", () => {
  const caps = (extra: Partial<NonNullable<OutputDevice["caps"]>>) => ({
    exclusive: true,
    max_rate: 384000,
    max_bits: 32,
    hw_volume: true,
    shared_rate: 48000,
    shared_bits: 32,
    source: "probed" as const,
    ...extra,
  });

  it("splits a format line into number runs", () => {
    expect(numberRuns("Up to 192 kHz · 24-bit")).toEqual([
      { text: "Up to ", number: false },
      { text: "192", number: true },
      { text: " kHz · ", number: false },
      { text: "24", number: true },
      { text: "-bit", number: false },
    ]);
    expect(numberRuns("352.8 kHz")).toEqual([
      { text: "352.8", number: true },
      { text: " kHz", number: false },
    ]);
    expect(numberRuns("Shared only")).toEqual([{ text: "Shared only", number: false }]);
  });

  it("summarises what a device supports", () => {
    expect(supportsLine(device("dac", { caps: caps({}) }))).toBe("Up to 384 kHz · 32-bit, hardware volume");
    expect(supportsLine(device("mic", { caps: caps({ exclusive: false, max_rate: null, hw_volume: null }) }))).toBe(
      "Shared only · 48 kHz",
    );
    expect(supportsLine(device("dac", { caps: caps({ source: "played", max_rate: 96000, max_bits: 24, hw_volume: null }) }))).toBe(
      "Up to 96 kHz · 24-bit, from playback",
    );
    expect(supportsLine(device("dac"))).toBeNull();
  });

  it("formats rates", () => {
    expect(formatRate(44100)).toBe("44.1 kHz");
    expect(formatRate(96000)).toBe("96 kHz");
    expect(formatRate(352800)).toBe("352.8 kHz");
  });
});

describe("pickerView", () => {
  it("groups by type, hides hidden and empty outputs, keeps the selection on top", () => {
    const devices = [
      device("asio", { group: "asio" }),
      device("gone", { connected: false }),
      device("spk"),
      device("hdmi2", { present: false }),
      device("hid", {}, { hidden: true }),
      device("sel", {}, { hidden: true }),
    ];
    const view = pickerView(list(devices, { selected: "sel" }), false, false);
    expect(view.groups.map((g) => [g.label, g.selected?.id ?? null, g.devices.map((d) => d.id)])).toEqual([
      ["Windows audio", "sel", ["spk", "gone"]],
      ["ASIO", null, ["asio"]],
    ]);
    expect(view.hiddenCount).toBe(1);
    expect(view.absent.map((d) => d.id)).toEqual(["hdmi2"]);

    const revealed = pickerView(list(devices, { selected: "sel" }), true, true);
    expect(revealed.groups[0].devices.map((d) => d.id)).toEqual(["spk", "hdmi2", "hid", "gone"]);
  });

  it("puts pinned devices first in pin order, unplugged ones last", () => {
    const devices = [
      device("a"),
      device("b"),
      device("c"),
      device("gone", { connected: false }),
      device("gone-pin", { connected: false }),
    ];
    const view = pickerView(list(devices, { pinned: ["gone-pin", "c", "b"] }), false, false);
    expect(view.groups[0].devices.map((d) => d.id)).toEqual(["c", "b", "a", "gone-pin", "gone"]);
  });

  it("keeps a group whose only visible device is the selected one", () => {
    const view = pickerView(list([device("asio", { group: "asio" })], { selected: "asio" }), false, false);
    expect(view.groups.map((g) => [g.label, g.selected?.id, g.devices.length])).toEqual([["ASIO", "asio", 0]]);
  });
});

describe("deviceError", () => {
  it("keeps backend reasons and hides page bugs behind the action", () => {
    expect(deviceError("Couldn't switch to DX5", "device is busy")).toBe("Couldn't switch to DX5: device is busy");
    const quiet = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(deviceError("Couldn't read the output devices", new ReferenceError("revealed is not defined"))).toBe(
      "Couldn't read the output devices. Reopen Settings to try again.",
    );
    expect(quiet).toHaveBeenCalled();
    quiet.mockRestore();
  });
});

describe("pinnedDevices", () => {
  it("keeps pin order and skips pins with no row", () => {
    const devices = [device("a"), device("b"), device("c")];
    expect(pinnedDevices(list(devices, { pinned: ["c", "x", "a"] })).map((d) => d.id)).toEqual(["c", "a"]);
  });
});

describe("modeLabel", () => {
  it("names the mode, ASIO always", () => {
    expect(modeLabel(device("a", {}, { mode: "exclusive" }))).toBe("Exclusive");
    expect(modeLabel(device("a"))).toBe("Shared");
    expect(modeLabel(device("a", { group: "asio" }))).toBe("ASIO");
  });
});
