import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import { api } from "./api";
import { createNotice } from "./notice.svelte";
import { createPlaylistActions } from "./playlists.svelte";

vi.mock("./api", () => ({
  api: {
    createPlaylist: vi.fn(),
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
  return { uuid, title, description: null, numberOfTracks: 0, duration: 0, image: null };
}

describe("playlist actions store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([]);
    vi.mocked(api.createPlaylist).mockResolvedValue(playlist("new"));
  });

  it("computes skips from the added ids and opens the duplicates step", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2), track(3)]);

    await actions.addTo(playlist("p"));

    expect(api.addPlaylistTracks).toHaveBeenCalledWith("p", [1, 2, 3], false);
    expect(actions.step).toBe("duplicates");
    expect(actions.duplicates.map((t) => t.id)).toEqual([2, 3]);
    expect(actions.open).toBe(true);
  });

  it("closes and notices when nothing was skipped", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1, 2]);
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1), track(2)]);

    await actions.addTo(playlist("p", "Chill"));

    expect(actions.open).toBe(false);
    expect(notice.message).toContain("2 added");
    expect(notice.message).toContain("Chill");
  });

  it("keeps one by default and sends no second write", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1]);
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1), track(2)]);
    await actions.addTo(playlist("p"));

    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenCalledTimes(1);
    expect(actions.open).toBe(false);
    expect(notice.message).toContain("kept one of");
  });

  it("sends only the add-both choices with allow_duplicates", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([1]).mockResolvedValueOnce([3]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2), track(3)]);
    await actions.addTo(playlist("p"));

    actions.setChoice(3, true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [3], true);
  });

  it("apply-to-all sets every duplicate to add both", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([1]).mockResolvedValueOnce([2, 3]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2), track(3)]);
    await actions.addTo(playlist("p"));

    actions.setAllChoices(true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [2, 3], true);
  });

  it("dedupes pending ids before adding", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([7]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(7), track(7)]);

    await actions.addTo(playlist("p"));

    expect(api.addPlaylistTracks).toHaveBeenCalledWith("p", [7], false);
  });

  it("dedupes the skipped list while preserving order", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(7), track(7)]);

    await actions.addTo(playlist("p"));

    expect(actions.duplicates.map((t) => t.id)).toEqual([7]);
    expect(actions.step).toBe("duplicates");
  });

  it("dedupes add-both ids in phase two", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([]).mockResolvedValueOnce([7]);
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(7), track(7)]);
    await actions.addTo(playlist("p"));

    actions.setChoice(7, true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [7], true);
    expect(api.addPlaylistTracks).toHaveBeenCalledTimes(2);
    expect(notice.message).toContain("1 added");
    expect(notice.message).toContain("0 kept one of");
  });

  it("surfaces a failed add and keeps the picker open with its pending tracks", async () => {
    vi.mocked(api.addPlaylistTracks).mockRejectedValue(new Error("boom"));
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1)]);

    await actions.addTo(playlist("p"));

    expect(notice.message).toContain("boom");
    expect(actions.open).toBe(true);
    expect(actions.pendingTrackIds).toEqual([1]);
  });

  it("creates then adds and bumps the revision", async () => {
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2)]);

    const created = await actions.createAndAdd("New", "desc");

    expect(api.createPlaylist).toHaveBeenCalledWith("New", "desc");
    expect(api.addPlaylistTracks).toHaveBeenCalledWith("new", [1, 2], false);
    expect(created?.uuid).toBe("new");
    expect(actions.revision).toBe(1);
    expect(actions.open).toBe(false);
  });

  it("returns the created playlist even when the follow-up add fails", async () => {
    vi.mocked(api.addPlaylistTracks).mockRejectedValue(new Error("nope"));
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1)]);

    const created = await actions.createAndAdd("New", "");

    expect(created?.uuid).toBe("new");
    expect(notice.message).toContain("nope");
  });

  it("tracks the picker origin and busy state", async () => {
    let release: ((ids: number[]) => void) | null = null;
    vi.mocked(api.addPlaylistTracks).mockImplementation(
      () =>
        new Promise<number[]>((resolve) => {
          release = resolve;
        }),
    );
    const actions = createPlaylistActions(createNotice());
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
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2)]);

    actions.reset();

    expect(actions.open).toBe(false);
    expect(actions.pendingTrackIds).toEqual([]);
    expect(actions.step).toBe("list");
  });
});
