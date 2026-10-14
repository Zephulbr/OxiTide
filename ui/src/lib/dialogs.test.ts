import { describe, expect, it } from "vitest";
import { createDialogs } from "./dialogs.svelte";

describe("dialog registry", () => {
  it("counts concurrent open dialogs and unregisters cleanly", () => {
    const dialogs = createDialogs();
    expect(dialogs.count).toBe(0);

    const closeA = dialogs.register();
    const closeB = dialogs.register();
    expect(dialogs.count).toBe(2);

    closeA();
    expect(dialogs.count).toBe(1);
    closeB();
    expect(dialogs.count).toBe(0);
  });
});
