import { describe, expect, it } from "vitest";
import type { Track } from "./api";
import { createTrackInfo } from "./trackInfo.svelte";

const track = { id: 4, title: "Purple Rain" } as Track;

describe("track info requests", () => {
  it("opens and closes a request slot", () => {
    const model = createTrackInfo();
    model.open(track);
    expect(model.request).toEqual({ id: 4, title: "Purple Rain" });
    model.close();
    expect(model.request).toBeNull();
  });
});
