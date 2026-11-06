import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { playArtistRadio, playTrackRadio, trackRadioOrigin } from "./radio";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: {
    getTrackRadio: vi.fn(),
    getArtistRadio: vi.fn(),
    playTracks: vi.fn(),
    setPlaySource: vi.fn(async () => {}),
  },
  trackToQueueItem: (t: { id: number }) => ({ track_id: t.id }),
}));

function fakeToasts() {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  const toasts: Toasts = {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
  };
  return { toasts, calls };
}

describe("playTrackRadio", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("queues the mapped tracks", async () => {
    vi.mocked(api.getTrackRadio).mockResolvedValue([{ id: 1 } as never, { id: 2 } as never]);
    const { toasts, calls } = fakeToasts();

    await playTrackRadio(7, toasts);

    expect(api.getTrackRadio).toHaveBeenCalledWith(7);
    expect(api.playTracks).toHaveBeenCalledWith([{ track_id: 1 }, { track_id: 2 }], 0);
    expect(calls).toHaveLength(0);
  });

  it("credits its plays to the radio's page", () => {
    expect(trackRadioOrigin(7, "Holy, Holy")).toEqual({
      kind: "radio",
      name: "Holy, Holy radio",
      route: { name: "radio", id: 7 },
    });
  });

  it("informs instead of playing when the radio is empty", async () => {
    vi.mocked(api.getTrackRadio).mockResolvedValue([]);
    const { toasts, calls } = fakeToasts();

    await playTrackRadio(7, toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls[0]?.message).toBe("There's no radio for this track yet.");
  });

  it("surfaces a fetch failure as an error toast instead of rejecting", async () => {
    vi.mocked(api.getTrackRadio).mockRejectedValue(new Error("boom"));
    const { toasts, calls } = fakeToasts();

    await playTrackRadio(7, toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls).toHaveLength(1);
    expect(calls[0]?.severity).toBe("error");
    expect(calls[0]?.message).toContain("boom");
  });

  it("shows radio-specific copy when the request is forbidden", async () => {
    vi.mocked(api.getTrackRadio).mockRejectedValue("forbidden");
    const { toasts, calls } = fakeToasts();

    await playTrackRadio(7, toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls[0]?.severity).toBe("error");
    expect(calls[0]?.message).not.toContain("playlists");
    expect(calls[0]?.message).toContain("Radio");
  });
});

describe("playArtistRadio", () => {
  it("fetches by artist id and plays", async () => {
    vi.mocked(api.getArtistRadio).mockResolvedValue([{ id: 3 } as never]);
    const { toasts } = fakeToasts();

    await playArtistRadio(9, toasts);

    expect(api.getArtistRadio).toHaveBeenCalledWith(9);
    expect(api.playTracks).toHaveBeenCalledWith([{ track_id: 3 }], 0);
  });

  // An artist with nothing playable (a new profile) has no station: TIDAL
  // answers 404, which read "not found: /v1/artists/58912082/radio".
  it("words TIDAL's 404 as an artist without a radio, not an error", async () => {
    vi.clearAllMocks();
    vi.mocked(api.getArtistRadio).mockRejectedValue("not found: /v1/artists/58912082/radio");
    const { toasts, calls } = fakeToasts();

    await playArtistRadio(58912082, toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls).toEqual([
      {
        message: "There's no radio for this artist yet. TIDAL builds one once they have tracks to play.",
        severity: "info",
      },
    ]);
  });
});
