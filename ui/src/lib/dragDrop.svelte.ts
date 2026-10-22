import { SvelteSet } from "svelte/reactivity";
import type { DragPayload } from "./dragPayload";
import { DRAG_THRESHOLD, edgeScroll } from "./queueDrag";

/// What a drop zone says about the payload over it: accepted (with the
/// ghost's hint, e.g. "Add to Road Trip") or refused (with the reason).
export type Verdict = { ok: true; hint: string } | { ok: false; reason: string };

export interface DragPoint {
  x: number;
  y: number;
}

/// A drop target, registered on its container with `use:dropZone`. Zones
/// nest: the innermost zone whose `accepts` returns a verdict is the target;
/// one that returns null (or has no `accepts`) lets the zone around it decide.
export interface DropZone {
  /// Names the zone for `dragDrop.targetKey` / `pending`, and tells two
  /// registrations of one zone apart from a new zone.
  key: string;
  accepts?(payload: DragPayload): Verdict | null;
  /// Each frame while this zone is the target (slot-aware zones).
  hover?(payload: DragPayload, point: DragPoint): void;
  /// The pointer left the zone, or the drag ended.
  leave?(): void;
  /// Called for an accepted drop. While a returned promise runs the zone's
  /// key is in `dragDrop.pending` (row spinners).
  drop?(payload: DragPayload, point: DragPoint): void | Promise<unknown>;
  /// Auto-scrolled while the pointer is near its top or bottom edge.
  scroller?(): HTMLElement | null;
  /// Opened after the pointer rests on the zone for `ms` (spring-loaded
  /// folders, the collapsed sidebar rail).
  spring?: { ms: number; open(): void };
}

/// Reads the payload when a press turns into a drag; `target` is the element
/// that was pressed. Null means nothing here is draggable right now.
export type PayloadGetter = (target: Element) => DragPayload | null;

/// Presses here never start a drag.
const NO_DRAG = "input, textarea, select, [contenteditable='true'], [data-no-drag]";

const zones = new WeakMap<Element, DropZone>();

export interface DragDrop {
  /// The payload being dragged, or null at rest.
  readonly active: DragPayload | null;
  /// The zone under the pointer that answered with a verdict.
  readonly targetKey: string | null;
  readonly verdict: Verdict | null;
  /// Zones whose drop is still running.
  readonly pending: ReadonlySet<string>;
  /// A press on a drag source; the drag starts once it moves past the
  /// threshold.
  press(event: PointerEvent, get: PayloadGetter): void;
  /// End any drag or press with no drop (route change, sign-out).
  cancel(): void;
  /// The ghost element, moved straight through its style each frame.
  bindGhost(element: HTMLElement | null): void;
}

