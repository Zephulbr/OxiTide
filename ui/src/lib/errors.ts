/// Map the client's exact backend error strings (crates/oxitide-tidal/src/error.rs)
/// to user-facing copy; anything unrecognized falls through unchanged.
export function errorMessage(e: unknown): string {
  const raw = String(e);
  if (raw === "forbidden") return "You can only edit your own playlists";
  if (raw === "rate limited") {
    return "TIDAL is rate-limiting requests — try again in a moment";
  }
  if (raw.startsWith("api error 412")) {
    return "The playlist changed on TIDAL — refresh and try again";
  }
  if (isApiDrift(raw)) return API_DRIFT_MESSAGE;
  return raw;
}

export const API_DRIFT_MESSAGE =
  "TIDAL changed something on its side — check for an OxiTide update (Settings → System)";

/// TIDAL answered in a shape the client no longer understands: a 2xx body
/// that does not decode (`oxitide-tidal`'s `Error::Json`, possibly wrapped
/// by a playback or command prefix; the details are in the log) or an
/// endpoint that is gone (410).
function isApiDrift(raw: string): boolean {
  return raw.includes("json error:") || /\bapi error 410\b/.test(raw);
}
