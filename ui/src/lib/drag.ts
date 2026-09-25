/// Insertion slot (0..count) under the pointer. `offset` is content above the
/// first row (e.g. a sticky table head). Rounding puts the slot at the closer
/// row edge.
export function dropTarget(
  clientY: number,
  scrollerTop: number,
  scrollTop: number,
  rowHeight: number,
  count: number,
  offset = 0,
): number {
  const y = clientY - scrollerTop + scrollTop - offset;
  const slot = Math.round(y / rowHeight);
  return Math.max(0, Math.min(count, slot));
}

/// Moving down removes the dragged row first, shifting the target up one.
export function normalizeDrop(from: number, to: number): number {
  return to > from ? to - 1 : to;
}

export interface DragParams {
  enabled: () => boolean;
  count: () => number;
  rowHeight: () => number;
  offset?: () => number;
  onChange: (from: number | null, at: number | null) => void;
  onDrop: (from: number, to: number) => void;
}

/// Svelte action for the scroll container. Rows carry `draggable` and
/// `data-drag-index`; listeners attach per container, never per row, so the
/// index math works even when virtualized rows mount and unmount mid-drag.
export function trackDrag(node: HTMLElement, initial: DragParams) {
  let params = initial;
  let from: number | null = null;

  function rowIndex(target: EventTarget | null): number | null {
    const element = target instanceof Element ? target.closest("[data-drag-index]") : null;
    if (!element) return null;
    const index = Number(element.getAttribute("data-drag-index"));
    return Number.isInteger(index) ? index : null;
  }

  function slotAt(clientY: number): number {
    return dropTarget(
      clientY,
      node.getBoundingClientRect().top,
      node.scrollTop,
      params.rowHeight(),
      params.count(),
      params.offset?.() ?? 0,
    );
  }

  function onDragStart(event: DragEvent) {
    if (!params.enabled()) return;
    const index = rowIndex(event.target);
    if (index === null) return;
    from = index;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", String(index));
    }
    params.onChange(from, null);
  }

  function onDragOver(event: DragEvent) {
    if (from === null || !params.enabled()) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    params.onChange(from, slotAt(event.clientY));
  }

  function onDrop(event: DragEvent) {
    if (from === null || !params.enabled()) return;
    event.preventDefault();
    const source = from;
    from = null;
    params.onChange(null, null);
    params.onDrop(source, normalizeDrop(source, slotAt(event.clientY)));
  }

  function clear() {
    if (from === null) return;
    from = null;
    params.onChange(null, null);
  }

  function onDragLeave(event: DragEvent) {
    if (event.relatedTarget && node.contains(event.relatedTarget as Node)) return;
    clear();
  }

  node.addEventListener("dragstart", onDragStart);
  node.addEventListener("dragover", onDragOver);
  node.addEventListener("drop", onDrop);
  node.addEventListener("dragend", clear);
  node.addEventListener("dragleave", onDragLeave);

  return {
    update(next: DragParams) {
      params = next;
    },
    destroy() {
      node.removeEventListener("dragstart", onDragStart);
      node.removeEventListener("dragover", onDragOver);
      node.removeEventListener("drop", onDrop);
      node.removeEventListener("dragend", clear);
      node.removeEventListener("dragleave", onDragLeave);
    },
  };
}
