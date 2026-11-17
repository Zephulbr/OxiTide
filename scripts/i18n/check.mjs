// CI gate for the translation catalogs (spec §7.1). Exit 1 on any error;
// warnings (stale messages, missing ones in unshipped languages, glossary
// misses, English messages no source uses) are printed but do not fail.
//
//   node scripts/i18n/check.mjs [--quiet]

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import {
  LOCALES,
  NAMESPACES,
  SOURCE,
  catalog,
  catalogFolders,
  compareMessages,
  derivedStatus,
  languageStates,
  languages,
  meta,
  paths,
  parse,
  parseArgs,
  readJson,
  stateOf,
} from "./lib.mjs";

const STATUSES = new Set(["source", "machine", "cross-checked", "reviewed"]);

export function check() {
  const errors = [];
  const warnings = [];
  const list = languages();
  const codes = list.map((l) => l.code);
  const folders = catalogFolders();

  // languages.json
  if (codes[0] !== SOURCE) errors.push(`languages.json: "${SOURCE}" must come first`);
  if (new Set(codes).size !== codes.length) errors.push("languages.json: duplicate codes");
  for (const entry of list) {
    if (!entry.code || !entry.endonym || !entry.script) errors.push(`languages.json: ${JSON.stringify(entry)} lacks code/endonym/script`);
    if (!STATUSES.has(entry.status)) errors.push(`languages.json: ${entry.code} has unknown status "${entry.status}"`);
    try {
      new Intl.Locale(entry.code);
    } catch {
      errors.push(`languages.json: ${entry.code} is not a BCP 47 tag`);
    }
    if (entry.ship && !folders.includes(entry.code)) errors.push(`${entry.code} ships but has no catalog folder`);
  }
  for (const folder of folders) {
    if (!codes.includes(folder)) errors.push(`locales/${folder}/ is not listed in languages.json`);
  }

  // English, notes and the shell's plain strings
  for (const ns of NAMESPACES) {
    const source = catalog(SOURCE, ns);
    const notes = readJson(paths.notes(ns), {});
    for (const [key, message] of Object.entries(source)) {
      try {
        parse(message);
      } catch (e) {
        errors.push(`en/${ns}.json ${key}: does not parse: ${e.message}`);
      }
      if (!notes[key]) errors.push(`notes/${ns}.json lacks a note for ${key}`);
      if (ns === "shell" && /[{}]/.test(message)) errors.push(`en/shell.json ${key}: shell text must be plain (no ICU)`);
    }
    for (const key of Object.keys(notes)) {
      if (!(key in source)) errors.push(`notes/${ns}.json has a note for unknown key ${key}`);
    }
  }

  // Each translation
  for (const lang of folders.filter((f) => f !== SOURCE)) {
    const entry = list.find((l) => l.code === lang);
    const ship = Boolean(entry?.ship);
    let missing = 0;
    let stale = 0;
    for (const ns of NAMESPACES) {
      const source = catalog(SOURCE, ns);
      const translated = catalog(lang, ns);
      const entries = meta(lang, ns);
      for (const [key, message] of Object.entries(translated)) {
        if (!(key in source)) {
          errors.push(`${lang}/${ns}.json has unknown key ${key}`);
          continue;
        }
        if (typeof message !== "string" || message.trim() === "") {
          errors.push(`${lang}/${ns}.json ${key}: empty translation`);
          continue;
        }
        if (ns === "shell" && /[{}]/.test(message)) errors.push(`${lang}/shell.json ${key}: shell text must be plain (no ICU)`);
        for (const problem of compareMessages(lang, source[key], message)) {
          errors.push(`${lang}/${ns}.json ${key}: ${problem}`);
        }
      }
      for (const [key, english] of Object.entries(source)) {
        const state = stateOf(english, translated[key], entries[key]);
        if (state === "missing") missing += 1;
        if (state === "stale") stale += 1;
      }
      for (const key of Object.keys(entries)) {
        if (!(key in source)) errors.push(`_meta/${lang}/${ns}.json names unknown key ${key}`);
      }
    }
    if (missing) (ship ? errors : warnings).push(`${lang}: ${missing} message(s) missing`);
    if (stale) warnings.push(`${lang}: ${stale} message(s) stale (English changed since the translation)`);
    if (entry && entry.status !== derivedStatus(languageStates(lang))) {
      warnings.push(`${lang}: languages.json says "${entry.status}", the messages say "${derivedStatus(languageStates(lang))}" (run sync.mjs)`);
    }
    warnings.push(...glossaryMisses(lang));
  }

  // Metadata for languages without a folder
  for (const lang of subfolders(join(LOCALES, "_meta"))) {
    if (!folders.includes(lang)) errors.push(`_meta/${lang}/ has no catalog folder`);
  }

  warnings.push(...unusedMessages());

  return { errors, warnings };
}

