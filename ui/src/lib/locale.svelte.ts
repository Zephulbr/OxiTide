/// The locale `Intl` formatting and sorting use (spec §4.4). The translators
/// (`i18nCore.svelte.ts`) set it when their language changes; `format.ts`
/// and the sorters read it. Shared with the phone remote, so it holds no
/// catalog. Reading `formatLocale.value` in a template or `$derived` tracks
/// it, so formatted text follows a language switch.

let current = $state("en");

export const formatLocale = {
  get value(): string {
    return current;
  },
};

export function setFormatLocale(code: string): void {
  current = code;
}

/// Compares names the way the UI language sorts them (accents and case
/// folded, numbers by value).
export function compareText(a: string, b: string): number {
  return collator().compare(a, b);
}

let cached: { locale: string; collator: Intl.Collator } | null = null;

export function collator(): Intl.Collator {
  const locale = current;
  if (cached?.locale !== locale) {
    cached = { locale, collator: new Intl.Collator(locale, { sensitivity: "base", numeric: true }) };
  }
  return cached.collator;
}
