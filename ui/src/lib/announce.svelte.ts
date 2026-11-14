/// Screen-reader announcements (v1.1 spec §12, A-4). `TrackAnnouncer.svelte`
/// renders `announcer.message` in a polite live region. Only track changes the
/// user asked for are announced (play from a list, next, previous, a queue
/// jump; see `trackIntent.ts`); automatic advances are not, so listening stays
/// quiet.

import type { QueueItem } from "./api";
import { t } from "./i18n.svelte";
import { takeTrackIntent } from "./trackIntent";

/// A polite region is read when its text changes: the text is cleared first
/// and set a moment later, so the same message twice is read twice.
export const ANNOUNCE_DELAY_MS = 100;
/// Then it is cleared, so the virtual cursor never meets stale text.
export const ANNOUNCE_CLEAR_MS = 7_000;

export function createAnnouncer() {
  let message = $state("");
  let setTimer: ReturnType<typeof setTimeout> | null = null;
  let clearTimer: ReturnType<typeof setTimeout> | null = null;

  function stop() {
    if (setTimer !== null) clearTimeout(setTimer);
    if (clearTimer !== null) clearTimeout(clearTimer);
    setTimer = clearTimer = null;
  }

  function announce(text: string) {
    stop();
    message = "";
    setTimer = setTimeout(() => {
      setTimer = null;
      message = text;
      clearTimer = setTimeout(() => {
        clearTimer = null;
        message = "";
      }, ANNOUNCE_CLEAR_MS);
    }, ANNOUNCE_DELAY_MS);
  }

  /// The engine's `now_playing`: announces the track when the user asked for
  /// the change. Returns whether it did.
  function trackStarted(item: Pick<QueueItem, "title" | "artist"> | null, now = Date.now()): boolean {
    if (item === null) return false;
    if (!takeTrackIntent(now)) return false;
    announce(
      item.artist
        ? t("announce.nowPlaying", { title: item.title, artists: item.artist })
        : t("announce.nowPlayingTitle", { title: item.title }),
    );
    return true;
  }

  return {
    get message() { return message; },
    announce,
    trackStarted,
  };
}

export type Announcer = ReturnType<typeof createAnnouncer>;

export const announcer = createAnnouncer();
