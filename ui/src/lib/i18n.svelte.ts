import { IntlMessageFormat } from "intl-messageformat";
import en from "../locales/en/app.json";
import { LANGUAGES, PSEUDO_LANGUAGE } from "./languages";

/// Translation layer (spec `2026-11-12-oxitide-v1.1-i18n-a11y-design.md` §4).
/// Catalogs are flat JSON in ICU MessageFormat, one per language and
/// namespace; English is bundled, the rest load as lazy chunks.

export type MessageKey = keyof typeof en;
export type MessageParams = Record<string, string | number | Date>;
type Catalog = Readonly<Record<string, string>>;
type Loader = () => Promise<Catalog>;
type Compiled = IntlMessageFormat<Record<string, string | number | Date>>;

/// Builds a translator over one namespace. The desktop app uses `i18n` below
/// (`app.json`); the phone remote makes its own over `remote.json`.
export function createI18n<K extends string>(source: Catalog, loaders: Record<string, Loader>) {
  let language = $state("en");
  const catalogs = new Map<string, Catalog>([["en", source]]);
  const formats = new Map<string, Compiled>();

  function loaderFor(code: string): Loader | undefined {
    const suffix = `/${code}/`;
    for (const [path, load] of Object.entries(loaders)) if (path.includes(suffix)) return load;
    return undefined;
  }

  /// Languages that have a catalog in this build.
  function available(): string[] {
    const codes = new Set(["en"]);
    for (const path of Object.keys(loaders)) {
      const match = /\/locales\/([^/]+)\//.exec(path);
      if (match) codes.add(match[1]);
    }
    return [...codes];
  }

  /// Loads a language's catalog, then switches to it, so text never shows a
  /// half-loaded language. An unknown or unloadable code means English.
  async function setLanguage(code: string): Promise<string> {
    let next = code;
    if (!catalogs.has(next)) {
      const load = loaderFor(next);
      try {
        if (!load) throw new Error(`no catalog for ${next}`);
        catalogs.set(next, await load());
      } catch {
        next = "en";
      }
    }
    language = next;
    return next;
  }

  function format(code: string, key: string, message: string, params?: MessageParams): string {
    // Plain text skips the parser; an apostrophe can still be an ICU escape.
    if (!message.includes("{") && !message.includes("'")) return message;
    const id = `${code}\u0000${key}`;
    let compiled = formats.get(id);
    if (!compiled) {
      // `ignoreTag`: text like "<Enter>" is literal, not an XML tag.
      compiled = new IntlMessageFormat(message, code === PSEUDO_LANGUAGE ? "en" : code, undefined, {
        ignoreTag: true,
      }) as Compiled;
      formats.set(id, compiled);
    }
    return String(compiled.format(params ?? {}));
  }

  /// The message for `key` in the current language, formatted with `params`.
  /// A message missing or broken in a translation falls back to English
  /// (`scripts/i18n/check.mjs` keeps that from shipping).
  function t(key: K, params?: MessageParams): string {
    const code = language;
    const translated = catalogs.get(code)?.[key];
    if (translated !== undefined) {
      try {
        return format(code, key, translated, params);
      } catch {
        // Fall through to English.
      }
    }
    const english = source[key];
    if (english === undefined) return key;
    try {
      return format("en", key, english, params);
    } catch {
      return english;
    }
  }

  /// The English message for `key`, whatever the language (search matches
  /// English terms in every language, spec §4.5).
  function english(key: K, params?: MessageParams): string {
    const message = source[key];
    if (message === undefined) return key;
    try {
      return format("en", key, message, params);
    } catch {
      return message;
    }
  }

  /// The message split around one placeholder, for a sentence that wraps
  /// markup (a styled name, a link): `[before, after]` the `slot` param.
  /// Word order stays the translation's; only the slot's text is markup.
  function split(key: K, slot: string, params?: MessageParams): [string, string] {
    const text = t(key, { ...params, [slot]: SLOT });
    const at = text.indexOf(SLOT);
    return at < 0 ? [text, ""] : [text.slice(0, at), text.slice(at + SLOT.length)];
  }

  return {
    /// The resolved language code (`en`, `de`, `zh-Hant`…), for `Intl` calls
    /// and `<html lang>`. Reading it in a template or `$derived` tracks it.
    get language() {
      return language;
    },
    /// The locale to hand to `Intl` APIs (the pseudo-locale formats as English).
    get locale() {
      return language === PSEUDO_LANGUAGE ? "en" : language;
    },
    available,
    setLanguage,
    t,
    split,
    english,
  };
}

/// Stands in for a `split` slot; a private-use character no message contains.
const SLOT = "";

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
