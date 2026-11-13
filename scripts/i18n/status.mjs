// Translation coverage per language (spec §7.2).
//
//   node scripts/i18n/status.mjs [--json]

import { pathToFileURL } from "node:url";
import { NAMESPACES, SOURCE, catalog, catalogFolders, derivedStatus, languageStates, languages, parseArgs } from "./lib.mjs";

export function status() {
  const total = NAMESPACES.reduce((sum, ns) => sum + Object.keys(catalog(SOURCE, ns)).length, 0);
  const folders = new Set(catalogFolders());
  return languages()
    .filter((l) => l.code !== SOURCE)
    .map((l) => {
      const counts = folders.has(l.code) ? languageStates(l.code) : { missing: total };
      return { code: l.code, ship: l.ship, total, status: derivedStatus(counts), ...counts };
    });
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { options } = parseArgs(process.argv.slice(2));
  const rows = status();
  if (options.json) {
    console.log(JSON.stringify(rows, null, 2));
  } else {
    const pct = (n, total) => `${Math.round(((n ?? 0) / total) * 100)}%`.padStart(5);
    console.log("lang      ship  reviewed  checked  machine  edited  stale  missing");
    for (const r of rows) {
      console.log(
        `${r.code.padEnd(9)} ${r.ship ? "yes " : "no  "} ${pct(r.reviewed, r.total)}     ${pct(r["cross-checked"], r.total)}    ${pct(r.machine, r.total)}   ${pct(r.edited, r.total)}  ${pct(r.stale, r.total)}  ${pct(r.missing, r.total)}`,
      );
    }
  }
}
