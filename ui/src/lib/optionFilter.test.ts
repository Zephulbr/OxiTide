import { describe, expect, it } from "vitest";
import { filterOptions, foldForSearch } from "./optionFilter";

const options = [
  { value: "system", label: "System (English)" },
  { value: "de", label: "Deutsch", keywords: ["German", "de"] },
  { value: "es", label: "Español", keywords: ["Spanish", "es"] },
  { value: "tr", label: "Türkçe", keywords: ["Turkish", "tr"] },
  { value: "ja", label: "日本語", keywords: ["Japanese", "ja"] },
];

const values = (query: string) => filterOptions(options, query).map((o) => o.value);

describe("filterOptions", () => {
  it("keeps everything for an empty or blank query", () => {
    expect(values("")).toHaveLength(5);
    expect(values("   ")).toHaveLength(5);
  });

  it("matches the label and the keywords, ignoring case and accents", () => {
    expect(values("deut")).toEqual(["de"]);
    expect(values("GERMAN")).toEqual(["de"]);
    expect(values("espanol")).toEqual(["es"]);
    expect(values("turkce")).toEqual(["tr"]);
    expect(values("日本")).toEqual(["ja"]);
    expect(values("ja")).toEqual(["ja"]);
  });

  it("returns nothing when no option matches", () => {
    expect(values("klingon")).toEqual([]);
  });

  it("folds for comparison", () => {
    expect(foldForSearch("  Ελληνικά ")).toBe("ελληνικα");
  });
});
