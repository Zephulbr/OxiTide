// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import type { LaterEntry, LaterPage } from "../lib/api";
import { api } from "../lib/api";
import { later } from "../lib/later.svelte";
import type { Nav } from "../lib/nav.svelte";
import { fakePlayer } from "../lib/test/fakes";
import SaveForLater from "./SaveForLater.svelte";

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    api: {
      getSaveForLaterPage: vi.fn(),
      getSaveForLaterIds: vi.fn(),
      setSaveForLater: vi.fn(),
    },
  };
});

beforeAll(() => {
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  globalThis.IntersectionObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof IntersectionObserver;
});

const track: LaterEntry = {
  kind: "track",
  added_at: "2026-10-15T07:45:45Z",
  item: {
    id: 7,
    title: "i like the way you kiss me",
    duration: 142,
    artists: [{ id: 1, name: "Artemas" }],
    album: { id: 70, title: "A", cover: null },
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  },
};
const album: LaterEntry = {
  kind: "album",
  item: { id: 8, title: "Album Eight", artists: [{ id: 2, name: "X" }], numberOfTracks: 1, duration: 1 },
};

function page(items: LaterEntry[], extra: Partial<LaterPage> = {}): LaterPage {
  return { items, cursor: null, videos: 0, ...extra };
}

const nav = { goto: vi.fn(), replace: vi.fn(), route: { name: "later" } } as unknown as Nav;
const props = () =>
  ({
    nav,
    player: fakePlayer(),
    favorites: { hasTrack: () => false },
    playlistActions: { openPicker: vi.fn() },
    toasts: { show: vi.fn() },
  }) as never;

beforeEach(() => {
  vi.clearAllMocks();
  later.reset();
  vi.mocked(api.setSaveForLater).mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("Save for Later page", () => {
  it("lists every page newest first, with chips only for kinds it holds", async () => {
    vi.mocked(api.getSaveForLaterPage).mockImplementation(async (cursor) =>
      cursor ? page([album], { videos: 2 }) : page([track], { cursor: "c2" }),
    );
    render(SaveForLater, props());
    await screen.findByText("Album Eight");
    expect(screen.getByText("i like the way you kiss me")).toBeTruthy();
    expect(api.getSaveForLaterPage).toHaveBeenCalledWith("c2");
    const chips = screen.getAllByRole("button", { pressed: false }).map((b) => b.textContent?.trim());
    expect(chips).toEqual(["Albums", "Tracks"]);
    expect(screen.getByRole("button", { name: "All", pressed: true })).toBeTruthy();
    expect(screen.getByText("Track")).toBeTruthy();
    expect(screen.getByText(/2 videos saved in TIDAL aren't shown here/)).toBeTruthy();
  });

  it("filters to the route's chip", async () => {
    vi.mocked(api.getSaveForLaterPage).mockResolvedValue(page([track, album]));
    render(SaveForLater, { ...(props() as object), kind: "album" } as never);
    await screen.findByText("Album Eight");
    expect(screen.queryByText("i like the way you kiss me")).toBeNull();
    screen.getByRole("button", { name: "All" }).click();
    expect(nav.replace).toHaveBeenCalledWith({ name: "later" });
  });

  it("lists saved mixes under Mixes & Radio and opens them", async () => {
    const mix: LaterEntry = {
      kind: "mix",
      item: { id: "0019ab", title: "a tombstone", subTitle: "Kensuke Ushio", image: null },
    };
    vi.mocked(api.getSaveForLaterPage).mockResolvedValue(page([mix, album]));
    render(SaveForLater, props());
    await screen.findByText("a tombstone");
    expect(screen.getByRole("button", { name: "Mixes & Radio" })).toBeTruthy();
    screen.getByText("a tombstone").click();
    expect(nav.goto).toHaveBeenCalledWith({ name: "mix", id: "0019ab" });
  });

  it("shows the empty state", async () => {
    vi.mocked(api.getSaveForLaterPage).mockResolvedValue(page([]));
    render(SaveForLater, props());
    await screen.findByText(/Nothing saved yet/);
  });

  it("drops an item removed from Save for Later", async () => {
    vi.mocked(api.getSaveForLaterIds).mockResolvedValue({
      tracks: [7],
      albums: [8],
      artists: [],
      playlists: [],
      mixes: [],
      videos: 0,
    });
    await later.hydrate();
    vi.mocked(api.getSaveForLaterPage).mockResolvedValue(page([track, album]));
    render(SaveForLater, props());
    await screen.findByText("Album Eight");
    await later.toggle("album", 8);
    await waitFor(() => expect(screen.queryByText("Album Eight")).toBeNull());
    expect(screen.getByText("i like the way you kiss me")).toBeTruthy();
  });
});
