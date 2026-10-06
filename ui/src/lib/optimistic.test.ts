import { describe, expect, it, vi } from "vitest";
import { optimistic } from "./optimistic";

describe("optimistic", () => {
  it("keeps the new value when persistence succeeds", async () => {
    let value = false;
    const set = (next: boolean) => (value = next);
    await optimistic(value, true, set, async () => {});
    expect(value).toBe(true);
  });

  it("reverts and reports when persistence fails", async () => {
    let value = false;
    const set = (next: boolean) => (value = next);
    const onError = vi.fn();
    const error = new Error("disk full");
    await optimistic(value, true, set, async () => Promise.reject(error), onError);
    expect(value).toBe(false);
    expect(onError).toHaveBeenCalledWith(error);
  });
});
