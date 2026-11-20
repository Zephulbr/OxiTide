// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LyricsDto, RemoteState, Track } from "../lib/api";
import { RemoteActions } from "./actions.svelte";
import type { RemoteApi } from "./api";
import Lyrics from "./components/Lyrics.svelte";
import { REMOTE_KEY } from "./context";
import Favorites from "./pages/Favorites.svelte";
import NowPlaying from "./pages/NowPlaying.svelte";
import Playlist from "./pages/Playlist.svelte";
import Search from "./pages/Search.svelte";
import { RemoteStore } from "./store.svelte";

/// Phone views against a fake API: the context every page reads.
function setup(api: Partial<RemoteApi>) {
  const full = api as RemoteApi;
  const store = new RemoteStore({
    call: async () => {
      throw new Error("no rpc in tests");
    },
  });
  const actions = new RemoteActions(full);
  const context = new Map<unknown, unknown>([[REMOTE_KEY, { api: full, store, actions }]]);
  return { store, actions, context };
}

function remoteState(patch: Partial<RemoteState> = {}): RemoteState {
  return {
    loggedIn: true,
    playing: true,
    buffering: false,
    current: { trackId: 1, title: "One", artist: "A", album: "B", cover: null, durationSecs: 200 },
    currentIndex: 0,
    positionMs: 1000,
    durationMs: 200_000,
    volume: 0.5,
    volumeFixed: false,
    shuffle: false,
    repeat: "off",
    queueLen: 3,
    queueRev: 0,
    stream: null,
    bitPerfect: false,
    dspStages: [],
    ...patch,
  };
}

function track(id: number, title = `Song ${id}`): Track {
  return {
    id,
    title,
    duration: 200,
    explicit: false,
    artists: [{ id: 9, name: "Artist" }],
    album: { id: 7, title: "Album", cover: null },
  } as unknown as Track;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

afterEach(() => {
  cleanup();
  sessionStorage.clear();
});

describe("state events for the same track", () => {
  it("do not refetch or blank the lyrics", async () => {
    const lyrics: LyricsDto = { track_id: 1, source: "tidal", synced: false, rtl: false, lines: [{ time_ms: null, text: "la la" }] };
    const getLyrics = vi.fn(async () => lyrics);
    const { context } = setup({ getLyrics });
    const current = remoteState().current!;
    const view = render(Lyrics, { props: { track: current, positionMs: 0 }, context });
    await screen.findByText("la la");
    // A pause or volume change re-sends the whole state: a new object, same track.
    await view.rerender({ track: { ...current }, positionMs: 0 });
    await sleep(0);
    expect(getLyrics).toHaveBeenCalledTimes(1);
    expect(screen.getByText("la la")).toBeTruthy();
  });

  it("keep Now Playing's artist and album links", async () => {
    const getTrack = vi.fn(async (id: number) => track(id, "One"));
    const { store, context } = setup({ getTrack });
    store.applyState(remoteState(), true);
    render(NowPlaying, { context });
    await waitFor(() => expect(screen.getByRole("link", { name: "A" })).toBeTruthy());
    store.applyState(remoteState({ playing: false }), false);
    await sleep(0);
    expect(getTrack).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("link", { name: "A" })).toBeTruthy();
  });
});

/// A TIDAL-like list: `total` counts every row, but `items` leaves the
/// unavailable ones out (they are never returned).
function tidalList(total: number, unavailable: Set<number>, titleOf: (i: number) => string) {
  const offsets: number[] = [];
  const fetch = vi.fn(async (offset: number, limit: number) => {
    offsets.push(offset);
    await sleep(0);
    const items: Track[] = [];
    for (let i = offset; i < Math.min(total, offset + limit); i++) {
      if (!unavailable.has(i)) items.push(track(i + 1, titleOf(i)));
    }
    return { items, total, offset, limit };
  });
  return { fetch, offsets };
}

