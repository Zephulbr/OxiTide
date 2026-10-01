import { describe, expect, it } from "vitest";
import { makeSyntheticTracks } from "./perfFixture";

describe("makeSyntheticTracks", () => {
  it("is deterministic and unique", () => {
    const a = makeSyntheticTracks(10_000);
    const b = makeSyntheticTracks(10_000);
    expect(a).toHaveLength(10_000);
    expect(new Set(a.map((t) => t.id)).size).toBe(10_000);
    expect(a[123].title).toBe(b[123].title);
    expect(a[123].artists[0].name).toBe(b[123].artists[0].name);
  });

  it("spreads titles so filter queries narrow the list", () => {
    const tracks = makeSyntheticTracks(10_000);
    const matches = tracks.filter((t) => t.title.toLowerCase().includes("the ")).length;
    expect(matches).toBeGreaterThan(0);
    expect(matches).toBeLessThan(tracks.length);
  });
});
