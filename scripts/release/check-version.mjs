// Verifies a release tag matches the versions baked into the app.
//   node scripts/release/check-version.mjs v0.2.0
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

export function workspaceVersion(cargoToml) {
  const section = cargoToml.split(/^\[workspace\.package\]\s*$/m)[1];
  if (section === undefined) throw new Error("no [workspace.package] in Cargo.toml");
  const body = section.split(/^\[/m)[0];
  const match = body.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error("no version in [workspace.package]");
  return match[1];
}

/// Returns the list of mismatch messages; empty means the tag is consistent.
export function versionProblems(tag, tauriVersion, cargoVersion) {
  const problems = [];
  const match = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/.exec(tag);
  if (!match) return [`tag "${tag}" is not of the form vX.Y.Z`];
  const version = match[1];
  if (tauriVersion !== version) {
    problems.push(`tauri.conf.json version is ${tauriVersion}, tag is ${version}`);
  }
  if (cargoVersion !== version) {
    problems.push(`workspace Cargo.toml version is ${cargoVersion}, tag is ${version}`);
  }
  return problems;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tag = process.argv[2] ?? "";
  const tauri = JSON.parse(readFileSync(join(root, "crates/oxitide-app/tauri.conf.json"), "utf8"));
  const cargo = workspaceVersion(readFileSync(join(root, "Cargo.toml"), "utf8"));
  const problems = versionProblems(tag, tauri.version, cargo);
  if (problems.length > 0) {
    for (const problem of problems) console.error(`error: ${problem}`);
    process.exit(1);
  }
  console.log(`ok: ${tag} matches tauri.conf.json and Cargo.toml`);
}
