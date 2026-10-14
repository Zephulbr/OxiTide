// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import SortSelect from "./SortSelect.svelte";

afterEach(cleanup);

const OPTIONS = [
  { value: "date", label: "Date" },
  { value: "name", label: "Name" },
  { value: "artist", label: "Artist" },
];

function setup() {
  const onchange = vi.fn();
  render(SortSelect, { value: "date", options: OPTIONS, label: "Sort by", onchange });
  return { onchange, trigger: screen.getByRole("button", { name: "Sort by" }) };
}

describe("SortSelect", () => {
  it("opens on ArrowDown and points aria-activedescendant at the current option", async () => {
    const { trigger } = setup();
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });

    const list = screen.getByRole("listbox");
    expect(list.getAttribute("aria-activedescendant")).toBe("sort-option-date");
    for (const option of screen.getAllByRole("option")) {
      expect(option.getAttribute("tabindex")).toBe("-1");
    }
  });

  it("moves with arrows and Home/End", async () => {
    const { trigger } = setup();
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const list = screen.getByRole("listbox");

    await fireEvent.keyDown(list, { key: "ArrowDown" });
    expect(list.getAttribute("aria-activedescendant")).toBe("sort-option-name");

    await fireEvent.keyDown(list, { key: "End" });
    expect(list.getAttribute("aria-activedescendant")).toBe("sort-option-artist");

    await fireEvent.keyDown(list, { key: "Home" });
    expect(list.getAttribute("aria-activedescendant")).toBe("sort-option-date");
  });

  it("chooses the active option on Enter and refocuses the trigger", async () => {
    const { onchange, trigger } = setup();
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const list = screen.getByRole("listbox");
    await fireEvent.keyDown(list, { key: "ArrowDown" });

    await fireEvent.keyDown(list, { key: "Enter" });

    expect(onchange).toHaveBeenCalledWith("name");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("closes on Escape and restores trigger focus", async () => {
    const { trigger } = setup();
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });

    await fireEvent.keyDown(screen.getByRole("listbox"), { key: "Escape" });

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("closes on Tab and restores trigger focus", async () => {
    const { trigger } = setup();
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });

    await fireEvent.keyDown(screen.getByRole("listbox"), { key: "Tab" });

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });
});
