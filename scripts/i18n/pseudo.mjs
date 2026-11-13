// Generates the development-only pseudo-locale `en-XA` (spec §7.6): every
// English message accented, about 35% longer and wrapped in ⟦ ⟧. On screen,
// text without brackets was never extracted; clipped text is an overflow bug.
// The output (ui/src/locales/en-XA/) is gitignored; dev builds offer it.
//
//   node scripts/i18n/pseudo.mjs

import { pathToFileURL } from "node:url";
import { NAMESPACES, PSEUDO, SOURCE, TYPE, catalog, parse, paths, printAST, writeJson } from "./lib.mjs";

const ACCENTS = {
  a: "á", b: "ƀ", c: "ç", d: "ð", e: "é", f: "ƒ", g: "ĝ", h: "ĥ", i: "î", j: "ĵ", k: "ķ", l: "ļ", m: "ɱ",
  n: "ñ", o: "ö", p: "þ", q: "ǫ", r: "ŕ", s: "š", t: "ţ", u: "û", v: "ṽ", w: "ŵ", x: "ẋ", y: "ý", z: "ž",
  A: "Á", B: "Ɓ", C: "Ç", D: "Ð", E: "É", F: "Ƒ", G: "Ĝ", H: "Ĥ", I: "Î", J: "Ĵ", K: "Ķ", L: "Ļ", M: "Ṁ",
  N: "Ñ", O: "Ö", P: "Þ", Q: "Ǫ", R: "Ŕ", S: "Š", T: "Ţ", U: "Û", V: "Ṽ", W: "Ŵ", X: "Ẋ", Y: "Ý", Z: "Ž",
};

export function accent(text) {
  return [...text].map((ch) => ACCENTS[ch] ?? ch).join("");
}

function padding(length) {
  return "~".repeat(Math.max(1, Math.round(length * 0.35)));
}

function transform(nodes) {
  for (const node of nodes) {
    if (node.type === TYPE.literal) node.value = accent(node.value);
    if (node.type === TYPE.select || node.type === TYPE.plural) {
      for (const option of Object.values(node.options)) transform(option.value);
    }
    if (node.type === TYPE.tag) transform(node.children);
  }
  return nodes;
}

export function pseudoMessage(message, plain = false) {
  if (plain) return `⟦${accent(message)} ${padding(message.length)}⟧`;
  const ast = transform(parse(message));
  return `⟦${printAST(ast)} ${padding(message.length)}⟧`;
}

export function generate() {
  const counts = {};
  for (const ns of NAMESPACES) {
    const source = catalog(SOURCE, ns);
    const out = {};
    for (const [key, message] of Object.entries(source)) out[key] = pseudoMessage(message, ns === "shell");
    writeJson(paths.catalog(PSEUDO, ns), out);
    counts[ns] = Object.keys(out).length;
  }
  return counts;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const counts = generate();
  console.log(`${PSEUDO}: ${Object.entries(counts).map(([ns, n]) => `${n} ${ns}`).join(", ")} messages`);
}
