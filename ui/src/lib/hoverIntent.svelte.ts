/// Hover-to-expand panels (the collapsed sidebar, the closed queue): open
/// after a short dwell so a pointer passing through doesn't flash a panel, and
/// close after a grace period so a brief overshoot doesn't drop it.
export const HOVER_OPEN_MS = 140;
export const HOVER_CLOSE_MS = 260;

/// The queue peek runs tighter than the sidebar: a wider edge strip to find,
/// a shorter dwell and grace, and a quicker slide than the docked glide.
export const QUEUE_PEEK_OPEN_MS = 100;
export const QUEUE_PEEK_CLOSE_MS = 280;
export const QUEUE_PEEK_EDGE_PX = 8;
export const QUEUE_PEEK_SLIDE_MS = 320;

export interface HoverIntent {
  readonly open: boolean;
  /// The pointer arrived; `immediate` skips the dwell (re-entry mid-close).
  enter(immediate?: boolean): void;
  leave(): void;
  /// Close at once, cancelling any pending change.
  reset(): void;
}

export function createHoverIntent({
  openMs = HOVER_OPEN_MS,
  closeMs = HOVER_CLOSE_MS,
  isHeld,
}: {
  openMs?: number;
  closeMs?: number;
  /// While true the panel stays open after the pointer leaves (a context
  /// menu opened from it sits outside its box); checked again every `closeMs`.
  isHeld?: () => boolean;
} = {}): HoverIntent {
  let open = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clear() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  }

  function tryClose() {
    timer = null;
    if (isHeld?.()) {
      timer = setTimeout(tryClose, closeMs);
      return;
    }
    open = false;
  }

  return {
    get open() {
      return open;
    },
    enter(immediate = false) {
      clear();
      if (open) return;
      if (immediate) {
        open = true;
        return;
      }
      timer = setTimeout(() => {
        timer = null;
        open = true;
      }, openMs);
    },
    leave() {
      clear();
      if (open) timer = setTimeout(tryClose, closeMs);
    },
    reset() {
      clear();
      open = false;
    },
  };
}

/// Whether a pointer at (x, y) sits on the window's right edge strip that
/// peeks the closed queue: below the title bar, above the player bar.
export function atQueueEdge(
  x: number,
  y: number,
  view: { width: number; height: number; top: number; bottom: number },
  edge = QUEUE_PEEK_EDGE_PX,
): boolean {
  return x >= view.width - edge && y >= view.top && y < view.height - view.bottom;
}
