// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import type { UpdateSupport } from "../api";

const mocks = vi.hoisted(() => ({
  api: {
    getUpdateSupport: vi.fn(),
    checkForUpdate: vi.fn(),
    installUpdate: vi.fn(),
    setUpdateCheckOnStart: vi.fn(async () => {}),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../api", () => ({ api: mocks.api, UPDATE_PROGRESS_EVENT: "update_progress" }));

import UpdatesSection from "./UpdatesSection.svelte";

function support(patch: Partial<UpdateSupport> = {}): UpdateSupport {
  return { supported: true, reason: null, current_version: "0.2.0", check_on_start: true, ...patch };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Settings → Updates", () => {
  it("reports an up-to-date build", async () => {
    mocks.api.getUpdateSupport.mockResolvedValue(support());
    mocks.api.checkForUpdate.mockResolvedValue(null);
    render(UpdatesSection);
    await fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("You are on the latest version.")).toBeTruthy();
  });

  async function offerRelease() {
    mocks.api.getUpdateSupport.mockResolvedValue(support());
    mocks.api.checkForUpdate.mockResolvedValue({
      version: "0.3.0",
      current_version: "0.2.0",
      notes: "Fixes",
      date: null,
    });
    mocks.api.installUpdate.mockReturnValue(new Promise(() => {}));
    render(UpdatesSection);
    await fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("Version 0.3.0 is available.")).toBeTruthy();
    expect(screen.getByText("Fixes")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Download and install" }));
  }

  it("installs a newer release only after the GitHub download is confirmed", async () => {
    await offerRelease();
    const dialog = await screen.findByRole("dialog", { name: "Install OxiTide 0.3.0?" });
    expect(dialog.textContent).toContain("github.com/Zephulbr/OxiTide");
    expect(mocks.api.installUpdate).not.toHaveBeenCalled();
    await fireEvent.click(within(dialog).getByRole("button", { name: "Download and install" }));
    expect(mocks.api.installUpdate).toHaveBeenCalledOnce();
    expect(await screen.findByRole("status")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("cancelling the confirmation downloads nothing", async () => {
    await offerRelease();
    const dialog = await screen.findByRole("dialog");
    await fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(mocks.api.installUpdate).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Download and install" })).toBeTruthy();
  });

  it("explains an unsupported install instead of offering buttons", async () => {
    mocks.api.getUpdateSupport.mockResolvedValue(
      support({ supported: false, reason: "Updates are managed by your package manager for this install." }),
    );
    render(UpdatesSection);
    expect(await screen.findByText(/package manager/)).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows a failed check and stays usable", async () => {
    mocks.api.getUpdateSupport.mockResolvedValue(support());
    mocks.api.checkForUpdate.mockRejectedValue("update: could not reach the server");
    render(UpdatesSection);
    await fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    expect((await screen.findByRole("alert")).textContent).toContain("could not reach the server");
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy();
  });

  it("saves the startup-check toggle and reverts on failure", async () => {
    mocks.api.getUpdateSupport.mockResolvedValue(support());
    render(UpdatesSection);
    const box = (await screen.findByRole("checkbox")) as HTMLInputElement;
    expect(box.checked).toBe(true);
    await fireEvent.click(box);
    expect(mocks.api.setUpdateCheckOnStart).toHaveBeenCalledWith(false);
    mocks.api.setUpdateCheckOnStart.mockRejectedValueOnce("disk full");
    await fireEvent.click(box);
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("disk full"));
  });
});
