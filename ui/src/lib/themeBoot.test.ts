import { describe, expect, it } from "vitest";
// @ts-expect-error — the svelte tsconfig has no node types; vitest runs in node.
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../../index.html", import.meta.url), "utf8");

interface WindowStub {
  matchMedia?: (query: string) => { matches: boolean };
}

interface BootResult {
  theme: string | undefined;
  colorScheme: string | undefined;
}

function runBootScript(options: {
  stored?: string | null;
  throws?: boolean;
  prefersDark?: boolean;
  withMatchMedia?: boolean;
} = {}): BootResult {
  const body = html.match(/<script>([\s\S]*?)<\/script>/);
  expect(body, "index.html should have a plain inline boot script").not.toBeNull();
  const dataset: Record<string, string> = {};
  const style: Record<string, string> = {};
  const localStorage = {
    getItem: () => {
      if (options.throws) throw new Error("storage unavailable");
      return options.stored ?? null;
    },
  };
  const window: WindowStub = {};
  if (options.withMatchMedia !== false) {
    window.matchMedia = () => ({ matches: options.prefersDark ?? false });
  }
  new Function("window", "localStorage", "document", body![1])(
    window,
    localStorage,
    { documentElement: { dataset, style } },
  );
  return { theme: dataset.theme, colorScheme: style.colorScheme };
}

describe("theme boot script", () => {
  it("runs before the module script", () => {
    const boot = html.indexOf("oxitide.theme");
    const module = html.indexOf('type="module"');
    expect(boot).toBeGreaterThan(-1);
    expect(module).toBeGreaterThan(-1);
    expect(boot).toBeLessThan(module);
  });

  it("applies an explicit dark mirror", () => {
    expect(runBootScript({ stored: "dark" })).toEqual({
      theme: "dark",
      colorScheme: "dark",
    });
  });

  it("applies an explicit light mirror", () => {
    expect(runBootScript({ stored: "light" })).toEqual({
      theme: "light",
      colorScheme: "light",
    });
  });

  it("resolves system against a dark OS preference", () => {
    expect(runBootScript({ stored: "system", prefersDark: true }).theme).toBe("dark");
  });

  it("resolves system against a light OS preference", () => {
    expect(runBootScript({ stored: "system", prefersDark: false }).theme).toBe("light");
  });

  it("defaults an absent mirror to dark", () => {
    expect(runBootScript({ stored: null }).theme).toBe("dark");
  });

  it("defaults a corrupt mirror to dark", () => {
    expect(runBootScript({ stored: "junk", prefersDark: false }).theme).toBe("dark");
  });

  it("defaults an unreadable store to dark", () => {
    expect(runBootScript({ throws: true, prefersDark: false }).theme).toBe("dark");
  });

  it("defaults system without matchMedia to dark", () => {
    expect(runBootScript({ stored: "system", withMatchMedia: false }).theme).toBe("dark");
  });
});
