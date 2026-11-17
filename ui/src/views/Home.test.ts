// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { homeSearch } from "../lib/homeSearch.svelte";
import type { Nav } from "../lib/nav.svelte";
import { searchHistory } from "../lib/searchHistory.svelte";
import type { Toasts } from "../lib/toasts.svelte";
import { fakePlayer } from "../lib/test/fakes";
import Home from "./Home.svelte";

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const api = new Proxy(
    {},
    {
      get: (_target, name) => {
        const item = (id: string, type: string) => ({
          id,
          title: `T${id}`,
          subtitle: "S",
          item_type: type,
          artwork: null,
        });
        const section = (title: string, type: string) => ({
          title,
          kind: "HORIZONTAL_LIST",
          items: [item(`${title}1`, type), item(`${title}2`, type)],
        });
        if (name === "getHome") return async () => [section("Editorial", "ALBUM")];
        if (name === "getForYou") return async () => [section("Mixes", "MIX")];
        if (name === "getUserPlaylists")
          return async () => [{ uuid: "p1", title: "Mine", numberOfTracks: 3, image: null, squareImage: null }];
        if (name === "getFeed")
          return async () => [{ id: 9, title: "New", subtitle: "A", artists: [], seen: false, releaseType: "ALBUM" }];
        if (name === "getHistoryPage") return async () => ({ items: [], total: 0 });
        return async () => [];
      },
    },
  );
  return { ...actual, api };
});

beforeAll(() => {
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
  });
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  globalThis.IntersectionObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof IntersectionObserver;
});

afterEach(cleanup);

describe("Home reveal", () => {
  it("leaves the pending state once the personal sources settle", async () => {
    render(Home, {
      nav: { goto: vi.fn() } as unknown as Nav,
      player: fakePlayer(),
      toasts: { show: vi.fn() } as unknown as Toasts,
    });
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull(), { timeout: 1000 });
    expect(screen.getByText("Made for you")).toBeTruthy();
    expect(screen.getByText("From TIDAL")).toBeTruthy();
  });
});

describe("Home search field", () => {
  function renderHome() {
    const goto = vi.fn();
    render(Home, {
      nav: { goto } as unknown as Nav,
      player: fakePlayer(),
      toasts: { show: vi.fn() } as unknown as Toasts,
    });
    return goto;
  }

  it("opens Explore with the submitted query", async () => {
    const goto = renderHome();
    const field = screen.getByRole("combobox");
    await fireEvent.input(field, { target: { value: "  daft punk " } });
    await fireEvent.keyDown(field, { key: "Enter" });
    expect(goto).toHaveBeenCalledWith({ name: "explore", q: "daft punk" });
  });

  it("does nothing on a blank Enter", async () => {
    const goto = renderHome();
    const field = screen.getByRole("combobox");
    await fireEvent.input(field, { target: { value: "   " } });
    await fireEvent.keyDown(field, { key: "Enter" });
    expect(goto).not.toHaveBeenCalled();
  });

  it("opens Explore from a picked recent search", async () => {
    searchHistory().record("Bonobo");
    const goto = renderHome();
    const field = screen.getByRole("combobox");
    await fireEvent.focus(field);
    await fireEvent.click(screen.getByRole("option", { name: /Bonobo/ }));
    expect(goto).toHaveBeenCalledWith({ name: "explore", q: "Bonobo" });
  });

  it("is hidden when turned off in Settings", () => {
    homeSearch().setShown(false);
    try {
      renderHome();
      expect(screen.queryByRole("combobox")).toBeNull();
    } finally {
      homeSearch().setShown(true);
    }
  });
});
