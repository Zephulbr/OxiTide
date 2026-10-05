/// Global-hotkey helpers. The action ids mirror `HotkeysConfig` in Rust and
/// the `set_hotkey` command's whitelist; the accelerator format is
/// `tauri-plugin-global-shortcut`'s (`"Ctrl+Alt+KeyP"`, `"MediaPlayPause"`).

export const HOTKEY_ACTIONS = [
  { id: "play_pause", label: "Play / pause" },
  { id: "next", label: "Next track" },
  { id: "prev", label: "Previous track" },
  { id: "volume_up", label: "Volume up" },
  { id: "volume_down", label: "Volume down" },
  { id: "show_hide", label: "Show / hide window" },
] as const;

export type HotkeyAction = (typeof HOTKEY_ACTIONS)[number]["id"];

export interface KeyLike {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

/// Accelerator for a captured keydown, or null for lone modifier presses and
/// unidentified keys so the capture keeps waiting.
export function acceleratorFromEvent(event: KeyLike): string | null {
  if (!event.code || event.code === "Unidentified" || MODIFIER_CODES.has(event.code)) {
    return null;
  }
  const tokens: string[] = [];
  if (event.ctrlKey) tokens.push("Ctrl");
  if (event.altKey) tokens.push("Alt");
  if (event.shiftKey) tokens.push("Shift");
  if (event.metaKey) tokens.push("Super");
  tokens.push(event.code);
  return tokens.join("+");
}

const KEY_LABELS: Record<string, string> = {
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ArrowUp: "Up",
  ArrowDown: "Down",
  Escape: "Esc",
  MediaPlayPause: "Media Play/Pause",
  MediaTrackNext: "Media Next",
  MediaTrackPrevious: "Media Previous",
};

/// Human-readable form of a stored accelerator.
export function displayAccelerator(accelerator: string): string {
  if (!accelerator) return "";
  return accelerator
    .split("+")
    .map((token) => {
      if (/^Key[A-Z]$/.test(token)) return token.slice(3);
      if (/^Digit\d$/.test(token)) return token.slice(5);
      return KEY_LABELS[token] ?? token;
    })
    .join(" + ");
}
