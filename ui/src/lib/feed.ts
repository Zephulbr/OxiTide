import type { FeedActivity } from "./api";
import { t, type MessageKey } from "./i18n.svelte";

export interface FeedGroup {
  label: string;
  activities: FeedActivity[];
}

const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

const LABELS = ["feed.group.new", "feed.group.lastWeek", "feed.group.thisMonth", "feed.group.earlier"] as const satisfies readonly MessageKey[];

function bucketFor(entry: FeedActivity, now: Date): (typeof LABELS)[number] {
  const occurred = new Date(entry.occurredAt);
  if (Number.isNaN(occurred.getTime())) return "feed.group.earlier";
  const age = now.getTime() - occurred.getTime();
  if (age < WEEK_MS) return "feed.group.new";
  if (age < 2 * WEEK_MS) return "feed.group.lastWeek";
  const sameMonth =
    occurred.getUTCFullYear() === now.getUTCFullYear() &&
    occurred.getUTCMonth() === now.getUTCMonth();
  return sameMonth ? "feed.group.thisMonth" : "feed.group.earlier";
}

/// Group the flattened feed activities into the official client's date
/// buckets, newest first. `now` is injected so the buckets are testable.
export function groupActivities(activities: FeedActivity[], now: Date): FeedGroup[] {
  const buckets = new Map<string, FeedActivity[]>(LABELS.map((label) => [label, []]));
  const sorted = [...activities].sort(
    (a, b) => Date.parse(b.occurredAt) - Date.parse(a.occurredAt),
  );
  for (const entry of sorted) {
    buckets.get(bucketFor(entry, now))!.push(entry);
  }
  return [...buckets.entries()]
    .filter(([, entries]) => entries.length > 0)
    .map(([label, entries]) => ({ label: t(label as MessageKey), activities: entries }));
}
