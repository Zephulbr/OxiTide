import { describe, expect, it } from "vitest";
import type { FeedActivity } from "./api";
import { groupActivities } from "./feed";

const NOW = new Date("2026-09-29T12:00:00Z");

function activity(occurredAt: string, title = occurredAt): FeedActivity {
  return {
    id: 1,
    title,
    subtitle: "Album · Artist",
    artwork: null,
    explicit: false,
    hires: false,
    occurredAt,
    seen: true,
  };
}

describe("groupActivities", () => {
  it("splits activities into the feed's date buckets", () => {
    const groups = groupActivities(
      [
        activity("2026-09-27T12:00:00Z", "new"),
        activity("2026-09-20T12:00:00Z", "last week"),
        activity("2026-09-09T12:00:00Z", "this month"),
        activity("2026-08-20T12:00:00Z", "older"),
      ],
      NOW,
    );
    expect(groups.map((group) => group.label)).toEqual([
      "New updates",
      "Last week",
      "This month",
      "Earlier",
    ]);
    expect(groups.map((group) => group.activities[0].title)).toEqual([
      "new",
      "last week",
      "this month",
      "older",
    ]);
  });

  it("treats exactly seven days as last week", () => {
    const groups = groupActivities([activity("2026-09-22T12:00:00Z")], NOW);
    expect(groups).toEqual([
      { label: "Last week", activities: [expect.objectContaining({ occurredAt: "2026-09-22T12:00:00Z" })] },
    ]);
  });

  it("keeps an older activity in this month only while the calendar month matches", () => {
    const groups = groupActivities(
      [activity("2026-09-13T12:00:00Z"), activity("2026-08-30T12:00:00Z")],
      NOW,
    );
    expect(groups.map((group) => group.label)).toEqual(["This month", "Earlier"]);
  });

  it("omits empty buckets", () => {
    const groups = groupActivities([activity("2026-09-28T12:00:00Z")], NOW);
    expect(groups.map((group) => group.label)).toEqual(["New updates"]);
  });

  it("sorts newest first within and across buckets", () => {
    const groups = groupActivities(
      [activity("2026-09-27T00:00:00Z", "older new"), activity("2026-09-29T00:00:00Z", "newest")],
      NOW,
    );
    expect(groups[0].activities.map((entry) => entry.title)).toEqual(["newest", "older new"]);
  });

  it("returns nothing for an empty feed", () => {
    expect(groupActivities([], NOW)).toEqual([]);
  });
});
