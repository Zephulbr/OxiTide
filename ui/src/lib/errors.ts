/// Map the client's exact backend error strings (crates/oxitide-tidal/src/error.rs)
/// to user-facing copy; anything unrecognized falls through unchanged.
export function errorMessage(e: unknown): string {
  const raw = String(e);
  if (raw === "forbidden") return "You can only edit your own playlists";
  if (raw === "rate limited") {
    return "TIDAL is rate-limiting requests — try again in a moment";
  }
  return raw;
}
