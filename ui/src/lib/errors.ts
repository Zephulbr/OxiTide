import { t } from "./i18n.svelte";

/// Map the client's exact backend error strings (crates/oxitide-tidal/src/error.rs)
/// to user-facing copy. Anything else is a backend detail that stays English
/// (v1.1 i18n spec §10.3); it goes through `error.detail`, whose English is
/// the detail alone and whose translations put a headline in front of it.
export function errorMessage(e: unknown): string {
  const raw = String(e);
  if (raw === "forbidden") return t("error.forbidden");
  if (raw === "rate limited") return t("error.rateLimited");
  if (raw.startsWith("api error 412")) return t("error.playlistChanged");
  if (isApiDrift(raw)) return apiDriftMessage();
  return t("error.detail", { detail: raw });
}

export function apiDriftMessage(): string {
  return t("error.apiDrift");
}

/// TIDAL answered in a shape the client no longer understands: a 2xx body
/// that does not decode (`oxitide-tidal`'s `Error::Json`, possibly wrapped
/// by a playback or command prefix; the details are in the log) or an
/// endpoint that is gone (410).
function isApiDrift(raw: string): boolean {
  return raw.includes("json error:") || /\bapi error 410\b/.test(raw);
}
