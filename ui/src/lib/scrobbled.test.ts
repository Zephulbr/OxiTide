import { describe, expect, it } from "vitest";
import { createScrobbled } from "./scrobbled.svelte";

describe("scrobbled badge", () => {
  it("marks a track and clears", () => {
    const badge = createScrobbled();
    expect(badge.trackId).toBeNull();
    badge.mark(7);
    expect(badge.trackId).toBe(7);
    badge.clear();
    expect(badge.trackId).toBeNull();
  });
});
