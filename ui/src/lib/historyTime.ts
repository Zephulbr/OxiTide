/// Local-day grouping and relative timestamps for the History view. `nowMs`
/// is injected so the logic is testable in any timezone.

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/// "just now", "24 min ago", "3 h ago", "2 d ago".
export function relativeTime(startedAtSecs: number, nowMs: number): string {
  const age = Math.max(0, nowMs - startedAtSecs * 1000);
  if (age < MINUTE) return "just now";
  if (age < HOUR) return `${Math.floor(age / MINUTE)} min ago`;
  if (age < DAY) return `${Math.floor(age / HOUR)} h ago`;
  return `${Math.floor(age / DAY)} d ago`;
}

function startOfLocalDay(ms: number): number {
  const date = new Date(ms);
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/// "Today" / "Yesterday" / "Thu, Oct 1" for a local calendar day.
export function dayLabel(dayMs: number, nowMs: number): string {
  const today = startOfLocalDay(nowMs);
  if (dayMs === today) return "Today";
  if (dayMs === today - DAY) return "Yesterday";
  return new Intl.DateTimeFormat("en-US", {
    weekday: "short",
    month: "short",
    day: "numeric",
  }).format(new Date(dayMs));
}

export interface HistoryGroup<T> {
  label: string;
  entries: T[];
}

/// Merge a later page into the loaded list, dropping rows already shown: the
/// server's in-flight prepend can shift the page boundary between fetches.
export function mergeHistoryEntries<T extends { started_at: number; track_id: number }>(
  existing: T[],
  incoming: T[],
): T[] {
  const seen = new Set(existing.map((entry) => `${entry.started_at}:${entry.track_id}`));
  const merged = [...existing];
  for (const entry of incoming) {
    const key = `${entry.started_at}:${entry.track_id}`;
    if (seen.has(key)) continue;
    seen.add(key);
    merged.push(entry);
  }
  return merged;
}

/// Group newest-first entries into consecutive local-day buckets.
export function groupByDay<T extends { started_at: number }>(
  entries: T[],
  nowMs: number,
): HistoryGroup<T>[] {
  const groups: HistoryGroup<T>[] = [];
  let currentDay: number | null = null;
  let current: HistoryGroup<T> | null = null;
  for (const entry of entries) {
    const day = startOfLocalDay(entry.started_at * 1000);
    if (day !== currentDay) {
      currentDay = day;
      current = { label: dayLabel(day, nowMs), entries: [] };
      groups.push(current);
    }
    current!.entries.push(entry);
  }
  return groups;
}
