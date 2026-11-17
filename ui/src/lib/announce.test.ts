// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { ANNOUNCE_CLEAR_MS, ANNOUNCE_DELAY_MS, createAnnouncer } from "./announce.svelte";
import {
  clearTrackIntent, noteTrackIntent, takeTrackIntent, TRACK_INTENT_MS, userTrackChange,
} from "./trackIntent";
import TrackAnnouncer from "./components/TrackAnnouncer.svelte";

const song = { title: "Teardrop", artist: "Massive Attack" };

describe("track-change announcements (A-4)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    clearTrackIntent();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("announces a track the user started, then clears", () => {
    const announcer = createAnnouncer();
    noteTrackIntent(1_000);
    expect(announcer.trackStarted(song, 1_500)).toBe(true);
    expect(announcer.message).toBe("");
    vi.advanceTimersByTime(ANNOUNCE_DELAY_MS);
    expect(announcer.message).toBe("Now playing: Teardrop by Massive Attack");
    vi.advanceTimersByTime(ANNOUNCE_CLEAR_MS);
    expect(announcer.message).toBe("");
  });

  it("stays quiet on automatic advances", () => {
    const announcer = createAnnouncer();
    expect(announcer.trackStarted(song, 1_000)).toBe(false);
    // One request is one announcement: the advance after it is automatic.
    noteTrackIntent(2_000);
    expect(announcer.trackStarted(song, 2_100)).toBe(true);
    expect(announcer.trackStarted({ title: "Angel", artist: "Massive Attack" }, 2_200)).toBe(false);
  });

  it("drops a request that never got its track", () => {
    const announcer = createAnnouncer();
    noteTrackIntent(1_000);
    expect(announcer.trackStarted(song, 1_000 + TRACK_INTENT_MS + 1)).toBe(false);
  });

  it("an end of queue does not use up the request", () => {
    const announcer = createAnnouncer();
    noteTrackIntent(1_000);
    expect(announcer.trackStarted(null, 1_100)).toBe(false);
    expect(announcer.trackStarted(song, 1_200)).toBe(true);
  });

  it("names the title alone when the artist is unknown", () => {
    const announcer = createAnnouncer();
    noteTrackIntent(1_000);
    announcer.trackStarted({ title: "Untitled", artist: "" }, 1_000);
    vi.advanceTimersByTime(ANNOUNCE_DELAY_MS);
    expect(announcer.message).toBe("Now playing: Untitled");
  });

  it("a failed command clears the request; a sent one keeps it", async () => {
    await expect(userTrackChange(() => Promise.reject(new Error("offline")))).rejects.toThrow("offline");
    expect(takeTrackIntent()).toBe(false);
    await userTrackChange(() => Promise.resolve());
    expect(takeTrackIntent()).toBe(true);
  });

  it("renders the message in a polite live region", () => {
    const announcer = createAnnouncer();
    const { container } = render(TrackAnnouncer, { announcer });
    const region = container.querySelector("[aria-live='polite']");
    expect(region).not.toBeNull();
    noteTrackIntent();
    announcer.trackStarted(song);
    vi.advanceTimersByTime(ANNOUNCE_DELAY_MS);
    flushSync();
    expect(region?.textContent).toBe("Now playing: Teardrop by Massive Attack");
  });
});
