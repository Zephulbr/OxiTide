import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createToasts } from "./toasts.svelte";

describe("toast store", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("defaults to info severity and returns an id", () => {
    const toasts = createToasts();

    const id = toasts.show("hello");

    expect(id).toBeGreaterThan(0);
    expect(toasts.toasts).toHaveLength(1);
    expect(toasts.toasts[0].severity).toBe("info");
  });

  it("auto-dismisses success and info after six seconds", () => {
    const toasts = createToasts();
    toasts.show("ok", { severity: "success" });
    toasts.show("fyi");

    vi.advanceTimersByTime(5999);
    expect(toasts.toasts).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(toasts.toasts).toHaveLength(0);
  });

  it("keeps errors until dismissed", () => {
    const toasts = createToasts();
    toasts.show("boom", { severity: "error" });

    vi.advanceTimersByTime(60_000);
    expect(toasts.toasts).toHaveLength(1);

    toasts.dismiss(toasts.toasts[0].id);
    expect(toasts.toasts).toHaveLength(0);
  });

  it("clears the timer on dismiss and leaves other timers running", () => {
    const toasts = createToasts();
    const first = toasts.show("one");
    toasts.show("two");

    toasts.dismiss(first);

    expect(vi.getTimerCount()).toBe(1);
    expect(toasts.toasts).toHaveLength(1);
  });

  it("caps at three, evicting the oldest auto-dismissing toast first", () => {
    const toasts = createToasts();
    toasts.show("one");
    toasts.show("two");
    toasts.show("boom", { severity: "error" });

    toasts.show("four");

    expect(toasts.toasts.map((t) => t.message)).toEqual(["two", "boom", "four"]);
  });

  it("evicts the oldest error when no auto-dismissing toast is present", () => {
    const toasts = createToasts();
    toasts.show("e1", { severity: "error" });
    toasts.show("e2", { severity: "error" });
    toasts.show("e3", { severity: "error" });

    toasts.show("e4", { severity: "error" });

    expect(toasts.toasts.map((t) => t.message)).toEqual(["e2", "e3", "e4"]);
  });

  it("retains the optional action", () => {
    const toasts = createToasts();
    const run = vi.fn();
    toasts.show("did it", { action: { label: "Undo", run } });

    expect(toasts.toasts[0].action?.label).toBe("Undo");
    toasts.toasts[0].action?.run();
    expect(run).toHaveBeenCalledTimes(1);
  });
});
