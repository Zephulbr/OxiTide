import { beforeEach, describe, expect, it, vi } from "vitest";
import { api, type Track, type TrackCredits } from "./api";
import { createInfo } from "./info.svelte";

vi.mock("./api", () => ({ api: { getTrack: vi.fn(), getTrackCredits: vi.fn() } }));

const track = (id: number): Track => ({
  id, title: `T${id}`, duration: 200, artists: [{ id: 5, name: "A" }],
  album: { id: 7, title: "B", cover: null }, audioQuality: null,
  trackNumber: 1, volumeNumber: 1, explicit: false, mediaMetadata: null,
});

const credits = (): TrackCredits => ({
  roles: [{ role: "Vocals", contributors: [{ name: "A", artistId: 5 }] }],
});

describe("info store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.getTrack).mockResolvedValue(track(1));
    vi.mocked(api.getTrackCredits).mockResolvedValue(credits());
  });

  it("loads track and credits and clears loading", async () => {
    const model = createInfo();
    await model.open(1);
    expect(model.track?.id).toBe(1);
    expect(model.credits?.roles[0].role).toBe("Vocals");
    expect(model.loading).toBe(false);
    expect(model.trackId).toBe(1);
  });

  it("caches both fetches per track id", async () => {
    const model = createInfo();
    await model.open(1);
    await model.open(1);
    expect(api.getTrack).toHaveBeenCalledTimes(1);
    expect(api.getTrackCredits).toHaveBeenCalledTimes(1);
  });

  it("drops a superseded response", async () => {
    let resolveFirst: (value: Track) => void = () => {};
    vi.mocked(api.getTrack)
      .mockImplementationOnce(() => new Promise<Track>((resolve) => (resolveFirst = resolve)))
      .mockResolvedValueOnce(track(2));
    const model = createInfo();
    const first = model.open(1);
    await model.open(2);
    resolveFirst(track(1));
    await first;
    expect(model.track?.id).toBe(2);
    expect(model.trackId).toBe(2);
  });

  it("a cache hit invalidates an in-flight fetch for another id", async () => {
    let resolveSecond: (value: Track) => void = () => {};
    vi.mocked(api.getTrack)
      .mockResolvedValueOnce(track(1))
      .mockImplementationOnce(() => new Promise<Track>((resolve) => (resolveSecond = resolve)));
    const model = createInfo();
    await model.open(1);
    const second = model.open(2);
    await model.open(1);
    resolveSecond(track(2));
    await second;
    expect(model.track?.id).toBe(1);
    expect(model.trackId).toBe(1);
  });

  it("a superseded track failure does not paint an error", async () => {
    let rejectFirst: (reason: unknown) => void = () => {};
    vi.mocked(api.getTrack)
      .mockImplementationOnce(
        () => new Promise<Track>((_resolve, reject) => (rejectFirst = reject)),
      )
      .mockResolvedValueOnce(track(2));
    const model = createInfo();
    const first = model.open(1);
    await model.open(2);
    rejectFirst(new Error("offline"));
    await first;
    expect(model.track?.id).toBe(2);
    expect(model.error).toBe("");
    expect(model.trackId).toBe(2);
  });

  it("surfaces a track failure and retries", async () => {
    vi.mocked(api.getTrack).mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce(track(1));
    const model = createInfo();
    await model.open(1);
    expect(model.error).toBe("Error: offline");
    expect(api.getTrackCredits).not.toHaveBeenCalled();
    model.retry();
    await vi.waitFor(() => expect(model.track?.id).toBe(1));
    expect(model.error).toBe("");
  });

  it("keeps track metadata when credits fail", async () => {
    vi.mocked(api.getTrackCredits).mockRejectedValue(new Error("credits down"));
    const model = createInfo();
    await model.open(1);
    expect(model.track?.id).toBe(1);
    expect(model.credits).toBeNull();
    expect(model.creditsError).toBe("Error: credits down");
  });

  it("reopens a failed id instead of serving a cache", async () => {
    vi.mocked(api.getTrackCredits).mockRejectedValue(new Error("down"));
    const model = createInfo();
    await model.open(1);
    await model.open(1);
    expect(api.getTrack).toHaveBeenCalledTimes(2);
  });
});
