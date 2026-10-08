import { describe, expect, it } from "vitest";
// @ts-expect-error — the svelte tsconfig has no node types; vitest runs in node.
import { readFileSync } from "node:fs";

const tokens = readFileSync(new URL("../styles/tokens.css", import.meta.url), "utf8");
const themes = readFileSync(new URL("../styles/themes.css", import.meta.url), "utf8");

function hexFor(css: string, name: string): string {
  const match = css.match(new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`));
  if (!match) throw new Error(`missing hex token --${name}`);
  return match[1];
}

function luminance(hex: string): number {
  const srgb = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const linear = srgb.map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const light = themes.slice(themes.indexOf('[data-theme="light"]'));

describe("light token layer", () => {
  /// Review Focus 4: every semantic color token in :root must have a light
  /// counterpart, including the --dyn-* fallbacks that apply when dynamic
  /// color is off or a fetch failed.
  it("overrides every semantic color token", () => {
    for (const name of [
      "bg", "text", "text-muted", "accent-brand", "on-accent-brand",
      "danger", "success", "warning",
      "surface-1", "surface-2", "surface-3", "surface-float", "line",
      "edge-lit", "glare-alpha",
      "dyn-primary", "dyn-on-primary", "dyn-secondary", "dyn-tertiary", "dyn-backdrop",
    ]) {
      expect(light, name).toMatch(new RegExp(`--${name}:`));
    }
  });

  it("keeps text and semantic colors at 4.5:1 on the light room", () => {
    const bg = hexFor(light, "bg");
    for (const name of ["text", "text-muted", "danger", "success", "warning", "dyn-primary"]) {
      const ratio = contrast(hexFor(light, name), bg);
      expect(ratio, `${name} on light bg`).toBeGreaterThanOrEqual(4.5);
    }
    expect(contrast(hexFor(light, "dyn-on-primary"), hexFor(light, "dyn-primary"))).toBeGreaterThanOrEqual(4.5);
  });

  it("defines the theme-paired room and scrim tokens in the dark layer", () => {
    for (const name of ["room-top", "scrim-top", "scrim-bottom", "grain-alpha"]) {
      expect(tokens, name).toMatch(new RegExp(`--${name}:`));
    }
  });
});

describe("static contrast (dark)", () => {
  it("keeps text and semantic colors at 4.5:1 on the dark room", () => {
    const bg = hexFor(tokens, "bg");
    for (const name of ["text", "text-muted", "danger", "success", "warning"]) {
      const ratio = contrast(hexFor(tokens, name), bg);
      expect(ratio, `${name} on dark bg`).toBeGreaterThanOrEqual(4.5);
    }
    expect(
      contrast(hexFor(tokens, "on-accent-brand"), hexFor(tokens, "accent-brand")),
    ).toBeGreaterThanOrEqual(4.5);
  });
});

describe("static contrast (light brand pair)", () => {
  it("keeps the light on-accent pair readable", () => {
    expect(
      contrast(hexFor(light, "on-accent-brand"), hexFor(light, "accent-brand")),
    ).toBeGreaterThanOrEqual(4.5);
  });
});
