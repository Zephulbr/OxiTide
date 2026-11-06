import { describe, expect, it } from "vitest";
import { isOwnArtist, ownCard, profileEditUrl } from "./ownProfile";

describe("ownCard", () => {
  it("is null without a profile", () => {
    expect(ownCard(null)).toBeNull();
    expect(ownCard({ profile: null, artist: null })).toBeNull();
  });

  it("prefers the artist's name and picture, and opens the artist", () => {
    const card = ownCard({
      profile: { userId: 42, name: "Profile name", picture: "p-1", artistId: 7 },
      artist: { id: 7, name: "Artist name", picture: "a-1" },
    });
    expect(card).toEqual({ name: "Artist name", picture: "a-1", artistId: 7 });
  });

  it("falls back to the profile when there is no artist", () => {
    expect(ownCard({ profile: { userId: 42, name: null, handle: "vee", picture: "p-1" }, artist: null })).toEqual({
      name: "vee",
      picture: "p-1",
      artistId: null,
    });
    expect(ownCard({ profile: { userId: 42 }, artist: null })?.name).toBe("Your profile");
  });
});

describe("isOwnArtist", () => {
  it("matches only the profile's artist", () => {
    const me = { profile: { userId: 42, artistId: 7 }, artist: null };
    expect(isOwnArtist(me, 7)).toBe(true);
    expect(isOwnArtist(me, 8)).toBe(false);
    expect(isOwnArtist({ profile: { userId: 42 }, artist: null }, 7)).toBe(false);
    expect(isOwnArtist(null, 7)).toBe(false);
  });
});

it("profileEditUrl opens TIDAL's editor", () => {
  expect(profileEditUrl(7)).toBe("https://tidal.com/artist/7?edit-profile=true");
});
