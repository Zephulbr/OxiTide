export type ThemeSetting = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

export const THEME_STORAGE_KEY = "oxitide.theme";

export interface ThemeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/// `System` follows the OS preference; explicit settings win (spec §6.1).
export function resolveTheme(setting: ThemeSetting, prefersDark: boolean): ResolvedTheme {
  if (setting === "system") return prefersDark ? "dark" : "light";
  return setting;
}

/// The pre-paint script reads the same mirror; an absent or unreadable value
/// means the config default (dark) so first paint is never wrong for
/// existing installs (Review Focus 1/3).
export function readThemeMirror(storage?: ThemeStorage): ThemeSetting {
  try {
    const value = storage?.getItem(THEME_STORAGE_KEY);
    if (value === "dark" || value === "light" || value === "system") return value;
  } catch {
    // Storage may be unavailable; the config default is the fallback.
  }
  return "dark";
}

export function writeThemeMirror(storage: ThemeStorage | undefined, setting: ThemeSetting): void {
  try {
    storage?.setItem(THEME_STORAGE_KEY, setting);
  } catch {
    // Non-fatal: the next boot falls back to dark and init reconciles.
  }
}
