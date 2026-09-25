export interface WindowRange {
  first: number;
  last: number;
  padTop: number;
  padBottom: number;
}

/// Slice of a fixed-row-height list to render, with spacer heights for the
/// rows omitted above and below. `first` is clamped so a list that shrinks
/// (refresh, removal) cannot leave the window pointing past the end.
export function windowRange(
  total: number,
  scrollTop: number,
  viewport: number,
  rowHeight: number,
  overscan = 8,
): WindowRange {
  const visible = Math.max(1, Math.ceil(viewport / rowHeight) + overscan * 2);
  const first = Math.min(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan),
    Math.max(0, total - visible),
  );
  const last = Math.min(total, first + visible);
  return {
    first,
    last,
    padTop: first * rowHeight,
    padBottom: Math.max(0, (total - last) * rowHeight),
  };
}
