/// Local-day grouping and relative timestamps for the History view. `nowMs`
/// is injected so the logic is testable in any timezone.

import { i18n, t } from "./i18n.svelte";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/// "just now", "24 min ago", "3 h ago", "2 d ago".
export function relativeTime(startedAtSecs: number, nowMs: number): string {
  const age = Math.max(0, nowMs - startedAtSecs * 1000);
  if (age < MINUTE) return t("remoteSettings.seen.now");
  if (age < HOUR) return t("remoteSettings.seen.minutes", { count: Math.floor(age / MINUTE) });
  if (age < DAY) return t("remoteSettings.seen.hours", { count: Math.floor(age / HOUR) });
  return t("history.daysAgo", { count: Math.floor(age / DAY) });
}

/// Local clock time of a play, e.g. "14:32" (or "2:32 PM" in 12-hour locales).
export function clockTime(startedAtSecs: number): string {
  return new Intl.DateTimeFormat(i18n.locale, { hour: "numeric", minute: "2-digit" }).format(
    new Date(startedAtSecs * 1000),
  );
}

function startOfLocalDay(ms: number): number {
  const date = new Date(ms);
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/// "Today" / "Yesterday" / "Thu, Oct 1" for a local calendar day.
export function dayLabel(dayMs: number, nowMs: number): string {
  const today = startOfLocalDay(nowMs);
  if (dayMs === today) return t("history.today");
  if (dayMs === today - DAY) return t("history.yesterday");
  return new Intl.DateTimeFormat(i18n.locale, {
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
