// Shared helpers for the translation scripts (spec
// `docs/superpowers/specs/2026-11-12-oxitide-v1.1-i18n-a11y-design.md` §6–7).
// No network: everything here reads and writes files under ui/src/locales.

import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const REPO = resolve(here, "../..");
export const UI = join(REPO, "ui");

/// Overridable for tests: `I18N_LOCALES=/tmp/x node check.mjs`.
export const LOCALES = process.env.I18N_LOCALES ?? join(UI, "src/locales");

export const NAMESPACES = ["app", "remote", "shell"];
export const SOURCE = "en";
export const PSEUDO = "en-XA";
/// Folders under locales/ that are not languages.
export const NOT_LANGUAGES = new Set(["notes", "_meta", PSEUDO]);

/// Status ranks: a language's tag is the lowest over its messages.
export const RANK = { machine: 0, "cross-checked": 1, reviewed: 2 };

// The ICU parser ships with the UI's `intl-messageformat` dependency.
const uiRequire = createRequire(join(UI, "package.json"));
const parserUrl = (spec) => pathToFileURL(uiRequire.resolve(spec)).href;
const { parse: icuParse, TYPE } = await import(parserUrl("@formatjs/icu-messageformat-parser"));
const { printAST } = await import(parserUrl("@formatjs/icu-messageformat-parser/printer.js"));
export { TYPE, printAST };

/// Parses like the runtime does (`ignoreTag`: `<Enter>` is text).
export function parse(message) {
  return icuParse(message, { ignoreTag: true });
}

export function hash(text) {
  return createHash("sha256").update(text, "utf8").digest("hex").slice(0, 8);
}

export function readJson(path, fallback) {
  if (!existsSync(path)) return fallback;
  return JSON.parse(readFileSync(path, "utf8"));
}

export function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

export const paths = {
  languages: () => join(LOCALES, "languages.json"),
  glossary: () => join(LOCALES, "glossary.json"),
  catalog: (lang, ns) => join(LOCALES, lang, `${ns}.json`),
  notes: (ns) => join(LOCALES, "notes", `${ns}.json`),
  meta: (lang, ns) => join(LOCALES, "_meta", lang, `${ns}.json`),
};

export function languages() {
  return readJson(paths.languages(), []);
}

/// Language folders that exist on disk (not notes/_meta/pseudo).
export function catalogFolders() {
  if (!existsSync(LOCALES)) return [];
  return readdirSync(LOCALES).filter(
    (name) => !NOT_LANGUAGES.has(name) && statSync(join(LOCALES, name)).isDirectory(),
  );
}

export function catalog(lang, ns) {
  return readJson(paths.catalog(lang, ns), {});
}

export function meta(lang, ns) {
  return readJson(paths.meta(lang, ns), {});
}

/// Rewrites `values` in the English catalog's key order (clean diffs).
export function ordered(values, source) {
  const out = {};
  for (const key of Object.keys(source)) if (key in values) out[key] = values[key];
  for (const key of Object.keys(values)) if (!(key in out)) out[key] = values[key];
  return out;
}

/// What a message's arguments look like, for comparing a translation with
/// its English source: argument name → kind, plus each select's keys and
/// each plural's option keys.
export function signature(ast) {
  const args = new Map();
  const selects = new Map();
  const plurals = [];
  const walk = (nodes) => {
    for (const node of nodes) {
      switch (node.type) {
        case TYPE.argument:
          setKind(args, node.value, "argument");
          break;
        case TYPE.number:
          setKind(args, node.value, "number");
          break;
        case TYPE.date:
          setKind(args, node.value, "date");
          break;
        case TYPE.time:
          setKind(args, node.value, "time");
          break;
        case TYPE.select:
          setKind(args, node.value, "select");
          selects.set(node.value, new Set(Object.keys(node.options).filter((k) => k !== "other")));
          for (const option of Object.values(node.options)) walk(option.value);
          break;
        case TYPE.plural:
          setKind(args, node.value, node.pluralType === "ordinal" ? "selectordinal" : "plural");
          plurals.push({ name: node.value, ordinal: node.pluralType === "ordinal", keys: Object.keys(node.options) });
          for (const option of Object.values(node.options)) walk(option.value);
          break;
        case TYPE.tag:
          walk(node.children);
          break;
        default:
          break;
      }
    }
  };
  walk(ast);
  return { args, selects, plurals };
}

