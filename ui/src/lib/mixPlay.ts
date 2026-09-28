import { api, trackToQueueItem } from "./api";
import type { Toasts } from "./toasts.svelte";

export async function playMix(id: string, toasts: Toasts): Promise<void> {
  const detail = await api.getMix(id);
  if (detail.tracks.length === 0) {
    toasts.show("This mix has no playable tracks.", { severity: "info" });
    return;
  }
  await api.playTracks(detail.tracks.map(trackToQueueItem), 0);
}
