import { api, trackToQueueItem, type Track } from "./api";
import { errorMessage } from "./errors";
import { playSource } from "./playSource.svelte";
import type { Toasts } from "./toasts.svelte";

async function playRadio(fetchTracks: () => Promise<Track[]>, toasts: Toasts): Promise<void> {
  try {
    const tracks = await fetchTracks();
    if (tracks.length === 0) {
      toasts.show("No radio tracks available.", { severity: "info" });
      return;
    }
    playSource.set(null);
    await api.playTracks(tracks.map(trackToQueueItem), 0);
  } catch (e) {
    const message =
      String(e) === "forbidden"
        ? "Radio isn't available for this track right now"
        : errorMessage(e);
    toasts.show(message, { severity: "error" });
  }
}

export function playTrackRadio(trackId: number, toasts: Toasts): Promise<void> {
  return playRadio(() => api.getTrackRadio(trackId), toasts);
}

export function playArtistRadio(artistId: number, toasts: Toasts): Promise<void> {
  return playRadio(() => api.getArtistRadio(artistId), toasts);
}
