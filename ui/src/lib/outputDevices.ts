// Output picker list logic (spec 2026-11-05-oxitide-output-device-picker-design.md).
// Pure: OutputDevicePicker.svelte and the player bar's quality menu render from it.
import type { DeviceGroup, DeviceKind, OutputDevice, OutputDeviceList } from "./api";
import type { IconName } from "./components/Icon.svelte";
import { t, type MessageKey } from "./i18n.svelte";

/** Fired by the backend when devices change or capability checks finish. */
export const OUTPUT_DEVICES_CHANGED_EVENT = "output_devices_changed";

/** Icon choices offered in a row's menu ("" = from the device type). */
export const DEVICE_ICON_CHOICES: { value: string; label: string; icon?: IconName }[] = [
  { value: "", get label() { return t("device.icon.auto"); } },
  { value: "speaker", get label() { return t("device.icon.speaker"); }, icon: "speaker" },
  { value: "headphones", get label() { return t("device.kind.headphones"); }, icon: "headphones" },
  { value: "radio-receiver", get label() { return t("device.icon.dac"); }, icon: "radio-receiver" },
  { value: "monitor", get label() { return t("device.icon.monitor"); }, icon: "monitor" },
  { value: "tv", get label() { return t("device.icon.tv"); }, icon: "tv" },
  { value: "laptop", get label() { return t("device.icon.laptop"); }, icon: "laptop" },
  { value: "usb", label: "USB", icon: "usb" },
  { value: "bluetooth", label: "Bluetooth", icon: "bluetooth" },
  { value: "audio-lines", get label() { return t("device.icon.lines"); }, icon: "audio-lines" },
  { value: "none", get label() { return t("device.icon.none"); } },
];

const GROUP_LABELS: Record<DeviceGroup, string> = {
  get wasapi() { return t("device.group.wasapi"); },
  asio: "ASIO",
  get alsa() { return t("device.group.alsa"); },
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
  get speakers() { return t("device.kind.speakers"); },
  get headphones() { return t("device.kind.headphones"); },
  get headset() { return t("device.kind.headset"); },
  hdmi: "HDMI",
  get spdif() { return t("device.kind.spdif"); },
  get line() { return t("device.kind.line"); },
  bluetooth: "Bluetooth",
  usb: "USB",
  get virtual() { return t("device.kind.virtual"); },
};

/** The row's muted second line: how the device is connected. */
export function connectionLabel(device: OutputDevice): string {
  if (!device.connected) return t("player.notConnected");
  if (device.group === "asio") return device.wrapper ? t("device.asioWrapper") : t("device.alwaysExclusive");
  switch (device.transport) {
    case "usb":
      return "USB";
    case "bluetooth":
      return "Bluetooth";
    case "hdmi":
      return "HDMI";
    case "internal":
      return device.kind === "spdif" ? t("device.kind.spdif") : t("device.internal");
    case "virtual":
      return t("device.kind.virtual");
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
    parts.push(
      caps.max_bits
        ? t("device.upToBits", { rate: formatRate(caps.max_rate), bits: caps.max_bits })
        : t("device.upTo", { rate: formatRate(caps.max_rate) }),
    );
  } else if (!caps.exclusive && caps.source === "probed") {
    parts.push(caps.shared_rate ? t("device.sharedOnlyRate", { rate: formatRate(caps.shared_rate) }) : t("device.sharedOnly"));
  }
  if (caps.hw_volume === true) parts.push(t("device.hardwareVolume"));
  if (caps.source === "played" && parts.length > 0) parts.push(t("device.fromPlayback"));
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
  return device.prefs.mode === "exclusive" ? t("device.mode.exclusive") : t("device.mode.shared");
}

/** Whether output `id` ("" = System default) plays exclusive: ASIO always,
 *  others by their own mode. */
export function playsExclusive(list: OutputDeviceList, id: string): boolean {
  if (id === "") return list.system_default.mode === "exclusive";
  const device = list.devices.find((d) => d.id === id);
  if (!device) return false;
  return device.group === "asio" || device.prefs.mode === "exclusive";
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
  if (typeof error === "string" && error.trim()) return t("device.errorDetail", { action, error });
  console.error(action, error);
  return t("device.errorRetry", { action });
}

/** The quality menu's Output section: pinned devices in pin order. */
export function pinnedDevices(list: OutputDeviceList): OutputDevice[] {
  return list.pinned
    .map((id) => list.devices.find((d) => d.id === id))
    .filter((d): d is OutputDevice => d !== undefined);
}
