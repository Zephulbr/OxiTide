import { describe, expect, it } from "vitest";
import { groupByDay, mergeHistoryEntries, relativeTime } from "./historyTime";

// Local time on purpose: the helpers are local-day based, so constructing the
// clock with the local Date constructor keeps the tests timezone-independent.
const NOW = new Date(2026, 9, 4, 12, 0, 0).getTime();

function hoursAgo(hours: number): number {
  return Math.floor((NOW - hours * 3_600_000) / 1000);
}

function secondsAgo(seconds: number): number {
  return Math.floor(NOW / 1000) - seconds;
}

describe("relativeTime", () => {
  it("formats just now, minutes, hours, and days", () => {
    expect(relativeTime(secondsAgo(20), NOW)).toBe("just now");
    expect(relativeTime(hoursAgo(0.4), NOW)).toBe("24 min ago");
    expect(relativeTime(hoursAgo(3), NOW)).toBe("3 h ago");
    expect(relativeTime(hoursAgo(50), NOW)).toBe("2 d ago");
  });
});

describe("groupByDay", () => {
  it("groups consecutive local days with Today/Yesterday labels", () => {
    const entries = [
      { started_at: hoursAgo(1) },
      { started_at: hoursAgo(5) },
      { started_at: hoursAgo(30) },
      { started_at: hoursAgo(74) },
    ];
    const groups = groupByDay(entries, NOW);
    expect(groups.map((group) => group.label)).toEqual([
      "Today",
      "Yesterday",
      "Thu, Oct 1",
    ]);
    expect(groups[0].entries).toHaveLength(2);
    expect(groups[1].entries).toHaveLength(1);
    expect(groups[2].entries).toHaveLength(1);
  });

  it("returns one group for a single day", () => {
    const entries = [{ started_at: hoursAgo(1) }];
    expect(groupByDay(entries, NOW)).toEqual([{ label: "Today", entries }]);
  });
});

describe("mergeHistoryEntries", () => {
  it("appends new entries and drops rows already shown", () => {
    const first = [
      { started_at: 100, track_id: 1 },
      { started_at: 90, track_id: 2 },
    ];
    const second = [
      { started_at: 90, track_id: 2 },
      { started_at: 80, track_id: 3 },
    ];
    expect(mergeHistoryEntries(first, second)).toEqual([
      { started_at: 100, track_id: 1 },
      { started_at: 90, track_id: 2 },
      { started_at: 80, track_id: 3 },
    ]);
  });
});
