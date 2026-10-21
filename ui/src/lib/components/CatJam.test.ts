// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { flushSync } from "svelte";
import type { EngineEvent } from "../api";
import type { Player } from "../player.svelte";

const mocks = vi.hoisted(() => ({
  api: { setBeatTracking: vi.fn(async () => {}) },
  handlers: [] as ((e: { payload: EngineEvent }) => void)[],
  unlisten: vi.fn(),
  renderSheet: vi.fn((coat: string) => `data:image/png;base64,${coat}`),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_name: string, handler: (e: { payload: EngineEvent }) => void) => {
    mocks.handlers.push(handler);
    return mocks.unlisten;
  }),
}));
vi.mock("../api", () => ({ api: mocks.api }));
vi.mock("../catjamSprite", async (original) => ({
  ...(await original<typeof import("../catjamSprite")>()),
  renderSheet: mocks.renderSheet,
}));

import CatJam from "./CatJam.svelte";

function fakePlayer(playing: boolean, trackId = 1): Player {
  return {
    state: { playing, position_ms: 0, current: { track_id: trackId } },
  } as unknown as Player;
}

const send = (payload: EngineEvent) => mocks.handlers.forEach((h) => h({ payload }));
const beat = (track_id: number): EngineEvent => ({
  type: "beat",
  track_id,
  bpm: 120,
  period_ms: 500,
  anchor_ms: 0,
  confidence: 0.9,
});

let raf: ReturnType<typeof vi.fn>;

beforeEach(() => {
  mocks.handlers.length = 0;
  raf = vi.fn(() => 1);
  vi.stubGlobal("requestAnimationFrame", raf);
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

async function mount(player: Player, coat = "white") {
  const view = render(CatJam, { player, coat: coat as never });
  await vi.waitFor(() => expect(mocks.handlers.length).toBe(1));
  return view;
}

describe("CatJam", () => {
  it("turns beat tracking on while mounted and off when removed", async () => {
    const view = await mount(fakePlayer(true));
    expect(mocks.api.setBeatTracking).toHaveBeenCalledWith(true);
    view.unmount();
    expect(mocks.api.setBeatTracking).toHaveBeenLastCalledWith(false);
    expect(mocks.unlisten).toHaveBeenCalled();
  });

  it("is hidden from assistive tech", async () => {
    const { container } = await mount(fakePlayer(true));
    const cat = container.querySelector(".catjam") as HTMLElement;
    expect(cat.getAttribute("aria-hidden")).toBe("true");
  });

  it("ignores a clock for another track", async () => {
    await mount(fakePlayer(true, 1));
    send(beat(2));
    expect(raf).not.toHaveBeenCalled();
    send(beat(1));
    expect(raf).toHaveBeenCalled();
  });

  it("runs no animation loop while paused", async () => {
    await mount(fakePlayer(false));
    send({ type: "position", position_ms: 1000, duration_ms: 9000, lead_ms: 200 });
    send(beat(1));
    expect(raf).not.toHaveBeenCalled();
  });

  it("shows the idle frame until a clock arrives, then a bob frame", async () => {
    const { container } = await mount(fakePlayer(true));
    const cat = container.querySelector(".catjam") as HTMLElement;
    expect(cat.style.backgroundPosition).toBe("-320px 0px");
    send(beat(1));
    expect(cat.style.backgroundPosition).not.toBe("-320px 0px");
  });

  it("draws at the chosen size", async () => {
    const view = render(CatJam, { player: fakePlayer(true), coat: "white", scale: 150 });
    await vi.waitFor(() => expect(mocks.handlers.length).toBe(1));
    const cat = view.container.querySelector(".catjam") as HTMLElement;
    expect(cat.style.width).toBe("48px");
    expect(cat.style.height).toBe("48px");
    expect(cat.style.backgroundSize).toBe("528px 48px");
    expect(cat.style.backgroundPosition).toBe("-480px 0px");
  });

  it("draws the chosen coat and redraws when it changes", async () => {
    const { container, rerender } = await mount(fakePlayer(true), "tuxedo");
    const cat = container.querySelector(".catjam") as HTMLElement;
    expect(mocks.renderSheet).toHaveBeenLastCalledWith("tuxedo");
    expect(cat.style.backgroundImage).toContain("tuxedo");
    await rerender({ coat: "ginger" as never });
    flushSync();
    expect(mocks.renderSheet).toHaveBeenLastCalledWith("ginger");
    expect(cat.style.backgroundImage).toContain("ginger");
  });
});
