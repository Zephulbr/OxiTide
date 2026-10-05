import { describe, expect, it } from "vitest";
import {
  acceleratorFromEvent,
  conflictFor,
  displayAccelerator,
  normalizeAccelerator,
  type HotkeyAction,
} from "./hotkeys";

function bindings(overrides: Partial<Record<HotkeyAction, string>> = {}) {
  return {
    play_pause: "",
    next: "",
    prev: "",
    volume_up: "",
    volume_down: "",
    show_hide: "",
    ...overrides,
  };
}

function key(
  code: string,
  mods: Partial<{ ctrl: boolean; alt: boolean; shift: boolean; meta: boolean }> = {},
) {
  return {
    code,
    ctrlKey: mods.ctrl ?? false,
    altKey: mods.alt ?? false,
    shiftKey: mods.shift ?? false,
    metaKey: mods.meta ?? false,
  };
}

describe("acceleratorFromEvent", () => {
  it("builds modifier+key accelerators", () => {
    expect(acceleratorFromEvent(key("KeyP", { ctrl: true, alt: true }))).toBe("Ctrl+Alt+KeyP");
    expect(acceleratorFromEvent(key("Space", { ctrl: true }))).toBe("Ctrl+Space");
    expect(acceleratorFromEvent(key("MediaPlayPause"))).toBe("MediaPlayPause");
    expect(acceleratorFromEvent(key("F12"))).toBe("F12");
  });

  it("ignores lone modifier presses and unidentified keys", () => {
    expect(acceleratorFromEvent(key("ControlLeft", { ctrl: true }))).toBeNull();
    expect(acceleratorFromEvent(key("ShiftRight", { shift: true }))).toBeNull();
    expect(acceleratorFromEvent(key("Unidentified"))).toBeNull();
  });
});

describe("normalizeAccelerator", () => {
  it("normalizes aliases, case, and modifier order", () => {
    expect(normalizeAccelerator("Ctrl+Alt+KeyP")).toBe(normalizeAccelerator("alt+control+P"));
    expect(normalizeAccelerator("CMD+KeyM")).toBe(normalizeAccelerator("super+KeyM"));
    expect(normalizeAccelerator("Digit1")).toBe(normalizeAccelerator("1"));
  });

  it("rejects empty, modifier-only, and multi-key input", () => {
    expect(normalizeAccelerator("")).toBeNull();
    expect(normalizeAccelerator("Ctrl+Shift")).toBeNull();
    expect(normalizeAccelerator("Ctrl+A+B")).toBeNull();
  });
});

describe("conflictFor", () => {
  it("finds the action holding the same chord", () => {
    const current = bindings({ volume_up: "Ctrl+Alt+Space" });
    expect(conflictFor(current, "play_pause", "ctrl+alt+space")).toBe("volume_up");
  });

  it("ignores the action itself, empty chords, and free chords", () => {
    const current = bindings({ play_pause: "Ctrl+Alt+Space" });
    expect(conflictFor(current, "play_pause", "Ctrl+Alt+Space")).toBeNull();
    expect(conflictFor(current, "next", "")).toBeNull();
    expect(conflictFor(current, "next", "F9")).toBeNull();
  });
});

describe("displayAccelerator", () => {
  it("prettifies stored tokens", () => {
    expect(displayAccelerator("Ctrl+Alt+KeyP")).toBe("Ctrl + Alt + P");
    expect(displayAccelerator("Shift+Digit1")).toBe("Shift + 1");
    expect(displayAccelerator("MediaPlayPause")).toBe("Media Play/Pause");
    expect(displayAccelerator("")).toBe("");
  });
});
