// Output picker list logic (spec 2026-11-05-oxitide-output-device-picker-design.md).
// Pure: OutputDevicePicker.svelte and the player bar's quality menu render from it.
import type { DeviceGroup, DeviceKind, OutputDevice, OutputDeviceList } from "./api";
import type { IconName } from "./components/Icon.svelte";

/** Fired by the backend when devices change or capability checks finish. */
export const OUTPUT_DEVICES_CHANGED_EVENT = "output_devices_changed";

/** Icon choices offered in a row's menu ("" = from the device type). */
export const DEVICE_ICON_CHOICES: { value: string; label: string; icon?: IconName }[] = [
  { value: "", label: "Automatic" },
  { value: "speaker", label: "Speaker", icon: "speaker" },
  { value: "headphones", label: "Headphones", icon: "headphones" },
  { value: "radio-receiver", label: "DAC / Amp", icon: "radio-receiver" },
  { value: "monitor", label: "Monitor", icon: "monitor" },
  { value: "tv", label: "TV", icon: "tv" },
  { value: "laptop", label: "Laptop", icon: "laptop" },
  { value: "usb", label: "USB", icon: "usb" },
  { value: "bluetooth", label: "Bluetooth", icon: "bluetooth" },
  { value: "audio-lines", label: "Lines", icon: "audio-lines" },
  { value: "none", label: "No icon" },
];

const GROUP_LABELS: Record<DeviceGroup, string> = {
  wasapi: "Windows audio",
  asio: "ASIO",
  alsa: "Sound cards",
};

const GROUP_ORDER: DeviceGroup[] = ["wasapi", "alsa", "asio"];

/** The name a row shows: the nickname, else the device's clean name. */
export function displayName(device: OutputDevice): string {
  const nickname = device.prefs.nickname.trim();
  return nickname || device.name;
}

/** The icon a device type gets when no icon was chosen. */
export function kindIcon(kind: DeviceKind): IconName {
  switch (kind) {
    case "headphones":
    case "headset":
      return "headphones";
    case "hdmi":
      return "monitor";
    case "spdif":
    case "line":
      return "cable";
    case "bluetooth":
      return "bluetooth";
    case "usb":
      return "usb";
    case "virtual":
      return "audio-lines";
    default:
      return "speaker";
  }
}

/** The row's icon, or null for "No icon". An unplugged device shows a plug. */
export function deviceIcon(device: OutputDevice): IconName | null {
  if (!device.connected) return "unplug";
  const chosen = device.prefs.icon;
  if (chosen === "none") return null;
  const choice = DEVICE_ICON_CHOICES.find((c) => c.value === chosen && c.icon);
  if (choice?.icon) return choice.icon;
  if (device.group === "asio") return device.wrapper ? "audio-lines" : kindIcon(device.kind === "unknown" ? "virtual" : device.kind);
  return kindIcon(device.kind);
}

const KIND_LABELS: Partial<Record<DeviceKind, string>> = {
  speakers: "Speakers",
  headphones: "Headphones",
  headset: "Headset",
  hdmi: "HDMI",
  spdif: "Digital (S/PDIF)",
  line: "Line out",
  bluetooth: "Bluetooth",
  usb: "USB",
  virtual: "Virtual device",
};

/** The row's muted second line: how the device is connected. */
export function connectionLabel(device: OutputDevice): string {
  if (!device.connected) return "Not connected";
  if (device.group === "asio") return device.wrapper ? "Wrapper over Windows audio" : "Always exclusive";
  switch (device.transport) {
    case "usb":
      return "USB";
    case "bluetooth":
      return "Bluetooth";
    case "hdmi":
      return "HDMI";
    case "internal":
      return device.kind === "spdif" ? "Digital (S/PDIF)" : "Internal";
    case "virtual":
      return "Virtual device";
    default:
      return KIND_LABELS[device.kind] ?? "";
  }
}

/** "44.1 kHz", "96 kHz", "352.8 kHz". */
export function formatRate(hz: number): string {
  const khz = hz / 1000;
  return `${Number.isInteger(khz) ? khz : khz.toFixed(1)} kHz`;
}

