// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { fakePlayer, queueItem } from "../test/fakes";
import { api } from "../api";
import ChangerUnit from "./ChangerUnit.svelte";

vi.mock("../api", () => ({
  api: { playTracks: vi.fn(async () => {}) },
}));

// jsdom has no ResizeObserver; QueueRail's `bind:clientHeight` only needs it to exist.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

afterEach(cleanup);

const queue = Array.from({ length: 7 }, (_, i) => queueItem(i + 1));

describe("ChangerUnit", () => {
  it("shows the page of disc keys holding the current track", () => {
    render(ChangerUnit, { player: fakePlayer({ queue, current: queue[5], current_index: 5 }), onClose: vi.fn() });
    const discs = screen.getByRole("group", { name: "Discs" }).querySelectorAll("button");
    expect([...discs].map((d) => d.textContent?.trim())).toEqual(["6", "7", "8", "9", "10"]);
    expect(discs[0].getAttribute("aria-current")).toBe("true");
    expect((discs[2] as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Next discs" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("pages back and plays a disc from the queue", async () => {
    render(ChangerUnit, { player: fakePlayer({ queue, current: queue[5], current_index: 5 }), onClose: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: "Previous discs" }));
    await fireEvent.click(screen.getByRole("button", { name: "Play disc 2: Track 2" }));
    expect(api.playTracks).toHaveBeenCalledWith(queue, 1);
  });

  it("closes from its head key", async () => {
    const onClose = vi.fn();
    render(ChangerUnit, { player: fakePlayer({ queue }), onClose });
    await fireEvent.click(screen.getByRole("button", { name: "Close queue" }));
    expect(onClose).toHaveBeenCalled();
  });
});
