import { existsSync, readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/// Spec §5.2 / RM-D9: the phone page never loads Tauri or the desktop shell.
/// Walks the runtime import graph from the remote entry; `import type` is
/// erased and therefore allowed (that is how it shares `lib/api.ts` types).

const SRC = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ENTRY = resolve(SRC, "remote/main.ts");

/// Pure shared modules the remote may run.
const SHARED = new Set([
  "lib/artwork.ts",
  "lib/format.ts",
  "lib/lyrics.ts",
  "lib/media.ts",
  "lib/components/Icon.svelte",
]);
const BARE = new Set(["svelte", "svelte/reactivity"]);

const IMPORT = /^\s*import\s+(type\s+)?(?:[\w*{}\s,$]+?\s+from\s+)?["']([^"']+)["']/gm;

function runtimeImports(file: string): string[] {
  const text = readFileSync(file, "utf8");
  const out: string[] = [];
  for (const match of text.matchAll(IMPORT)) {
    if (!match[1]) out.push(match[2]);
  }
  return out;
}

function resolveImport(from: string, spec: string): string {
  const base = resolve(dirname(from), spec);
  for (const candidate of [base, `${base}.ts`, `${base}.svelte.ts`, `${base}/index.ts`]) {
    if (existsSync(candidate)) return candidate;
  }
  throw new Error(`cannot resolve ${spec} from ${from}`);
}

function walk(): { files: string[]; violations: string[] } {
  const seen = new Set<string>();
  const violations: string[] = [];
  const queue = [ENTRY];
  while (queue.length > 0) {
    const file = queue.pop()!;
    if (seen.has(file)) continue;
    seen.add(file);
    for (const spec of runtimeImports(file)) {
      const where = relative(SRC, file).replace(/\\/g, "/");
      if (spec.startsWith("@tauri-apps")) {
        violations.push(`${where} imports ${spec}`);
        continue;
      }
      if (!spec.startsWith(".")) {
        if (!BARE.has(spec)) violations.push(`${where} imports package ${spec}`);
        continue;
      }
      if (spec.endsWith(".css")) continue;
      const target = resolveImport(file, spec);
      const rel = relative(SRC, target).replace(/\\/g, "/");
      if (!rel.startsWith("remote/") && !SHARED.has(rel)) {
        violations.push(`${where} imports ${rel} at runtime`);
        continue;
      }
      queue.push(target);
    }
  }
  return { files: [...seen], violations };
}

describe("remote entry isolation", () => {
  it("never reaches Tauri or a desktop-shell module", () => {
    const { files, violations } = walk();
    expect(files.length).toBeGreaterThan(20);
    expect(violations).toEqual([]);
  });

  it("detects a forbidden import", () => {
    const sample = 'import { api } from "../lib/api";\nimport type { Track } from "../lib/api";';
    const found = [...sample.matchAll(IMPORT)].map((m) => [Boolean(m[1]), m[2]]);
    expect(found).toEqual([
      [false, "../lib/api"],
      [true, "../lib/api"],
    ]);
  });
});
