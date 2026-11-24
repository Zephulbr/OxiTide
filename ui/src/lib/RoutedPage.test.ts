// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import RoutedPage from "./RoutedPage.svelte";

const logged = vi.hoisted(() => [] as Array<[string, string]>);
const sorting = vi.hoisted(() => ({ throws: true }));

vi.mock("./api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./api")>();
  const api = new Proxy(
    {},
    {
      get: (_t, name) => {
        if (name === "getFavoriteArtists") return async () => [{ id: 3, name: "Artist One", picture: null }];
        if (name === "getMyProfile") return async () => null;
        if (name === "logPageError")
          return async (page: string, message: string) => void logged.push([page, message]);
        return async () => [];
      },
    },
  );
  return { ...actual, api };
});

// A page that throws while drawing: Profiles' list, once it arrives.
vi.mock("./artistSort", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./artistSort")>();
  return {
    ...actual,
    visibleArtists: (...args: Parameters<typeof actual.visibleArtists>) => {
      if (sorting.throws && args[0].length > 0) throw new Error("drawing broke");
      return actual.visibleArtists(...args);
    },
  };
});

afterEach(cleanup);

const props = () =>
  ({
    nav: { route: { name: "profiles" }, entry: 0, nowPlayingOpen: false, goto: vi.fn() },
    player: { state: { playing: false } },
    favorites: {},
    playlistActions: {},
    collection: {},
    collectionActions: {},
    toasts: { show: vi.fn() },
    qualityDisplay: "badge",
    onLogout: vi.fn(),
    onQualityDisplay: vi.fn(),
  }) as unknown as Parameters<typeof RoutedPage>[1];

describe("RoutedPage: a page that fails to draw", () => {
  it("shows the error with Retry, logs it, and draws again on Retry (issue #10)", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    render(RoutedPage, { props: props() });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("drawing broke");
    expect(logged).toContainEqual(["profiles", "drawing broke"]);

    sorting.throws = false;
    await fireEvent.click(screen.getByRole("button", { name: /retry/i }));
    expect(await screen.findByText("Artist One")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
