// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import type { QueueItem, RemoteState } from "../lib/api";
import { RemoteActions } from "./actions.svelte";
import type { RemoteApi } from "./api";
import { REMOTE_KEY } from "./context";
import Queue from "./pages/Queue.svelte";
import { RemoteStore } from "./store.svelte";

function item(id: number): QueueItem {
  return { track_id: id, title: `Song ${id}`, artist: "Artist", album: "Album", duration_secs: 200 };
}

function state(patch: Partial<RemoteState> = {}): RemoteState {
  return {
    loggedIn: true,
    playing: true,
    buffering: false,
    current: { trackId: 3, title: "Song 3", artist: "Artist", album: "Album", cover: null, durationSecs: 200 },
    currentIndex: 2,
    positionMs: 0,
    durationMs: 200_000,
    volume: 0.5,
    shuffle: false,
    repeat: "off",
    queueLen: 5,
    queueRev: 1,
    stream: null,
    bitPerfect: false,
    dspStages: [],
    ...patch,
  };
}

function setup() {
  const items = [1, 2, 3, 4, 5].map(item);
  const store = new RemoteStore({
    call: (async (method: string) => {
      if (method !== "get_queue_page") throw new Error(`unexpected ${method}`);
      return { items, total: 5, offset: 0, limit: 100, currentIndex: 2, queueRev: 1 };
    }) as never,
  });
  store.applyState(state(), true);
  const api = {} as RemoteApi;
  const context = new Map<unknown, unknown>([[REMOTE_KEY, { api, store, actions: new RemoteActions(api) }]]);
  return { store, context };
}

afterEach(cleanup);

describe("phone queue", () => {
  it("marks the playing row and fades the played ones", async () => {
    const { context } = setup();
    render(Queue, { context });
    await screen.findByText("Song 5");
    const rows = screen.getAllByRole("listitem");
    const current = rows.find((row) => row.getAttribute("aria-current") === "true");
    expect(current?.textContent).toContain("Song 3");
    expect(current?.querySelector(".now")).not.toBeNull();
    const played = rows.filter((row) => row.classList.contains("played")).map((row) => row.textContent);
    expect(played).toHaveLength(2);
    expect(played.join(" ")).toContain("Song 1");
    expect(played.join(" ")).toContain("Song 2");
  });

  it("stops the disc while the PC is paused", async () => {
    const { store, context } = setup();
    store.applyState(state({ playing: false }), false);
    render(Queue, { context });
    await screen.findByText("Song 5");
    const now = document.querySelector(".now");
    expect(now?.classList.contains("spinning")).toBe(false);
  });
});
