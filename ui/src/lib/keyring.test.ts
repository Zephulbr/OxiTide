import { describe, expect, it } from "vitest";
import { keyringNotice } from "./keyring";

describe("keyringNotice", () => {
  it("is hidden while loading and when persistence is available", () => {
    expect(keyringNotice(null)).toBeNull();
    expect(keyringNotice({ persistent: true, reason: null })).toBeNull();
    expect(keyringNotice({ persistent: true })).toBeNull();
  });

  it("explains the session-only consequence and links the providers", () => {
    const copy = keyringNotice({ persistent: false, reason: "no secret service" });
    expect(copy).not.toBeNull();
    expect(copy!.body).toContain("Secret Service");
    expect(copy!.body).toContain("session");
    expect(copy!.links.map((link) => link.label)).toEqual(["gnome-keyring", "KWallet"]);
    expect(copy!.links.every((link) => link.url.startsWith("https://"))).toBe(true);
  });
});