function setKind(args, name, kind) {
  // `{n}` and `{n, number}` for the same name are compatible.
  const previous = args.get(name);
  if (!previous || previous === "argument") args.set(name, kind);
}

/// Problems with `translated` against `english` in `lang` (empty when fine).
export function compareMessages(lang, english, translated) {
  let source;
  let target;
  try {
    source = signature(parse(english));
  } catch (e) {
    return [`English message does not parse: ${e.message}`];
  }
  try {
    target = signature(parse(translated));
  } catch (e) {
    return [`does not parse: ${e.message}`];
  }
  const problems = [];
  const names = (map) => [...map.keys()].sort().join(", ");
  if (names(source.args) !== names(target.args)) {
    problems.push(`placeholders {${names(target.args)}} differ from English {${names(source.args)}}`);
  } else {
    for (const [name, kind] of source.args) {
      const other = target.args.get(name);
      if (other !== kind && !(kind === "argument" || other === "argument")) {
        problems.push(`{${name}} is ${other}, English has ${kind}`);
      }
    }
  }
  for (const [name, keys] of source.selects) {
    const theirs = target.selects.get(name);
    if (!theirs) continue;
    const a = [...keys].sort().join(" ");
    const b = [...theirs].sort().join(" ");
    if (a !== b) problems.push(`select {${name}} has keys [${b}], English has [${a}]`);
  }
  const locale = lang === PSEUDO ? SOURCE : lang;
  for (const plural of target.plurals) {
    if (!plural.keys.includes("other")) problems.push(`plural {${plural.name}} lacks "other"`);
    const needed = new Intl.PluralRules(locale, { type: plural.ordinal ? "ordinal" : "cardinal" })
      .resolvedOptions().pluralCategories;
    const missing = needed.filter((category) => !plural.keys.includes(category));
    if (missing.length) problems.push(`plural {${plural.name}} lacks ${missing.join(", ")} for ${lang}`);
  }
  return problems;
}

/// A message's state (spec §6.3): missing, stale, edited, or its status.
export function stateOf(english, translated, entry) {
  if (translated === undefined) return "missing";
  if (!entry) return "edited";
  if (entry.source !== hash(english)) return "stale";
  if (entry.draft !== hash(translated)) return "edited";
  return entry.status;
}

/// Counts of each state for one language over every namespace.
export function languageStates(lang) {
  const counts = { missing: 0, stale: 0, edited: 0, machine: 0, "cross-checked": 0, reviewed: 0 };
  for (const ns of NAMESPACES) {
    const source = catalog(SOURCE, ns);
    const translated = catalog(lang, ns);
    const entries = meta(lang, ns);
    for (const [key, english] of Object.entries(source)) {
      counts[stateOf(english, translated[key], entries[key])] += 1;
    }
  }
  return counts;
}

/// The picker tag a language has earned: its lowest status, where a person's
/// edit counts as reviewed and a stale message as machine.
export function derivedStatus(counts) {
  if (counts.machine || counts.stale || counts.missing) return "machine";
  if (counts["cross-checked"]) return "cross-checked";
  return "reviewed";
}

/// Writes every non-source language's derived status into languages.json.
export function refreshLanguageStatus() {
  const list = languages();
  const folders = new Set(catalogFolders());
  let changed = false;
  for (const entry of list) {
    if (entry.code === SOURCE || !folders.has(entry.code)) continue;
    const status = derivedStatus(languageStates(entry.code));
    if (entry.status !== status) {
      entry.status = status;
      changed = true;
    }
  }
  if (changed) writeLanguages(list);
  return changed;
}

/// languages.json keeps one entry per line, like the hand-written original.
export function writeLanguages(list) {
  const lines = list.map((entry) => `  ${JSON.stringify(entry).replace(/":/g, '": ').replace(/,"/g, ', "').replace(/^\{/, "{ ").replace(/\}$/, " }")}`);
  writeFileSync(paths.languages(), `[\n${lines.join(",\n")}\n]\n`, "utf8");
}

/// Splits `--name value` / `--flag` arguments from positional ones.
export function parseArgs(argv) {
  const options = {};
  const positional = [];
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg.startsWith("--")) {
      const name = arg.slice(2);
      const next = argv[i + 1];
      if (next !== undefined && !next.startsWith("--")) {
        options[name] = next;
        i += 1;
      } else {
        options[name] = true;
      }
    } else {
      positional.push(arg);
    }
  }
  return { options, positional };
}
