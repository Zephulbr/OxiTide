const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

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
