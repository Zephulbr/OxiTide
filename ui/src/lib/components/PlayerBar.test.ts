// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { fakePlayer, queueItem } from "../test/fakes";
import { api } from "../api";
import { dragDrop } from "../dragDrop.svelte";
import { menu } from "../menu.svelte";
import { playSource } from "../playSource.svelte";
import type { Favorites } from "../favorites.svelte";
import type { Mini } from "../mini.svelte";
import type { Nav, Route } from "../nav.svelte";
import { locatePlaying } from "../locatePlaying.svelte";
import type { SleepTimer } from "../sleepTimer.svelte";
import ContextMenu from "./ContextMenu.svelte";
import PlayerBar from "./PlayerBar.svelte";

vi.mock("../api", () => ({
  api: {
    getTrack: vi.fn(async () => ({ album: { id: 42 } })),
    setPlaySource: vi.fn(async () => {}),
    setPreferredQuality: vi.fn(async () => {}),
  },
}));

function setup(
  state: Parameters<typeof fakePlayer>[0] = {},
  qualityDisplay: "compact" | "detailed" = "detailed",
  route: Route = { name: "home" },
  favorite = false,
) {
  const nav = {
    route,
    queueOpen: false,
    goto: vi.fn(),
    closeNowPlaying: vi.fn(),
    toggleNowPlaying: vi.fn(),
    toggleQueue: vi.fn(),
  } as unknown as Nav;
  render(PlayerBar, {
    player: fakePlayer(state),
    qualityDisplay,
    favorites: { hasTrack: () => favorite, toggleTrack: vi.fn() } as unknown as Favorites,
    nav,
    mini: { enter: vi.fn() } as unknown as Mini,
    sleepTimer: { mode: "off", remainingMs: null, totalMinutes: null } as unknown as SleepTimer,
  });
  return { nav };
}

afterEach(() => {
  cleanup();
  menu.close(false);
  playSource.set(null);
});
beforeEach(() => vi.clearAllMocks());

describe("PlayerBar: From line", () => {
  it("names the source that queued the track and opens it", async () => {
    playSource.set({ kind: "playlist", name: "Road trip", route: { name: "playlist", id: "p1" } }, [1]);
    const { nav } = setup();

    expect(screen.getByText("From playlist")).toBeTruthy();
    await fireEvent.click(screen.getByText("Road trip"));
    expect(nav.goto).toHaveBeenCalledWith({ name: "playlist", id: "p1" });
  });

  it("shows the playing track instead when the source's page is already open", async () => {
    playSource.set({ kind: "playlist", name: "Road trip", route: { name: "playlist", id: "p1" } }, [1]);
    const { nav } = setup({}, "detailed", { name: "playlist", id: "p1" });
    const before = locatePlaying.requests;

    await fireEvent.click(screen.getByText("Road trip"));

    expect(nav.goto).not.toHaveBeenCalled();
    expect(nav.closeNowPlaying).toHaveBeenCalled();
    expect(locatePlaying.requests).toBe(before + 1);
  });

  it("credits an Autoplay suggestion to Autoplay, not its album", () => {
    playSource.set({ kind: "playlist", name: "Road trip", route: { name: "playlist", id: "p1" } }, [1]);
    setup({ current: { ...queueItem(1), autoplay: true } });

    expect(screen.getByText("Autoplay")).toBeTruthy();
    expect(screen.queryByText("Road trip")).toBeNull();
    expect(screen.queryByText("From album")).toBeNull();
  });

  it("falls back to the track's album when the source did not queue it", () => {
    playSource.set({ kind: "playlist", name: "Road trip", route: { name: "playlist", id: "p1" } }, [9]);
    setup();

    expect(screen.getByText("From album")).toBeTruthy();
    expect(screen.queryByText("Road trip")).toBeNull();
    expect(screen.getAllByText("Album").length).toBeGreaterThan(0);
  });
});

describe("PlayerBar: track menu", () => {
  it("opens the track's context menu on a right-click on its names", async () => {
    setup();
    await fireEvent.contextMenu(screen.getByText("Track 1"));

    const labels = menu.open?.items.map((item) => item.label) ?? [];
    expect(labels).toContain("Favorite");
    expect(labels).toContain("Go to album");
    expect(labels).toContain("Copy title");
    expect(labels).toContain("Track info");
    // No toasts / picker passed: those entries are left out.
    expect(labels).not.toContain("Track Radio");
    expect(labels).not.toContain("Add to playlist");
  });
});

