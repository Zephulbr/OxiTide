// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { flushSync } from "svelte";

import Neko from "./Neko.svelte";

/// The loop is driven by hand: `advance(n)` runs n 100 ms ticks.
let frameCallback: ((now: number) => void) | null = null;
let clock = 0;
function advance(ticks: number) {
  for (let i = 0; i < ticks; i += 1) {
    clock += 100;
    const cb = frameCallback;
    frameCallback = null;
    cb?.(clock);
  }
  flushSync();
}

function memoryStorage(): Storage {
  const items = new Map<string, string>();
  return {
    get length() { return items.size; },
    clear: () => items.clear(),
    getItem: (key) => items.get(key) ?? null,
    key: (i) => [...items.keys()][i] ?? null,
    removeItem: (key) => void items.delete(key),
    setItem: (key, value) => void items.set(key, String(value)),
  };
}

beforeEach(() => {
  vi.stubGlobal("localStorage", memoryStorage());
  clock = 0;
  frameCallback = null;
  vi.stubGlobal("requestAnimationFrame", (cb: (now: number) => void) => {
    frameCallback = cb;
    return 1;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {});
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const cats = (container: HTMLElement) => ({
  oneko: container.querySelector(".neko:not(.kuro)") as HTMLElement,
  kuro: container.querySelector(".kuro") as HTMLElement | null,
});

const move = (x: number, y: number) =>
  window.dispatchEvent(new MouseEvent("mousemove", { clientX: x, clientY: y }));

/// Oneko starts idle at (32, 32); five moves across her make three turns.
function rubOneko() {
  for (const x of [26, 38, 26, 38, 26]) move(x, 32);
  flushSync();
}

describe("Neko: kuroneko", () => {
  it("right-clicking oneko summons kuroneko, and right-clicking her sends her away", async () => {
    const { container } = render(Neko);
    expect(cats(container).kuro).toBeNull();
    await fireEvent.contextMenu(cats(container).oneko);
    expect(cats(container).kuro).not.toBeNull();
    expect(localStorage.getItem("oxitide:kuroneko")).toBe("1");
    await fireEvent.contextMenu(cats(container).kuro!);
    expect(cats(container).kuro).toBeNull();
    expect(localStorage.getItem("oxitide:kuroneko")).toBeNull();
  });

  it("the right-click opens no other menu", () => {
    const { container } = render(Neko);
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    cats(container).oneko.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });

  it("comes back where she was after a restart", () => {
    localStorage.setItem("oxitide:kuroneko", "1");
    localStorage.setItem("oxitide:oneko", JSON.stringify({ x: 300, y: 200, kx: 250, ky: 210 }));
    const { container } = render(Neko);
    flushSync();
    const { kuro } = cats(container);
    expect(kuro?.style.left).toBe(`${250 - 16}px`);
    expect(kuro?.style.top).toBe(`${210 - 16}px`);
  });

  it("still loads a saved position without her", () => {
    localStorage.setItem("oxitide:oneko", JSON.stringify({ x: 300, y: 200 }));
    const { container } = render(Neko);
    expect(cats(container).oneko.style.left).toBe(`${300 - 16}px`);
  });

  it("follows oneko", () => {
    localStorage.setItem("oxitide:kuroneko", "1");
    localStorage.setItem("oxitide:oneko", JSON.stringify({ x: 500, y: 300, kx: 100, ky: 300 }));
    const { container } = render(Neko);
    move(500, 300);
    advance(20);
    const left = parseFloat(cats(container).kuro!.style.left);
    expect(left).toBeGreaterThan(100);
  });

  it("double-clicking either cat puts both to sleep", async () => {
    const sleeping = ["-64px 0px", "-64px -32px"];
    localStorage.setItem("oxitide:kuroneko", "1");
    localStorage.setItem("oxitide:oneko", JSON.stringify({ x: 500, y: 300, kx: 400, ky: 300 }));
    const { container } = render(Neko);
    await fireEvent.dblClick(cats(container).kuro!);
    advance(60);
    expect(sleeping).toContain(cats(container).oneko.style.backgroundPosition);
    expect(sleeping).toContain(cats(container).kuro!.style.backgroundPosition);
  });
});

describe("Neko: petting", () => {
  it("rubbing over oneko floats hearts that clear themselves", async () => {
    const { container } = render(Neko);
    rubOneko();
    const hearts = container.querySelectorAll(".heart");
    expect(hearts).toHaveLength(3);
    for (const heart of hearts) await fireEvent.animationEnd(heart);
    expect(container.querySelectorAll(".heart")).toHaveLength(0);
  });

  it("passing over her floats nothing", () => {
    const { container } = render(Neko);
    for (const x of [10, 20, 30, 40, 50]) move(x, 32);
    flushSync();
    expect(container.querySelectorAll(".heart")).toHaveLength(0);
  });

  it("shaking her while dragging floats nothing", async () => {
    const { container } = render(Neko);
    await fireEvent.mouseDown(cats(container).oneko, { button: 0, clientX: 32, clientY: 32 });
    rubOneko();
    expect(container.querySelectorAll(".heart")).toHaveLength(0);
    window.dispatchEvent(new MouseEvent("mouseup"));
  });

  it("keeps at most six hearts", () => {
    const { container } = render(Neko);
    rubOneko();
    rubOneko();
    rubOneko();
    expect(container.querySelectorAll(".heart")).toHaveLength(6);
  });
});
