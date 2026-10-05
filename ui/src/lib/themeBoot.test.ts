import { describe, expect, it } from "vitest";
// @ts-expect-error — the svelte tsconfig has no node types; vitest runs in node.
import { readFileSync } from "node:fs";

describe("theme boot script", () => {
  it("applies the stored theme before the module script", () => {
    const html = readFileSync(new URL("../../index.html", import.meta.url), "utf8");
    const boot = html.indexOf("oxitide.theme");
    const module = html.indexOf('type="module"');
    expect(boot).toBeGreaterThan(-1);
    expect(module).toBeGreaterThan(-1);
    expect(boot).toBeLessThan(module);
    expect(html).toContain("documentElement.dataset.theme");
    expect(html).toContain("prefers-color-scheme: dark");
  });
});
