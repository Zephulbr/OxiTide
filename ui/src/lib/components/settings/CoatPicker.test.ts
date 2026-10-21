// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";

vi.mock("../../catjamSprite", async (original) => ({
  ...(await original<typeof import("../../catjamSprite")>()),
  renderIdle: (coat: string) => `data:image/png;base64,${coat}`,
}));

import CoatPicker from "./CoatPicker.svelte";

afterEach(cleanup);

describe("CoatPicker", () => {
  it("offers every coat by name and marks the chosen one", () => {
    render(CoatPicker, { value: "ginger", onchange: () => {} });
    const group = screen.getByRole("group", { name: "Coat" });
    const buttons = [...group.querySelectorAll("button")];
    expect(buttons.map((b) => b.getAttribute("aria-label"))).toEqual([
      "White", "Black", "Ginger tabby", "Grey tabby", "Tuxedo", "Cream",
    ]);
    expect(screen.getByRole("button", { name: "Ginger tabby" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "White" }).getAttribute("aria-pressed")).toBe("false");
  });

  it("reports the coat that was picked", async () => {
    const onchange = vi.fn();
    render(CoatPicker, { value: "white", onchange });
    await fireEvent.click(screen.getByRole("button", { name: "Tuxedo" }));
    expect(onchange).toHaveBeenCalledWith("tuxedo");
  });

  it("moves focus with the arrow keys and keeps them from the transport shortcuts", async () => {
    render(CoatPicker, { value: "white", onchange: () => {} });
    const white = screen.getByRole("button", { name: "White" });
    white.focus();
    const outside = vi.fn();
    document.addEventListener("keydown", outside);
    await fireEvent.keyDown(white, { key: "ArrowRight" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Black" }));
    await fireEvent.keyDown(document.activeElement as Element, { key: "End" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Cream" }));
    await fireEvent.keyDown(document.activeElement as Element, { key: "ArrowRight" });
    expect(document.activeElement).toBe(white);
    expect(outside).not.toHaveBeenCalled();
    document.removeEventListener("keydown", outside);
  });
});
