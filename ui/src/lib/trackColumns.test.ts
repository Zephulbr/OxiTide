import { describe, expect, it } from "vitest";
import { COLUMNS_KEY, createTrackColumns } from "./trackColumns.svelte";

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    data,
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
  };
}

describe("trackColumns", () => {
  it("shows date added by default only where it exists", () => {
    const columns = createTrackColumns(fakeStorage());
    expect(columns.chosen("playlist")).toEqual(["added"]);
    expect(columns.chosen("mix")).toEqual([]);
    expect(columns.available("album")).toEqual(["popularity", "plays"]);
  });

  it("toggles per table kind, keeps display order, and persists", () => {
    const storage = fakeStorage();
    const columns = createTrackColumns(storage);
    columns.toggle("favorites", "plays");
    columns.toggle("favorites", "released");
    expect(columns.chosen("favorites")).toEqual(["added", "released", "plays"]);
    columns.toggle("favorites", "added");
    expect(columns.chosen("favorites")).toEqual(["released", "plays"]);
    expect(columns.chosen("playlist")).toEqual(["added"]);

    const reopened = createTrackColumns(storage);
    expect(reopened.chosen("favorites")).toEqual(["released", "plays"]);
    expect(JSON.parse(storage.data.get(COLUMNS_KEY)!)).toEqual({ favorites: ["released", "plays"] });
  });

  it("ignores unknown stored columns", () => {
    const columns = createTrackColumns(
      fakeStorage({ [COLUMNS_KEY]: JSON.stringify({ mix: ["plays", "bogus"], album: "x" }) }),
    );
    expect(columns.chosen("mix")).toEqual(["plays"]);
  });
});
