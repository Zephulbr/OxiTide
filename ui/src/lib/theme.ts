export type ThemeSetting = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

export const THEME_STORAGE_KEY = "oxitide.theme";
/// Mode and Studio tone are mirrored the same way, so the pre-paint script
/// (index.html) draws the first frame in the right look.
export const MODE_STORAGE_KEY = "oxitide.mode";
export const TONE_STORAGE_KEY = "oxitide.tone";

export interface ThemeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/// `System` follows the OS preference; explicit settings win (spec §6.1).
export function resolveTheme(setting: ThemeSetting, prefersDark: boolean): ResolvedTheme {
  if (setting === "system") return prefersDark ? "dark" : "light";
  return setting;
}

/// Reads a mirrored setting; an absent, unknown or unreadable value means the
/// config default, so first paint is never wrong for existing installs.
export function readMirror<T extends string>(
  storage: ThemeStorage | undefined,
  key: string,
  allowed: readonly T[],
  fallback: T,
): T {
  try {
    const value = storage?.getItem(key);
    if (value && (allowed as readonly string[]).includes(value)) return value as T;
  } catch {
    // Storage may be unavailable; the config default is the fallback.
  }
  return fallback;
}

export function writeMirror(storage: ThemeStorage | undefined, key: string, value: string): void {
  try {
    storage?.setItem(key, value);
  } catch {
    // Non-fatal: the next boot falls back to the default and init reconciles.
  }
}

/// The pre-paint script reads the same mirror (Review Focus 1/3).
export function readThemeMirror(storage?: ThemeStorage): ThemeSetting {
  return readMirror(storage, THEME_STORAGE_KEY, ["dark", "light", "system"], "dark");
}

export function writeThemeMirror(storage: ThemeStorage | undefined, setting: ThemeSetting): void {
  writeMirror(storage, THEME_STORAGE_KEY, setting);
}
