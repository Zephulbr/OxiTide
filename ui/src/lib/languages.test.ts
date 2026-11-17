import { describe, expect, it } from "vitest";
import { LANGUAGES, matchLanguage } from "./languages";

const ALL = LANGUAGES.map((l) => l.code);

describe("matchLanguage", () => {
  it("matches by base language", () => {
    expect(matchLanguage(["de-AT"], ALL)).toBe("de");
    expect(matchLanguage(["fr-CA", "en-US"], ALL)).toBe("fr");
    expect(matchLanguage(["el-GR"], ALL)).toBe("el");
  });

  it("picks Chinese by script", () => {
    expect(matchLanguage(["zh-CN"], ALL)).toBe("zh-Hans");
    expect(matchLanguage(["zh-SG"], ALL)).toBe("zh-Hans");
    expect(matchLanguage(["zh"], ALL)).toBe("zh-Hans");
    expect(matchLanguage(["zh-TW"], ALL)).toBe("zh-Hant");
    expect(matchLanguage(["zh-HK"], ALL)).toBe("zh-Hant");
    expect(matchLanguage(["zh-MO"], ALL)).toBe("zh-Hant");
    expect(matchLanguage(["zh-Hant-CN"], ALL)).toBe("zh-Hant");
  });

  it("sends every Portuguese to pt-BR and Norwegian to nb", () => {
    expect(matchLanguage(["pt-PT"], ALL)).toBe("pt-BR");
    expect(matchLanguage(["pt"], ALL)).toBe("pt-BR");
    expect(matchLanguage(["no"], ALL)).toBe("nb");
    expect(matchLanguage(["nn-NO"], ALL)).toBe("nb");
    expect(matchLanguage(["nb-NO"], ALL)).toBe("nb");
  });

  it("walks the preference list in order and falls back to English", () => {
    expect(matchLanguage(["th-TH", "ja-JP"], ALL)).toBe("ja");
    expect(matchLanguage(["th-TH", "ar"], ALL)).toBe("en");
    expect(matchLanguage([], ALL)).toBe("en");
    expect(matchLanguage(["not a tag"], ALL)).toBe("en");
  });

  it("only returns languages that are available", () => {
    expect(matchLanguage(["de", "fr"], ["en", "fr"])).toBe("fr");
    expect(matchLanguage(["zh-TW"], ["en", "zh-Hans"])).toBe("en");
  });
});

describe("LANGUAGES", () => {
  it("lists English first and every code once", () => {
    expect(ALL[0]).toBe("en");
    expect(new Set(ALL).size).toBe(ALL.length);
    expect(ALL).toHaveLength(24);
  });

  it("uses valid BCP 47 codes", () => {
    for (const code of ALL) expect(() => new Intl.Locale(code)).not.toThrow();
  });
});
