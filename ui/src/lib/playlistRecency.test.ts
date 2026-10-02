import { describe, expect, it } from "vitest";
import { RECENCY_KEY, createPlaylistRecency } from "./playlistRecency.svelte";

type RecencyStorage = Pick<Storage, "getItem" | "setItem">;

function fakeStorage(seed?: string): RecencyStorage {
  const map = new Map<string, string>();
  if (seed !== undefined) map.set(RECENCY_KEY, seed);
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
  };
}

describe("playlist recency store", () => {
  it("ranks unknown playlists as 0", () => {
    const recency = createPlaylistRecency(fakeStorage());
    expect(recency.rank("nope")).toBe(0);
  });

  it("remembers when a playlist was played", () => {
    let now = 1000;
    const recency = createPlaylistRecency(fakeStorage(), () => now);
    recency.record("p1");
    expect(recency.rank("p1")).toBe(1000);
    now = 2000;
    recency.record("p2");
    recency.record("p1");
    expect(recency.rank("p1")).toBe(2000);
  });

  it("persists ranks so a new store instance loads them", () => {
    const storage = fakeStorage();
    createPlaylistRecency(storage, () => 42).record("p1");
    expect(createPlaylistRecency(storage).rank("p1")).toBe(42);
  });

  it("starts empty when the stored value is corrupt", () => {
    const recency = createPlaylistRecency(fakeStorage("{not json"), () => 7);
    expect(recency.rank("p1")).toBe(0);
    recency.record("p1");
    expect(recency.rank("p1")).toBe(7);
  });

  it("ignores stored values that are not numbers", () => {
    const recency = createPlaylistRecency(
      fakeStorage(JSON.stringify({ good: 5, bad: "later" })),
    );
    expect(recency.rank("good")).toBe(5);
    expect(recency.rank("bad")).toBe(0);
  });

  it("caps the stored history, dropping the oldest entries", () => {
    const storage = fakeStorage();
    let now = 0;
    const recency = createPlaylistRecency(storage, () => (now += 1));
    for (let i = 0; i < 505; i += 1) recency.record(`p${i}`);
    expect(recency.rank("p0")).toBe(0);
    expect(recency.rank("p4")).toBe(0);
    expect(recency.rank("p5")).toBe(6);
  });
});