/// The app's one drag-and-drop gesture (drag-to-playlist spec DD-D1):
/// pointer-driven, its own ghost, targets hit-tested with
/// `elementFromPoint`. Window listeners exist only from a press to its end,
/// and frames run only while a drag is active (DD-D10).
export function createDragDrop(): DragDrop {
  // Raw: a payload's tracks must keep their identity (selection is keyed by
  // row object), so they are never wrapped in proxies.
  let active = $state.raw<DragPayload | null>(null);
  let verdict = $state.raw<Verdict | null>(null);
  let targetKey = $state<string | null>(null);
  const pending = new SvelteSet<string>();

  let ghost: HTMLElement | null = null;
  let held: { x: number; y: number; target: Element; get: PayloadGetter } | null = null;
  let point: DragPoint = { x: 0, y: 0 };
  let frame = 0;
  let target: DropZone | null = null;
  let scrollZone: DropZone | null = null;
  let springKey: string | null = null;
  let springTimer: ReturnType<typeof setTimeout> | undefined;
  let verdicts = new Map<string, Verdict | null>();

  function listen(on: boolean) {
    if (on) {
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
      window.addEventListener("pointercancel", onCancel);
      window.addEventListener("blur", onCancel);
      window.addEventListener("keydown", onKey, true);
    } else {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
      window.removeEventListener("blur", onCancel);
      window.removeEventListener("keydown", onKey, true);
    }
  }

  function press(event: PointerEvent, get: PayloadGetter) {
    if (held || active) return;
    if (event.button !== 0 || event.pointerType === "touch") return;
    const pressed = event.target;
    if (!(pressed instanceof Element) || pressed.closest(NO_DRAG)) return;
    held = { x: event.clientX, y: event.clientY, target: pressed, get };
    point = { x: event.clientX, y: event.clientY };
    listen(true);
  }

  function verdictFor(zone: DropZone, payload: DragPayload): Verdict | null {
    if (!zone.accepts) return null;
    if (!verdicts.has(zone.key)) verdicts.set(zone.key, zone.accepts(payload));
    return verdicts.get(zone.key) ?? null;
  }

  /// Find the target, spring and scroll zones under the pointer.
  function evaluate(payload: DragPayload) {
    const hit = document.elementFromPoint?.(point.x, point.y) ?? null;
    let found: DropZone | null = null;
    let foundVerdict: Verdict | null = null;
    let spring: DropZone | null = null;
    let scroll: DropZone | null = null;
    let element = hit?.closest("[data-drop-zone]") ?? null;
    while (element) {
      const zone = zones.get(element);
      if (zone) {
        spring ??= zone.spring ? zone : null;
        scroll ??= zone.scroller ? zone : null;
        if (!found) {
          const answer = verdictFor(zone, payload);
          if (answer) {
            found = zone;
            foundVerdict = answer;
          }
        }
      }
      element = element.parentElement?.closest("[data-drop-zone]") ?? null;
    }

    if (found?.key !== target?.key) {
      target?.leave?.();
      targetKey = found?.key ?? null;
      verdict = foundVerdict;
    }
    target = found;
    if (found && foundVerdict?.ok) found.hover?.(payload, point);
    scrollZone = scroll;

    if (spring?.key !== springKey) {
      clearTimeout(springTimer);
      springKey = spring?.key ?? null;
      const open = spring?.spring?.open;
      if (spring && open) {
        const key = spring.key;
        springTimer = setTimeout(() => {
          if (springKey === key) open();
        }, spring.spring?.ms ?? 0);
      }
    }
  }

  function moveGhost() {
    if (ghost) ghost.style.transform = `translate3d(${point.x}px, ${point.y}px, 0)`;
  }

  function tick() {
    frame = 0;
    const payload = active;
    if (!payload) return;
    const scroller = scrollZone?.scroller?.();
    if (scroller) {
      const rect = scroller.getBoundingClientRect();
      const speed = edgeScroll(point.y, rect.top, rect.bottom);
      if (speed !== 0) scroller.scrollTop += speed;
    }
    evaluate(payload);
    moveGhost();
    frame = requestAnimationFrame(tick);
  }

  function start(payload: DragPayload) {
    active = payload;
    verdicts = new Map();
    document.body.classList.add("dragging");
    window.getSelection?.()?.removeAllRanges();
    evaluate(payload);
    moveGhost();
    frame = requestAnimationFrame(tick);
  }

  function onMove(event: PointerEvent) {
    point = { x: event.clientX, y: event.clientY };
    if (active || !held) return;
    if (Math.hypot(point.x - held.x, point.y - held.y) < DRAG_THRESHOLD) return;
    const payload = held.get(held.target);
    if (!payload) {
      end();
      return;
    }
    start(payload);
  }

  function onUp(event: PointerEvent) {
    point = { x: event.clientX, y: event.clientY };
    const payload = active;
    if (!payload) {
      end();
      return;
    }
    evaluate(payload);
    const zone = target;
    const accepted = verdict?.ok === true;
    const at = point;
    end();
    swallowClick(false);
    if (zone && accepted) runDrop(zone, payload, at);
  }

  function onCancel() {
    const dragging = active !== null;
    end();
    if (dragging) swallowClick(true);
  }

  function onKey(event: KeyboardEvent) {
    if (event.key !== "Escape" || !active) return;
    event.preventDefault();
    event.stopPropagation();
    onCancel();
  }

  function end() {
    listen(false);
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    clearTimeout(springTimer);
    springKey = null;
    target?.leave?.();
    target = null;
    scrollZone = null;
    held = null;
    if (active) document.body.classList.remove("dragging");
    active = null;
    verdict = null;
    targetKey = null;
  }

  /// The click that follows a drag's release must not open or play what the
  /// drag started on. After a cancel the button may still be down, so the
  /// swallow waits for that release; a fresh press disarms it.
  function swallowClick(stillPressed: boolean) {
    const swallow = (event: MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      disarm();
    };
    const afterRelease = () => setTimeout(disarm);
    function disarm() {
      window.removeEventListener("click", swallow, true);
      window.removeEventListener("pointerup", afterRelease, true);
      window.removeEventListener("pointerdown", disarm, true);
    }
    window.addEventListener("click", swallow, true);
    window.addEventListener("pointerdown", disarm, true);
    if (stillPressed) window.addEventListener("pointerup", afterRelease, true);
    else setTimeout(disarm);
  }

  function runDrop(zone: DropZone, payload: DragPayload, at: DragPoint) {
    let result: void | Promise<unknown>;
    try {
      result = zone.drop?.(payload, at);
    } catch (e) {
      console.error("drop failed", e);
      return;
    }
    if (!(result instanceof Promise)) return;
    const key = zone.key;
    pending.add(key);
    result
      .catch((e) => console.error("drop failed", e))
      .finally(() => pending.delete(key));
  }

  return {
    get active() {
      return active;
    },
    get targetKey() {
      return targetKey;
    },
    get verdict() {
      return verdict;
    },
    get pending() {
      return pending;
    },
    press,
    cancel() {
      if (active || held) onCancel();
    },
    bindGhost(element) {
      ghost = element;
      moveGhost();
    },
  };
}

export const dragDrop = createDragDrop();

/// Makes `node` a drag source: a press inside it can start a drag carrying
/// `get(pressedElement)`. Native drags (cover images) are suppressed.
export function dragSource(node: HTMLElement, get: PayloadGetter) {
  let current = get;
  const onDown = (event: PointerEvent) => dragDrop.press(event, (target) => current(target));
  const noNativeDrag = (event: DragEvent) => event.preventDefault();
  node.addEventListener("pointerdown", onDown);
  node.addEventListener("dragstart", noNativeDrag);
  return {
    update(next: PayloadGetter) {
      current = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", onDown);
      node.removeEventListener("dragstart", noNativeDrag);
    },
  };
}

/// Registers `node` as a drop zone. Attach it to containers (a list's
/// scroller, a tree row), never to virtualized rows that unmount mid-drag.
export function dropZone(node: HTMLElement, zone: DropZone) {
  zones.set(node, zone);
  node.setAttribute("data-drop-zone", "");
  return {
    update(next: DropZone) {
      zones.set(node, next);
    },
    destroy() {
      zones.delete(node);
      node.removeAttribute("data-drop-zone");
    },
  };
}
