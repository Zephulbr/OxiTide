import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, displayAccelerator } from "./hotkeys";

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

describe("displayAccelerator", () => {
  it("prettifies stored tokens", () => {
    expect(displayAccelerator("Ctrl+Alt+KeyP")).toBe("Ctrl + Alt + P");
    expect(displayAccelerator("Shift+Digit1")).toBe("Shift + 1");
    expect(displayAccelerator("MediaPlayPause")).toBe("Media Play/Pause");
    expect(displayAccelerator("")).toBe("");
  });
});
