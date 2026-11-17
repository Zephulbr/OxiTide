import en from "../locales/en/remote.json";
import { createI18n, type Catalog } from "../lib/i18nCore.svelte";
import { LANGUAGES, matchLanguage, PSEUDO_LANGUAGE } from "../lib/languages";

/// The phone remote's translator over `locales/<lang>/remote.json`. The
/// phone has no language setting: it follows the phone's own languages
/// (`navigator.languages`, spec §5.4).

export type RemoteKey = keyof typeof en;

const loaders = {
  ...import.meta.glob<Catalog>(
    [
      "../locales/*/remote.json",
      "!../locales/en/remote.json",
      "!../locales/notes/remote.json",
      "!../locales/en-XA/remote.json",
    ],
    { import: "default" },
  ),
  ...(import.meta.env.DEV
    ? import.meta.glob<Catalog>("../locales/en-XA/remote.json", { import: "default" })
    : {}),
};

export const remoteI18n = createI18n<RemoteKey>(en, loaders);

/// Shorthand for `remoteI18n.t`.
export const t = remoteI18n.t;

/// Shorthand for `remoteI18n.split`.
export const tSplit = remoteI18n.split;

/// The languages the phone may pick: shipped ones with a catalog (any with
/// a catalog in development builds).
export function remoteLanguages(dev = import.meta.env.DEV): string[] {
  const present = new Set(remoteI18n.available());
  return LANGUAGES.filter((l) => present.has(l.code) && (l.ship || dev)).map((l) => l.code);
}

/// Picks the phone's language, loads it and sets `<html lang>`. Never fails:
/// anything wrong leaves English.
export async function initRemoteLanguage(preferred: readonly string[] = navigator.languages ?? []): Promise<string> {
  const code = await remoteI18n.setLanguage(matchLanguage(preferred, remoteLanguages()));
  document.documentElement.lang = code === PSEUDO_LANGUAGE ? "en" : code;
  return code;
}
