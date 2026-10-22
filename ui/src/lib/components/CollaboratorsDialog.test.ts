// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { api } from "../api";
import { copyText } from "../clipboard";
import { collaboratorsDialog } from "../collaborators.svelte";
import CollaboratorsDialog from "./CollaboratorsDialog.svelte";

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return { ...actual, api: { playlistInvite: vi.fn(), playlistMembers: vi.fn() } };
});
vi.mock("../clipboard", () => ({ copyText: vi.fn() }));

const invite = (code: string) => ({
  share_id: `s-${code}`,
  code,
  expires_at: null,
  url: `https://tidal.com/playlist/p-1/invite?code=${code}`,
});

function setup() {
  const toasts = { show: vi.fn() };
  render(CollaboratorsDialog, { toasts } as never);
  return toasts;
}

beforeEach(() => {
  vi.clearAllMocks();
  collaboratorsDialog.reset();
  vi.mocked(api.playlistMembers).mockResolvedValue([
    { id: 42, name: "CheeseMan", picture: null, owner: true },
    { id: 7, name: "Friend", picture: null, owner: false },
  ]);
  vi.mocked(api.playlistInvite).mockResolvedValue(invite("a"));
  vi.mocked(copyText).mockResolvedValue(true);
});
afterEach(cleanup);

describe("CollaboratorsDialog", () => {
  it("renders nothing until a playlist is chosen", () => {
    setup();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("lists the owner and collaborators", async () => {
    setup();
    collaboratorsDialog.open({ uuid: "p-1", title: "Road" });
    await screen.findByText("Friend");
    expect(api.playlistMembers).toHaveBeenCalledWith("p-1");
    expect(screen.getByText("Owner")).toBeTruthy();
    expect(screen.getByText("CH")).toBeTruthy();
  });

  it("copies the invitation link and only then offers Reset link", async () => {
    const toasts = setup();
    collaboratorsDialog.open({ uuid: "p-1", title: "Road" });
    expect(await screen.findByText("Copy invitation link")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Reset link" })).toBeNull();
    await fireEvent.click(screen.getByText("Copy invitation link"));
    await waitFor(() => expect(copyText).toHaveBeenCalledWith("https://tidal.com/playlist/p-1/invite?code=a"));
    expect(api.playlistInvite).toHaveBeenCalledWith("p-1", false);
    expect(toasts.show).toHaveBeenCalledWith("Invitation link copied", { severity: "success" });
    expect(await screen.findByRole("button", { name: "Reset link" })).toBeTruthy();
  });

  it("asks before resetting, then renews the link", async () => {
    setup();
    collaboratorsDialog.markLink("p-1");
    collaboratorsDialog.open({ uuid: "p-1", title: "Road" });
    vi.mocked(api.playlistInvite).mockResolvedValue(invite("b"));
    await fireEvent.click(await screen.findByRole("button", { name: "Reset link" }));
    expect(screen.getByText("The current link will stop working.")).toBeTruthy();
    expect(api.playlistInvite).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Reset link" }));
    await waitFor(() => expect(api.playlistInvite).toHaveBeenCalledWith("p-1", true));
    await waitFor(() => expect(copyText).toHaveBeenCalledWith("https://tidal.com/playlist/p-1/invite?code=b"));
  });

  it("reports a failed copy", async () => {
    vi.mocked(api.playlistInvite).mockRejectedValue("forbidden");
    const toasts = setup();
    collaboratorsDialog.open({ uuid: "p-1", title: "Road" });
    await fireEvent.click(await screen.findByText("Copy invitation link"));
    await waitFor(() =>
      expect(toasts.show).toHaveBeenCalledWith(expect.any(String), { severity: "error" }),
    );
    expect(copyText).not.toHaveBeenCalled();
  });
});
