// Renders packaging/aur/PKGBUILD.in for a release tag, so the AUR recipe builds
// exactly the tagged source and never carries a hand-kept version or commit.
//   node scripts/release/make-pkgbuild.mjs <tag> <outDir>
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const PLACEHOLDERS = ["@PKGVER@", "@TAG@"];

/// Arch pkgver for a vX.Y.Z[-pre] tag. pkgver may not contain "-", and the
/// prerelease must sort before the release: vercmp has 1.0.0rc.1 < 1.0.0, but
/// 1.0.0_rc.1 > 1.0.0, so hyphens are dropped rather than replaced. Accepts the
/// same tags as check-version.mjs.
export function archPkgver(tag) {
  const match = /^v(\d+\.\d+\.\d+)(?:-([0-9A-Za-z.-]+))?$/.exec(tag);
  if (!match) throw new Error(`tag "${tag}" is not of the form vX.Y.Z[-pre]`);
  return match[1] + (match[2] ?? "").replaceAll("-", "");
}

export function renderPkgbuild(template, tag) {
  for (const placeholder of PLACEHOLDERS) {
    if (template.split(placeholder).length !== 2) {
      throw new Error(`template must contain ${placeholder} exactly once`);
    }
  }
  // LF always: a Windows checkout has CRLF, and bash rejects a CRLF PKGBUILD.
  return template
    .replace(/\r\n/g, "\n")
    .replace("@PKGVER@", archPkgver(tag))
    .replace("@TAG@", tag);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [tag, outDir] = process.argv.slice(2);
  if (!tag || !outDir) {
    console.error("usage: make-pkgbuild.mjs <tag> <outDir>");
    process.exit(2);
  }
  const template = readFileSync(join(root, "packaging/aur/PKGBUILD.in"), "utf8");
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, "PKGBUILD"), renderPkgbuild(template, tag));
  console.log(`wrote PKGBUILD (pkgver ${archPkgver(tag)}, source tag ${tag})`);
}
