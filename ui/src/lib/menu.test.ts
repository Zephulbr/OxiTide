import { describe, expect, it, vi } from "vitest";
import { createMenu, type MenuItem } from "./menu.svelte";

function item(id: string, extra: Partial<MenuItem> = {}): MenuItem {
  return { id, label: id, run: vi.fn(), ...extra };
}

describe("menu store", () => {
  it("starts closed", () => {
    const menu = createMenu();

    expect(menu.open).toBeNull();
  });

  it("opens at the pointer with the given items", () => {
    const menu = createMenu();
    const items = [item("play")];

    menu.show(10, 20, items);

    expect(menu.open).toEqual({ x: 10, y: 20, items });
  });

  it("replaces the previous menu", () => {
    const menu = createMenu();
    menu.show(1, 2, [item("a")]);
    const next = [item("b")];

    menu.show(3, 4, next);

    expect(menu.open?.items).toBe(next);
    expect(menu.open?.x).toBe(3);
  });

  it("keeps disabled items in the list", () => {
    const menu = createMenu();
    const disabled = item("up", { disabled: true });

    menu.show(0, 0, [disabled, item("down")]);

    expect(menu.open?.items[0].disabled).toBe(true);
    expect(menu.open?.items).toHaveLength(2);
  });

  it("closes", () => {
    const menu = createMenu();
    menu.show(0, 0, [item("a")]);

    menu.close();

    expect(menu.open).toBeNull();
  });
});
