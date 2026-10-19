import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { atQueueEdge, createHoverIntent } from "./hoverIntent.svelte";

describe("createHoverIntent", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("opens only after the dwell", () => {
    const intent = createHoverIntent({ openMs: 100, closeMs: 200 });
    intent.enter();
    vi.advanceTimersByTime(99);
    expect(intent.open).toBe(false);
    vi.advanceTimersByTime(1);
    expect(intent.open).toBe(true);
  });

  it("a pointer passing through never opens it", () => {
    const intent = createHoverIntent({ openMs: 100, closeMs: 200 });
    intent.enter();
    vi.advanceTimersByTime(50);
    intent.leave();
    vi.advanceTimersByTime(500);
    expect(intent.open).toBe(false);
  });

  it("closes after the grace period, and re-entry within it keeps it open", () => {
    const intent = createHoverIntent({ openMs: 100, closeMs: 200 });
    intent.enter(true);
    expect(intent.open).toBe(true);
    intent.leave();
    vi.advanceTimersByTime(150);
    intent.enter();
    vi.advanceTimersByTime(500);
    expect(intent.open).toBe(true);
    intent.leave();
    vi.advanceTimersByTime(200);
    expect(intent.open).toBe(false);
  });

  it("stays open while held, then closes", () => {
    let held = true;
    const intent = createHoverIntent({ openMs: 100, closeMs: 200, isHeld: () => held });
    intent.enter(true);
    intent.leave();
    vi.advanceTimersByTime(1000);
    expect(intent.open).toBe(true);
    held = false;
    vi.advanceTimersByTime(200);
    expect(intent.open).toBe(false);
  });

  it("reset closes at once and cancels a pending open", () => {
    const intent = createHoverIntent({ openMs: 100, closeMs: 200 });
    intent.enter();
    intent.reset();
    vi.advanceTimersByTime(500);
    expect(intent.open).toBe(false);
  });
});

describe("atQueueEdge", () => {
  const view = { width: 1200, height: 800, top: 32, bottom: 80 };

  it("matches the right edge strip between the title bar and the player bar", () => {
    expect(atQueueEdge(1199, 400, view)).toBe(true);
    expect(atQueueEdge(1192, 400, view)).toBe(true);
    expect(atQueueEdge(1191, 400, view)).toBe(false);
    expect(atQueueEdge(1199, 10, view)).toBe(false);
    expect(atQueueEdge(1199, 750, view)).toBe(false);
  });
});
