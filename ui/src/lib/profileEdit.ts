import type { EditableProfile, ProfileEdit, ProfileLink } from "./api";
import { isHttpUrl, type Brand } from "./artistLinks";

/// One row of the editor's Links: a fixed social field (`social` is its
/// TIDAL link type) or a free link (`social` null, typed by its host).
export interface LinkRow {
  href: string;
  social: SocialKind | null;
}

/// The profile editor's form: what the person typed.
export interface ProfileDraft {
  name: string;
  handle: string;
  bio: string;
  /// In order; empty rows are dropped.
  links: LinkRow[];
}

export type SocialKind = "INSTAGRAM" | "TIKTOK" | "FACEBOOK" | "TWITTER" | "SNAPCHAT";

export interface Social {
  kind: SocialKind;
  label: string;
  brand: Brand;
  /// The site's hosts; a link typed into the field must be on one.
  hosts: string[];
  /// The profile link for a bare username.
  profileUrl: (username: string) => string;
}

/// TIDAL's social media fields, in its editor's order. These are the
/// social types of TIDAL's `externalLinks` (OpenAPI `External_Link_Meta`).
export const SOCIALS: Social[] = [
  {
    kind: "INSTAGRAM",
    label: "Instagram",
    brand: "instagram",
    hosts: ["instagram.com", "instagr.am"],
    profileUrl: (u) => `https://www.instagram.com/${u}`,
  },
  {
    kind: "TIKTOK",
    label: "TikTok",
    brand: "tiktok",
    hosts: ["tiktok.com"],
    profileUrl: (u) => `https://www.tiktok.com/@${u}`,
  },
  {
    kind: "FACEBOOK",
    label: "Facebook",
    brand: "facebook",
    hosts: ["facebook.com", "fb.com", "fb.me"],
    profileUrl: (u) => `https://www.facebook.com/${u}`,
  },
  { kind: "TWITTER", label: "X", brand: "x", hosts: ["x.com", "twitter.com"], profileUrl: (u) => `https://x.com/${u}` },
  {
    kind: "SNAPCHAT",
    label: "Snapchat",
    brand: "snapchat",
    hosts: ["snapchat.com"],
    profileUrl: (u) => `https://www.snapchat.com/add/${u}`,
  },
];

export function socialFor(kind: string | null | undefined): Social | undefined {
  return SOCIALS.find((social) => social.kind === kind);
}

function hostOf(url: string): string | null {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch {
    return null;
  }
}

function onHost(host: string, domains: string[]): boolean {
  return domains.some((domain) => host === domain || host.endsWith(`.${domain}`));
}

export function draftFrom(profile: EditableProfile): ProfileDraft {
  return {
    name: profile.name,
    handle: profile.handle ?? "",
    bio: profile.bio ?? "",
    links: profile.links.map((link) => ({ href: link.href, social: socialFor(link.kind)?.kind ?? null })),
  };
}

/// A free link's TIDAL type: a social site's own type, else the homepage.
/// (TIDAL has no YouTube type, so a YouTube link is a homepage link.)
export function linkKindFor(url: string): string {
  const host = hostOf(url);
  if (host) for (const social of SOCIALS) if (onHost(host, social.hosts)) return social.kind;
  return "OFFICIAL_HOMEPAGE";
}

/// "instagram.com/me" reads as https://instagram.com/me.
export function normalizeLink(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed === "" || /^[a-z][a-z\d+.-]*:/i.test(trimmed)) return trimmed;
  return `https://${trimmed}`;
}

/// What a social field holds as a link: a bare username ("@me" or "me")
/// becomes the site's profile link; anything with a "/" or ":" is a link.
export function socialHref(kind: SocialKind, raw: string): string {
  const trimmed = raw.trim();
  if (trimmed === "") return "";
  if (/[/:]/.test(trimmed)) return normalizeLink(trimmed);
  return socialFor(kind)!.profileUrl(trimmed.replace(/^@/, ""));
}

/// A row's link as it will be saved ("" when empty).
export function rowHref(row: LinkRow): string {
  return row.social ? socialHref(row.social, row.href) : normalizeLink(row.href);
}

/// A row's TIDAL type: the field's for a social row, else the type TIDAL
/// gave the same link, else one from its host.
export function rowKind(row: LinkRow, original?: EditableProfile): string {
  if (row.social) return row.social;
  const href = rowHref(row);
  return original?.links.find((link) => link.href === href)?.kind ?? linkKindFor(href);
}

/// The social fields the form has no row for yet: the quick-add buttons.
export function missingSocials(links: LinkRow[]): Social[] {
  const present = new Set(links.map((row) => row.social ?? linkKindFor(normalizeLink(row.href))));
  return SOCIALS.filter((social) => !present.has(social.kind));
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

/// What is wrong with a link row, or null.
function rowProblem(row: LinkRow): string | null {
  const href = rowHref(row);
  if (href === "") return null;
  if (!isHttpUrl(href)) return `"${href}" isn't a web link.`;
  const social = socialFor(row.social);
  if (social && !onHost(hostOf(href) ?? "", social.hosts)) return `"${href}" isn't a link to ${social.label}.`;
  return null;
}

/// What is wrong with the form, or null. TIDAL validates the rest (a taken
/// username) in the save's dry run. A username that was set can't be
/// cleared.
export function draftProblem(draft: ProfileDraft, original?: EditableProfile): string | null {
  if (draft.name.trim() === "") return "Your profile needs a name.";
  if (!usernameValid(draft.handle)) return USERNAME_RULE;
  if (original?.handle && cleanUsername(draft.handle) === "") return "Your profile needs a username.";
  for (const row of draft.links) {
    const problem = rowProblem(row);
    if (problem) return problem;
  }
  return null;
}

/// Only what changed: TIDAL is sent nothing it already has.
export function profileChanges(
  original: EditableProfile,
  draft: ProfileDraft,
): { edit: ProfileEdit; bio: string | null } {
  const edit: ProfileEdit = {};
  const name = draft.name.trim();
  if (name !== original.name) edit.name = name;
  const handle = cleanUsername(draft.handle);
  if (handle !== (original.handle ?? "")) edit.handle = handle;

  const links = draft.links
    .filter((row) => rowHref(row) !== "")
    .map((row): ProfileLink => ({ href: rowHref(row), kind: rowKind(row, original) }));
  const before = original.links;
  if (
    links.length !== before.length ||
    links.some((link, i) => link.href !== before[i].href || link.kind !== before[i].kind)
  ) {
    edit.links = links;
  }

  const bio = draft.bio.trim();
  const bioChanged = original.bioId != null && bio !== (original.bio ?? "").trim();
  return { edit, bio: bioChanged ? bio : null };
}

export function hasChanges(changes: { edit: ProfileEdit; bio: string | null }): boolean {
  return Object.keys(changes.edit).length > 0 || changes.bio !== null;
}
