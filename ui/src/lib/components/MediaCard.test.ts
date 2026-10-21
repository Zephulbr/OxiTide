// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import type { PageItem } from "../api";
import type { CardActions } from "../cardActions";
import MediaCard from "./MediaCard.svelte";

const fake = vi.hoisted(() => ({ actions: undefined as CardActions | undefined }));

vi.mock("../api", () => ({
  api: { analyzeCover: vi.fn(async () => ({ primary: "#111", secondary: "#222", tertiary: "#333" })) },
  trackToQueueItem: vi.fn(),
}));

vi.mock("../cardActions", async (importOriginal) => {
  const original = await importOriginal<typeof import("../cardActions")>();
  return { ...original, getCardActions: () => fake.actions };
});

afterEach(() => {
  cleanup();
  fake.actions = undefined;
});

function actions(): CardActions {
  return { play: vi.fn(async () => {}), openArtist: vi.fn(), menu: vi.fn(() => []) };
}

const album: PageItem = {
  id: "7",
  title: "Evangelion Finally",
  item_type: "ALBUM",
  artwork: null,
  artist_refs: [{ id: 3, name: "Arianne" }],
};

describe("MediaCard collection cards", () => {
  it("splits into open, play, title and artist targets", async () => {
    const a = actions();
    fake.actions = a;
    const open = vi.fn();
    render(MediaCard, { item: album, variant: "carousel", onclick: open });

    await fireEvent.click(screen.getByRole("button", { name: "Open Evangelion Finally" }));
    expect(open).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByText("Evangelion Finally"));
    expect(open).toHaveBeenCalledTimes(2);

    await fireEvent.click(screen.getByRole("link", { name: "Arianne" }));
    expect(a.openArtist).toHaveBeenCalledWith(3);

    await fireEvent.click(screen.getByRole("button", { name: "Play Evangelion Finally" }));
    expect(a.play).toHaveBeenCalledWith({ kind: "album", id: 7, title: "Evangelion Finally" });
    expect(open).toHaveBeenCalledTimes(2);
  });

  it("opens the context menu with the card's items", async () => {
    const a = actions();
    fake.actions = a;
    render(MediaCard, { item: album, variant: "tile", onclick: vi.fn() });
    await fireEvent.contextMenu(screen.getByRole("group", { name: "Evangelion Finally" }));
    expect(a.menu).toHaveBeenCalledWith(album, { kind: "album", id: 7, title: "Evangelion Finally" });
  });

  it("stays a single button without a provider, for tracks, and for action cards", () => {
    render(MediaCard, { item: album, variant: "tile", onclick: vi.fn() });
    expect(screen.queryByRole("group")).toBeNull();
    cleanup();

    fake.actions = actions();
    render(MediaCard, {
      item: { id: "9", title: "Song", item_type: "TRACK" },
      variant: "tile",
      onclick: vi.fn(),
    });
    expect(screen.queryByRole("group")).toBeNull();
    cleanup();

    render(MediaCard, {
      item: { id: "artist-1", title: "A Radio", item_type: "MIX_LIST" },
      variant: "row",
      playButton: true,
      onclick: vi.fn(),
    });
    expect(screen.queryByRole("group")).toBeNull();
  });
});
