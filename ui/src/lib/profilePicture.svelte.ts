/// A profile picture changed here, shown before TIDAL's artist record
/// catches up. The v1 `artists/{id}` record keeps the old `picture` for a
/// while after an upload, so the new picture's URL (from the upload) stands
/// in for it, but only while the record still shows the picture it replaced:
/// once TIDAL's record changes, the record wins again.

interface PictureChange {
  artistId: number;
  /// The new picture (URL or uuid); null after a removal.
  picture: string | null;
  /// The record's picture when the change was made.
  replaced: string | null;
}

let change = $state<PictureChange | null>(null);

export const profilePicture = {
  /// Record a change: `replaced` is the record's picture at the time.
  changed(artistId: number, picture: string | null, replaced: string | null | undefined) {
    change = { artistId, picture, replaced: replaced ?? null };
  },

  /// The picture to show for artist `artistId`, whose record says `picture`.
  for(artistId: number, picture: string | null | undefined): string | null | undefined {
    if (!change || change.artistId !== artistId) return picture;
    return (picture ?? null) === change.replaced ? change.picture : picture;
  },

  /// Forget the change (tests; sign-out).
  reset() {
    change = null;
  },
};
