// Verifies a release tag matches the app version. The workspace Cargo.toml
// ([workspace.package]) is the only place the version lives: Tauri falls back to
// it when tauri.conf.json has no "version", and the Rust code reads it through
// CARGO_PKG_VERSION. A version reappearing in tauri.conf.json (which would
// override Cargo.toml) or ui/package.json is reported as an error.
//   node scripts/release/check-version.mjs v0.2.0
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

/// Files that must not carry their own version, relative to the repo root.
export const NO_VERSION_FILES = ["crates/oxitide-app/tauri.conf.json", "ui/package.json"];

export function workspaceVersion(cargoToml) {
  const section = cargoToml.split(/^\[workspace\.package\]\s*$/m)[1];
  if (section === undefined) throw new Error("no [workspace.package] in Cargo.toml");
  const body = section.split(/^\[/m)[0];
  const match = body.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error("no version in [workspace.package]");
  return match[1];
}

/// Returns the list of problem messages; empty means the tag is consistent.
/// `strayVersions` maps a NO_VERSION_FILES path to its "version" field (undefined when absent).
export function versionProblems(tag, cargoVersion, strayVersions = {}) {
  const problems = [];
  for (const [file, stray] of Object.entries(strayVersions)) {
    if (stray !== undefined) {
      problems.push(`${file} sets version ${stray}; the version lives only in the workspace Cargo.toml`);
    }
  }
  const match = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/.exec(tag);
  if (!match) return [`tag "${tag}" is not of the form vX.Y.Z`, ...problems];
  const version = match[1];
  if (cargoVersion !== version) {
    problems.push(`workspace Cargo.toml version is ${cargoVersion}, tag is ${version}`);
  }
  return problems;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tag = process.argv[2] ?? "";
  const cargo = workspaceVersion(readFileSync(join(root, "Cargo.toml"), "utf8"));
  const strays = Object.fromEntries(
    NO_VERSION_FILES.map((file) => [file, JSON.parse(readFileSync(join(root, file), "utf8")).version]),
  );
  const problems = versionProblems(tag, cargo, strays);
  if (problems.length > 0) {
    for (const problem of problems) console.error(`error: ${problem}`);
    process.exit(1);
  }
  console.log(`ok: ${tag} matches the workspace Cargo.toml`);
}
