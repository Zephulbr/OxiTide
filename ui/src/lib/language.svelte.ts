import { api } from "./api";
import { i18n } from "./i18n.svelte";
import { LANGUAGES, PSEUDO_LANGUAGE, SYSTEM_LANGUAGE, matchLanguage } from "./languages";
import { writeMirror, type ThemeStorage } from "./theme";
import { clearViewCache } from "./viewCache";

/// The resolved language, mirrored so `index.html` sets `<html lang>` and
/// `main.ts` loads the right catalog before the first frame (spec §4.3).
export const LANGUAGE_STORAGE_KEY = "oxitide.language";

/// What `"system"` means here: the webview's languages (they follow the OS
/// display language) matched against the catalogs this build has.
export function systemLanguage(
  preferred: readonly string[] = typeof navigator === "undefined" ? [] : navigator.languages,
  available: readonly string[] = i18n.available(),
): string {
  const shipped = available.filter((code) => code !== PSEUDO_LANGUAGE);
  return matchLanguage(preferred, shipped);
}

/// A config value the picker understands; anything else acts as "system".
export function normalizeSetting(value: string | undefined): string {
  if (!value || value === SYSTEM_LANGUAGE) return SYSTEM_LANGUAGE;
  if (value === PSEUDO_LANGUAGE) return value;
  return LANGUAGES.some((l) => l.code === value) ? value : SYSTEM_LANGUAGE;
}

export function resolveSetting(setting: string, system: () => string = systemLanguage): string {
  return setting === SYSTEM_LANGUAGE ? system() : setting;
}

function storage(): ThemeStorage | undefined {
  try {
    return typeof localStorage === "undefined" ? undefined : localStorage;
  } catch {
    return undefined;
  }
}

/// Reads the mirror for boot; an absent or unknown value means English.
export function readLanguageMirror(store: ThemeStorage | undefined = storage()): string {
  try {
    const value = store?.getItem(LANGUAGE_STORAGE_KEY);
    if (value && (value === PSEUDO_LANGUAGE || LANGUAGES.some((l) => l.code === value))) return value;
  } catch {
    // Unreadable storage: English until the config lands.
  }
  return "en";
}

function createLanguageSetting() {
  let setting = $state(SYSTEM_LANGUAGE);

  /// Loads and switches to the language `value` resolves to, then mirrors it
  /// for the next boot and tells the shell (tray, taskbar).
  async function apply(value: string | undefined): Promise<void> {
    setting = normalizeSetting(value);
    const code = await i18n.setLanguage(resolveSetting(setting));
    if (typeof document !== "undefined") document.documentElement.lang = i18n.locale;
    writeMirror(storage(), LANGUAGE_STORAGE_KEY, code);
    // Best effort: the tray and taskbar keep their old labels on a failure.
    Promise.resolve()
      .then(() => api.setShellLanguage(code))
      .catch(() => {});
  }

  /// Applies first (local-first, like the appearance setters), then
  /// persists; a failed persist restores the previous language.
  async function set(value: string): Promise<void> {
    const previous = setting;
    await apply(value);
    // Cached pages may hold text from TIDAL in the old language.
    clearViewCache();
    try {
      await api.setLanguage(setting);
    } catch (e) {
      await apply(previous);
      throw e;
    }
  }

  return {
    /// The saved choice: "system" or a code.
    get setting() {
      return setting;
    },
    apply,
    set,
  };
}

export const language = createLanguageSetting();
