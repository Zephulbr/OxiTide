import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  restoreSession: vi.fn(),
  getPlaybackState: vi.fn(),
  queueRemove: vi.fn(),
  queueInsert: vi.fn(),
  getFavoriteIds: vi.fn(),
  setFavoriteTrack: vi.fn(),
  setFavoriteAlbum: vi.fn(),
  setFavoriteArtist: vi.fn(),
}));

vi.mock("./api", () => ({ api: mocks }));

import { createFavorites } from "./favorites.svelte";
import { createPlayer, emptyState } from "./player.svelte";
import { createToasts } from "./toasts.svelte";

const queue = [
  { track_id: 1, title: "A", artist: "X", album: "", duration_secs: 10, cover: null },
  { track_id: 2, title: "B", artist: "Y", album: "", duration_secs: 10, cover: null },
];

beforeEach(() => {
  for (const mock of Object.values(mocks)) mock.mockReset().mockResolvedValue(undefined);
  mocks.getPlaybackState.mockResolvedValue({ ...emptyState(), queue, current_index: 0 });
});

describe("undo toasts", () => {
  it("a removed queue row goes back to its slot", async () => {
    const toasts = createToasts();
    const player = createPlayer(toasts);
    await player.restore(queue, 0, 0, false, "off");
    await player.remove(1);

    expect(mocks.queueRemove).toHaveBeenCalledWith(1);
    const toast = toasts.toasts[toasts.toasts.length - 1];
    expect(toast.action?.label).toBe("Undo");
    toast.action!.run();
    expect(mocks.queueInsert).toHaveBeenCalledWith(1, [queue[1]]);
  });

  it("an unfavorite can be undone, but not twice over a re-favorite", async () => {
    const toasts = createToasts();
    const favorites = createFavorites(toasts);
    await favorites.toggleTrack(7);
    await favorites.toggleTrack(7);
    expect(favorites.hasTrack(7)).toBe(false);

    const toast = toasts.toasts[toasts.toasts.length - 1];
    expect(toast.message).toBe("Removed from Favorites");
    toast.action!.run();
    await vi.waitFor(() => expect(favorites.hasTrack(7)).toBe(true));
    expect(mocks.setFavoriteTrack).toHaveBeenLastCalledWith(7, true);

    // Already favorite again: a second Undo must not unfavorite it.
    toast.action!.run();
    expect(favorites.hasTrack(7)).toBe(true);
    expect(mocks.setFavoriteTrack).toHaveBeenCalledTimes(3);
  });
});
