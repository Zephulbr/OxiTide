/// Pure geometry for the queue's pointer drag (QueueRail). Indexes are queue
/// positions; `to` is the dragged row's final index, which is exactly what
/// `queue_reorder(from, to)` takes.

/// Movement (px) before a press becomes a drag; below it the press is a click.
export const DRAG_THRESHOLD = 4;

/// Final index for a dragged row whose center sits `centerY` px below the top
/// of the list content.
export function dragTarget(centerY: number, rowHeight: number, count: number): number {
  if (count <= 0) return 0;
  return Math.max(0, Math.min(count - 1, Math.floor(centerY / rowHeight)));
}

/// How a resting row moves while another is dragged from `from` to `to`:
/// rows between the two slide one slot toward `from` to open a gap at `to`.
export function rowShift(index: number, from: number, to: number): -1 | 0 | 1 {
  if (from < to && index > from && index <= to) return -1;
  if (to < from && index >= to && index < from) return 1;
  return 0;
}

/// Auto-scroll speed (px per frame) while the pointer is within `edge` px of
/// the list's visible top or bottom; faster the deeper it goes.
export function edgeScroll(
  clientY: number,
  top: number,
  bottom: number,
  edge = 48,
  max = 18,
): number {
  if (bottom - top < edge * 3) return 0;
  if (clientY < top + edge) return -Math.ceil(max * Math.min(1, (top + edge - clientY) / edge));
  if (clientY > bottom - edge) return Math.ceil(max * Math.min(1, (clientY - (bottom - edge)) / edge));
  return 0;
}
