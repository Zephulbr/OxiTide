const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Weekday names in the stats' order (Monday first).
export const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// Axis/tooltip label for a stats day ("2026-09-11" -> "Sep 11"); the date is
/// already a local calendar day, so it is read as text, never through Date.
export function dayTick(date: string): string {
  const [, month, day] = date.split("-").map(Number);
  return MONTHS[month - 1] && day ? `${MONTHS[month - 1]} ${day}` : date;
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

/// Where in the day an hour falls, phrased to follow "Mostly".
export function dayPart(hour: number): string {
  if (hour < 5) return "late at night";
  if (hour < 12) return "in the morning";
  if (hour < 17) return "in the afternoon";
  if (hour < 22) return "in the evening";
  return "at night";
}

/// Whole percent of `part` in `whole` (0 when there is no whole).
export function percentOf(part: number, whole: number): number {
  return whole > 0 ? Math.round((part / whole) * 100) : 0;
}
