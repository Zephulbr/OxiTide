import { describe, expect, it } from "vitest";
import { formsMatch, searchForms, searchNeedle, textMatches } from "./searchText";

describe("list filter text", () => {
  it.each([
    ["AC/DC", "ac dc"],
    ["AC/DC", "AC DC"],
    ["AC/DC", "acdc"],
    ["AC/DC", "ac/dc"],
    ["AC/DC", "ac-dc"],
    ["Guns N' Roses", "guns n roses"],
    ["Don't Stop Me Now", "dont stop"],
    ["Don't Stop Me Now", "don't"],
    ["blink-182", "blink 182"],
    ["blink-182", "blink182"],
    ["Beyoncé", "beyonce"],
    ["Mötley Crüe", "motley crue"],
    ["a-ha", "a ha"],
    ["Liar Veil", "  veil "],
    ["Ελληνικά", "ελληνικα"],
    ["東京事変", "東京"],
  ])("%s is found by %j", (text, query) => {
    expect(textMatches(text, query)).toBe(true);
  });

  it.each([
    ["The Arcade", "ear"],
    ["AC/DC", "acd c"],
    ["Green Day", "red"],
  ])("%s is not found by %j", (text, query) => {
    expect(textMatches(text, query)).toBe(false);
  });

  it("treats a query with no letters or digits as no filter", () => {
    expect(searchNeedle(" / - ")).toBe("");
    expect(textMatches("anything", "/")).toBe(true);
  });

  it("keeps one form when punctuation changes nothing", () => {
    expect(searchForms("Green Day")).toEqual(["green day"]);
    expect(searchForms("AC/DC")).toEqual(["ac dc", "acdc"]);
    expect(formsMatch(searchForms("AC/DC"), searchNeedle("acdc"))).toBe(true);
  });
});
