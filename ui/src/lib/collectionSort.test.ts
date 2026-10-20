import { describe, expect, it } from "vitest";
import type { CollectionEntry } from "./api";
import { sortEntries } from "./collectionSort";

function folder(id: string, name: string): CollectionEntry {
  return { kind: "folder", id, name, totalNumberOfItems: 1 };
}

function playlist(
  uuid: string,
  title: string,
  lastItemAddedAt: string | null = null,
): CollectionEntry {
  return {
    kind: "playlist",
    uuid,
    title,
    description: null,
    numberOfTracks: 0,
    duration: 0,
    image: null,
    owned: true,
    lastItemAddedAt,
  };
}

function labels(entries: CollectionEntry[]): string[] {
  return entries.map((entry) => (entry.kind === "folder" ? entry.name : entry.title));
}

describe("sortEntries", () => {
  it("date mode sorts playlists newest-first without moving folders", () => {
    const entries = [
      playlist("old", "Old", "2024-01-01T00:00:00.000+0000"),
      folder("f", "Folder"),
      playlist("new", "New", "2026-10-05T21:45:51.041+0000"),
      playlist("mid", "Middle", "2025-06-01T12:00:00.000+0000"),
    ];
    expect(labels(sortEntries(entries, "date"))).toEqual(["New", "Folder", "Middle", "Old"]);
  });

  it("date mode keeps service order for ties and missing dates", () => {
    const stamp = "2026-01-01T00:00:00.000+0000";
    const entries = [playlist("a", "Alpha", stamp), playlist("b", "Beta", stamp), playlist("c", "Gamma")];
    expect(labels(sortEntries(entries, "date"))).toEqual(["Alpha", "Beta", "Gamma"]);
  });

  it("date mode falls back to created when nothing was added", () => {
    const entries = [
      { ...playlist("old", "Old"), created: "2024-01-01T00:00:00.000+0000" },
      { ...playlist("new", "New"), created: "2026-10-05T21:45:51.041+0000" },
    ];
    expect(labels(sortEntries(entries, "date"))).toEqual(["New", "Old"]);
  });

  it("played mode ranks by last play, never-played by date, folders in place", () => {
    const played: Record<string, number> = { b: 200, c: 100 };
    const entries = [
      playlist("a", "Never old", "2024-01-01T00:00:00.000+0000"),
      folder("f", "Folder"),
      playlist("b", "Played last"),
      playlist("c", "Played first"),
      playlist("d", "Never new", "2026-01-01T00:00:00.000+0000"),
    ];
    expect(labels(sortEntries(entries, "played", (uuid) => played[uuid] ?? 0))).toEqual([
      "Played last",
      "Folder",
      "Played first",
      "Never new",
      "Never old",
    ]);
  });

  it("sorts folders first then playlists by name", () => {
    const entries = [
      playlist("z", "zebra"),
      folder("a", "alpha"),
      playlist("a", "Apple"),
      folder("z", "Zulu"),
    ];
    expect(sortEntries(entries, "name").map((e) => (e.kind === "folder" ? e.name : e.title))).toEqual([
      "alpha",
      "Zulu",
      "Apple",
      "zebra",
    ]);
  });
});
