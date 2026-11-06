import { api, trackToQueueItem, type Track } from "./api";
import { errorMessage } from "./errors";
import { playSource, type PlayFrom } from "./playSource.svelte";
import { trackFlags } from "./trackFlags.svelte";
import type { Toasts } from "./toasts.svelte";

async function playRadio(
  fetchTracks: () => Promise<Track[]>,
  toasts: Toasts,
  from: PlayFrom,
  subject: RadioSubject,
): Promise<void> {
  try {
    const tracks = await fetchTracks();
    if (tracks.length === 0) {
      toasts.show(noRadioMessage(subject), { severity: "info" });
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
    // No station is not a failure: an artist with nothing playable (a new
    // profile) has no radio, and TIDAL answers 404.
    if (isNotFound(e)) toasts.show(noRadioMessage(subject), { severity: "info" });
    else toasts.show(radioErrorMessage(e, subject), { severity: "error" });
  }
}

type RadioSubject = "track" | "artist";

function isNotFound(e: unknown): boolean {
  return String(e).startsWith("not found");
}

function noRadioMessage(subject: RadioSubject): string {
  return subject === "artist"
    ? "There's no radio for this artist yet. TIDAL builds one once they have tracks to play."
    : "There's no radio for this track yet.";
}

/// The station's name, in the player bar's "From" line and the radio page
/// ("Californication radio").
export function trackRadioName(title?: string): string {
  return title ? `${title} radio` : "Track radio";
}

/// What a track radio's plays come from: its page, so "From … radio" opens it.
export function trackRadioOrigin(trackId: number, title?: string): PlayFrom {
  return { kind: "radio", name: trackRadioName(title), route: { name: "radio", id: trackId } };
}

/// Plays a track radio at once (Home's radio cards). Menus open the radio's
/// page instead (`{ name: "radio" }`), where it plays like a mix.
export function playTrackRadio(trackId: number, toasts: Toasts, title?: string): Promise<void> {
  return playRadio(() => api.getTrackRadio(trackId), toasts, trackRadioOrigin(trackId, title), "track");
}

/// The radio's error, worded for a person: TIDAL's bare "not found: /v1/…"
/// and "forbidden" never reach the screen.
export function radioErrorMessage(e: unknown, subject: RadioSubject = "track"): string {
  if (isNotFound(e)) return noRadioMessage(subject);
  if (String(e) === "forbidden") return `Radio isn't available for this ${subject} right now`;
  return errorMessage(e);
}

export function playArtistRadio(artistId: number, toasts: Toasts, name?: string): Promise<void> {
  return playRadio(
    () => api.getArtistRadio(artistId),
    toasts,
    { kind: "radio", name: name ? `${name} radio` : "Artist radio", route: { name: "artist", id: artistId } },
    "artist",
  );
}
