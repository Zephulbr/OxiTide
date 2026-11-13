// A side-by-side sheet for reading one language end to end (spec §7.5):
// key, English, translator note, translation and state, grouped by area,
// stale and machine-made first. Written to a temp file; open it in a browser.
//
//   node scripts/i18n/review-sheet.mjs <lang> [--out sheet.html]

import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { NAMESPACES, SOURCE, catalog, languages, meta, parseArgs, paths, readJson, stateOf } from "./lib.mjs";

const ORDER = { stale: 0, missing: 1, machine: 2, "cross-checked": 3, edited: 4, reviewed: 5 };

const escape = (text) =>
  String(text ?? "").replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[ch]);

export function rows(lang) {
  const out = [];
  for (const ns of NAMESPACES) {
    const source = catalog(SOURCE, ns);
    const notes = readJson(paths.notes(ns), {});
    const translated = catalog(lang, ns);
    const entries = meta(lang, ns);
    for (const [key, english] of Object.entries(source)) {
      out.push({
        ns,
        key,
        area: `${ns}: ${key.split(".")[0]}`,
        english,
        note: notes[key] ?? "",
        translation: translated[key],
        state: stateOf(english, translated[key], entries[key]),
      });
    }
  }
  return out.sort((a, b) => a.area.localeCompare(b.area) || ORDER[a.state] - ORDER[b.state]);
}

export function sheet(lang) {
  const name = languages().find((l) => l.code === lang)?.endonym ?? lang;
  const list = rows(lang);
  const areas = [...new Set(list.map((r) => r.area))];
  const body = areas
    .map((area) => {
      const lines = list
        .filter((r) => r.area === area)
        .map(
          (r) => `<tr class="${r.state}"><td><code>${escape(r.key)}</code></td><td>${escape(r.english)}<div class="note">${escape(r.note)}</div></td><td lang="${escape(lang)}">${escape(r.translation ?? "—")}</td><td>${r.state}</td></tr>`,
        )
        .join("\n");
      return `<h2>${escape(area)}</h2>\n<table><thead><tr><th>Key</th><th>English</th><th>${escape(name)}</th><th>State</th></tr></thead><tbody>\n${lines}\n</tbody></table>`;
    })
    .join("\n");
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>OxiTide ${escape(name)} review</title>
<style>
body{font:14px/1.45 system-ui,sans-serif;margin:24px;color:#1b1b1b;background:#fff}
table{border-collapse:collapse;width:100%;margin-bottom:24px}
th,td{border:1px solid #ddd;padding:6px 8px;vertical-align:top;text-align:left}
td:first-child{width:18%;word-break:break-all}td:last-child{width:8%}
.note{color:#666;font-size:12px;margin-top:4px}
tr.stale td:last-child,tr.missing td:last-child{background:#fde2e1}
tr.machine td:last-child{background:#fff4d6}
tr.reviewed td:last-child,tr.edited td:last-child{background:#e3f4e1}
@media (prefers-color-scheme:dark){body{background:#151515;color:#eee}th,td{border-color:#333}.note{color:#aaa}
tr.stale td:last-child,tr.missing td:last-child{background:#4a2120}tr.machine td:last-child{background:#463a17}
tr.reviewed td:last-child,tr.edited td:last-child{background:#1f3a1e}}
</style></head><body>
<h1>${escape(name)} (${escape(lang)}): ${list.length} messages</h1>
<p>Fix a translation in <code>ui/src/locales/${escape(lang)}/&lt;ns&gt;.json</code>, then run <code>node scripts/i18n/sync.mjs</code>.
After reading everything, <code>node scripts/i18n/sync.mjs --review ${escape(lang)}</code> marks the rest reviewed.</p>
${body}
</body></html>
`;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { options, positional } = parseArgs(process.argv.slice(2));
  const [lang] = positional;
  if (!lang) {
    console.error("usage: review-sheet.mjs <lang> [--out sheet.html]");
    process.exit(2);
  }
  const out = typeof options.out === "string" ? options.out : join(tmpdir(), `oxitide-review-${lang}.html`);
  writeFileSync(out, sheet(lang), "utf8");
  console.log(out);
}
