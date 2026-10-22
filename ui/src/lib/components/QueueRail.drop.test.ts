// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { api, type Track } from "../api";
import { appearance } from "../appearance.svelte";
import { dragDrop, dragSource } from "../dragDrop.svelte";
import type { DragPayload } from "../dragPayload";
import { queueRowHeightFor } from "../rowHeight";
import { fakePlayer, queueItem } from "../test/fakes";
import type { Toasts } from "../toasts.svelte";
import QueueRail from "./QueueRail.svelte";

vi.mock("../api", () => ({
  api: {
    playTracks: vi.fn(async () => {}),
    queueAdd: vi.fn(async () => {}),
    queueInsert: vi.fn(async () => {}),
    getTrackFlags: vi.fn(async () => []),
  },
  trackToQueueItem: (t: Track) => ({ track_id: t.id, title: t.title }),
}));

globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

const dropped = [
  { id: 8, title: "New A", artists: [], explicit: false },
  { id: 9, title: "New B", artists: [], explicit: false },
] as unknown as Track[];
const payload: DragPayload = { kind: "tracks", tracks: dropped };

function pointer(type: string, target: EventTarget, x = 0, y = 0) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 });
  Object.defineProperty(event, "pointerType", { value: "mouse" });
  target.dispatchEvent(event);
}

let cleanups: (() => void)[] = [];

function setup(queueLength: number) {
  const queue = Array.from({ length: queueLength }, (_, i) => queueItem(i + 1));
  const toasts = { show: vi.fn() } as unknown as Toasts;
  render(QueueRail, { player: fakePlayer({ queue }), toasts });
  const source = document.createElement("div");
  document.body.append(source);
  const handle = dragSource(source, () => payload);
  cleanups.push(() => handle.destroy());
  const rail = document.querySelector<HTMLElement>(".queue-rail")!;
  document.elementFromPoint = vi.fn(() => rail);
  return { source, toasts };
}

/// Drag onto the rail and release at `y` (rail content px; jsdom lays
/// everything out at 0).
async function dropAt(source: HTMLElement, y: number) {
  pointer("pointerdown", source);
  pointer("pointermove", window, 10, y);
  pointer("pointerup", window, 10, y);
  await vi.waitFor(() => expect(dragDrop.pending.size).toBe(0));
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "requestAnimationFrame", "cancelAnimationFrame"] });
});

afterEach(() => {
  dragDrop.cancel();
  pointer("pointerup", window);
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
  for (const done of cleanups) done();
  cleanups = [];
  cleanup();
});

describe("QueueRail drops", () => {
  it("inserts between rows at the slot under the pointer", async () => {
    const { source, toasts } = setup(3);
    const rowH = queueRowHeightFor(appearance.density);
    await dropAt(source, rowH * 1);
    await vi.waitFor(() => expect(api.queueInsert).toHaveBeenCalled());
    expect(api.queueInsert).toHaveBeenCalledWith(1, [
      { track_id: 8, title: "New A" },
      { track_id: 9, title: "New B" },
    ]);
    expect(api.queueAdd).not.toHaveBeenCalled();
    expect(toasts.show).toHaveBeenCalledWith("Added 2 tracks to the queue");
  });

  it("appends below the last row", async () => {
    const { source } = setup(2);
    await dropAt(source, 5000);
    await vi.waitFor(() => expect(api.queueAdd).toHaveBeenCalled());
    expect(api.queueInsert).not.toHaveBeenCalled();
  });

  it("appends to an empty queue", async () => {
    const { source } = setup(0);
    await dropAt(source, 0);
    await vi.waitFor(() => expect(api.queueAdd).toHaveBeenCalled());
  });

  it("shows the insertion line while over the list", async () => {
    const { source } = setup(3);
    pointer("pointerdown", source);
    pointer("pointermove", window, 10, 40);
    await vi.waitFor(() => expect(document.querySelector(".q-drop-line")).not.toBeNull());
    expect(dragDrop.verdict).toEqual({ ok: true, hint: "Add to queue" });
  });
});
