export interface ScrobbledBadge {
  readonly trackId: number | null;
  mark(trackId: number): void;
  clear(): void;
}

export function createScrobbled(): ScrobbledBadge {
  let trackId = $state<number | null>(null);
  return {
    get trackId() {
      return trackId;
    },
    mark: (id) => {
      trackId = id;
    },
    clear: () => {
      trackId = null;
    },
  };
}

/// One app-wide badge: set by the scrobble event, cleared when the current
/// queue item changes (Now Playing owns the clear).
export const scrobbled = createScrobbled();
