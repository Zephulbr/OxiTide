// Records human work in the translation metadata (spec §7.4). Run by the
// maintainer after merging translation edits; never by CI.
//
//   node scripts/i18n/sync.mjs                          stamp edited messages as reviewed
//   node scripts/i18n/sync.mjs --review el [--keys a,b] also mark untouched drafts reviewed
//
// A message is "edited" when its text differs from what the drafting process
// wrote (or it has no metadata, e.g. a volunteer's new language). Stamping
// records it as reviewed against the current English.

import { pathToFileURL } from "node:url";
import {
  NAMESPACES,
  SOURCE,
  catalog,
  catalogFolders,
  hash,
  meta,
  ordered,
  parseArgs,
  paths,
  refreshLanguageStatus,
  stateOf,
  writeJson,
} from "./lib.mjs";

export function sync({ review, keys } = {}) {
  const only = keys ? new Set(keys) : null;
  const stamped = {};
  for (const lang of catalogFolders().filter((f) => f !== SOURCE)) {
    for (const ns of NAMESPACES) {
      const source = catalog(SOURCE, ns);
      const translated = catalog(lang, ns);
      const entries = { ...meta(lang, ns) };
      let changed = false;
      for (const [key, english] of Object.entries(source)) {
        const message = translated[key];
        const state = stateOf(english, message, entries[key]);
        const reviewing =
          review === lang && (state === "machine" || state === "cross-checked") && (!only || only.has(key));
        if (state === "edited" || reviewing) {
          entries[key] = { status: "reviewed", source: hash(english), draft: hash(message) };
          stamped[lang] = (stamped[lang] ?? 0) + 1;
          changed = true;
        }
      }
      if (changed) writeJson(paths.meta(lang, ns), ordered(entries, source));
    }
  }
  refreshLanguageStatus();
  return stamped;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { options } = parseArgs(process.argv.slice(2));
  const stamped = sync({
    review: typeof options.review === "string" ? options.review : undefined,
    keys: typeof options.keys === "string" ? options.keys.split(",").map((k) => k.trim()) : undefined,
  });
  const lines = Object.entries(stamped).map(([lang, n]) => `${lang}: ${n} marked reviewed`);
  console.log(lines.length ? lines.join("\n") : "nothing to stamp");
}
