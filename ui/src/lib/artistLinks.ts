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

export function isHttpUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}
