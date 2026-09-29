import type { FeedActivity } from "./api";

export interface FeedGroup {
  label: string;
  activities: FeedActivity[];
}

const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

const LABELS = ["New updates", "Last week", "This month", "Earlier"] as const;

function bucketFor(entry: FeedActivity, now: Date): (typeof LABELS)[number] {
  const occurred = new Date(entry.occurredAt);
  if (Number.isNaN(occurred.getTime())) return "Earlier";
  const age = now.getTime() - occurred.getTime();
  if (age < WEEK_MS) return "New updates";
  if (age < 2 * WEEK_MS) return "Last week";
  const sameMonth =
    occurred.getUTCFullYear() === now.getUTCFullYear() &&
    occurred.getUTCMonth() === now.getUTCMonth();
  return sameMonth ? "This month" : "Earlier";
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
    .map(([label, entries]) => ({ label, activities: entries }));
}
