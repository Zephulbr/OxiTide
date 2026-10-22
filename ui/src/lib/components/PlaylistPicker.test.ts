// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { SvelteMap } from "svelte/reactivity";
import type { Track } from "../api";
import type { PlaylistActions } from "../playlists.svelte";
import PlaylistPicker from "./PlaylistPicker.svelte";

vi.mock("../api", () => ({ api: { getUserPlaylists: vi.fn(async () => []) } }));

function confirming(from: string | null): PlaylistActions {
  return {
    open: true,
    step: "confirm",
    busy: false,
    confirming: { playlist: { uuid: "p", title: "Road Trip" }, count: 12, from },
    confirmAdd: vi.fn(async () => {}),
    close: vi.fn(),
  } as unknown as PlaylistActions;
}

afterEach(cleanup);

describe("PlaylistPicker: confirm step", () => {
  it("names the count, the dropped collection and the playlist, and adds on Add", async () => {
    const actions = confirming("Blue");
    render(PlaylistPicker, { actions, onnavigate: vi.fn() });

    expect(screen.getByText("Add 12 tracks?")).toBeTruthy();
    expect(screen.getByText("From").nextElementSibling?.textContent).toBe("Blue");
    expect(screen.getByText("To").nextElementSibling?.textContent).toBe("Road Trip");
    await fireEvent.click(screen.getByRole("button", { name: "Add 12 tracks" }));
    expect(actions.confirmAdd).toHaveBeenCalledTimes(1);
  });

  it("leaves out the source for dropped rows, and Cancel closes", async () => {
    const actions = confirming(null);
    render(PlaylistPicker, { actions, onnavigate: vi.fn() });

    expect(screen.queryByText("From")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(actions.close).toHaveBeenCalled();
    expect(actions.confirmAdd).not.toHaveBeenCalled();
  });
});

describe("PlaylistPicker: duplicates step", () => {
  function duplicates(choices: [number, boolean][]) {
    const tracks = choices.map(([id]) => ({ id, title: `T${id}`, artists: [{ name: "A" }] })) as Track[];
    return {
      open: true,
      step: "duplicates",
      busy: false,
      target: { uuid: "p", title: "Road Trip" },
      duplicates: tracks,
      choices: new SvelteMap(choices),
      setChoice: vi.fn(),
      setAllChoices: vi.fn(),
      confirmDuplicates: vi.fn(async () => {}),
      close: vi.fn(),
    } as unknown as PlaylistActions;
  }

  it("names the playlist and counts the copies chosen", async () => {
    const actions = duplicates([
      [1, true],
      [2, false],
    ]);
    render(PlaylistPicker, { actions, onnavigate: vi.fn() });

    expect(screen.getByText("Road Trip")).toBeTruthy();
    const row = screen.getByRole("group", { name: "T2: keep one or add both" });
    await fireEvent.click(row.querySelectorAll("button")[1]);
    expect(actions.setChoice).toHaveBeenCalledWith(2, true);
    // Mixed rows: neither "All" half is pressed.
    const all = screen.getByRole("group", { name: "All duplicates" });
    expect(all.querySelectorAll('[aria-pressed="true"]')).toHaveLength(0);

    await fireEvent.click(screen.getByRole("button", { name: "Add 1 copy" }));
    expect(actions.confirmDuplicates).toHaveBeenCalledTimes(1);
  });

  it("disables adding with no copies chosen; Skip closes", async () => {
    const actions = duplicates([[1, false]]);
    render(PlaylistPicker, { actions, onnavigate: vi.fn() });

    expect((screen.getByRole("button", { name: "Add copies" }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Skip duplicates" }));
    expect(actions.close).toHaveBeenCalled();
  });
});
