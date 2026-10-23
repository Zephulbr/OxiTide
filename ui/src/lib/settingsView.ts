import type { OutputDevice } from "./api";
import type { IconName } from "./components/Icon.svelte";
import type { SettingsTab } from "./settingsTabs";

/// Label and icon for each settings category, in the order of `SETTINGS_TABS`.
export const TAB_META: Record<SettingsTab, { label: string; icon: IconName }> = {
  appearance: { label: "Appearance", icon: "palette" },
  playback: { label: "Playback", icon: "headphones" },
  content: { label: "Content", icon: "shield-check" },
  connections: { label: "Connections", icon: "link" },
  storage: { label: "Storage", icon: "hard-drive" },
  system: { label: "System", icon: "monitor" },
};

/// "812.4 MB" below a gigabyte, "1.3 GB" above.
export function formatSize(bytes: number): string {
  const mb = bytes / 1024 / 1024;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb.toFixed(1)} MB`;
}

/// The audio device picker: the system default, every listed device, and the
/// stored device when it is missing from the list (unplugged), so the choice
/// survives until it returns instead of silently becoming the default.
export function outputDeviceOptions(
  devices: OutputDevice[],
  selected: string,
): { value: string; label: string }[] {
  const options = [
    { value: "", label: "System default" },
    ...devices.map((d) => ({ value: d.id, label: `${d.name}${d.is_default ? " (default)" : ""}` })),
  ];
  if (selected && !devices.some((d) => d.id === selected)) {
    options.push({ value: selected, label: `${selected} (not connected)` });
  }
  return options;
}

/// How full the cache is, 0–1 (0 when there is no limit to measure against).
export function usageFraction(used: number, max: number): number {
  if (!(max > 0) || !(used > 0)) return 0;
  return Math.min(1, used / max);
}
