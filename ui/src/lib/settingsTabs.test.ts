import { describe, it, expect } from "vitest";
import { nextTab } from "./settingsTabs";

describe("nextTab", () => {
  it("steps forward", () => {
    expect(nextTab("appearance", 1)).toBe("playback");
  });
  it("steps into the content tab after playback", () => {
    expect(nextTab("playback", 1)).toBe("content");
  });
  it("steps backward", () => {
    expect(nextTab("storage", -1)).toBe("connections");
  });
  it("wraps forward from the last tab", () => {
    expect(nextTab("system", 1)).toBe("appearance");
  });
  it("wraps backward from the first tab", () => {
    expect(nextTab("appearance", -1)).toBe("system");
  });
});
