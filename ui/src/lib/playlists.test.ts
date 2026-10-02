import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import { api } from "./api";
import { createPlaylistActions } from "./playlists.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: {
    createPlaylist: vi.fn(),
    setPlaylistPublic: vi.fn(),
    addPlaylistTracks: vi.fn(),
    getUserPlaylists: vi.fn(),
  },
}));

function track(id: number): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

function playlist(uuid: string, title = "Mix"): Playlist {
  return { uuid, title, description: null, numberOfTracks: 0, duration: 0, image: null, owned: true };
}

interface FakeToasts extends Toasts {
  readonly calls: { message: string; severity: ToastSeverity }[];
}

function fakeToasts(): FakeToasts {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  return {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
    get calls() {
      return calls;
    },
  };
}

describe("playlist actions store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([]);
    vi.mocked(api.createPlaylist).mockResolvedValue(playlist("new"));
    vi.mocked(api.setPlaylistPublic).mockResolvedValue(undefined);
  });

  it("computes skips from the added ids and opens the duplicates step", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1]);
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(1), track(2), track(3)]);

    await actions.addTo(playlist("p"));

    expect(api.addPlaylistTracks).toHaveBeenCalledWith("p", [1, 2, 3], false);
    expect(actions.step).toBe("duplicates");
    expect(actions.duplicates.map((t) => t.id)).toEqual([2, 3]);
    expect(actions.open).toBe(true);
  });

  it("closes and shows a success toast when nothing was skipped", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1, 2]);
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(1), track(2)]);

    await actions.addTo(playlist("p", "Chill"));

    expect(actions.open).toBe(false);
    expect(toasts.calls).toEqual([{ message: "2 added to Chill", severity: "success" }]);
  });

  it("keeps one by default and sends no second write", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1]);
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(1), track(2)]);
    await actions.addTo(playlist("p"));

    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenCalledTimes(1);
    expect(actions.open).toBe(false);
    expect(toasts.calls).toEqual([{ message: "0 added, 1 kept one of", severity: "info" }]);
  });

  it("sends only the add-both choices with allow_duplicates", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([1]).mockResolvedValueOnce([3]);
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(1), track(2), track(3)]);
    await actions.addTo(playlist("p"));

    actions.setChoice(3, true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [3], true);
  });

  it("apply-to-all sets every duplicate to add both", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([1]).mockResolvedValueOnce([2, 3]);
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(1), track(2), track(3)]);
    await actions.addTo(playlist("p"));

    actions.setAllChoices(true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [2, 3], true);
  });

  it("dedupes pending ids before adding", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([7]);
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(7), track(7)]);

    await actions.addTo(playlist("p"));

    expect(api.addPlaylistTracks).toHaveBeenCalledWith("p", [7], false);
  });

  it("dedupes the skipped list while preserving order", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([]);
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(7), track(7)]);

    await actions.addTo(playlist("p"));

    expect(actions.duplicates.map((t) => t.id)).toEqual([7]);
    expect(actions.step).toBe("duplicates");
  });

  it("dedupes add-both ids in phase two", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([]).mockResolvedValueOnce([7]);
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(7), track(7)]);
    await actions.addTo(playlist("p"));

    actions.setChoice(7, true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [7], true);
    expect(api.addPlaylistTracks).toHaveBeenCalledTimes(2);
    expect(toasts.calls).toEqual([{ message: "1 added, 0 kept one of", severity: "info" }]);
  });

  it("surfaces a failed add and keeps the picker open with its pending tracks", async () => {
    vi.mocked(api.addPlaylistTracks).mockRejectedValue(new Error("boom"));
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(1)]);

    await actions.addTo(playlist("p"));

    expect(toasts.calls).toEqual([{ message: "Error: boom", severity: "error" }]);
    expect(actions.open).toBe(true);
    expect(actions.pendingTrackIds).toEqual([1]);
  });

  it("creates then adds and bumps the revision", async () => {
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(1), track(2)]);

    const created = await actions.createAndAdd("New", "desc", true);

    expect(api.createPlaylist).toHaveBeenCalledWith("New", "desc");
    expect(api.setPlaylistPublic).toHaveBeenCalledWith("new", true);
    expect(api.addPlaylistTracks).toHaveBeenCalledWith("new", [1, 2], false);
    expect(created?.uuid).toBe("new");
    expect(actions.revision).toBe(1);
    expect(actions.open).toBe(false);
    expect(toasts.calls).toEqual([{ message: "Created Mix with 2 tracks", severity: "success" }]);
  });

  it("does not touch visibility for a private playlist", async () => {
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(1)]);

    await actions.createAndAdd("New", "desc", false);

    expect(api.createPlaylist).toHaveBeenCalledWith("New", "desc");
    expect(api.setPlaylistPublic).not.toHaveBeenCalled();
  });

  it("reports a public failure but still adds tracks and returns the playlist", async () => {
    vi.mocked(api.setPlaylistPublic).mockRejectedValue(new Error("nope"));
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1]);
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(1)]);

    const created = await actions.createAndAdd("New", "desc", true);

    expect(created?.uuid).toBe("new");
    expect(api.addPlaylistTracks).toHaveBeenCalledWith("new", [1], false);
    expect(toasts.calls).toEqual([
      { message: "Created Mix, but making it public failed: Error: nope", severity: "error" },
      { message: "Created Mix with 1 track", severity: "success" },
    ]);
    expect(actions.open).toBe(false);
  });

  it("returns the created playlist even when the follow-up add fails", async () => {
    vi.mocked(api.addPlaylistTracks).mockRejectedValue(new Error("nope"));
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(1)]);

    const created = await actions.createAndAdd("New", "", false);

    expect(created?.uuid).toBe("new");
    expect(toasts.calls).toEqual([
      {
        message: "Created Mix, but adding 1 track failed: Error: nope",
        severity: "error",
      },
    ]);
  });

  it("reports a phase-two failure without claiming the whole add failed", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([1]).mockRejectedValueOnce(
      new Error("nope"),
    );
    const toasts = fakeToasts();
    const actions = createPlaylistActions(toasts);
    actions.openPicker([track(1), track(2)]);
    await actions.addTo(playlist("p", "Chill"));

    actions.setChoice(2, true);
    await actions.confirmDuplicates();

    expect(toasts.calls).toEqual([
      {
        message: "Some tracks were already added, but the duplicate update failed: Error: nope",
        severity: "error",
      },
    ]);
    expect(actions.open).toBe(true);
  });

  it("tracks the picker origin and busy state", async () => {
    let release: ((ids: number[]) => void) | null = null;
    vi.mocked(api.addPlaylistTracks).mockImplementation(
      () =>
        new Promise<number[]>((resolve) => {
          release = resolve;
        }),
    );
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(1)]);
    expect(actions.fromPicker).toBe(true);

    const pending = actions.addTo(playlist("p"));
    expect(actions.busy).toBe(true);

    release!([1]);
    await pending;
    expect(actions.busy).toBe(false);

    actions.openCreate();
    expect(actions.fromPicker).toBe(false);
    expect(actions.step).toBe("create");
  });

  it("reset closes the picker and clears the pending tracks", () => {
    const actions = createPlaylistActions(fakeToasts());
    actions.openPicker([track(1), track(2)]);

    actions.reset();

    expect(actions.open).toBe(false);
    expect(actions.pendingTrackIds).toEqual([]);
    expect(actions.step).toBe("list");
  });
});
