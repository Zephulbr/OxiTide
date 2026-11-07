// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { tick } from "svelte";
import type { CollectionActions } from "../collectionActions.svelte";
import type { CollectionTree } from "../collection.svelte";
import { collectionLayout, visibleKeys, hiddenKeys } from "../collectionLayout.svelte";
import type { Nav } from "../nav.svelte";
import type { PlaylistActions } from "../playlists.svelte";
import Sidebar from "./Sidebar.svelte";

vi.mock("../api", () => ({ api: {} }));
vi.mock("../viewData", () => ({ prefetchIntent: () => ({}) }));
vi.mock("../edgeFade", () => ({ edgeFade: () => ({}) }));
vi.mock("../playlistView.svelte", () => ({
  playlistView: () => ({ prefs: { sort: "alpha", dir: "asc", filter: "all" } }),
}));

const layout = collectionLayout();

function setup(rail = false) {
  const nav = { route: { name: "home" }, goto: vi.fn(), canBack: false, canForward: false } as unknown as Nav;
  const collection = { root: [], error: null } as unknown as CollectionTree;
  render(Sidebar, {
    nav,
    rail,
    onToggleRail: () => {},
    collection,
    playlistActions: {} as PlaylistActions,
    actions: {} as CollectionActions,
  });
  return { nav };
}

/// Entry labels in the sidebar's collection, in DOM order.
function entries(): string[] {
  const names = ["Playlists", "Albums", "Tracks", "Save for Later", "Mixes & Radio", "Profiles", "Purchases"];
  return screen
    .queryAllByRole("button")
    .map((b) => b.getAttribute("aria-label") ?? "")
    .filter((label) => names.includes(label));
}

/// Gives the edit list a real geometry: 36px rows on a 38px pitch.
function layOutEditList() {
  const slots = [...document.querySelectorAll<HTMLElement>("[data-slot]")];
  slots.forEach((slot, i) => {
    Object.defineProperty(slot, "offsetTop", { configurable: true, value: i * 38 });
    const row = slot.querySelector<HTMLElement>("[data-row]")!;
    row.getBoundingClientRect = () => ({ top: i * 38, bottom: i * 38 + 36, height: 36 }) as DOMRect;
  });
  const list = document.querySelector<HTMLElement>(".edit-list")!;
  list.getBoundingClientRect = () => ({ top: 0, bottom: slots.length * 38 - 2 }) as DOMRect;
}

function pointer(type: string, target: EventTarget, y: number) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: 10, clientY: y, button: 0 });
  Object.defineProperty(event, "pointerType", { value: "mouse" });
  target.dispatchEvent(event);
}

beforeEach(() => {
  layout.reset();
  layout.setOpen(false);
});

afterEach(() => cleanup());

describe("Sidebar My Collection", () => {
  it("shows the entries above the line and folds the rest under Show more", async () => {
    setup();
    expect(entries()).toEqual(["Playlists", "Albums", "Tracks", "Save for Later"]);
    await fireEvent.click(screen.getByRole("button", { name: "Show more" }));
    expect(entries()).toHaveLength(7);
    expect(screen.getByRole("button", { name: "Show less" })).toBeTruthy();
    expect(layout.prefs.open).toBe(true);
  });

  it("drops Show more when nothing is below the line", () => {
    layout.move(4, 7);
    setup();
    expect(entries()).toHaveLength(7);
    expect(screen.queryByRole("button", { name: /Show (more|less)/ })).toBeNull();
    // The header is a plain label, not a toggle.
    expect(screen.queryByRole("button", { name: "My Collection" })).toBeNull();
  });

  it("moves the toggle to the header when everything is below the line", async () => {
    layout.move(4, 0);
    setup();
    expect(entries()).toEqual([]);
    expect(screen.queryByRole("button", { name: /Show (more|less)/ })).toBeNull();
    const head = screen.getByRole("button", { name: "My Collection" });
    expect(head.getAttribute("aria-expanded")).toBe("false");
    await fireEvent.click(head);
    expect(entries()).toHaveLength(7);
    expect(head.getAttribute("aria-expanded")).toBe("true");
  });

  it("keeps the rail's chevron only while something is hidden", () => {
    setup(true);
    expect(screen.getByRole("button", { name: "Show more" })).toBeTruthy();
    cleanup();
    layout.move(4, 7);
    setup(true);
    expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Edit My Collection" })).toBeNull();
  });

  it("edits with the keyboard: Alt+Down moves an entry below the line", async () => {
    const { nav } = setup();
    await fireEvent.click(screen.getByRole("button", { name: "Edit My Collection" }));
    expect(document.activeElement?.getAttribute("aria-label")).toBe("Done editing My Collection");
    const later = document.querySelector<HTMLElement>('[data-row="later"]')!;
    // Clicking an entry in edit mode never navigates.
    await fireEvent.click(later);
    expect(nav.goto).not.toHaveBeenCalled();
    await fireEvent.keyDown(later, { key: "ArrowDown", altKey: true });
    await tick();
    expect(hiddenKeys(layout.prefs)[0]).toBe("later");
    expect(document.activeElement).toBe(document.querySelector('[data-row="later"]'));
    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    await tick();
    expect(screen.queryByRole("list", { name: "Arrange My Collection" })).toBeNull();
    expect(entries()).toEqual(["Playlists", "Albums", "Tracks"]);
  });

  it("drags the line to the top to hide everything", async () => {
    setup();
    await fireEvent.click(screen.getByRole("button", { name: "Edit My Collection" }));
    layOutEditList();
    const line = document.querySelector<HTMLElement>('[data-row="line"]')!;
    // The line is row 4 (y 152..188); drag it up past every entry.
    pointer("pointerdown", line, 160);
    pointer("pointermove", window, 120);
    pointer("pointermove", window, 0);
    await tick();
    pointer("pointerup", window, 0);
    await tick();
    expect(visibleKeys(layout.prefs)).toEqual([]);
  });

  it("Reset restores the default layout", async () => {
    layout.move(0, 6);
    setup();
    await fireEvent.click(screen.getByRole("button", { name: "Edit My Collection" }));
    await fireEvent.click(screen.getByRole("button", { name: "Reset My Collection to default" }));
    expect(visibleKeys(layout.prefs)).toEqual(["playlists", "albums", "tracks", "later"]);
  });
});
