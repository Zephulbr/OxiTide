import { i18n } from "./i18n.svelte";

/// A weekday's name in the stats' order (0 = Monday), in the UI language.
export function weekdayName(day: number, style: "long" | "short" = "long"): string {
  // 2024-01-01 was a Monday; UTC keeps the date fixed.
  return new Intl.DateTimeFormat(i18n.locale, { weekday: style, timeZone: "UTC" }).format(Date.UTC(2024, 0, 1 + day));
}

/// Axis/tooltip label for a stats day ("2026-09-11" -> "Sep 11"); the date is
/// already a local calendar day, so it is formatted in UTC, never shifted.
export function dayTick(date: string): string {
  const [year, month, day] = date.split("-").map(Number);
  if (!year || !month || !day || month > 12) return date;
  return new Intl.DateTimeFormat(i18n.locale, { month: "short", day: "numeric", timeZone: "UTC" }).format(
    Date.UTC(year, month - 1, day),
  );
}

/// Bar heights (percent of the tallest day) for the 30-day plays chart.
export function barPercents(plays: number[]): number[] {
  const max = Math.max(1, ...plays);
  return plays.map((value) => (value / max) * 100);
}

/// Index of the largest value (the first on ties), or -1 when all are zero.
export function peakIndex(values: number[]): number {
  let best = -1;
  values.forEach((value, i) => {
    if (value > 0 && (best < 0 || value > values[best])) best = i;
  });
  return best;
}

/// "22:00" for an hour of the day.
export function hourLabel(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`;
}

export type DayPart = "lateNight" | "morning" | "afternoon" | "evening" | "night";

/// Where in the day an hour falls (`stats.mostly` words it).
export function dayPart(hour: number): DayPart {
  if (hour < 5) return "lateNight";
  if (hour < 12) return "morning";
  if (hour < 17) return "afternoon";
  if (hour < 22) return "evening";
  return "night";
}

/// Whole percent of `part` in `whole` (0 when there is no whole).
export function percentOf(part: number, whole: number): number {
  return whole > 0 ? Math.round((part / whole) * 100) : 0;
}
