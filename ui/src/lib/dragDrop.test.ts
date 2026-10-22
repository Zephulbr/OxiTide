// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dragDrop, dragSource, dropZone, type DropZone } from "./dragDrop.svelte";
import type { DragPayload } from "./dragPayload";

const PAYLOAD: DragPayload = { kind: "track-id", id: 1, title: "Song" };

function pointer(type: string, x: number, y: number, init: PointerEventInit = {}): PointerEvent {
  // jsdom has no PointerEvent constructor; a MouseEvent with the pointer
  // fields the controller reads stands in.
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0, ...init });
  Object.defineProperty(event, "pointerType", { value: init.pointerType ?? "mouse" });
  return event as PointerEvent;
}

let source: HTMLElement;
let zoneEl: HTMLElement;
let hit: Element | null;
let cleanups: (() => void)[];

function mountZone(el: HTMLElement, zone: DropZone) {
  const handle = dropZone(el, zone);
  cleanups.push(() => handle.destroy());
}

function mountSource(get: () => DragPayload | null = () => PAYLOAD) {
  const handle = dragSource(source, get);
  cleanups.push(() => handle.destroy());
}

/// Press on the source, move past the threshold onto (x, y).
function dragTo(x: number, y: number) {
  source.dispatchEvent(pointer("pointerdown", 0, 0));
  window.dispatchEvent(pointer("pointermove", x, y));
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "requestAnimationFrame", "cancelAnimationFrame"] });
  cleanups = [];
  document.body.innerHTML = "";
  source = document.createElement("div");
  zoneEl = document.createElement("div");
  const inner = document.createElement("span");
  zoneEl.append(inner);
  document.body.append(source, zoneEl);
  hit = null;
  document.elementFromPoint = vi.fn(() => hit);
});

afterEach(() => {
  dragDrop.cancel();
  vi.runOnlyPendingTimers();
  for (const cleanup of cleanups) cleanup();
  vi.useRealTimers();
});

