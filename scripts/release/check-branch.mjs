// Verifies a release tag sits on its release branch: a vN.* tag must point at a
// commit contained in origin/release/N.x, so a 1.x release can never be built
// from V2 work on main (both carry the same Cargo.toml version after a forward
// merge, so check-version.mjs alone does not catch a tag on the wrong branch).
// A major with no release branch yet (V2 before release/2.x is cut, or a
// v0.0.0-test pipeline tag) is not checked. Needs full history and tags
// (actions/checkout with fetch-depth: 0).
//   node scripts/release/check-branch.mjs v1.1.1
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/// The branch a tag must be on: "v1.2.3" -> "release/1.x"; null for a non-version tag.
export function releaseBranch(tag) {
  const match = /^v(\d+)\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.exec(tag);
  return match ? `release/${match[1]}.x` : null;
}

/// Returns a problem message, or null when the tag may be released.
/// `branchExists` / `onBranch` answer for origin/<releaseBranch(tag)>.
export function branchProblem(tag, branchExists, onBranch) {
  const branch = releaseBranch(tag);
  if (branch === null) return `tag "${tag}" is not of the form vX.Y.Z`;
  if (!branchExists || onBranch) return null;
  return `${tag} is not on ${branch}; ${branch.slice("release/".length)} releases are tagged there (docs/release.md, "Branches")`;
}

function git(...args) {
  return execFileSync("git", args, { stdio: ["ignore", "pipe", "pipe"] });
}

function succeeds(...args) {
  try {
    git(...args);
    return true;
  } catch {
    return false;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tag = process.argv[2] ?? "";
  const branch = releaseBranch(tag);
  const ref = branch && `refs/remotes/origin/${branch}`;
  const exists = branch !== null && succeeds("rev-parse", "--verify", "--quiet", ref);
  const onBranch =
    exists && succeeds("merge-base", "--is-ancestor", `refs/tags/${tag}^{commit}`, ref);
  const problem = branchProblem(tag, exists, onBranch);
  if (problem !== null) {
    console.error(`error: ${problem}`);
    process.exit(1);
  }
  console.log(exists ? `ok: ${tag} is on ${branch}` : `ok: no ${branch} yet; ${tag} not checked`);
}
