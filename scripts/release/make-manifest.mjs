// Builds the updater manifest (latest.json) and SHA256SUMS.txt from the bundles
// the release workflow collected.
//   node scripts/release/make-manifest.mjs <artifactsDir> <outDir> <tag> <owner/repo> [notes]
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

/// Bundle file name -> updater platform keys it serves. NSIS is the default
/// Windows entry (per-user install, no admin).
const BUNDLES = [
  { pattern: /_x64-setup\.exe$/, keys: ["windows-x86_64-nsis", "windows-x86_64"] },
  { pattern: /_x64_en-US\.msi$/, keys: ["windows-x86_64-msi"] },
  { pattern: /_amd64\.AppImage$/, keys: ["linux-x86_64-appimage", "linux-x86_64"] },
];

/// Files whose checksums are published (installers, not signatures).
const CHECKSUMMED = [/\.exe$/, /\.msi$/, /\.deb$/, /\.AppImage$/];

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

export function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/// `files` are full paths; `readText(path)` returns a file's text (the .sig).
export function buildManifest({ files, tag, repo, notes, pubDate, readText }) {
  const version = tag.replace(/^v/, "");
  const byName = new Map(files.map((file) => [basename(file), file]));
  const platforms = {};
  for (const name of byName.keys()) {
    const bundle = BUNDLES.find((b) => b.pattern.test(name));
    if (!bundle) continue;
    const sigPath = byName.get(`${name}.sig`);
    if (!sigPath) throw new Error(`missing signature for ${name}`);
    const entry = {
      signature: readText(sigPath).trim(),
      url: `https://github.com/${repo}/releases/download/${tag}/${name}`,
    };
    for (const key of bundle.keys) platforms[key] = entry;
  }
  if (Object.keys(platforms).length === 0) throw new Error("no updater bundles found");
  return { version, notes, pub_date: pubDate, platforms };
}

export function checksumLines(files, hash = sha256) {
  return files
    .filter((file) => CHECKSUMMED.some((re) => re.test(file)))
    .sort((a, b) => (basename(a) < basename(b) ? -1 : 1))
    .map((file) => `${hash(file)}  ${basename(file)}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [artifactsDir, outDir, tag, repo, notes = ""] = process.argv.slice(2);
  if (!artifactsDir || !outDir || !tag || !repo) {
    console.error("usage: make-manifest.mjs <artifactsDir> <outDir> <tag> <owner/repo> [notes]");
    process.exit(2);
  }
  const files = walk(artifactsDir);
  const manifest = buildManifest({
    files,
    tag,
    repo,
    notes: notes || `OxiTide ${tag.replace(/^v/, "")}`,
    pubDate: new Date().toISOString(),
    readText: (path) => readFileSync(path, "utf8"),
  });
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  writeFileSync(join(outDir, "SHA256SUMS.txt"), `${checksumLines(files).join("\n")}\n`);
  console.log(`wrote latest.json (${Object.keys(manifest.platforms).join(", ")}) and SHA256SUMS.txt`);
}
