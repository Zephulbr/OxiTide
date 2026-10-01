import type { Track } from "./api";

const FIRST = ["The", "A", "My", "Your", "Our", "Every", "Last", "First"];
const SECOND = ["Midnight", "Golden", "Silent", "Electric", "Broken", "Endless", "Hidden", "Neon"];
const THIRD = ["Dream", "River", "Fire", "City", "Echo", "Heart", "Signal", "Sky"];
const ARTISTS = ["Aurora", "Muse", "Kite", "Pulse", "Radiohead", "Mono", "Vega", "Nova"];

/// Deterministic synthetic tracks for the S6 filter/sort scenario: the same
/// count always yields the same rows so run-to-run numbers are comparable.
export function makeSyntheticTracks(count: number): Track[] {
  const tracks: Track[] = [];
  for (let i = 0; i < count; i++) {
    const title = `${FIRST[i % FIRST.length]} ${SECOND[(i * 7) % SECOND.length]} ${
      THIRD[(i * 13) % THIRD.length]
    } ${String(i).padStart(5, "0")}`;
    tracks.push({
      id: 900_000_000 + i,
      title,
      duration: 120 + (i % 240),
      artists: [{ id: 90_000 + (i % 50), name: ARTISTS[(i * 3) % ARTISTS.length] }],
      album: { id: 800_000_000 + i, title: `${THIRD[(i * 5) % THIRD.length]} Sessions`, cover: null },
      audioQuality: "LOSSLESS",
      trackNumber: (i % 12) + 1,
      volumeNumber: 1,
      explicit: false,
    });
  }
  return tracks;
}
