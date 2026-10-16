// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applySnapshot, createScrollMemory, snapshotScroll } from "./scrollMemory";

/// jsdom has no layout: give an element writable scroll offsets and a
/// controllable scroll height.
function scrollable(scrollHeight = 0, clientHeight = 500) {
  const el = document.createElement("div");
  let top = 0;
  let left = 0;
  const box = { scrollHeight };
  Object.defineProperty(el, "scrollTop", { get: () => top, set: (v: number) => (top = v) });
  Object.defineProperty(el, "scrollLeft", { get: () => left, set: (v: number) => (left = v) });
  Object.defineProperty(el, "scrollHeight", { get: () => box.scrollHeight });
  Object.defineProperty(el, "clientHeight", { get: () => clientHeight });
  document.body.append(el);
  return { el, box };
}

let observers: Array<() => void> = [];

beforeEach(() => {
  observers = [];
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private callback: () => void) {}
      observe() {
        observers.push(this.callback);
      }
      disconnect() {
        observers = observers.filter((cb) => cb !== this.callback);
      }
    },
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const grow = () => observers.forEach((cb) => cb());

describe("createScrollMemory", () => {
  it("starts a fresh entry at the top", () => {
    const memory = createScrollMemory();
    const { el } = scrollable(3000);
    el.scrollTop = 900;
    memory.restore(7, el, document.createElement("div"));
    expect(el.scrollTop).toBe(0);
  });

  it("restores a saved entry once the page is tall enough", () => {
    const memory = createScrollMemory();
    const { el, box } = scrollable(400);
    memory.save(1, 1200);
    memory.restore(1, el, document.createElement("div"));
    expect(el.scrollTop).toBe(0);
    box.scrollHeight = 1000;
    grow();
    expect(el.scrollTop).toBe(0);
    box.scrollHeight = 1700;
    grow();
    expect(el.scrollTop).toBe(1200);
    expect(observers).toHaveLength(0);
  });

  it("restores at once when the page is already tall enough", () => {
    const memory = createScrollMemory();
    const { el } = scrollable(5000);
    memory.save(2, 800);
    memory.restore(2, el, document.createElement("div"));
    expect(el.scrollTop).toBe(800);
  });

  it("gives up when the user scrolls first", () => {
    const memory = createScrollMemory();
    const { el, box } = scrollable(400);
    memory.save(3, 1200);
    memory.restore(3, el, document.createElement("div"));
    el.dispatchEvent(new Event("wheel"));
    box.scrollHeight = 5000;
    grow();
    expect(el.scrollTop).toBe(0);
  });

  it("a newer restore replaces a pending one", () => {
    const memory = createScrollMemory();
    const { el, box } = scrollable(400);
    memory.save(4, 1200);
    memory.restore(4, el, document.createElement("div"));
    memory.restore(5, el, document.createElement("div"));
    box.scrollHeight = 5000;
    grow();
    expect(el.scrollTop).toBe(0);
  });
});

describe("snapshotScroll", () => {
  it("records and puts back the scroller and inner scrollers", () => {
    const { el: main } = scrollable(3000);
    const { el: inner } = scrollable(3000);
    const page = document.createElement("div");
    page.append(inner);
    document.body.append(page);
    main.scrollTop = 640;
    inner.scrollTop = 120;
    inner.scrollLeft = 30;
    const snapshot = snapshotScroll(main, page);
    main.scrollTop = 0;
    inner.scrollTop = 0;
    inner.scrollLeft = 0;
    applySnapshot(snapshot);
    expect(main.scrollTop).toBe(640);
    expect(inner.scrollTop).toBe(120);
    expect(inner.scrollLeft).toBe(30);
  });
});
