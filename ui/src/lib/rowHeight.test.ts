import { describe, it, expect } from "vitest";
// @ts-expect-error — the svelte tsconfig has no node types; vitest runs in node.
import { readFileSync } from "node:fs";
import { rowHeightFor } from "./rowHeight";

describe("rowHeightFor", () => {
  it("returns the comfortable/compact track-row heights", () => {
    expect(rowHeightFor("comfortable")).toBe(46);
    expect(rowHeightFor("compact")).toBe(38);
  });

  it("stays in sync with the --row-height density tokens", () => {
    const css = readFileSync(new URL("../styles/tokens.css", import.meta.url), "utf8");
    const base = css.match(/:root\s*\{[\s\S]*?--row-height:\s*(\d+)px/);
    const compact = css.match(/\[data-density="compact"\]\s*\{[\s\S]*?--row-height:\s*(\d+)px/);
    expect(Number(base?.[1])).toBe(rowHeightFor("comfortable"));
    expect(Number(compact?.[1])).toBe(rowHeightFor("compact"));
  });
});