describe("drag controller", () => {
  it("a press that never moves past the threshold stays a click", () => {
    const get = vi.fn(() => PAYLOAD);
    mountSource(get);
    source.dispatchEvent(pointer("pointerdown", 0, 0));
    window.dispatchEvent(pointer("pointermove", 2, 1));
    window.dispatchEvent(pointer("pointerup", 2, 1));
    expect(get).not.toHaveBeenCalled();
    expect(dragDrop.active).toBeNull();

    const clicked = vi.fn();
    source.addEventListener("click", clicked);
    source.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it("adds window listeners only from a press to its end", () => {
    mountSource();
    const add = vi.spyOn(window, "addEventListener");
    const remove = vi.spyOn(window, "removeEventListener");

    dragTo(40, 40);
    expect(dragDrop.active).toBe(PAYLOAD);
    window.dispatchEvent(pointer("pointerup", 40, 40));
    vi.runAllTimers();

    // Every listener type added was removed again (removing one that was
    // never added is harmless, so compare the kinds, not the call counts).
    const added = new Set(add.mock.calls.map(([type]) => type));
    const removed = new Set(remove.mock.calls.map(([type]) => type));
    expect(added.size).toBeGreaterThan(0);
    expect(removed).toEqual(added);
    add.mockRestore();
    remove.mockRestore();
  });

  it("ignores touch and presses on inputs", () => {
    const get = vi.fn(() => PAYLOAD);
    mountSource(get);
    source.dispatchEvent(pointer("pointerdown", 0, 0, { pointerType: "touch" }));
    window.dispatchEvent(pointer("pointermove", 50, 50));
    const input = document.createElement("input");
    source.append(input);
    input.dispatchEvent(pointer("pointerdown", 0, 0));
    window.dispatchEvent(pointer("pointermove", 50, 50));
    expect(get).not.toHaveBeenCalled();
  });

  it("a null payload ends the press without a drag", () => {
    mountSource(() => null);
    dragTo(40, 40);
    expect(dragDrop.active).toBeNull();
    expect(document.body.classList.contains("dragging")).toBe(false);
  });

  it("asks a zone once per drag and drops on an accepting zone", () => {
    const accepts = vi.fn(() => ({ ok: true as const, hint: "Add" }));
    const drop = vi.fn();
    mountZone(zoneEl, { key: "z", accepts, drop });
    mountSource();
    hit = zoneEl.firstElementChild;

    dragTo(40, 40);
    expect(dragDrop.targetKey).toBe("z");
    expect(dragDrop.verdict).toEqual({ ok: true, hint: "Add" });
    vi.advanceTimersToNextFrame();
    vi.advanceTimersToNextFrame();
    window.dispatchEvent(pointer("pointerup", 41, 41));

    expect(accepts).toHaveBeenCalledTimes(1);
    expect(drop).toHaveBeenCalledWith(PAYLOAD, { x: 41, y: 41 });
    expect(dragDrop.active).toBeNull();
  });

  it("never drops on a refusing zone", () => {
    const drop = vi.fn();
    mountZone(zoneEl, { key: "z", accepts: () => ({ ok: false, reason: "No" }), drop });
    mountSource();
    hit = zoneEl;
    dragTo(40, 40);
    expect(dragDrop.verdict).toEqual({ ok: false, reason: "No" });
    window.dispatchEvent(pointer("pointerup", 40, 40));
    expect(drop).not.toHaveBeenCalled();
  });

  it("passes through a zone that has no verdict to the one around it", () => {
    const outer = vi.fn(() => ({ ok: true as const, hint: "Outer" }));
    const innerEl = zoneEl.firstElementChild as HTMLElement;
    mountZone(zoneEl, { key: "outer", accepts: outer });
    mountZone(innerEl, { key: "inner", accepts: () => null });
    mountSource();
    hit = innerEl;
    dragTo(40, 40);
    expect(dragDrop.targetKey).toBe("outer");
  });

  it("calls leave when the pointer moves off the target", () => {
    const leave = vi.fn();
    mountZone(zoneEl, { key: "z", accepts: () => ({ ok: true, hint: "" }), leave });
    mountSource();
    hit = zoneEl;
    dragTo(40, 40);
    hit = source;
    vi.advanceTimersToNextFrame();
    expect(leave).toHaveBeenCalledTimes(1);
    expect(dragDrop.targetKey).toBeNull();
  });

  it("tracks a running drop as pending", async () => {
    let finish: () => void = () => {};
    const drop = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    mountZone(zoneEl, { key: "z", accepts: () => ({ ok: true, hint: "" }), drop });
    mountSource();
    hit = zoneEl;
    dragTo(40, 40);
    window.dispatchEvent(pointer("pointerup", 40, 40));
    expect(dragDrop.pending.has("z")).toBe(true);
    finish();
    await vi.waitFor(() => expect(dragDrop.pending.has("z")).toBe(false));
  });

  it("Escape, pointercancel and blur cancel without dropping", () => {
    const drop = vi.fn();
    mountZone(zoneEl, { key: "z", accepts: () => ({ ok: true, hint: "" }), drop });
    mountSource();
    hit = zoneEl;
    for (const end of [
      () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })),
      () => window.dispatchEvent(pointer("pointercancel", 40, 40)),
      () => window.dispatchEvent(new Event("blur")),
    ]) {
      dragTo(40, 40);
      expect(dragDrop.active).not.toBeNull();
      end();
      expect(dragDrop.active).toBeNull();
      window.dispatchEvent(pointer("pointerup", 40, 40));
      vi.runAllTimers();
    }
    expect(drop).not.toHaveBeenCalled();
  });

  it("swallows exactly the click that follows a drag", () => {
    mountSource();
    const clicked = vi.fn();
    source.addEventListener("click", clicked);
    dragTo(40, 40);
    window.dispatchEvent(pointer("pointerup", 40, 40));
    source.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(clicked).not.toHaveBeenCalled();
    vi.runAllTimers();
    source.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it("swallows the release click after an Escape mid-drag", () => {
    mountSource();
    const clicked = vi.fn();
    source.addEventListener("click", clicked);
    dragTo(40, 40);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    window.dispatchEvent(pointer("pointerup", 40, 40));
    source.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(clicked).not.toHaveBeenCalled();
    vi.runAllTimers();
    source.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it("prevents native drags on the source", () => {
    mountSource();
    const event = new Event("dragstart", { bubbles: true, cancelable: true });
    source.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });

  it("opens a spring zone after the pointer rests on it", () => {
    const open = vi.fn();
    mountZone(zoneEl, { key: "folder", spring: { ms: 600, open } });
    mountSource();
    hit = zoneEl;
    dragTo(40, 40);
    vi.advanceTimersByTime(599);
    expect(open).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(open).toHaveBeenCalledTimes(1);
  });

  it("a spring left early never opens", () => {
    const open = vi.fn();
    mountZone(zoneEl, { key: "folder", spring: { ms: 600, open } });
    mountSource();
    hit = zoneEl;
    dragTo(40, 40);
    vi.advanceTimersByTime(300);
    hit = source;
    vi.advanceTimersToNextFrame();
    vi.advanceTimersByTime(600);
    expect(open).not.toHaveBeenCalled();
  });

  it("moves the ghost through its style", () => {
    mountSource();
    const ghost = document.createElement("div");
    dragDrop.bindGhost(ghost);
    dragTo(40, 50);
    expect(ghost.style.transform).toBe("translate3d(40px, 50px, 0)");
    window.dispatchEvent(pointer("pointermove", 60, 70));
    vi.advanceTimersToNextFrame();
    expect(ghost.style.transform).toBe("translate3d(60px, 70px, 0)");
    dragDrop.bindGhost(null);
  });

  it("marks the body while dragging", () => {
    mountSource();
    dragTo(40, 40);
    expect(document.body.classList.contains("dragging")).toBe(true);
    window.dispatchEvent(pointer("pointerup", 40, 40));
    expect(document.body.classList.contains("dragging")).toBe(false);
  });
});
