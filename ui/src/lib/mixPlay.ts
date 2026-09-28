import { api, trackToQueueItem } from "./api";
import { errorMessage } from "./errors";
import { playSource } from "./playSource.svelte";
import type { Toasts } from "./toasts.svelte";

export async function playMix(id: string, toasts: Toasts): Promise<void> {
  try {
    const detail = await api.getMix(id);
    if (detail.tracks.length === 0) {
      toasts.show("This mix has no playable tracks.", { severity: "info" });
      return;
    }
    playSource.set(null);
    await api.playTracks(detail.tracks.map(trackToQueueItem), 0);
  } catch (e) {
    toasts.show(errorMessage(e), { severity: "error" });
  }
}
