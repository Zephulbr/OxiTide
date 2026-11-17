// Writes machine-drafted translations into a catalog (spec §7.3).
//
//   node scripts/i18n/apply-draft.mjs <lang> <draft.json> [--ns app] [--status machine|cross-checked]
//
// <draft.json> maps keys to translated messages. A draft never overwrites a
// message a person edited or reviewed; it fills missing and stale messages
// and may replace earlier machine drafts (a cross-check pass). Each message is
// validated like check.mjs does; invalid ones are reported and skipped.

import { pathToFileURL } from "node:url";
import {
  NAMESPACES,
  SOURCE,
  catalog,
  compareMessages,
  hash,
  languages,
  meta,
  ordered,
  parseArgs,
  paths,
  readJson,
  refreshLanguageStatus,
  stateOf,
  writeJson,
} from "./lib.mjs";

const WRITABLE = new Set(["missing", "stale", "machine", "cross-checked"]);

export function applyDraft(lang, draft, { ns = "app", status = "machine" } = {}) {
  if (!NAMESPACES.includes(ns)) throw new Error(`unknown namespace ${ns}`);
  if (lang === SOURCE) throw new Error("English is the source; edit en/*.json by hand");
  if (!languages().some((l) => l.code === lang)) throw new Error(`${lang} is not in languages.json`);
  if (status !== "machine" && status !== "cross-checked") throw new Error(`status must be machine or cross-checked`);

  const source = catalog(SOURCE, ns);
  const translated = { ...catalog(lang, ns) };
  const entries = { ...meta(lang, ns) };
  const report = { written: [], kept: [], invalid: [], unknown: [] };

  for (const [key, message] of Object.entries(draft)) {
    if (!(key in source)) {
      report.unknown.push(key);
      continue;
    }
    const state = stateOf(source[key], translated[key], entries[key]);
    if (!WRITABLE.has(state)) {
      report.kept.push(`${key} (${state})`);
      continue;
    }
    const problems = typeof message === "string" && message.trim() !== ""
      ? compareMessages(lang, source[key], message)
      : ["empty"];
    if (ns === "shell" && /[{}]/.test(message)) problems.push("shell text must be plain");
    if (problems.length) {
      report.invalid.push(`${key}: ${problems.join("; ")}`);
      continue;
    }
    translated[key] = message;
    entries[key] = { status, source: hash(source[key]), draft: hash(message) };
    report.written.push(key);
  }

  writeJson(paths.catalog(lang, ns), ordered(translated, source));
  writeJson(paths.meta(lang, ns), ordered(entries, source));
  refreshLanguageStatus();
  return report;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { options, positional } = parseArgs(process.argv.slice(2));
  const [lang, file] = positional;
  if (!lang || !file) {
    console.error("usage: apply-draft.mjs <lang> <draft.json> [--ns app] [--status machine|cross-checked]");
    process.exit(2);
  }
  const report = applyDraft(lang, readJson(file, {}), { ns: options.ns, status: options.status });
  console.log(`${lang}/${options.ns ?? "app"}: ${report.written.length} written, ${report.kept.length} kept (edited/reviewed), ${report.invalid.length} invalid, ${report.unknown.length} unknown`);
  for (const line of report.invalid) console.error(`invalid: ${line}`);
  for (const key of report.unknown) console.error(`unknown key: ${key}`);
  process.exit(report.invalid.length || report.unknown.length ? 1 : 0);
}
