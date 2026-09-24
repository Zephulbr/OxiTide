import { describe, expect, it } from "vitest";
import { isTypingTarget, matchShortcut, SHORTCUTS } from "./shortcuts";

function keyEvent(key: string, overrides: Partial<KeyboardEvent> = {}): KeyboardEvent {
  return {
    key,
    repeat: false,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    target: null,
    ...overrides,
  } as unknown as KeyboardEvent;
}

function target(tagName: string, isContentEditable = false): EventTarget {
  return { tagName, isContentEditable } as unknown as EventTarget;
}

const app = { overlayOpen: false };

describe("matchShortcut", () => {
  it("maps every documented key", () => {
    expect(matchShortcut(keyEvent(" "), app)).toBe("play-pause");
    expect(matchShortcut(keyEvent("ArrowLeft"), app)).toBe("seek-back");
    expect(matchShortcut(keyEvent("ArrowRight"), app)).toBe("seek-forward");
    expect(matchShortcut(keyEvent("ArrowLeft", { shiftKey: true }), app)).toBe("prev-track");
    expect(matchShortcut(keyEvent("ArrowRight", { shiftKey: true }), app)).toBe("next-track");
    expect(matchShortcut(keyEvent("ArrowUp"), app)).toBe("volume-up");
    expect(matchShortcut(keyEvent("ArrowDown"), app)).toBe("volume-down");
    expect(matchShortcut(keyEvent("f", { ctrlKey: true }), app)).toBe("focus-search");
    expect(matchShortcut(keyEvent("/"), app)).toBe("focus-search");
    expect(matchShortcut(keyEvent("ArrowLeft", { altKey: true }), app)).toBe("history-back");
    expect(matchShortcut(keyEvent("ArrowRight", { altKey: true }), app)).toBe("history-forward");
    expect(matchShortcut(keyEvent("Escape"), app)).toBe("escape");
  });

  it("ignores shortcuts while typing except Esc and Ctrl+F", () => {
    const input = target("INPUT");

    expect(matchShortcut(keyEvent(" ", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("ArrowUp", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("ArrowLeft", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("/", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("Escape", { target: input }), app)).toBe("escape");
    expect(matchShortcut(keyEvent("f", { ctrlKey: true, target: input }), app)).toBe(
      "focus-search",
    );
  });

  it("ignores a held Space repeat but allows seek and volume repeats", () => {
    expect(matchShortcut(keyEvent(" ", { repeat: true }), app)).toBeNull();
    expect(matchShortcut(keyEvent("ArrowRight", { repeat: true }), app)).toBe("seek-forward");
    expect(matchShortcut(keyEvent("ArrowUp", { repeat: true }), app)).toBe("volume-up");
  });

  it("leaves Space on a focused button to the button", () => {
    expect(matchShortcut(keyEvent(" ", { target: target("BUTTON") }), app)).toBeNull();
  });

  it("skips Esc when a dialog owns it", () => {
    expect(matchShortcut(keyEvent("Escape"), { overlayOpen: true })).toBeNull();
  });
});

describe("isTypingTarget", () => {
  it("accepts inputs, textareas, selects, and contenteditable", () => {
    expect(isTypingTarget(target("INPUT"))).toBe(true);
    expect(isTypingTarget(target("TEXTAREA"))).toBe(true);
    expect(isTypingTarget(target("SELECT"))).toBe(true);
    expect(isTypingTarget(target("DIV", true))).toBe(true);
  });

  it("rejects everything else", () => {
    expect(isTypingTarget(null)).toBe(false);
    expect(isTypingTarget(target("DIV"))).toBe(false);
    expect(isTypingTarget(target("BUTTON"))).toBe(false);
  });
});

describe("SHORTCUTS", () => {
  it("documents the seven rows shown in Settings", () => {
    expect(SHORTCUTS).toHaveLength(7);
    for (const hint of SHORTCUTS) {
      expect(hint.keys.length).toBeGreaterThan(0);
      expect(hint.description.length).toBeGreaterThan(0);
    }
  });
});
