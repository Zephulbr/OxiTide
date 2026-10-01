import type { Album, Track } from "./api";

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

/// Deterministic synthetic album for the 500-track open gate; reuses the track
/// generator so list rendering and filtering see the same shapes as S6.
export function makeSyntheticAlbum(count: number): { album: Album; tracks: Track[] } {
  const tracks = makeSyntheticTracks(count).map((track) => ({
    ...track,
    album: { id: -1, title: `Synthetic Album (${count})`, cover: null },
  }));
  return {
    album: {
      id: -1,
      title: `Synthetic Album (${count})`,
      artists: [{ id: -1, name: "OxiTide Perf" }],
      cover: null,
      numberOfTracks: count,
      duration: tracks.reduce((sum, t) => sum + t.duration, 0),
      audioQuality: "LOSSLESS",
    },
    tracks,
  };
}
