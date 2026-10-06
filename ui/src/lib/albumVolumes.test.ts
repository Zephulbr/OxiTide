import { describe, expect, it } from "vitest";
import type { Track } from "./api";
import { volumeSections } from "./albumVolumes";

const track = (id: number, volumeNumber: number, title = `T${id}`): Track => ({
  id, title, duration: 200, artists: [{ id: 1, name: "A" }], album: null,
  audioQuality: null, trackNumber: id, volumeNumber, explicit: false, mediaMetadata: null,
});

describe("volumeSections", () => {
  it("returns null for an empty or single-volume album", () => {
    expect(volumeSections([])).toBeNull();
    expect(volumeSections([track(1, 1), track(2, 1)])).toBeNull();
    expect(volumeSections([track(1, 0), track(2, 0)])).toBeNull();
  });

  it("groups by volume and preserves source order", () => {
    const sections = volumeSections([track(1, 1), track(2, 2), track(3, 1), track(4, 2)]);
    expect(sections?.map((section) => section.label)).toEqual(["Volume 1", "Volume 2"]);
    expect(sections?.[0].tracks.map((t) => t.id)).toEqual([1, 3]);
    expect(sections?.[1].tracks.map((t) => t.id)).toEqual([2, 4]);
  });

  it("treats zero numbering as volume one", () => {
    const sections = volumeSections([track(1, 0), track(2, 2)]);
    expect(sections?.[0].label).toBe("Volume 1");
    expect(sections?.[0].tracks.map((t) => t.id)).toEqual([1]);
    expect(sections?.[1].label).toBe("Volume 2");
  });

  it("keeps sparse volume numbers in first-seen order", () => {
    const sections = volumeSections([track(1, 3), track(2, 1)]);
    expect(sections?.map((section) => section.label)).toEqual(["Volume 3", "Volume 1"]);
  });
});
