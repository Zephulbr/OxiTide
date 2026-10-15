import type { FeedActivity, PageItem, PageSection, Stats } from "./api";
import { shapeForItemType } from "./media";

/// Time-of-day greeting for the Home hero (local hour, 0-23).
export function greeting(hour: number): string {
  if (hour >= 5 && hour < 12) return "Good morning";
  if (hour >= 12 && hour < 18) return "Good afternoon";
  if (hour >= 18 && hour < 23) return "Good evening";
  return "Good night";
}

/// "Made for you": the mix cards from the For You feed (daily/discovery/
/// artist mixes). A feed with no mixes falls back to its first non-empty row so
/// the shelf still shows something personal.
export function madeForYou(sections: PageSection[], cap = 12): PageItem[] {
  const all = sections.flatMap((section) => section.items);
  let picked = all.filter((item) => shapeForItemType(item.item_type) === "mix");
  if (picked.length === 0) picked = sections.find((s) => s.items.length > 0)?.items ?? [];
  const seen = new Set<string>();
  const out: PageItem[] = [];
  for (const item of picked) {
    if (!item.id || seen.has(item.id)) continue;
    seen.add(item.id);
    out.push(item);
    if (out.length >= cap) break;
  }
  return out;
}

/// Feed releases as album cards for a shelf.
export function feedAlbumItems(activities: FeedActivity[], cap = 12): PageItem[] {
  return activities.slice(0, cap).map((activity) => ({
    id: String(activity.id),
    title: activity.title,
    subtitle: activity.subtitle,
    item_type: "ALBUM",
    artwork: activity.artwork ?? null,
    explicit: activity.explicit,
  }));
}

/// Number of unseen releases in the feed (drives the shelf's "N new" label).
export function unseenCount(activities: FeedActivity[]): number {
  return activities.filter((activity) => !activity.seen).length;
}

/// Editorial rows trail the personal shelves, so only the first few show.
export function editorialSections(sections: PageSection[], cap = 4): PageSection[] {
  return sections.filter((section) => section.items.length > 0).slice(0, cap);
}

/// The last `days` entries of the stats' daily series (oldest first).
export function lastDays(stats: Stats, days = 7) {
  return stats.daily.slice(-days);
}
