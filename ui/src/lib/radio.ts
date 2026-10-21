import { api, trackToQueueItem, type Track } from "./api";
import { errorMessage } from "./errors";
import { playSource, type PlayFrom } from "./playSource.svelte";
import { trackFlags } from "./trackFlags.svelte";
import type { Toasts } from "./toasts.svelte";

async function playRadio(
  fetchTracks: () => Promise<Track[]>,
  toasts: Toasts,
  from: PlayFrom,
): Promise<void> {
  try {
    const tracks = await fetchTracks();
    if (tracks.length === 0) {
      toasts.show("No radio tracks available.", { severity: "info" });
      return;
    }
    const prepared = await trackFlags.filterPlayable(tracks, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(from, prepared.items.map((t) => t.id));
    await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  } catch (e) {
    const message =
      String(e) === "forbidden"
        ? "Radio isn't available for this track right now"
        : errorMessage(e);
    toasts.show(message, { severity: "error" });
  }
}

/// `title` / `name` name the station in the player bar ("Californication radio").
export function playTrackRadio(trackId: number, toasts: Toasts, title?: string): Promise<void> {
  return playRadio(() => api.getTrackRadio(trackId), toasts, {
    kind: "radio",
    name: title ? `${title} radio` : "Track radio",
    route: null,
  });
}

export function playArtistRadio(artistId: number, toasts: Toasts, name?: string): Promise<void> {
  return playRadio(() => api.getArtistRadio(artistId), toasts, {
    kind: "radio",
    name: name ? `${name} radio` : "Artist radio",
    route: { name: "artist", id: artistId },
  });
}
