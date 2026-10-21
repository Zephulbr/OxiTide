// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { createMenu, menu as sharedMenu, menuButton, menuFor, type MenuItem } from "./menu.svelte";

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

    expect(menu.open).toEqual({ x: 10, y: 20, items, opener: null, placement: "below", align: "start" });
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

describe("menu store focus restore", () => {
  it("restores focus to a connected opener when closed with restore", () => {
    const m = createMenu();
    const opener = document.createElement("button");
    document.body.append(opener);
    m.show(0, 0, [item("a")], opener);

    m.close();

    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it("leaves focus alone when closed without restore", () => {
    const m = createMenu();
    const opener = document.createElement("button");
    const other = document.createElement("input");
    document.body.append(opener, other);
    other.focus();
    m.show(0, 0, [item("a")], opener);

    m.close(false);

    expect(document.activeElement).toBe(other);
    opener.remove();
    other.remove();
  });

  it("skips a disconnected opener", () => {
    const m = createMenu();
    const opener = document.createElement("button");
    m.show(0, 0, [item("a")], opener);

    m.close();

    expect(document.activeElement).not.toBe(opener);
  });
});

describe("menuFor keyboard invocation", () => {
  it("opens at the node on Shift+F10 and the ContextMenu key", () => {
    const node = document.createElement("div");
    document.body.append(node);
    const handle = menuFor(node, () => [item("a")]);

    node.dispatchEvent(new KeyboardEvent("keydown", { key: "F10", shiftKey: true, bubbles: true }));
    expect(sharedMenu.open?.opener).toBe(node);

    sharedMenu.close(false);
    node.dispatchEvent(new KeyboardEvent("keydown", { key: "ContextMenu", bubbles: true }));
    expect(sharedMenu.open).not.toBeNull();

    sharedMenu.close(false);
    node.dispatchEvent(new KeyboardEvent("keydown", { key: "F10", bubbles: true }));
    expect(sharedMenu.open).toBeNull();

    handle.destroy();
    node.remove();
  });
});

describe("menuButton", () => {
  function press(node: HTMLElement, detail = 1) {
    node.dispatchEvent(new Event("pointerdown"));
    // The menu host closes an open menu on the window's mousedown.
    if (sharedMenu.open) sharedMenu.close(false);
    node.dispatchEvent(new MouseEvent("click", { detail }));
  }

  it("opens on a press and closes on the next press instead of reopening", () => {
    const button = document.createElement("button");
    document.body.append(button);
    const open = vi.fn((node: HTMLElement) => sharedMenu.show(0, 0, [item("a")], node));
    const handle = menuButton(button, open);

    press(button);
    expect(sharedMenu.open?.opener).toBe(button);

    press(button);
    expect(sharedMenu.open).toBeNull();
    expect(open).toHaveBeenCalledTimes(1);

    press(button);
    expect(sharedMenu.open?.opener).toBe(button);

    sharedMenu.close(false);
    handle.destroy();
    button.remove();
  });

  it("reopens when another menu was open", () => {
    const button = document.createElement("button");
    const other = document.createElement("button");
    document.body.append(button, other);
    const handle = menuButton(button, (node) => sharedMenu.show(0, 0, [item("a")], node));
    sharedMenu.show(0, 0, [item("b")], other);

    press(button);
    expect(sharedMenu.open?.opener).toBe(button);

    sharedMenu.close(false);
    handle.destroy();
    button.remove();
    other.remove();
  });

  it("always opens on a keyboard click", () => {
    const button = document.createElement("button");
    document.body.append(button);
    const handle = menuButton(button, (node) => sharedMenu.show(0, 0, [item("a")], node));
    sharedMenu.show(0, 0, [item("a")], button);

    press(button, 0);
    expect(sharedMenu.open?.opener).toBe(button);

    sharedMenu.close(false);
    handle.destroy();
    button.remove();
  });
});
