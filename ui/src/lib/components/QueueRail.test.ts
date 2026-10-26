// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { fakePlayer, queueItem } from "../test/fakes";
import QueueRail from "./QueueRail.svelte";
import { api } from "../api";

vi.mock("../api", () => ({
  api: { playTracks: vi.fn(async () => {}) },
}));

// jsdom has no ResizeObserver; `bind:clientHeight` only needs it to exist.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

afterEach(cleanup);
beforeEach(() => vi.clearAllMocks());

describe("QueueRail", () => {
  it("renders the queue as a labelled list with set positions", () => {
    const queue = [queueItem(1), queueItem(2), queueItem(3)];
    render(QueueRail, { player: fakePlayer({ queue }) });

    const list = screen.getByRole("list");
    const rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(rows.every((row) => row.parentElement === list)).toBe(true);
    expect(rows[0].getAttribute("aria-posinset")).toBe("1");
    expect(rows[0].getAttribute("aria-setsize")).toBe("3");
    // Each item holds two sibling controls, never one control inside another.
    const play = screen.getByRole("button", { name: "Track 1 — Artist" });
    expect(play.parentElement).toBe(rows[0]);
    expect(screen.getByRole("button", { name: "Remove Track 1" }).parentElement).toBe(rows[0]);
    expect(rows[0].querySelector("button button")).toBeNull();
  });

  it("plays a row from its main button", async () => {
    const queue = [queueItem(1), queueItem(2)];
    render(QueueRail, { player: fakePlayer({ queue }) });

    await fireEvent.click(screen.getByRole("button", { name: "Track 2 — Artist" }));

    expect(api.playTracks).toHaveBeenCalledWith(queue, 1);
  });

  it("marks only the engine's current position when a track is queued twice", () => {
    const queue = [queueItem(1), queueItem(2), queueItem(1)];
    render(QueueRail, { player: fakePlayer({ queue, current: queue[2], current_index: 2 }) });

    const rows = screen.getAllByRole("listitem");
    expect(rows.map((row) => row.getAttribute("aria-current"))).toEqual([null, null, "true"]);
    expect(rows[0].classList.contains("played")).toBe(true);
    expect(rows[1].classList.contains("played")).toBe(true);
    expect(rows[2].classList.contains("played")).toBe(false);
  });

  it("summarizes what is left to play", () => {
    const queue = [queueItem(1), queueItem(2), queueItem(3)];
    // 2 upcoming × 200 s + 30 s left of the current track = 430 s.
    render(QueueRail, { player: fakePlayer({ queue, current_index: 0 }) });

    expect(screen.getByText("2 up next · 7 min left")).toBeTruthy();
  });

  it("opens a row's artist and album from its names without playing it", async () => {
    const item = { ...queueItem(2), artist_refs: [{ id: 7, name: "Artist" }], album_id: 9 };
    const go = vi.fn();
    render(QueueRail, { player: fakePlayer({ queue: [queueItem(1), item] }), go });

    await fireEvent.click(screen.getAllByText("Artist")[1]);
    expect(go).toHaveBeenCalledWith({ name: "artist", id: 7 });
    await fireEvent.click(screen.getByText("Track 2"));
    await vi.waitFor(() => expect(go).toHaveBeenLastCalledWith({ name: "album", id: 9 }));
    expect(api.playTracks).not.toHaveBeenCalled();
    // Nested in the play button: pointer-only spans, never a link role.
    expect(screen.queryAllByRole("link")).toHaveLength(0);
  });

  it("labels the first upcoming Autoplay suggestion once", () => {
    const queue = [
      queueItem(1),
      { ...queueItem(2), autoplay: true },
      { ...queueItem(3), autoplay: true },
      { ...queueItem(4), autoplay: true },
    ];
    // The current track is itself a suggestion: the label goes on the next.
    render(QueueRail, { player: fakePlayer({ queue, current: queue[1], current_index: 1 }) });

    const labels = document.querySelectorAll(".q-autoplay");
    expect(labels).toHaveLength(1);
    expect(labels[0].closest("[data-q-index]")?.getAttribute("data-q-index")).toBe("2");
    expect(screen.getByRole("button", { name: "Track 3 — Artist (Autoplay)" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Track 1 — Artist" })).toBeTruthy();
  });

  it("shows no Autoplay label without suggestions to come", () => {
    const queue = [queueItem(1), { ...queueItem(2), autoplay: true }];
    render(QueueRail, { player: fakePlayer({ queue, current: queue[1], current_index: 1 }) });
    expect(document.querySelector(".q-autoplay")).toBeNull();
  });

  it("keeps names as plain text without a go handler", () => {
    render(QueueRail, { player: fakePlayer({ queue: [queueItem(1)] }) });
    expect(document.querySelector(".name-link")).toBeNull();
  });

  it("can hide its heading for hosts that draw their own", () => {
    render(QueueRail, { player: fakePlayer({ queue: [queueItem(1)] }), title: false });
    expect(screen.queryByRole("heading", { name: "Queue" })).toBeNull();
  });

  it("moves a row with Alt+Arrow and shows the new order at once", async () => {
    const queue = [queueItem(1), queueItem(2), queueItem(3)];
    const player = fakePlayer({ queue, current_index: 0 });
    const reorder = vi.fn(async () => {});
    Object.assign(player, { reorder });
    render(QueueRail, { player });

    await fireEvent.keyDown(screen.getByRole("button", { name: "Track 2 — Artist" }), {
      key: "ArrowDown",
      altKey: true,
    });

    expect(reorder).toHaveBeenCalledWith(1, 2);
    const titles = screen.getAllByRole("listitem").map((row) => row.querySelector(".title")?.textContent);
    expect(titles).toEqual(["Track 1", "Track 3", "Track 2"]);
  });

  it("shows an empty state with no list", () => {
    render(QueueRail, { player: fakePlayer({ queue: [], current: null, current_index: null }) });
    expect(screen.queryByRole("list")).toBeNull();
    expect(screen.getByText("Your queue is empty")).toBeTruthy();
  });
});
