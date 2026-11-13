import en from "../locales/en/app.json";
import { createI18n, type Catalog } from "./i18nCore.svelte";
import { LANGUAGES, PSEUDO_LANGUAGE } from "./languages";

/// The desktop app's translator over `locales/<lang>/app.json` (the core is
/// `i18nCore.svelte.ts`). Catalogs are flat JSON in ICU MessageFormat;
/// English is bundled, the rest load as lazy chunks.

export { createI18n } from "./i18nCore.svelte";
export type { MessageParams } from "./i18nCore.svelte";
export type MessageKey = keyof typeof en;

// `notes/` sits next to the language folders but is not one; the
// pseudo-locale (generated, gitignored) is only bundled in development.
const appLoaders = {
  ...import.meta.glob<Catalog>(
    ["../locales/*/app.json", "!../locales/en/app.json", "!../locales/notes/app.json", "!../locales/en-XA/app.json"],
    { import: "default" },
  ),
  ...(import.meta.env.DEV
    ? import.meta.glob<Catalog>("../locales/en-XA/app.json", { import: "default" })
    : {}),
};

export const i18n = createI18n<MessageKey>(en, appLoaders);

/// Shorthand for `i18n.t`.
export const t = i18n.t;

/// Shorthand for `i18n.split`.
export const tSplit = i18n.split;

/// Shorthand for `i18n.english`.
export const tEn = i18n.english;

/// Languages the picker offers in this build: shipped ones with a catalog,
/// plus unshipped ones and the pseudo-locale in development builds.
export function pickerLanguages(dev = import.meta.env.DEV): string[] {
  const present = new Set(i18n.available());
  const codes = LANGUAGES.filter((l) => present.has(l.code) && (l.ship || dev)).map((l) => l.code);
  if (dev && present.has(PSEUDO_LANGUAGE)) codes.push(PSEUDO_LANGUAGE);
  return codes;
}
