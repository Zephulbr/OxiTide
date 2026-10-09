import { describe, expect, it } from "vitest";

/// Spec §5.2 / RM-D9: the phone page never loads Tauri or the desktop shell.
/// Walks the runtime import graph from the remote entry; `import type` is
/// erased and therefore allowed (that is how it shares `lib/api.ts` types).

/// Every source under `src/`, keyed `src/...` (Vite raw imports).
const SOURCES: Record<string, string> = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>("/src/**/*.{ts,svelte}", { query: "?raw", import: "default", eager: true }),
  ).map(([key, text]) => [key.slice(1), text]),
);

/// Pure shared modules the remote may run.
const SHARED = new Set([
  "src/lib/artwork.ts",
  "src/lib/format.ts",
  "src/lib/lyrics.ts",
  "src/lib/media.ts",
  "src/lib/components/Icon.svelte",
]);
const BARE = new Set(["svelte", "svelte/reactivity"]);

const IMPORT = /^\s*import\s+(type\s+)?(?:[\w*{}\s,$]+?\s+from\s+)?["']([^"']+)["']/gm;

function runtimeImports(text: string): string[] {
  return [...text.matchAll(IMPORT)].filter((m) => !m[1]).map((m) => m[2]);
}

/// POSIX-style join of a module's directory and a relative specifier.
function join(from: string, spec: string): string {
  const parts = from.split("/").slice(0, -1);
  for (const segment of spec.split("/")) {
    if (segment === "..") parts.pop();
    else if (segment !== ".") parts.push(segment);
  }
  return parts.join("/");
}

function resolveImport(from: string, spec: string): string {
  const base = join(from, spec);
  for (const candidate of [base, `${base}.ts`, `${base}.svelte.ts`, `${base}/index.ts`]) {
    if (candidate in SOURCES) return candidate;
  }
  throw new Error(`cannot resolve ${spec} from ${from}`);
}

function walk(): { files: string[]; violations: string[] } {
  const seen = new Set<string>();
  const violations: string[] = [];
  const queue = ["src/remote/main.ts"];
  while (queue.length > 0) {
    const file = queue.pop()!;
    if (seen.has(file)) continue;
    seen.add(file);
    for (const spec of runtimeImports(SOURCES[file])) {
      if (spec.startsWith("@tauri-apps")) {
        violations.push(`${file} imports ${spec}`);
        continue;
      }
      if (!spec.startsWith(".")) {
        if (!BARE.has(spec)) violations.push(`${file} imports package ${spec}`);
        continue;
      }
      if (spec.endsWith(".css")) continue;
      const target = resolveImport(file, spec);
      if (!target.startsWith("src/remote/") && !SHARED.has(target)) {
        violations.push(`${file} imports ${target} at runtime`);
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
    expect(runtimeImports(sample)).toEqual(["../lib/api"]);
    expect(resolveImport("src/remote/pages/Album.svelte", "../../lib/api")).toBe("src/lib/api.ts");
  });
});
