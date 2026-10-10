/// Where a dragged queue row lands: whole rows moved, clamped to the queue.
export function dropIndex(from: number, deltaPx: number, rowHeight: number, total: number): number {
  const moved = Math.round(deltaPx / rowHeight);
  return Math.max(0, Math.min(total - 1, from + moved));
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
