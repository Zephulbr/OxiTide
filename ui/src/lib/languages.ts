import list from "../locales/languages.json";

/// Translation status shown in the language picker (spec §5.3, §6.3).
export type LanguageStatus = "source" | "machine" | "cross-checked" | "reviewed";

export interface Language {
  /// BCP 47 code; also the catalog folder name under `locales/`.
  code: string;
  /// The language's own name (Deutsch, 日本語): what the picker shows.
  endonym: string;
  /// Font stack family (spec §9).
  script: string;
  /// Shipped in release builds. A language is drafted before it ships.
  ship: boolean;
  status: LanguageStatus;
}

export const LANGUAGES: readonly Language[] = list as Language[];

/// The development-only pseudo-locale (`scripts/i18n/pseudo.mjs`, spec §7.6).
export const PSEUDO_LANGUAGE = "en-XA";

/// The config value that follows the operating system's language.
export const SYSTEM_LANGUAGE = "system";

/// Picks the first preferred language (BCP 47 tags, best first, as in
/// `navigator.languages`) that one of `available` serves, else English
/// (spec §3): Chinese by script, any Portuguese to `pt-BR`, Norwegian to
/// `nb`, everything else by its base language.
export function matchLanguage(preferred: readonly string[], available: readonly string[]): string {
  for (const tag of preferred) {
    const hit = matchOne(tag, available);
    if (hit) return hit;
  }
  return "en";
}

function matchOne(tag: string, available: readonly string[]): string | null {
  let locale: Intl.Locale;
  try {
    locale = new Intl.Locale(tag);
  } catch {
    return null;
  }
  const has = (code: string) => (available.includes(code) ? code : null);
  switch (locale.language) {
    case "zh":
      // `maximize` fills in the likely script: zh-TW/HK/MO → Hant, zh/CN/SG → Hans.
      return has(locale.maximize().script === "Hant" ? "zh-Hant" : "zh-Hans");
    case "pt":
      return has("pt-BR");
    case "no":
    case "nb":
    case "nn":
      return has("nb");
    default:
      return has(locale.language);
  }
}
