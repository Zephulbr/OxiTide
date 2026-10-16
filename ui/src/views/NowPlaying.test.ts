// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { Nav } from "../lib/nav.svelte";
import { fakePlayer } from "../lib/test/fakes";
import NowPlaying from "./NowPlaying.svelte";

vi.mock("../lib/api", () => ({
  api: {
    getLyrics: vi.fn(async (trackId: number) => ({
      track_id: trackId,
      source: "tidal",
      synced: false,
      rtl: false,
      lines: [],
    })),
    getTrack: vi.fn(async () => null),
    getTrackCredits: vi.fn(async () => ({ roles: [] })),
  },
}));

beforeAll(() => {
  // jsdom gaps the view touches at mount: layout observers and the Web
  // Animations API behind Svelte transitions.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  Element.prototype.animate ??= function () {
    return {
      cancel() {},
      finish() {},
      onfinish: null,
      finished: Promise.resolve(),
      currentTime: 0,
      playState: "finished",
    } as unknown as Animation;
  };
  Element.prototype.getAnimations ??= () => [];
});

afterEach(cleanup);

function nav(): Nav {
  return { closeNowPlaying: vi.fn(), goto: vi.fn() } as unknown as Nav;
}

async function renderView() {
  render(NowPlaying, { player: fakePlayer(), nav: nav(), qualityDisplay: "compact" });
  await waitFor(() =>
    expect(document.getElementById("np-tab-lyrics")?.getAttribute("aria-selected")).toBe("true"),
  );
  return screen.getByRole("tablist");
}

describe("NowPlaying tabs", () => {
  it("wires tabs to panels with roving tabindex", async () => {
    await renderView();
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((tab) => tab.id)).toEqual(["np-tab-lyrics", "np-tab-next", "np-tab-info"]);

    for (const tab of tabs) {
      expect(tab.getAttribute("aria-controls")).toMatch(/^np-panel-/);
    }
    const selected = tabs.filter((tab) => tab.getAttribute("aria-selected") === "true");
    expect(selected).toHaveLength(1);
    expect(selected[0].tabIndex).toBe(0);
    const panel = document.getElementById(selected[0].getAttribute("aria-controls")!);
    expect(panel?.getAttribute("role")).toBe("tabpanel");
    expect(panel?.getAttribute("aria-labelledby")).toBe(selected[0].id);
    for (const tab of tabs.filter((candidate) => candidate !== selected[0])) {
      expect(tab.tabIndex).toBe(-1);
    }
  });

  it("moves focus and selection with arrows and Home/End", async () => {
    await renderView();

    await fireEvent.keyDown(document.getElementById("np-tab-lyrics")!, { key: "ArrowDown" });
    expect(document.activeElement).toBe(document.getElementById("np-tab-next"));
    expect(document.getElementById("np-tab-next")!.getAttribute("aria-selected")).toBe("true");

    await fireEvent.keyDown(document.getElementById("np-tab-next")!, { key: "End" });
    expect(document.activeElement).toBe(document.getElementById("np-tab-info"));
    expect(document.getElementById("np-tab-info")!.getAttribute("aria-selected")).toBe("true");

    await fireEvent.keyDown(document.getElementById("np-tab-info")!, { key: "Home" });
    expect(document.activeElement).toBe(document.getElementById("np-tab-lyrics"));
    expect(document.getElementById("np-tab-lyrics")!.getAttribute("aria-selected")).toBe("true");
  });

  it("byline links close Now Playing and open the artist and album", async () => {
    const current = { ...fakePlayer().state.current!, artist_refs: [{ id: 7, name: "Artist" }], album_id: 9 };
    const navigation = nav();
    render(NowPlaying, { player: fakePlayer({ current }), nav: navigation, qualityDisplay: "compact" });

    const byline = document.querySelector(".byline")!;
    const [artist, album] = [...byline.querySelectorAll<HTMLElement>('[role="link"]')];
    expect([artist.textContent, album.textContent]).toEqual(["Artist", "Album"]);

    await fireEvent.click(artist);
    expect(navigation.closeNowPlaying).toHaveBeenCalled();
    expect(navigation.goto).toHaveBeenCalledWith({ name: "artist", id: 7 });
    await fireEvent.keyDown(album, { key: "Enter" });
    await waitFor(() => expect(navigation.goto).toHaveBeenLastCalledWith({ name: "album", id: 9 }));
  });

  it("marks handled key events so the global shortcut handler can ignore them", async () => {
    await renderView();
    const event = new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true });
    document.getElementById("np-tab-lyrics")!.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });
});