// Issue #4's library: 3,119 playable favorites and 248 unavailable ones, which
// sit together (one import), between the newest match and the older ones.
const VEILS = [5, 1500, 1800, 2200, 2600, 3300];
const veilTitle = (i: number) => (VEILS.includes(i) ? `Veil ${i}` : `Song ${i}`);

describe("paged lists", () => {
  it("a filter covers favorites past a run of unavailable tracks", { timeout: 20_000 }, async () => {
    const run = new Set(Array.from({ length: 248 }, (_, k) => 1000 + k));
    const { fetch, offsets } = tidalList(3367, run, veilTitle);
    const { context } = setup({ getFavoriteTracksPage: fetch });
    render(Favorites, { props: { kind: "tracks" }, context });
    await fireEvent.input(await screen.findByRole("searchbox"), { target: { value: "veil" } });
    await screen.findByText("Veil 3300", {}, { timeout: 15_000 });
    await waitFor(() => expect(screen.queryByText("Loading all tracks…")).toBeNull());
    for (const i of VEILS) expect(screen.getByText(`Veil ${i}`)).toBeTruthy();
    // Every window once, each advancing by the page size.
    expect(offsets).toEqual(Array.from({ length: 34 }, (_, k) => k * 100));
  });

  it("scattered unavailable tracks neither repeat nor drop rows", { timeout: 20_000 }, async () => {
    // Smaller than issue #4's list: "Add all" renders every row in jsdom.
    const scattered = new Set(Array.from({ length: 248 }, (_, k) => 7 + k * 5));
    const { fetch } = tidalList(1367, scattered, (i) => `Song ${i}`);
    const enqueue = vi.fn(async () => {});
    const { context, actions } = setup({ getFavoriteTracksPage: fetch });
    actions.enqueue = enqueue;
    render(Favorites, { props: { kind: "tracks" }, context });
    await fireEvent.click(await screen.findByRole("button", { name: "Add all to queue" }));
    await waitFor(() => expect(enqueue).toHaveBeenCalledTimes(1), { timeout: 15_000 });
    const rows = (enqueue.mock.calls[0] as unknown as [{ id: number }[]])[0];
    expect(rows).toHaveLength(1367 - 248);
    expect(new Set(rows.map((row) => row.id)).size).toBe(rows.length);
  });

  it("a playlist keeps a track that is in it twice", async () => {
    const getPlaylistTracksPage = vi.fn(async (_id: string, offset: number, limit: number) => ({
      items: offset === 0 ? [track(1, "Twice"), track(2, "Once"), track(1, "Twice")] : [],
      total: 3,
      offset,
      limit,
    }));
    const getPlaylist = vi.fn(async () => ({ uuid: "abc", title: "Mine", numberOfTracks: 3 }));
    const { context } = setup({ getPlaylistTracksPage, getPlaylist } as unknown as Partial<RemoteApi>);
    render(Playlist, { props: { id: "abc" }, context });
    await screen.findByText("Once");
    expect(screen.getAllByText("Twice")).toHaveLength(2);
    expect(getPlaylistTracksPage).toHaveBeenCalledTimes(1);
  });
});

describe("search", () => {
  it("drops a reply that lands after the box was cleared", async () => {
    let reply: (value: unknown) => void = () => {};
    const search = vi.fn(() => new Promise((resolve) => (reply = resolve)));
    const { context } = setup({ search } as Partial<RemoteApi>);
    render(Search, { context });
    const box = screen.getByRole("searchbox");
    await fireEvent.input(box, { target: { value: "veil" } });
    await waitFor(() => expect(search).toHaveBeenCalledTimes(1), { timeout: 2000 });
    await fireEvent.input(box, { target: { value: "" } });
    reply({
      tracks: { items: [track(1, "Liar Veil")] },
      artists: { items: [] },
      albums: { items: [] },
      playlists: { items: [] },
    });
    await sleep(0);
    expect(screen.queryByText("Liar Veil")).toBeNull();
    expect(screen.queryByText(/Searching/)).toBeNull();
  });
});
