// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { api, type Track } from "../lib/api";
import type { Favorites } from "../lib/favorites.svelte";
import type { Nav } from "../lib/nav.svelte";
import { playSource } from "../lib/playSource.svelte";
import { fakePlayer } from "../lib/test/fakes";
import { clearViewCache } from "../lib/viewCache";
import Radio from "./Radio.svelte";

const track = (id: number, title: string) =>
  ({ id, title, duration: 180, artists: [{ id: 3, name: "Band" }], album: { id: 9, title: "LP", cover: null } }) as unknown as Track;

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      getTrack: vi.fn(async () => track(7, "Holy, Holy")),
      getTrackRadio: vi.fn(async () => [track(7, "Holy, Holy"), track(8, "Next One")]),
      getTrackFlags: vi.fn(async () => []),
      playTracks: vi.fn(async () => {}),
      setPlaySource: vi.fn(async () => {}),
    },
  };
});

beforeAll(() => {
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});

beforeEach(() => {
  clearViewCache();
  vi.clearAllMocks();
});
afterEach(() => {
  cleanup();
  playSource.set(null);
});

function setup() {
  const nav = { goto: vi.fn(), route: { name: "radio", id: 7 } } as unknown as Nav;
  render(Radio, {
    id: 7,
    nav,
    player: fakePlayer({ current: null, current_index: null }),
    favorites: { hasTrack: () => false, toggleTrack: vi.fn() } as unknown as Favorites,
    playlistActions: { openPicker: vi.fn() } as never,
    toasts: { show: vi.fn() } as never,
  });
  return { nav };
}

describe("Radio page", () => {
  it("lists the station under its seed track without playing it", async () => {
    setup();
    await waitFor(() => expect(screen.getByRole("heading", { name: /Holy, Holy radio/ })).toBeTruthy());
    expect(screen.getByText("Next One")).toBeTruthy();
    expect(api.getTrackRadio).toHaveBeenCalledWith(7);
    expect(api.playTracks).not.toHaveBeenCalled();
  });

  it("plays from Play, crediting the radio page", async () => {
    setup();
    await screen.findByText("Next One");
    await fireEvent.click(document.querySelector<HTMLButtonElement>(".btn-play")!);
    await waitFor(() => expect(api.playTracks).toHaveBeenCalled());
    expect(playSource.fromFor(8)).toMatchObject({
      kind: "radio",
      name: "Holy, Holy radio",
      route: { name: "radio", id: 7 },
    });
  });
});
