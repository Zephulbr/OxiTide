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
