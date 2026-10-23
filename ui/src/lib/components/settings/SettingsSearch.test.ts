// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import SettingsSearch from "./SettingsSearch.svelte";

const scope = { rack: false, studio: false, exclusive: false };

afterEach(cleanup);

function box() {
  return screen.getByRole("combobox", { name: "Search settings" }) as HTMLInputElement;
}

describe("SettingsSearch", () => {
  it("lists matches with where they live and jumps on Enter", async () => {
    const onselect = vi.fn();
    render(SettingsSearch, { scope, onselect });
    await fireEvent.input(box(), { target: { value: "font" } });
    const option = await screen.findByRole("option", { name: /Typeface/ });
    expect(option.textContent).toContain("Appearance");
    expect(option.textContent).toContain("Typography & layout");
    await fireEvent.keyDown(box(), { key: "Enter" });
    expect(onselect).toHaveBeenCalledWith(expect.objectContaining({ id: "typeface", tab: "appearance" }));
    expect(box().value).toBe("");
  });

  it("moves through results with the arrow keys and picks with a click", async () => {
    const onselect = vi.fn();
    render(SettingsSearch, { scope, onselect });
    await fireEvent.input(box(), { target: { value: "crossfade" } });
    const options = await screen.findAllByRole("option");
    expect(options.length).toBeGreaterThan(1);
    expect(box().getAttribute("aria-activedescendant")).toBe(options[0].id);
    await fireEvent.keyDown(box(), { key: "ArrowDown" });
    expect(box().getAttribute("aria-activedescendant")).toBe(options[1].id);
    await fireEvent.click(options[1]);
    expect(onselect).toHaveBeenCalledOnce();
  });

  it("says so when nothing matches", async () => {
    render(SettingsSearch, { scope, onselect: vi.fn() });
    await fireEvent.input(box(), { target: { value: "zzzz" } });
    expect((await screen.findByRole("status")).textContent).toContain("No settings match");
    expect(screen.queryByRole("option")).toBeNull();
  });

  it("clears on Escape without leaking the key to the app", async () => {
    render(SettingsSearch, { scope, onselect: vi.fn() });
    await fireEvent.input(box(), { target: { value: "cache" } });
    const leaked = vi.fn();
    window.addEventListener("keydown", leaked);
    await fireEvent.keyDown(box(), { key: "Escape" });
    window.removeEventListener("keydown", leaked);
    expect(box().value).toBe("");
    expect(leaked).not.toHaveBeenCalled();
  });
});
