import { describe, expect, it } from "vitest";
import type { Track } from "./api";
import { volumeDisplayRows, volumeSections } from "./albumVolumes";

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

describe("volumeDisplayRows", () => {
  const displayRows = (source: Track[]) => {
    const sections = volumeSections(source);
    if (!sections) throw new Error("expected multi-volume sections");
    const sourceIndex = new Map(source.map((t, i) => [t, i]));
    return volumeDisplayRows(sections, sourceIndex);
  };

  it("keeps source positions for a volume-block-ordered album", () => {
    const source = [track(1, 1), track(2, 1), track(3, 2), track(4, 2)];
    const rows = displayRows(source);
    expect(rows.map((row) => row.label)).toEqual(["Volume 1", null, null, "Volume 2", null, null]);
    const tracks = rows.filter((row) => row.track !== undefined);
    expect(tracks.map((row) => row.track?.id)).toEqual([1, 2, 3, 4]);
    expect(tracks.map((row) => row.index)).toEqual([0, 1, 2, 3]);
    expect(tracks.map((row) => row.number)).toEqual([1, 2, 1, 2]);
    for (const row of tracks) expect(source[row.index ?? -1]).toBe(row.track);
  });

  it("maps interleaved display rows back to their source positions", () => {
    const source = [track(1, 1), track(2, 2), track(3, 1), track(4, 2)];
    const rows = displayRows(source);
    expect(rows[0].label).toBe("Volume 1");
    expect(rows[3].label).toBe("Volume 2");
    const tracks = rows.filter((row) => row.track !== undefined);
    expect(tracks.map((row) => row.track?.id)).toEqual([1, 3, 2, 4]);
    expect(tracks.map((row) => row.index)).toEqual([0, 2, 1, 3]);
    expect(tracks.map((row) => row.number)).toEqual([1, 2, 1, 2]);
    for (const row of tracks) expect(source[row.index ?? -1]).toBe(row.track);
  });
});
