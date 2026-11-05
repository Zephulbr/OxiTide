// A one-shot request to open Settings at one row (the search jump: tab, scroll,
// ring, focus). Used by the quality menu's "Output settings…". A request
// instead of a route field: going to Settings while already there is a no-op
// in `nav`, and a route field would be persisted with the history.

let pending = $state<string | null>(null);

export const settingsJump = {
  /** The setting id waiting to be shown, if any. */
  get pending(): string | null {
    return pending;
  },
  request(id: string) {
    pending = id;
  },
  /** Take the request (Settings shows it once). */
  take(): string | null {
    const id = pending;
    pending = null;
    return id;
  },
};
