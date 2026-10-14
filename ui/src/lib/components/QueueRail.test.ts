// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
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
});
