import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createNotice } from "./notice.svelte";

describe("notice store", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("shows a message and auto-dismisses after six seconds", () => {
    const notice = createNotice();

    notice.show("hello");

    expect(notice.message).toBe("hello");
    vi.advanceTimersByTime(5999);
    expect(notice.message).toBe("hello");
    vi.advanceTimersByTime(1);
    expect(notice.message).toBeNull();
  });

  it("resets the timer when a new message arrives", () => {
    const notice = createNotice();

    notice.show("one");
    vi.advanceTimersByTime(5000);
    notice.show("two");
    vi.advanceTimersByTime(5000);
    expect(notice.message).toBe("two");
    vi.advanceTimersByTime(1000);
    expect(notice.message).toBeNull();
  });

  it("dismiss clears immediately", () => {
    const notice = createNotice();

    notice.show("x");
    notice.dismiss();

    expect(notice.message).toBeNull();
    vi.advanceTimersByTime(6000);
    expect(notice.message).toBeNull();
  });
});
