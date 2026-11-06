import type { Artist, UserProfile } from "./api";

/// What `myProfileSource` loads: the account's profile and the artist that
/// is its page. TIDAL keeps no separate user page; a profile is an artist
/// with `ownerType: "USER"` (plans/notes/2026-11-06-tidal-profiles-research.md).
export interface OwnProfile {
  profile: UserProfile | null;
  artist: Artist | null;
}

/// The Profiles page's own card.
export interface OwnCard {
  name: string;
  /// Image uuid: the artist's picture (or its cover fallback), else the
  /// profile picture.
  picture: string | null;
  /// The page the card opens; null when TIDAL has none for this profile.
  artistId: number | null;
}

export function ownCard(me: OwnProfile | null | undefined): OwnCard | null {
  const profile = me?.profile;
  if (!profile) return null;
  const artist = me.artist;
  return {
    name: artist?.name || profile.name || profile.handle || "Your profile",
    picture: artist?.picture ?? artist?.cover_fallback ?? profile.picture ?? null,
    artistId: profile.artistId ?? null,
  };
}

/// Whether artist `id` is the signed-in account's own profile.
export function isOwnArtist(me: OwnProfile | null | undefined, id: number): boolean {
  return me?.profile?.artistId != null && me.profile.artistId === id;
}

/// TIDAL's profile editor on the web; OxiTide sends no profile writes.
export function profileEditUrl(artistId: number): string {
  return `https://tidal.com/artist/${artistId}?edit-profile=true`;
}
