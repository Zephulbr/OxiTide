import { beforeEach, describe, expect, it, vi } from "vitest";
import { api, type LyricsDto, type QueueItem } from "./api";
import { createLyrics } from "./lyrics.svelte";

vi.mock("./api", () => ({ api: { getLyrics: vi.fn() } }));

const track = (id: number): QueueItem => ({
  track_id: id, title: `T${id}`, artist: "A", album: "B", duration_secs: 200, cover: null,
});

const dto = (id: number): LyricsDto => ({
  track_id: id, source: "tidal", synced: true, rtl: false,
  lines: [{ time_ms: 0, text: "hi" }],
});

describe("lyrics store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads lyrics for a track and clears loading", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    expect(api.getLyrics).toHaveBeenCalledWith(1, "T1", "A", "B", 200);
    expect(model.lyrics?.track_id).toBe(1);
    expect(model.loading).toBe(false);
  });

  it("does not refetch the same track", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    await model.open(track(1));
    expect(api.getLyrics).toHaveBeenCalledTimes(1);
  });

  it("drops a stale response when the track changed mid-flight", async () => {
    let resolveFirst: (value: LyricsDto | null) => void = () => {};
    vi.mocked(api.getLyrics)
      .mockImplementationOnce(
        () => new Promise<LyricsDto | null>((resolve) => (resolveFirst = resolve)),
      )
      .mockResolvedValueOnce(dto(2));
    const model = createLyrics();
    const first = model.open(track(1));
    await model.open(track(2));
    resolveFirst(dto(1));
    await first;
    expect(model.lyrics?.track_id).toBe(2);
    expect(model.loading).toBe(false);
  });

  it("treats failures as no lyrics", async () => {
    vi.mocked(api.getLyrics).mockRejectedValue(new Error("offline"));
    const model = createLyrics();
    await model.open(track(3));
    expect(model.lyrics).toBeNull();
    expect(model.loading).toBe(false);
  });

  it("keeps the chosen tab on a track change", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    model.setTab("next");
    vi.mocked(api.getLyrics).mockResolvedValue(dto(2));
    await model.open(track(2));
    expect(model.tab).toBe("next");
  });

  it("keeps the info tab on a track change and resets on stop", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    model.setTab("info");
    vi.mocked(api.getLyrics).mockResolvedValue(dto(2));
    await model.open(track(2));
    expect(model.tab).toBe("info");
    await model.open(null);
    expect(model.tab).toBe("lyrics");
  });

  it("clears state for a null track", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    model.setTab("next");
    await model.open(null);
    expect(model.lyrics).toBeNull();
    expect(model.loading).toBe(false);
    expect(model.tab).toBe("lyrics");
    expect(model.trackId).toBeNull();
  });

  it("marks the track as current before the fetch resolves", async () => {
    let resolveFetch: (value: LyricsDto | null) => void = () => {};
    vi.mocked(api.getLyrics).mockImplementationOnce(
      () => new Promise<LyricsDto | null>((resolve) => (resolveFetch = resolve)),
    );
    const model = createLyrics();
    const pending = model.open(track(7));
    expect(model.trackId).toBe(7);
    expect(model.loading).toBe(true);
    resolveFetch(dto(7));
    await pending;
    expect(model.trackId).toBe(7);
    expect(model.loading).toBe(false);
  });

  it("keeps the track id after a settled miss", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(null);
    const model = createLyrics();
    await model.open(track(5));
    expect(model.lyrics).toBeNull();
    expect(model.loading).toBe(false);
    expect(model.trackId).toBe(5);
  });

  it("retries a track after a failed fetch", async () => {
    vi.mocked(api.getLyrics).mockRejectedValueOnce(new Error("offline"));
    vi.mocked(api.getLyrics).mockResolvedValueOnce(dto(4));
    const model = createLyrics();
    await model.open(track(4));
    expect(model.lyrics).toBeNull();
    await model.open(track(4));
    expect(api.getLyrics).toHaveBeenCalledTimes(2);
    expect(model.lyrics?.track_id).toBe(4);
  });

  it("re-queries a track that had no lyrics", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(null);
    const model = createLyrics();
    await model.open(track(5));
    await model.open(track(5));
    expect(api.getLyrics).toHaveBeenCalledTimes(2);
  });
});
