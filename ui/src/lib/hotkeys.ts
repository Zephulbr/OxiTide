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

export function actionLabel(action: HotkeyAction): string {
  return HOTKEY_ACTIONS.find((entry) => entry.id === action)?.label ?? action;
}

const MODIFIER_ALIASES: Record<string, "alt" | "ctrl" | "shift" | "super"> = {
  ALT: "alt",
  OPTION: "alt",
  CONTROL: "ctrl",
  CTRL: "ctrl",
  COMMAND: "super",
  CMD: "super",
  SUPER: "super",
  META: "super",
  SHIFT: "shift",
};

const KEY_ALIASES: Record<string, string> = {
  LEFT: "ARROWLEFT",
  RIGHT: "ARROWRIGHT",
  UP: "ARROWUP",
  DOWN: "ARROWDOWN",
  ESC: "ESCAPE",
  "`": "BACKQUOTE",
  "\\": "BACKSLASH",
  "[": "BRACKETLEFT",
  "]": "BRACKETRIGHT",
  ",": "COMMA",
  "-": "MINUS",
  ".": "PERIOD",
  "'": "QUOTE",
  ";": "SEMICOLON",
  "/": "SLASH",
  "=": "EQUAL",
};

/// Parser-equivalent fingerprint (modifier order, aliases, key spelling) used
/// only for conflict detection; null when the accelerator is invalid.
export function normalizeAccelerator(accelerator: string): string | null {
  const tokens = accelerator.split("+").map((token) => token.trim()).filter(Boolean);
  if (tokens.length === 0) return null;
  const modifiers = new Set<string>();
  let key: string | null = null;
  for (const token of tokens) {
    const upper = token.toUpperCase();
    const modifier = MODIFIER_ALIASES[upper];
    if (modifier) {
      modifiers.add(modifier);
      continue;
    }
    if (key !== null) return null;
    if (/^KEY[A-Z]$/.test(upper)) key = upper.slice(3);
    else if (/^DIGIT\d$/.test(upper)) key = upper.slice(5);
    else key = KEY_ALIASES[upper] ?? upper;
  }
  if (key === null) return null;
  return `${[...modifiers].sort().join("+")}|${key}`;
}

/// The other action already bound to `accelerator`, if any.
export function conflictFor(
  bindings: Record<HotkeyAction, string>,
  action: HotkeyAction,
  accelerator: string,
): HotkeyAction | null {
  const target = normalizeAccelerator(accelerator);
  if (!target) return null;
  for (const entry of HOTKEY_ACTIONS) {
    if (entry.id === action) continue;
    if (normalizeAccelerator(bindings[entry.id]) === target) return entry.id;
  }
  return null;
}

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
