/// Display labels for TIDAL external-link site keys, plus a host fallback.
const LABELS: Record<string, string> = {
  TWITTER: "Twitter",
  FACEBOOK: "Facebook",
  INSTAGRAM: "Instagram",
  YOUTUBE: "YouTube",
  TIKTOK: "TikTok",
  OFFICIAL_HOMEPAGE: "Website",
  WEBSITE: "Website",
  HOMEPAGE: "Website",
  LAST_FM: "Last.fm",
  WIKIPEDIA: "Wikipedia",
  DISCOGS: "Discogs",
  ALLMUSIC: "AllMusic",
  BBC_MUSICPAGE: "BBC Music",
  MYSPACE: "MySpace",
  SOUNDCLOUD: "SoundCloud",
  BANDCAMP: "Bandcamp",
};

export function linkLabel(site: string, url: string): string {
  const known = LABELS[site.toUpperCase()];
  if (known) return known;
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return site || "Link";
  }
}

export type Brand =
  | "facebook"
  | "instagram"
  | "x"
  | "youtube"
  | "tiktok"
  | "lastfm"
  | "wikipedia"
  | "discogs"
  | "myspace"
  | "soundcloud"
  | "bandcamp";

const SITE_BRANDS: Record<string, Brand> = {
  TWITTER: "x",
  FACEBOOK: "facebook",
  INSTAGRAM: "instagram",
  YOUTUBE: "youtube",
  TIKTOK: "tiktok",
  LAST_FM: "lastfm",
  WIKIPEDIA: "wikipedia",
  DISCOGS: "discogs",
  MYSPACE: "myspace",
  SOUNDCLOUD: "soundcloud",
  BANDCAMP: "bandcamp",
};

const HOST_BRANDS: [string, Brand][] = [
  ["twitter.com", "x"],
  ["x.com", "x"],
  ["facebook.com", "facebook"],
  ["instagram.com", "instagram"],
  ["youtube.com", "youtube"],
  ["youtu.be", "youtube"],
  ["tiktok.com", "tiktok"],
  ["last.fm", "lastfm"],
  ["wikipedia.org", "wikipedia"],
  ["discogs.com", "discogs"],
  ["myspace.com", "myspace"],
  ["soundcloud.com", "soundcloud"],
  ["bandcamp.com", "bandcamp"],
];

/// Brand logo for a link, by site key then host; null means a generic globe.
export function linkBrand(site: string, url: string): Brand | null {
  const known = SITE_BRANDS[site.toUpperCase()];
  if (known) return known;
  let host: string;
  try {
    host = new URL(url).hostname.toLowerCase();
  } catch {
    return null;
  }
  for (const [domain, brand] of HOST_BRANDS) {
    if (host === domain || host.endsWith(`.${domain}`)) return brand;
  }
  return null;
}

export function isHttpUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}
