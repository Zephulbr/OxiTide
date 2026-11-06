import type { EditableProfile, ProfileEdit, ProfileLink } from "./api";
import { isHttpUrl } from "./artistLinks";

/// The profile editor's form: what the person typed.
export interface ProfileDraft {
  name: string;
  handle: string;
  bio: string;
  /// One entry per link row, in order; empty rows are dropped.
  links: string[];
}

export function draftFrom(profile: EditableProfile): ProfileDraft {
  return {
    name: profile.name,
    handle: profile.handle ?? "",
    bio: profile.bio ?? "",
    links: profile.links.map((link) => link.href),
  };
}

/// Hosts with their own TIDAL link type (the types seen in TIDAL's data);
/// any other link is the profile's homepage.
const HOST_KINDS: [string, string][] = [
  ["instagram.com", "INSTAGRAM"],
  ["twitter.com", "TWITTER"],
  ["x.com", "TWITTER"],
  ["facebook.com", "FACEBOOK"],
  ["youtube.com", "YOUTUBE"],
  ["youtu.be", "YOUTUBE"],
  ["tiktok.com", "TIKTOK"],
];

export function linkKindFor(url: string): string {
  let host: string;
  try {
    host = new URL(url).hostname.toLowerCase();
  } catch {
    return "OFFICIAL_HOMEPAGE";
  }
  for (const [domain, kind] of HOST_KINDS) {
    if (host === domain || host.endsWith(`.${domain}`)) return kind;
  }
  return "OFFICIAL_HOMEPAGE";
}

/// "instagram.com/me" reads as https://instagram.com/me.
export function normalizeLink(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed === "" || /^[a-z][a-z\d+.-]*:/i.test(trimmed)) return trimmed;
  return `https://${trimmed}`;
}

/// TIDAL's rule for a username (its `handle`), worded as TIDAL's editor
/// words it under the field.
export const USERNAME_RULE = "Use only the letters a-z, numbers 0-9 and underscores.";

/// The username as it will be saved: trimmed, without a leading "@".
export function cleanUsername(raw: string): string {
  return raw.trim().replace(/^@/, "");
}

export function usernameValid(raw: string): boolean {
  return /^[a-z0-9_]*$/.test(cleanUsername(raw));
}

/// What is wrong with the form, or null. TIDAL validates the rest (a taken
/// username) in the save's dry run. A username that was set can't be
/// cleared.
export function draftProblem(draft: ProfileDraft, original?: EditableProfile): string | null {
  if (draft.name.trim() === "") return "Your profile needs a name.";
  if (!usernameValid(draft.handle)) return USERNAME_RULE;
  if (original?.handle && cleanUsername(draft.handle) === "") return "Your profile needs a username.";
  const bad = draft.links.map(normalizeLink).find((link) => link !== "" && !isHttpUrl(link));
  if (bad !== undefined) return `"${bad}" isn't a web link.`;
  return null;
}

/// Only what changed: TIDAL is sent nothing it already has. Unchanged links
/// keep the type TIDAL gave them; new ones get one from their host.
export function profileChanges(
  original: EditableProfile,
  draft: ProfileDraft,
): { edit: ProfileEdit; bio: string | null } {
  const edit: ProfileEdit = {};
  const name = draft.name.trim();
  if (name !== original.name) edit.name = name;
  const handle = cleanUsername(draft.handle);
  if (handle !== (original.handle ?? "")) edit.handle = handle;

  const hrefs = draft.links.map(normalizeLink).filter((link) => link !== "");
  const before = original.links.map((link) => link.href);
  if (hrefs.length !== before.length || hrefs.some((href, i) => href !== before[i])) {
    edit.links = hrefs.map(
      (href): ProfileLink => ({
        href,
        kind: original.links.find((link) => link.href === href)?.kind ?? linkKindFor(href),
      }),
    );
  }

  const bio = draft.bio.trim();
  const bioChanged = original.bioId != null && bio !== (original.bio ?? "").trim();
  return { edit, bio: bioChanged ? bio : null };
}

export function hasChanges(changes: { edit: ProfileEdit; bio: string | null }): boolean {
  return Object.keys(changes.edit).length > 0 || changes.bio !== null;
}
