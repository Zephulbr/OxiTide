import { describe, expect, it } from "vitest";
import { coverUrl } from "./artwork";

describe("coverUrl", () => {
  it("maps a dashed uuid onto the image CDN path", () => {
    expect(coverUrl("abc-def-123", 320)).toBe(
      "https://resources.tidal.com/images/abc/def/123/320x320.jpg",
    );
  });

  it("supports every sanctioned size", () => {
    for (const size of [80, 160, 320, 640, 1280] as const) {
      expect(coverUrl("u", size)).toContain(`/${size}x${size}.jpg`);
    }
  });

  /// Review Focus: an empty uuid must not produce a broken-image request.
  it("returns empty for an empty uuid", () => {
    expect(coverUrl("", 320)).toBe("");
  });
});
