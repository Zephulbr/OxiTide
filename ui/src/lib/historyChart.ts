/// Bar heights (percent of the tallest day) for the 30-day plays chart.
export function barPercents(plays: number[]): number[] {
  const max = Math.max(1, ...plays);
  return plays.map((value) => (value / max) * 100);
}
