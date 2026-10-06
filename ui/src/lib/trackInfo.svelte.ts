import type { Track } from "./api";

export interface TrackInfoRequest {
  id: number;
  title: string;
}

export interface TrackInfoState {
  readonly request: TrackInfoRequest | null;
  open(track: Track): void;
  close(): void;
}

export function createTrackInfo(): TrackInfoState {
  let request = $state<TrackInfoRequest | null>(null);
  return {
    get request() {
      return request;
    },
    open: (track) => {
      request = { id: track.id, title: track.title };
    },
    close: () => {
      request = null;
    },
  };
}

/// One app-wide request slot; TrackTable opens it and App hosts the surface.
export const trackInfo = createTrackInfo();
