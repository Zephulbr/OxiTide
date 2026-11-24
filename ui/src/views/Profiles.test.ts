// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { Nav } from "../lib/nav.svelte";
import { clearViewCache } from "../lib/viewCache";
import Profiles from "./Profiles.svelte";

const me = vi.hoisted(() => ({ profile: null as Record<string, unknown> | null }));
/// How the favorites answer: the list, never (a load that does not settle),
/// or an error.
const favorites = vi.hoisted(() => ({ mode: "list" as "list" | "never" | "fail" | "repeat" }));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  const api = new Proxy(
    {},
    {
      get: (_t, name) => {
        if (name === "getFavoriteArtists")
          return async () => {
            if (favorites.mode === "never") return new Promise(() => {});
            if (favorites.mode === "fail") throw new Error("api error 500: down");
            if (favorites.mode === "repeat")
              return [
                { id: 3, name: "Artist One", picture: null },
                { id: 5, name: "Artist Two", picture: null },
                { id: 3, name: "Artist One", picture: null },
              ];
            return [
              { id: 3, name: "Artist One", picture: null },
              { id: 7, name: "Own Artist", picture: null },
            ];
          };
        if (name === "getMyProfile") return async () => me.profile;
        if (name === "getArtist") return async (id: number) => ({ id, name: "Own Artist", picture: "pic" });
        return async () => [];
      },
    },
  );
  return { ...actual, api };
});

beforeEach(() => {
  clearViewCache();
  favorites.mode = "list";
});
afterEach(cleanup);

const nav = () => ({ goto: vi.fn(), route: { name: "profiles" } }) as unknown as Nav & { goto: ReturnType<typeof vi.fn> };

describe("Profiles: the own profile", () => {
  it("leads the grid, opens its artist page and is not listed twice", async () => {
    me.profile = { userId: 42, name: "Vee", artistId: 7 };
    const n = nav();
    render(Profiles, { nav: n });
    const caption = await screen.findByText("Your profile");
    const card = caption.closest(".card") as HTMLElement;
    expect(card.tagName).toBe("BUTTON");
    // First in the grid, and the favorite copy of the same artist is gone.
    expect(card.parentElement?.firstElementChild).toBe(card);
    expect(screen.getAllByText("Own Artist")).toHaveLength(1);
    expect(screen.getByText("1 artist")).toBeTruthy();
    await fireEvent.click(card);
    expect(n.goto).toHaveBeenCalledWith({ name: "artist", id: 7 });
  });

  it("without an artist, shows the profile but links nowhere", async () => {
    me.profile = { userId: 42, name: "Vee" };
    render(Profiles, { nav: nav() });
    const caption = await screen.findByText("Your profile");
    const card = caption.closest(".card") as HTMLElement;
    expect(card.tagName).toBe("DIV");
    expect(screen.getByText("Vee")).toBeTruthy();
    expect(screen.getByText("2 artists")).toBeTruthy();
  });

  it("is left out when TIDAL has no profile", async () => {
    me.profile = null;
    render(Profiles, { nav: nav() });
    await screen.findByText("Artist One");
    await waitFor(() => expect(screen.queryByText("Your profile")).toBeNull());
  });

  it("shows while the favorites are still loading (issue #10)", async () => {
    me.profile = { userId: 42, name: "Vee", artistId: 7 };
    favorites.mode = "never";
    render(Profiles, { nav: nav() });
    const caption = await screen.findByText("Your profile");
    expect(caption.closest(".card")?.tagName).toBe("BUTTON");
    expect(screen.getByRole("status")).toBeTruthy();
    expect(screen.queryByText("Artist One")).toBeNull();
  });

  it("shows over the error when the favorites fail", async () => {
    me.profile = { userId: 42, name: "Vee", artistId: 7 };
    favorites.mode = "fail";
    render(Profiles, { nav: nav() });
    await screen.findByText(/api error 500/);
    expect(screen.getByText("Your profile")).toBeTruthy();
  });

  /// Issue #10: TIDAL's pages can repeat an artist (offset windows over a
  /// date order with ties). A repeated key made the grid throw, leaving the
  /// page on "Loading…" and every later visit dead.
  it("draws a list that repeats an artist, once each", async () => {
    me.profile = null;
    favorites.mode = "repeat";
    render(Profiles, { nav: nav() });
    await screen.findByText("Artist Two");
    expect(screen.getAllByText("Artist One")).toHaveLength(1);
    expect(screen.getByText("2 artists")).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("follows the search filter", async () => {
    me.profile = { userId: 42, name: "Vee" };
    render(Profiles, { nav: nav() });
    await screen.findByText("Your profile");
    await fireEvent.input(screen.getByLabelText("Search artists"), { target: { value: "artist one" } });
    expect(screen.queryByText("Your profile")).toBeNull();
    expect(screen.getByText("Artist One")).toBeTruthy();
  });
});
