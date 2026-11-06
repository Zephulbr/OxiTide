// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { dismissBootScreen } from "./bootScreen";

describe("dismissBootScreen", () => {
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("fades the boot screen out, then removes it", () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="boot"></div><div id="app"></div>';
    dismissBootScreen();
    const boot = document.getElementById("boot");
    expect(boot?.classList.contains("boot-done")).toBe(true);
    vi.advanceTimersByTime(400);
    expect(document.getElementById("boot")).toBeNull();
    expect(document.getElementById("app")).not.toBeNull();
  });

  it("removes it as soon as the fade ends", () => {
    document.body.innerHTML = '<div id="boot"></div>';
    dismissBootScreen();
    document.getElementById("boot")!.dispatchEvent(new Event("transitionend"));
    expect(document.getElementById("boot")).toBeNull();
  });

  it("is a no-op without a boot screen or when called twice", () => {
    expect(() => dismissBootScreen()).not.toThrow();
    document.body.innerHTML = '<div id="boot"></div>';
    dismissBootScreen();
    expect(() => dismissBootScreen()).not.toThrow();
  });
});
