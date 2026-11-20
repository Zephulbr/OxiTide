/// Where a dragged queue row lands: whole rows moved, clamped to the queue.
export function dropIndex(from: number, deltaPx: number, rowHeight: number, total: number): number {
  const moved = Math.round(deltaPx / rowHeight);
  return Math.max(0, Math.min(total - 1, from + moved));
}

/// The window scroll that brings row `index` near the top of the page, one
/// row of context above it (the desktop queue rail's `scrollToRow`).
export function scrollTopFor(index: number, rowHeight: number, listTop: number): number {
  return Math.max(0, listTop + (index - 1) * rowHeight);
}

/// Whether row `index` is on screen: below the top of the window and above
/// the dock (mini bar + tab bar) covering its bottom.
export function rowOnScreen(
  index: number,
  rowHeight: number,
  listTop: number,
  scrollY: number,
  viewport: number,
  dock: number,
): boolean {
  const top = listTop + index * rowHeight - scrollY;
  return top >= -rowHeight / 2 && top + rowHeight / 2 <= viewport - dock;
}

/// The rendered window for a scroll position (with overscan rows).
export function visibleRange(
  scrollTop: number,
  viewport: number,
  rowHeight: number,
  total: number,
  overscan = 8,
): { from: number; to: number } {
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const count = Math.ceil(viewport / rowHeight);
  return {
    from: Math.max(0, first - overscan),
    to: Math.min(total, first + count + overscan),
  };
}
