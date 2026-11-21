// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, configure, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { QueueItem } from "../lib/api";
import { clearPagerCache } from "../lib/pager.svelte";
import { fakePlayer } from "../lib/test/fakes";
import type { Nav } from "../lib/nav.svelte";
import CollectionTracks from "./CollectionTracks.svelte";

const server = vi.hoisted(() => {
  const server = {
    total: 250,
    /// Windows past the first page wait here until a test releases them.
    held: [] as Array<(ok: boolean) => void>,
    /// The engine's context: every play starts a new one.
    context: 0,
    playTracks: vi.fn(async (..._args: unknown[]) => ++server.context),
    queueExtendContext: vi.fn(async (_items: unknown, context: unknown) => context === server.context),
  };
  return server;
});
const TOTAL = server.total;

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const TOTAL = server.total;
  const track = (id: number) => ({
    id,
    title: `Track ${id}`,
    duration: 200,
    artists: [{ id: 1, name: "Artist" }],
    album: { id: 2, title: "Album", cover: null },
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  });
  const page = (offset: number, limit: number) => {
    const count = Math.max(0, Math.min(limit, TOTAL - offset));
    const items = Array.from({ length: count }, (_, i) => track(offset + i + 1));
    return { items, total: TOTAL, offset, limit };
  };
  const overrides: Record<string, unknown> = {
    getFavoriteTracksPage: async (offset: number, limit: number) => {
      if (offset > 0) {
        const ok = await new Promise<boolean>((resolve) => server.held.push(resolve));
        if (!ok) throw new Error("offline");
      }
      return page(offset, limit);
    },
    playTracks: server.playTracks,
    queueExtendContext: server.queueExtendContext,
  };
  const api = new Proxy(
    {},
    { get: (_t, name: string) => (name in overrides ? overrides[name] : async () => []) },
  );
  return { ...actual, api };
});

beforeAll(() => {
  // Mounting the page and its pager takes a while on a loaded CI runner.
  configure({ asyncUtilTimeout: 3000 });
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});

beforeEach(() => {
  clearPagerCache();
  server.held.length = 0;
  server.context = 0;
  server.playTracks.mockClear();
  server.queueExtendContext.mockClear();
});
afterEach(cleanup);

const ids = (items: QueueItem[]) => items.map((item) => item.track_id);

function mount() {
  const favorites = {
    trackRevision: 0,
    pendingRemovalsToDrop: () => [],
    hasTrack: () => false,
    toggleTrack: vi.fn(),
  };
  render(CollectionTracks, {
    nav: { goto: vi.fn(), route: { name: "tracks" } } as unknown as Nav,
    player: fakePlayer({ current: null, playing: false }),
    favorites,
    playlistActions: { openPicker: vi.fn() },
    toasts: { show: vi.fn() },
  } as never);
}

/// The first page is on screen and the rest of the list is still loading.
async function firstPageShown() {
  await waitFor(() => expect(document.querySelectorAll("button.thumb").length).toBeGreaterThan(2));
  await waitFor(() => expect(server.held.length).toBeGreaterThan(0));
}

function releaseRest(ok = true) {
  for (const resolve of server.held.splice(0)) resolve(ok);
}

describe("Tracks plays while the list loads (issue #7)", () => {
  it("a row plays the loaded rows at once, then the rest joins its context", async () => {
    mount();
    await firstPageShown();

    await fireEvent.click(document.querySelectorAll<HTMLButtonElement>("button.thumb")[2]);

    await waitFor(() => expect(server.playTracks).toHaveBeenCalledTimes(1));
    const [head, start, shuffle] = server.playTracks.mock.calls[0] as [QueueItem[], number, boolean];
    expect(head).toHaveLength(100);
    expect(start).toBe(2);
    expect(shuffle).toBe(false);
    expect(server.queueExtendContext).not.toHaveBeenCalled();

    releaseRest();
    await waitFor(() => expect(server.queueExtendContext).toHaveBeenCalledTimes(1));
    const [rest, context] = server.queueExtendContext.mock.calls[0] as [QueueItem[], number];
    expect(context).toBe(1);
    expect(ids(rest)).toEqual(Array.from({ length: TOTAL - 100 }, (_, i) => 101 + i));
  });

  it("Shuffle waits for the whole list", async () => {
    mount();
    await firstPageShown();

    await fireEvent.click(document.querySelector<HTMLButtonElement>("button.list-action")!);
    await new Promise((r) => setTimeout(r, 20));
    expect(server.playTracks).not.toHaveBeenCalled();

    releaseRest();
    await waitFor(() => expect(server.playTracks).toHaveBeenCalledTimes(1));
    const [items, , shuffle] = server.playTracks.mock.calls[0] as [QueueItem[], number, boolean];
    expect(items).toHaveLength(TOTAL);
    expect(shuffle).toBe(true);
    expect(server.queueExtendContext).not.toHaveBeenCalled();
  });

  it("a row clicked while Shuffle waits wins", async () => {
    mount();
    await firstPageShown();

    await fireEvent.click(document.querySelector<HTMLButtonElement>("button.list-action")!);
    await fireEvent.click(document.querySelectorAll<HTMLButtonElement>("button.thumb")[1]);
    await waitFor(() => expect(server.playTracks).toHaveBeenCalledTimes(1));

    releaseRest();
    await waitFor(() => expect(server.queueExtendContext).toHaveBeenCalledTimes(1));
    await new Promise((r) => setTimeout(r, 20));
    expect(server.playTracks).toHaveBeenCalledTimes(1);
    expect(server.playTracks.mock.calls[0][2]).toBe(false);
  });

  /// The engine keeps only the second (the queue tests cover that).
  it("each row click's rest goes to its own play's context", async () => {
    mount();
    await firstPageShown();

    const thumbs = document.querySelectorAll<HTMLButtonElement>("button.thumb");
    await fireEvent.click(thumbs[1]);
    await fireEvent.click(thumbs[2]);
    await waitFor(() => expect(server.playTracks).toHaveBeenCalledTimes(2));

    releaseRest();
    await waitFor(() => expect(server.queueExtendContext).toHaveBeenCalledTimes(2));
    const contexts = server.queueExtendContext.mock.calls.map((call) => call[1]).sort();
    expect(contexts).toEqual([1, 2]);
  });

  it("a failed load leaves the first rows playing, and Retry adds the rest", async () => {
    mount();
    await firstPageShown();

    await fireEvent.click(document.querySelectorAll<HTMLButtonElement>("button.thumb")[0]);
    await waitFor(() => expect(server.playTracks).toHaveBeenCalledTimes(1));
    releaseRest(false);
    const retry = await screen.findByRole("button", { name: /retry/i });
    // Let the failed play settle, as it would before anyone could click.
    await new Promise((r) => setTimeout(r));
    expect(server.queueExtendContext).not.toHaveBeenCalled();

    await fireEvent.click(retry);
    await waitFor(() => expect(server.held.length).toBeGreaterThan(0));
    releaseRest();
    await waitFor(() => expect(server.queueExtendContext).toHaveBeenCalledTimes(1));
    const [rest, context] = server.queueExtendContext.mock.calls[0] as [QueueItem[], number];
    expect(context).toBe(1);
    expect(rest).toHaveLength(TOTAL - 100);
  });
});