describe("PlayerBar: drag", () => {
  function pointer(type: string, target: EventTarget, x: number, y: number) {
    const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 });
    Object.defineProperty(event, "pointerType", { value: "mouse" });
    target.dispatchEvent(event);
  }

  // A drag's release swallows the next click until a timeout; let it lapse.
  afterEach(async () => {
    dragDrop.cancel();
    await new Promise((resolve) => setTimeout(resolve));
  });

  it("drags the playing track from the cover and from its title", () => {
    setup();
    document.elementFromPoint = vi.fn(() => null);
    for (const from of [screen.getByRole("button", { name: "Open now playing" }), screen.getByText("Track 1")]) {
      pointer("pointerdown", from, 0, 0);
      pointer("pointermove", window, 30, 30);
      expect(dragDrop.active).toMatchObject({ kind: "track-id", id: 1, title: "Track 1" });
      pointer("pointerup", window, 30, 30);
      expect(dragDrop.active).toBeNull();
    }
  });
});

describe("PlayerBar: heart", () => {
  it("breaks the heart on unfavorite only", async () => {
    setup({}, "detailed", { name: "home" }, true);

    expect(document.querySelector(".heart-break")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Unfavorite" }));
    expect(document.querySelectorAll(".heart-break .half")).toHaveLength(2);
  });

  it("does not break it when favoriting", async () => {
    setup();

    await fireEvent.click(screen.getByRole("button", { name: "Favorite" }));
    expect(document.querySelector(".heart-break")).toBeNull();
  });
});

describe("PlayerBar: quality plate", () => {
  const lossless = {
    quality: "lossless" as const,
    codec: "flac",
    sample_rate: 44100,
    bit_depth: 16,
    channels: 2,
    bitrate_kbps: 1004,
  };

  it("shows tier, format and a bit-perfect mark with its tooltip", () => {
    setup({ stream: lossless, preferred_quality: "lossless", bit_perfect: true });

    const plate = screen.getByRole("button", { name: "Streaming quality: LOSSLESS, 16-bit · 44.1 kHz, bit-perfect" });
    expect(plate.textContent).toContain("BP");
    expect(plate.querySelector('[title="Bit Perfect"]')).not.toBeNull();
  });

  it("opens the tier menu upward and switches quality", async () => {
    setup({ stream: lossless, preferred_quality: "lossless", bit_perfect: true });

    await fireEvent.click(screen.getByRole("button", { name: /^Streaming quality/ }));

    const open = menu.open;
    expect(open?.placement).toBe("above");
    expect(open?.align).toBe("end");
    const ids = open?.items.map((item) => item.id);
    expect(ids).toEqual([
      "quality-max",
      "quality-lossless",
      "quality-high",
      "quality-low",
      "quality-now",
      "quality-settings",
    ]);
    expect(open?.items.find((item) => item.id === "quality-lossless")?.selected).toBe(true);
    const now = open?.items.find((item) => item.id === "quality-now");
    expect(now?.note).toBe(true);
    expect(now?.label).toBe("FLAC, Bit-perfect, 16-bit / 44.1 kHz · 1004 kbps");

    open?.items.find((item) => item.id === "quality-max")?.run();
    expect(api.setPreferredQuality).toHaveBeenCalledWith("max");
  });

  it("draws the Now playing note as file type, output path, then format", async () => {
    setup({ stream: lossless, preferred_quality: "lossless", output_mode: "shared" });
    render(ContextMenu, { menu });

    await fireEvent.click(screen.getByRole("button", { name: /^Streaming quality/ }));

    const note = document.getElementById("menu-item-quality-now");
    expect(note?.tagName).toBe("DIV");
    expect(note?.querySelector(".np-codec")?.textContent).toBe("FLAC");
    expect(note?.querySelector(".np-out")?.textContent?.trim()).toBe("Shared output");
    expect(note?.querySelector(".np-spec")?.textContent).toBe("16-bit / 44.1 kHz · 1004 kbps");
  });

  it("does not restart the track when the current tier is picked again", async () => {
    setup({ stream: lossless, preferred_quality: "lossless" });

    await fireEvent.click(screen.getByRole("button", { name: /^Streaming quality/ }));
    menu.open?.items.find((item) => item.id === "quality-lossless")?.run();

    expect(api.setPreferredQuality).not.toHaveBeenCalled();
  });
});
