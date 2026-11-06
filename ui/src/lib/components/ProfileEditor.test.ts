// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { profilePicture } from "../profilePicture.svelte";
import ProfileEditor from "./ProfileEditor.svelte";

const calls = vi.hoisted(() => ({
  updates: [] as unknown[],
  uploads: [] as string[],
  removed: 0,
  failUpdate: "",
  bioId: "bio-1" as string | null,
}));

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  const profile = () => ({
    artistId: 7,
    name: "CheeseMan",
    handle: "cheese",
    bio: "Cheese?",
    bioId: calls.bioId,
    links: [{ href: "https://instagram.com/c", kind: "INSTAGRAM" }],
    hasPicture: true,
  });
  return {
    ...actual,
    api: {
      getEditableProfile: async () => profile(),
      updateMyProfile: async (edit: unknown, bio: unknown) => {
        if (calls.failUpdate) throw calls.failUpdate;
        calls.updates.push({ edit, bio });
        return profile();
      },
      uploadMyProfilePicture: async (b64: string) => {
        calls.uploads.push(b64);
        return "https://resources.tidal.com/images/ab/cd/750x750.jpg";
      },
      removeMyProfilePicture: async () => {
        calls.removed += 1;
      },
      openExternal: async () => {},
    },
  };
});

beforeAll(() => {
  URL.createObjectURL ??= () => "blob:preview";
  URL.revokeObjectURL ??= () => {};
});
beforeEach(() => {
  calls.updates = [];
  calls.uploads = [];
  calls.removed = 0;
  calls.failUpdate = "";
  calls.bioId = "bio-1";
  profilePicture.reset();
});
afterEach(cleanup);

function open() {
  const onsaved = vi.fn();
  const onclose = vi.fn();
  render(ProfileEditor, { picture: "pic", onsaved, onclose });
  return { onsaved, onclose };
}

describe("ProfileEditor", () => {
  it("fills the form from TIDAL and saves only what changed", async () => {
    const { onsaved, onclose } = open();
    const bio = (await screen.findByLabelText("Bio")) as HTMLTextAreaElement;
    expect(bio.value).toBe("Cheese?");
    expect((screen.getByLabelText("Link 1") as HTMLInputElement).value).toBe("https://instagram.com/c");
    await fireEvent.input(bio, { target: { value: "Gouda." } });
    await fireEvent.click(screen.getByText("Add link"));
    await fireEvent.input(screen.getByLabelText("Link 2"), { target: { value: "x.com/cheese" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls.updates).toEqual([
      {
        edit: {
          links: [
            { href: "https://instagram.com/c", kind: "INSTAGRAM" },
            { href: "https://x.com/cheese", kind: "TWITTER" },
          ],
        },
        bio: "Gouda.",
      },
    ]);
    expect(onsaved).toHaveBeenCalled();
  });

  it("asks before changing the username, as TIDAL does", async () => {
    const { onclose } = open();
    await fireEvent.input(await screen.findByLabelText("Username"), { target: { value: "new_cheese" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByRole("dialog").getAttribute("aria-label")).toBe("Save username?");
    expect(screen.getByText("Your username will be @new_cheese")).toBeTruthy();
    expect(calls.updates).toEqual([]);

    // Cancel goes back to the form with the typing kept.
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect((screen.getByLabelText("Username") as HTMLInputElement).value).toBe("new_cheese");

    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls.updates).toEqual([{ edit: { handle: "new_cheese" }, bio: null }]);
  });

  it("holds the username to a-z, 0-9 and underscores, with TIDAL's hint", async () => {
    open();
    const username = await screen.findByLabelText("Username");
    const rule = screen.getByText("Use only the letters a-z, numbers 0-9 and underscores.");
    expect(rule.classList.contains("rule-broken")).toBe(false);
    await fireEvent.input(username, { target: { value: "Cheese-Man" } });
    expect(rule.classList.contains("rule-broken")).toBe(true);
    expect(username.getAttribute("aria-invalid")).toBe("true");
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows TIDAL's rejection and stays open", async () => {
    calls.failUpdate = "Handle is already taken";
    const { onclose } = open();
    await fireEvent.input(await screen.findByLabelText("Username"), { target: { value: "taken" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Handle is already taken");
    expect(onclose).not.toHaveBeenCalled();
  });

  it("blocks a save the form already knows is wrong", async () => {
    open();
    await fireEvent.input(await screen.findByLabelText("Name"), { target: { value: " " } });
    expect(screen.getByRole("alert").textContent).toMatch(/needs a name/);
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("uploads a chosen picture on save, and can remove the current one", async () => {
    const { onclose } = open();
    const input = (await screen.findByLabelText("Profile picture")) as HTMLInputElement;
    const file = new File([new Uint8Array([1, 2, 3])], "me.png", { type: "image/png" });
    Object.defineProperty(input, "files", { value: [file] });
    await fireEvent.change(input);
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls.uploads).toEqual(["AQID"]);
    expect(calls.updates).toEqual([]);
    // Shown at once, while TIDAL's record still has the old picture.
    expect(profilePicture.for(7, "pic")).toBe("https://resources.tidal.com/images/ab/cd/750x750.jpg");

    cleanup();
    const second = open();
    await fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(second.onclose).toHaveBeenCalled());
    expect(calls.removed).toBe(1);
    expect(profilePicture.for(7, "pic")).toBeNull();
  });

  it("keeps the bio read-only when the profile has none yet", async () => {
    calls.bioId = null;
    open();
    const bio = (await screen.findByLabelText("Bio")) as HTMLTextAreaElement;
    expect(bio.disabled).toBe(true);
    expect(screen.getByText("TIDAL's editor")).toBeTruthy();
  });
});