/** What the selected row says the device supports (one line, or null). */
export function supportsLine(device: OutputDevice): string | null {
  const caps = device.caps;
  if (!caps) return null;
  const parts: string[] = [];
  if (caps.exclusive && caps.max_rate) {
    parts.push(`Up to ${formatRate(caps.max_rate)}${caps.max_bits ? ` · ${caps.max_bits}-bit` : ""}`);
  } else if (!caps.exclusive && caps.source === "probed") {
    parts.push(caps.shared_rate ? `Shared only · ${formatRate(caps.shared_rate)}` : "Shared only");
  }
  if (caps.hw_volume === true) parts.push("hardware volume");
  if (caps.source === "played" && parts.length > 0) parts.push("from playback");
  return parts.length ? parts.join(", ") : null;
}

/**
 * A format line split into text and number runs, so the numbers can be
 * coloured: "Up to 192 kHz · 24-bit" → "Up to ", "192", " kHz · ", "24", "-bit".
 */
export function numberRuns(text: string): { text: string; number: boolean }[] {
  return text
    .split(/(\d+(?:\.\d+)?)/)
    .map((run, i) => ({ text: run, number: i % 2 === 1 }))
    .filter((run) => run.text !== "");
}

/** The mode label of a device in the quality menu. */
export function modeLabel(device: OutputDevice): string {
  if (device.group === "asio") return "ASIO";
  return device.prefs.mode === "exclusive" ? "Exclusive" : "Shared";
}

export interface DeviceGroupView {
  group: DeviceGroup;
  label: string;
  /** The selected device when it is in this group: drawn first, outside the scroll. */
  selected: OutputDevice | null;
  /** The group's other devices (the part that scrolls). */
  devices: OutputDevice[];
}

export interface PickerView {
  groups: DeviceGroupView[];
  /** Hidden devices (shown only when revealed). */
  hiddenCount: number;
  /** Outputs with nothing attached (HDMI with no monitor), by name. */
  absent: OutputDevice[];
}

/**
 * Group the list for Settings: Windows audio / Sound cards / ASIO, connected
 * devices before unplugged ones. Hidden devices and empty outputs drop out
 * unless revealed, but the selected device always shows, at the top of its
 * group. Pinned devices come next, in pin order, then the rest.
 */
export function pickerView(list: OutputDeviceList, showHidden: boolean, showAbsent: boolean): PickerView {
  const selected = list.selected;
  const absent = list.devices.filter((d) => d.connected && !d.present && d.id !== selected);
  const hiddenCount = list.devices.filter((d) => d.prefs.hidden && d.id !== selected).length;
  const visible = list.devices.filter((d) => {
    if (d.id === selected) return true;
    if (d.prefs.hidden && !showHidden) return false;
    if (d.connected && !d.present && !showAbsent) return false;
    return true;
  });
  // Connected pins, other connected devices, unplugged pins, other unplugged.
  const rank = (d: OutputDevice) => {
    const pin = list.pinned.indexOf(d.id);
    return [(d.connected ? 0 : 2) + (pin >= 0 ? 0 : 1), pin];
  };
  const order = (a: OutputDevice, b: OutputDevice) => {
    const [ra, pa] = rank(a);
    const [rb, pb] = rank(b);
    return ra - rb || pa - pb;
  };
  const groups = GROUP_ORDER.map((group) => {
    const members = visible.filter((d) => d.group === group);
    return {
      group,
      label: GROUP_LABELS[group],
      selected: members.find((d) => d.id === selected) ?? null,
      devices: members.filter((d) => d.id !== selected).sort(order),
    };
  }).filter((g) => g.selected || g.devices.length > 0);
  return { groups, hiddenCount, absent };
}

/**
 * A toast for a failed picker action. Backend errors are text worth showing
 * after the action; anything else is a bug in the page, so the toast says only
 * what failed and the details go to the console.
 */
export function deviceError(action: string, error: unknown): string {
  if (typeof error === "string" && error.trim()) return `${action}: ${error}`;
  console.error(action, error);
  return `${action}. Reopen Settings to try again.`;
}

/** The quality menu's Output section: pinned devices in pin order. */
export function pinnedDevices(list: OutputDeviceList): OutputDevice[] {
  return list.pinned
    .map((id) => list.devices.find((d) => d.id === id))
    .filter((d): d is OutputDevice => d !== undefined);
}
