import type { IconName } from "./components/Icon.svelte";
import type { SettingsTab } from "./settingsTabs";
import { t } from "./i18n.svelte";

/// Label and icon for each settings category, in the order of `SETTINGS_TABS`.
export const TAB_META: Record<SettingsTab, { label: string; icon: IconName }> = {
  appearance: { get label() { return t("settings.tab.appearance"); }, icon: "palette" },
  playback: { get label() { return t("settings.tab.playback"); }, icon: "headphones" },
  content: { get label() { return t("settings.tab.content"); }, icon: "shield-check" },
  connections: { get label() { return t("settings.tab.connections"); }, icon: "link" },
  storage: { get label() { return t("settings.tab.storage"); }, icon: "hard-drive" },
  system: { get label() { return t("settings.tab.system"); }, icon: "monitor" },
};

/// "812.4 MB" below a gigabyte, "1.3 GB" above.
export function formatSize(bytes: number): string {
  const mb = bytes / 1024 / 1024;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb.toFixed(1)} MB`;
}

/// How full the cache is, 0–1 (0 when there is no limit to measure against).
export function usageFraction(used: number, max: number): number {
  if (!(max > 0) || !(used > 0)) return 0;
  return Math.min(1, used / max);
}
