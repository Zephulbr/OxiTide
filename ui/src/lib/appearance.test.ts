import { describe, expect, it } from "vitest";
import { clampQueueWidth, fontStack } from "./appearance.svelte";

describe("clampQueueWidth", () => {
  it("clamps to the 260–420 dock range", () => {
    expect(clampQueueWidth(100)).toBe(260);
    expect(clampQueueWidth(320)).toBe(320);
    expect(clampQueueWidth(999)).toBe(420);
    expect(clampQueueWidth(319.6)).toBe(320);
  });
});

describe("fontStack", () => {
  it("bundled and system picks", () => {
    expect(fontStack("manrope", "")).toBe('"Manrope", system-ui, sans-serif');
    expect(fontStack("inter", "")).toBe('"Inter", system-ui, sans-serif');
    expect(fontStack("system", "")).toBe("system-ui, sans-serif");
  });

  it("custom names get quoted and fall back to system-ui", () => {
    expect(fontStack("custom", "Commit Mono")).toBe('"Commit Mono", system-ui, sans-serif');
    expect(fontStack("custom", "  ")).toBe("system-ui, sans-serif");
  });

  /// Review Focus: quotes and backslashes in a custom name must not break
  /// out of the CSS value.
  it("strips css-breaking characters from custom names", () => {
    expect(fontStack("custom", 'Evil"; } * { display:none')).toBe(
      '"Evil; } * { display:none", system-ui, sans-serif',
    );
    expect(fontStack("custom", "a\\b")).toBe('"ab", system-ui, sans-serif');
  });
});