/// English messages no UI source names (as a quoted key), so nobody is asked
/// to translate dead text. Keys are always written out in full (`t("a.b")`,
/// or a key table), never assembled, which is what makes this check work.
/// Shell text is looked up from Rust (`shell_text.rs` has its own test).
function unusedMessages() {
  // The UI sources next to the catalogs; with throwaway catalogs
  // (I18N_LOCALES) only an explicit I18N_SRC is scanned.
  const src = process.env.I18N_SRC ?? (process.env.I18N_LOCALES ? null : join(LOCALES, ".."));
  if (!src || !existsSync(src)) return [];
  const text = sourceFiles(src)
    .map((file) => readFileSync(file, "utf8"))
    .join("\n");
  const out = [];
  for (const ns of NAMESPACES.filter((n) => n !== "shell")) {
    for (const key of Object.keys(catalog(SOURCE, ns))) {
      if (!text.includes(`"${key}"`)) out.push(`en/${ns}.json ${key}: not used by any source file`);
    }
  }
  return out;
}

function sourceFiles(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "locales" && entry.name !== "node_modules") files.push(...sourceFiles(path));
    } else if (/\.(ts|svelte)$/.test(entry.name) && !/\.test\.ts$/.test(entry.name)) {
      files.push(path);
    }
  }
  return files;
}

function subfolders(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name);
}

/// A translation whose English uses a glossary term should use that term's
/// rendering for the language (warning only). Renderings may carry a note in
/// parentheses and alternatives separated by " / " or ";"; any alternative
/// counts. Words match by stem (the last two letters may change), so declined
/// and inflected forms ("playlista" → "playlisty") are not misses.
function glossaryMisses(lang) {
  const glossary = readJson(paths.glossary(), { terms: [] });
  const misses = [];
  for (const term of glossary.terms ?? []) {
    const rendering = term[lang];
    if (!rendering) continue;
    const alternatives = renderingAlternatives(rendering, lang);
    if (alternatives.length === 0) continue;
    const pattern = new RegExp(`\\b${term.en.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`, "i");
    for (const ns of NAMESPACES) {
      const source = catalog(SOURCE, ns);
      const translated = catalog(lang, ns);
      for (const [key, english] of Object.entries(source)) {
        const message = translated[key];
        // Placeholder names ({artist}) and ICU keywords are not prose.
        const prose = english.replace(/\{\s*\w+\s*(?:,\s*\w+\s*)?[,}]/g, " ");
        if (!message || !pattern.test(prose)) continue;
        const text = message.toLocaleLowerCase(lang);
        if (!alternatives.some((stems) => stems.every((stem) => text.includes(stem)))) {
          misses.push(`${lang}/${ns}.json ${key}: glossary "${term.en}" → "${rendering}" not used`);
        }
      }
    }
  }
  return misses;
}

/// The stems of each alternative in a glossary rendering.
export function renderingAlternatives(rendering, lang) {
  return rendering
    .replace(/\([^)]*\)/g, " ")
    .split(/\s+\/\s+|;/)
    .map((alt) =>
      alt
        .trim()
        .toLocaleLowerCase(lang)
        .split(/\s+/)
        .filter(Boolean)
        .map((word) => (word.length > 4 ? word.slice(0, -2) : word)),
    )
    .filter((stems) => stems.length > 0);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { options } = parseArgs(process.argv.slice(2));
  const { errors, warnings } = check();
  if (!options.quiet) for (const w of warnings) console.warn(`warning: ${w}`);
  for (const e of errors) console.error(`error: ${e}`);
  console.log(`i18n check: ${errors.length} error(s), ${warnings.length} warning(s)`);
  process.exit(errors.length ? 1 : 0);
}
