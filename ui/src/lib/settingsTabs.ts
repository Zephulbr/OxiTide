export const SETTINGS_TABS = [
  "appearance",
  "playback",
  "content",
  "connections",
  "storage",
  "system",
] as const;

export type SettingsTab = (typeof SETTINGS_TABS)[number];

export function nextTab(current: SettingsTab, direction: 1 | -1): SettingsTab {
  const index = SETTINGS_TABS.indexOf(current);
  return SETTINGS_TABS[(index + direction + SETTINGS_TABS.length) % SETTINGS_TABS.length];
}
