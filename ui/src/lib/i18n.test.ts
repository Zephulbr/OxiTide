import { describe, expect, it } from "vitest";
import { createI18n, i18n, t } from "./i18n.svelte";
import { routeTitle } from "./titles";

const source = {
  hello: "Hello",
  greet: "Hello, {name}",
  tracks: "{count, plural, one {# track} other {# tracks}}",
  quote: "Don't stop",
  key: "Press <Enter>",
  onlyEnglish: "Only in English",
};
type Key = keyof typeof source;

function make(extra: Record<string, Record<string, string>> = {}) {
  const loaders: Record<string, () => Promise<Record<string, string>>> = {
    "../locales/de/app.json": async () => ({
      hello: "Hallo",
      greet: "Hallo, {name}",
      tracks: "{count, plural, one {# Titel} other {# Titel}}",
    }),
    "../locales/pl/app.json": async () => ({
      tracks: "{count, plural, one {# utwór} few {# utwory} many {# utworów} other {# utworu}}",
    }),
    "../locales/ja/app.json": async () => ({ tracks: "{count}曲", broken: "{oops" }),
  };
  for (const [code, catalog] of Object.entries(extra)) {
    loaders[`../locales/${code}/app.json`] = async () => catalog;
  }
  return createI18n<Key | "broken">(source, loaders);
}

describe("createI18n", () => {
  it("formats English by default", () => {
    const tr = make();
    expect(tr.language).toBe("en");
    expect(tr.t("hello")).toBe("Hello");
    expect(tr.t("greet", { name: "V" })).toBe("Hello, V");
    expect(tr.t("tracks", { count: 1 })).toBe("1 track");
    expect(tr.t("tracks", { count: 3 })).toBe("3 tracks");
  });

  it("keeps apostrophes and angle brackets literal", () => {
    const tr = make();
    expect(tr.t("quote")).toBe("Don't stop");
    expect(tr.t("key")).toBe("Press <Enter>");
  });

  it("switches language after the catalog loads", async () => {
    const tr = make();
    await expect(tr.setLanguage("de")).resolves.toBe("de");
    expect(tr.language).toBe("de");
    expect(tr.t("greet", { name: "V" })).toBe("Hallo, V");
  });

  it("uses each language's plural rules", async () => {
    const tr = make();
    await tr.setLanguage("pl");
    expect(tr.t("tracks", { count: 1 })).toBe("1 utwór");
    expect(tr.t("tracks", { count: 3 })).toBe("3 utwory");
    expect(tr.t("tracks", { count: 5 })).toBe("5 utworów");
    await tr.setLanguage("ja");
    expect(tr.t("tracks", { count: 5 })).toBe("5曲");
  });

  it("falls back to English for a missing or broken message", async () => {
    const tr = make();
    await tr.setLanguage("de");
    expect(tr.t("onlyEnglish")).toBe("Only in English");
    await tr.setLanguage("ja");
    expect(tr.t("broken")).toBe("broken");
  });

  it("falls back to English for an unknown language", async () => {
    const tr = make();
    await expect(tr.setLanguage("xx")).resolves.toBe("en");
    expect(tr.language).toBe("en");
  });

  it("lists the languages it has catalogs for", () => {
    expect(make({ fr: {} }).available().sort()).toEqual(["de", "en", "fr", "ja", "pl"]);
  });

  it("formats numbers in placeholders for the language", async () => {
    const tr = createI18n<"n">({ n: "{value, number}" }, {
      "../locales/de/app.json": async () => ({ n: "{value, number}" }),
    });
    expect(tr.t("n", { value: 1234.5 })).toBe("1,234.5");
    await tr.setLanguage("de");
    expect(tr.t("n", { value: 1234.5 })).toBe("1.234,5");
  });
});

describe("app catalog", () => {
  it("serves the bundled English messages", () => {
    expect(i18n.language).toBe("en");
    expect(t("player.next")).toBe("Next");
    expect(t("app.windowTitle", { page: t("route.home") })).toBe("Home · OxiTide");
  });

  it("lazy-loads a translation and switches back", async () => {
    try {
      await expect(i18n.setLanguage("de")).resolves.toBe("de");
      expect(t("player.next")).toBe("Weiter");
      expect(routeTitle({ name: "settings" })).toBe("Einstellungen · OxiTide");
      expect(i18n.available()).toContain("de");
      expect(i18n.available()).not.toContain("notes");
    } finally {
      await i18n.setLanguage("en");
    }
    expect(t("player.next")).toBe("Next");
  });
});
