import type { PageItem, PageSection } from "./api";
import { classifySection, isPageLink } from "./media";

/// Explore's page-link tiles (spec 2026-10-16 Explore redesign). TIDAL sends
/// genres, moods and decades without artwork, so each tile gets a colour wash
/// whose hue is derived from its name: stable across launches and modes.

/// Hue pair (degrees) for a tile's two-stop wash, from an FNV-1a hash of the
/// lowercased title. The second stop sits 40° on, so neighbours differ in
/// direction as well as base hue.
export function linkHues(title: string): [number, number] {
  let hash = 0x811c9dc5;
  for (const char of title.trim().toLowerCase()) {
    hash ^= char.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  const hue = hash % 360;
  return [hue, (hue + 40) % 360];
}

const DECADE = /^(?:19|20)?\d0['’]?s$/i;

/// A row whose every title is a decade ("1990s", "80s") renders as numerals.
export function isDecadeRow(items: PageItem[]): boolean {
  return items.length > 0 && items.every((item) => DECADE.test(item.title.trim()));
}

const VIDEO = /video/i;

/// Page links to video pages: OxiTide cannot play videos, so they go nowhere useful.
export function isVideoLink(item: PageItem): boolean {
  return isPageLink(item) && (VIDEO.test(item.api_path ?? "") || VIDEO.test(item.title));
}

/// Sections without video links; a section left empty by that is dropped.
export function withoutVideoLinks(sections: PageSection[]): PageSection[] {
  return sections.flatMap((section) => {
    const items = section.items.filter((item) => !isVideoLink(item));
    if (items.length === 0 && section.items.length > 0) return [];
    return [items.length === section.items.length ? section : { ...section, items }];
  });
}

/// Explore's untitled page-link rows (New, Top, HiRes, …) lead the page, right
/// under "New to you"; titled rows (Genres, Moods, Decades) keep their order.
export function splitQuickLinks(sections: PageSection[]): {
  quick: PageSection[];
  rest: PageSection[];
} {
  const quick: PageSection[] = [];
  const rest: PageSection[] = [];
  for (const section of sections) {
    const isQuick =
      section.title.trim().length === 0 && classifySection(section.kind, section.items) === "links";
    (isQuick ? quick : rest).push(section);
  }
  return { quick, rest };
}
