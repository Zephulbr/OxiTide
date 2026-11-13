// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { initRemoteLanguage, remoteI18n, remoteLanguages, t } from "./i18n.svelte";

describe("remote translator", () => {
  it("serves remote.json, not the app catalog", () => {
    expect(t("tab.queue")).toBe("Queue");
    expect(t("common.trackCount", { count: 2 })).toBe("2 tracks");
  });

  it("offers only languages with a remote catalog, English first", () => {
    const codes = remoteLanguages(true);
    expect(codes[0]).toBe("en");
    expect(codes.every((code) => code === "en" || remoteI18n.available().includes(code))).toBe(true);
  });

  it("follows the phone's languages and falls back to English", async () => {
    await expect(initRemoteLanguage(["xx-YY"])).resolves.toBe("en");
    expect(document.documentElement.lang).toBe("en");
  });
});
