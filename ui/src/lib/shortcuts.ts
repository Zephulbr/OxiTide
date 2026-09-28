export type ShortcutAction =
  | "play-pause"
  | "seek-back"
  | "seek-forward"
  | "prev-track"
  | "next-track"
  | "volume-up"
  | "volume-down"
  | "focus-search"
  | "history-back"
  | "history-forward"
  | "escape";

export type EscapeTarget = "menu" | "mini-queue" | "exit-mini" | "now-playing" | "queue" | null;

export interface EscapeLayers {
  menuOpen: boolean;
  miniActive: boolean;
  miniQueueOpen: boolean;
  nowPlayingOpen: boolean;
  queueOpen: boolean;
}

/// Which layer Esc closes (MP-D6/MP-D10), most modal first. `null` means Esc is
/// a no-op and the key keeps its native behavior.
export function escapeTarget(layers: EscapeLayers): EscapeTarget {
  if (layers.menuOpen) return "menu";
  if (layers.miniActive && layers.miniQueueOpen) return "mini-queue";
  if (layers.miniActive) return "exit-mini";
  if (layers.nowPlayingOpen) return "now-playing";
  if (layers.queueOpen) return "queue";
  return null;
}

export interface ShortcutHint {
  keys: string;
  description: string;
}

export const SHORTCUTS: ShortcutHint[] = [
  { keys: "Space", description: "Play / pause" },
  { keys: "← / →", description: "Seek −5 s / +5 s" },
  { keys: "Shift+← / Shift+→", description: "Previous / next track" },
  { keys: "↑ / ↓", description: "Volume +5% / −5%" },
  { keys: "Ctrl+F or /", description: "Focus search" },
  { keys: "Alt+← / Alt+→", description: "History back / forward" },
  { keys: "Esc", description: "Close the menu, mini queue, mini player, now playing, or queue" },
];

/// Duck-typed so the matcher stays testable without a DOM environment.
export function isTypingTarget(target: EventTarget | null): boolean {
  const el = target as { tagName?: unknown; isContentEditable?: unknown } | null;
  if (!el || typeof el.tagName !== "string") return false;
  return (
    el.tagName === "INPUT" ||
    el.tagName === "TEXTAREA" ||
    el.tagName === "SELECT" ||
    el.isContentEditable === true
  );
}

function isButtonLike(target: EventTarget | null): boolean {
  const el = target as { tagName?: unknown } | null;
  return el?.tagName === "BUTTON";
}

/// Whether the app will actually act on a matched shortcut. Transport keys
/// with nothing loaded are no-ops, so the key must keep its native behavior
/// (scrolling) instead of being swallowed by preventDefault.
export function isShortcutHandled(
  action: ShortcutAction,
  opts: { hasCurrent: boolean; hasQueue: boolean },
): boolean {
  switch (action) {
    case "play-pause":
    case "seek-back":
    case "seek-forward":
      return opts.hasCurrent;
    case "prev-track":
    case "next-track":
      return opts.hasQueue;
    default:
      return true;
  }
}

/// Map a keydown to an app action. Esc and Ctrl+F work while typing; every
/// other binding is ignored then. Esc is skipped entirely when a dialog is
/// open because the dialog owns it.
export function matchShortcut(
  event: KeyboardEvent,
  opts: { overlayOpen: boolean },
): ShortcutAction | null {
  const key = event.key;

  if (key === "Escape") return opts.overlayOpen ? null : "escape";
  if ((key === "f" || key === "F") && (event.ctrlKey || event.metaKey)) return "focus-search";
  if (isTypingTarget(event.target)) return null;

  if (key === " ") {
    if (event.repeat || isButtonLike(event.target)) return null;
    return "play-pause";
  }
  if (event.altKey && key === "ArrowLeft") return "history-back";
  if (event.altKey && key === "ArrowRight") return "history-forward";
  if (event.shiftKey && key === "ArrowLeft") return "prev-track";
  if (event.shiftKey && key === "ArrowRight") return "next-track";
  if (!event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
    if (key === "ArrowLeft") return "seek-back";
    if (key === "ArrowRight") return "seek-forward";
    if (key === "ArrowUp") return "volume-up";
    if (key === "ArrowDown") return "volume-down";
  }
  if (key === "/" && !event.altKey && !event.ctrlKey && !event.metaKey) return "focus-search";
  return null;
}
