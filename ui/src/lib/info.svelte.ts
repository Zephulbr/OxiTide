import { api, type Track, type TrackCredits } from "./api";

/// Per-session track-info cache (spec §5.3): track metadata + credits load
/// once per id, lazily on first Info open; failures stay uncached so reopening
/// retries. No disk cache.
export interface InfoStore {
  readonly trackId: number | null;
  readonly loading: boolean;
  readonly track: Track | null;
  readonly credits: TrackCredits | null;
  readonly error: string;
  readonly creditsError: string;
  open(id: number): Promise<void>;
  retry(): void;
}

interface Entry {
  track: Track;
  credits: TrackCredits | null;
  creditsError: string;
}

export function createInfo(): InfoStore {
  let trackId = $state<number | null>(null);
  let loading = $state(false);
  let track = $state<Track | null>(null);
  let credits = $state<TrackCredits | null>(null);
  let error = $state("");
  let creditsError = $state("");
  const cache = new Map<number, Entry>();
  let requestId = 0;

  async function open(id: number) {
    trackId = id;
    const cached = cache.get(id);
    if (cached) {
      // A cache hit supersedes any fetch still in flight: without bumping the
      // token the pending request would keep the newest token and overwrite
      // this cached track.
      ++requestId;
      track = cached.track;
      credits = cached.credits;
      creditsError = cached.creditsError;
      error = "";
      loading = false;
      return;
    }
    const token = ++requestId;
    loading = true;
    track = null;
    credits = null;
    error = "";
    creditsError = "";
    let loaded: Track | null = null;
    let trackFailure = "";
    try {
      loaded = await api.getTrack(id);
    } catch (e) {
      trackFailure = String(e);
    }
    if (token !== requestId) return; // a newer open superseded this fetch
    let loadedCredits: TrackCredits | null = null;
    let failure = "";
    if (loaded !== null) {
      try {
        loadedCredits = await api.getTrackCredits(id);
      } catch (e) {
        failure = String(e);
      }
    }
    if (token !== requestId) return; // superseded during the credits fetch
    if (loaded !== null && failure === "") {
      cache.set(id, { track: loaded, credits: loadedCredits, creditsError: "" });
    }
    track = loaded;
    credits = loadedCredits;
    error = loaded !== null ? "" : trackFailure;
    creditsError = failure;
    loading = false;
  }

  function retry() {
    if (trackId === null) return;
    cache.delete(trackId);
    void open(trackId);
  }

  return {
    get trackId() { return trackId; },
    get loading() { return loading; },
    get track() { return track; },
    get credits() { return credits; },
    get error() { return error; },
    get creditsError() { return creditsError; },
    open,
    retry,
  };
}

export const info = createInfo();
