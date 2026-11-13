import { describe, expect, it } from "vitest";
import {
  LANGUAGE_STORAGE_KEY,
  normalizeSetting,
  readLanguageMirror,
  resolveSetting,
  systemLanguage,
} from "./language.svelte";

function store(value: string | null) {
  return { getItem: (key: string) => (key === LANGUAGE_STORAGE_KEY ? value : null), setItem: () => {} };
}

describe("language setting", () => {
  it("keeps known codes and treats anything else as system", () => {
    expect(normalizeSetting("de")).toBe("de");
    expect(normalizeSetting("zh-Hant")).toBe("zh-Hant");
    expect(normalizeSetting("en-XA")).toBe("en-XA");
    expect(normalizeSetting("system")).toBe("system");
    expect(normalizeSetting(undefined)).toBe("system");
    expect(normalizeSetting("xx")).toBe("system");
  });

  it("resolves system through the webview's languages", () => {
    expect(resolveSetting("system", () => "fr")).toBe("fr");
    expect(resolveSetting("de", () => "fr")).toBe("de");
  });

  it("matches the system language only against catalogs that exist", () => {
    expect(systemLanguage(["de-DE", "en-US"], ["en", "de"])).toBe("de");
    expect(systemLanguage(["de-DE"], ["en"])).toBe("en");
    // The pseudo-locale is never a system match.
    expect(systemLanguage(["en-XA"], ["en", "en-XA"])).toBe("en");
  });

  it("reads the boot mirror defensively", () => {
    expect(readLanguageMirror(store("ja"))).toBe("ja");
    expect(readLanguageMirror(store(null))).toBe("en");
    expect(readLanguageMirror(store("<script>"))).toBe("en");
    expect(
      readLanguageMirror({
        getItem: () => {
          throw new Error("blocked");
        },
        setItem: () => {},
      }),
    ).toBe("en");
    expect(readLanguageMirror(undefined)).toBe("en");
  });
});
