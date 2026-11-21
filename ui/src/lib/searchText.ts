/// Text matching for the in-list filters (track lists, Library grids; desktop
/// and phone remote). Case, accents and punctuation are folded away on both
/// sides, so a filter finds what a person types: "ac dc", "acdc" and "ac/dc"
/// find AC/DC, "beyonce" finds Beyoncé, "guns n roses" finds Guns N' Roses,
/// "dont" finds Don't. Pure: the phone page imports it (`remote/imports.test.ts`).

/// Runs of anything but letters, digits and combining marks (scripts such as
/// Devanagari carry their vowels as marks) separate words.
const SEPARATORS = /[^\p{L}\p{N}\p{M}]+/gu;
/// Punctuation alone: dropped inside a word ("AC/DC" -> "acdc").
const PUNCTUATION = /[^\p{L}\p{N}\p{M}\s]+/gu;
const SPACES = /\s+/gu;

function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLocaleLowerCase();
}

/// The query as matched: folded, its words joined by single spaces. Empty
/// when the query has no letters or digits (nothing to filter by).
export function searchNeedle(query: string): string {
  return fold(query).replace(SEPARATORS, " ").trim();
}

/// What one field is matched against: its words joined by single spaces
/// ("ac dc"), and the same with in-word punctuation dropped ("acdc"). Two
/// forms rather than one with every space removed, so "ear" does not find
/// "The Arcade".
export function searchForms(text: string): string[] {
  const folded = fold(text);
  const spaced = folded.replace(SEPARATORS, " ").trim();
  const compact = folded.replace(PUNCTUATION, "").replace(SPACES, " ").trim();
  return compact === spaced ? [spaced] : [spaced, compact];
}

/// Whether any form contains a non-empty needle from `searchNeedle`.
export function formsMatch(forms: readonly string[], needle: string): boolean {
  return forms.some((form) => form.includes(needle));
}

/// One-off match of a field against a query; lists that filter on every
/// keystroke should cache `searchForms` per row instead.
export function textMatches(text: string, query: string): boolean {
  const needle = searchNeedle(query);
  return needle === "" || formsMatch(searchForms(text), needle);
}
