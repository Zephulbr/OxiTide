// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { createMenu } from "../menu.svelte";
import ContextMenu from "./ContextMenu.svelte";

afterEach(() => {
  cleanup();
  document.body.innerHTML = "";
});

function setup() {
  const menu = createMenu();
  const runA = vi.fn();
  const opener = document.createElement("button");
  document.body.append(opener);
  opener.focus();
  menu.show(
    20,
    20,
    [
      { id: "a", label: "Alpha", run: runA },
      { id: "b", label: "Beta", disabled: true, run: vi.fn() },
      { id: "c", label: "Gamma", run: vi.fn() },
    ],
    opener,
  );
  render(ContextMenu, { menu });
  return { menu, runA, opener };
}

describe("ContextMenu keyboard", () => {
  it("moves aria-activedescendant with arrows and Home/End, skipping disabled items", async () => {
    setup();
    const panel = screen.getByRole("menu");
    expect(panel.getAttribute("aria-activedescendant")).toBe("menu-item-a");

    await fireEvent.keyDown(panel, { key: "ArrowDown" });
    expect(panel.getAttribute("aria-activedescendant")).toBe("menu-item-c");

    await fireEvent.keyDown(panel, { key: "ArrowDown" });
    expect(panel.getAttribute("aria-activedescendant")).toBe("menu-item-a");

    await fireEvent.keyDown(panel, { key: "End" });
    expect(panel.getAttribute("aria-activedescendant")).toBe("menu-item-c");

    await fireEvent.keyDown(panel, { key: "Home" });
    expect(panel.getAttribute("aria-activedescendant")).toBe("menu-item-a");
  });

  it("runs the active item on Enter and restores focus to the opener", async () => {
    const { menu, runA, opener } = setup();

    await fireEvent.keyDown(screen.getByRole("menu"), { key: "Enter" });

    expect(runA).toHaveBeenCalledOnce();
    expect(menu.open).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("runs the clicked item", async () => {
    const { runA } = setup();
    await userEvent.click(screen.getByRole("menuitem", { name: "Alpha" }));
    expect(runA).toHaveBeenCalledOnce();
  });

  it("closes on Escape and restores focus", async () => {
    const { menu, opener } = setup();
    await fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(menu.open).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("closes on Tab and restores focus so Tab continues from the opener", async () => {
    const { menu, opener } = setup();
    await fireEvent.keyDown(screen.getByRole("menu"), { key: "Tab" });
    expect(menu.open).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("closes without stealing focus on an outside mousedown", async () => {
    const { menu, opener } = setup();

    await fireEvent.mouseDown(document.body);

    expect(menu.open).toBeNull();
    expect(document.activeElement).not.toBe(opener);
  });
});
