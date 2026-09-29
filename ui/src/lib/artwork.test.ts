import { describe, expect, it } from "vitest";
import { collageCovers, coverUrl } from "./artwork";

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

  /// Mix rows from pages endpoints carry a ready-made CDN url instead of a
  /// dashed uuid; it must not be mangled into `.../https://...`.
  it("leaves an absolute CDN url untouched", () => {
    const url = "https://resources.tidal.com/images/a/b/640x640.jpg";
    expect(coverUrl(url, 320)).toBe(url);
  });
});

describe("collageCovers", () => {
  it("keeps first-seen order and drops duplicate albums", () => {
    expect(collageCovers(["a", "a", "b", "a", "c", "d", "e"])).toEqual(["a", "b", "c", "d"]);
  });

  it("skips null and empty covers", () => {
    expect(collageCovers([null, "", undefined, "a", "b", "c", "d"])).toEqual(["a", "b", "c", "d"]);
  });

  it("returns empty when no track has a cover", () => {
    expect(collageCovers([null, "", undefined])).toEqual([]);
    expect(collageCovers([])).toEqual([]);
  });

  it("leaves a single distinct album as one tile", () => {
    expect(collageCovers(["a", "a", "a"])).toEqual(["a"]);
  });

  it("repeat-fills four tiles from two distinct albums", () => {
    expect(collageCovers(["a", "b"])).toEqual(["a", "b", "a", "b"]);
  });

  it("repeat-fills four tiles from three distinct albums", () => {
    expect(collageCovers(["a", "b", "c"])).toEqual(["a", "b", "c", "a"]);
  });

  it("respects a smaller tile count", () => {
    expect(collageCovers(["a", "b", "c", "d", "e"], 2)).toEqual(["a", "b"]);
  });
});
