import { describe, expect, it } from "vitest";
import type { EditableProfile } from "./api";
import {
  draftFrom,
  draftProblem,
  hasChanges,
  linkKindFor,
  normalizeLink,
  profileChanges,
  USERNAME_RULE,
} from "./profileEdit";

const profile: EditableProfile = {
  artistId: 7,
  name: "CheeseMan",
  handle: "cheese",
  bio: "Cheese?",
  bioId: "bio-1",
  links: [{ href: "https://www.instagram.com/c", kind: "INSTAGRAM" }],
  hasPicture: true,
};

describe("profileChanges", () => {
  it("sends nothing for an untouched form", () => {
    const changes = profileChanges(profile, draftFrom(profile));
    expect(changes).toEqual({ edit: {}, bio: null });
    expect(hasChanges(changes)).toBe(false);
  });

  it("sends only the changed fields, trimmed, without a leading @", () => {
    const draft = { ...draftFrom(profile), name: " Cheese Man ", handle: "@newcheese", bio: "Gouda. " };
    expect(profileChanges(profile, draft)).toEqual({
      edit: { name: "Cheese Man", handle: "newcheese" },
      bio: "Gouda.",
    });
  });

  it("keeps known link types and types new links by host", () => {
    const draft = { ...draftFrom(profile), links: ["https://www.instagram.com/c", "x.com/cheese", "cheese.example", ""] };
    expect(profileChanges(profile, draft).edit.links).toEqual([
      { href: "https://www.instagram.com/c", kind: "INSTAGRAM" },
      { href: "https://x.com/cheese", kind: "TWITTER" },
      { href: "https://cheese.example", kind: "OFFICIAL_HOMEPAGE" },
    ]);
  });

  it("removing every link sends an empty list", () => {
    expect(profileChanges(profile, { ...draftFrom(profile), links: [] }).edit.links).toEqual([]);
  });

  it("never sends a bio the profile has no biography for", () => {
    const noBio = { ...profile, bio: null, bioId: null };
    expect(profileChanges(noBio, { ...draftFrom(noBio), bio: "New" }).bio).toBeNull();
  });
});

describe("draftProblem", () => {
  it("needs a name and web links", () => {
    expect(draftProblem(draftFrom(profile), profile)).toBeNull();
    expect(draftProblem({ ...draftFrom(profile), name: "  " }, profile)).toMatch(/name/);
    expect(draftProblem({ ...draftFrom(profile), links: ["mailto:me@x.com"] }, profile)).toMatch(/web link/);
  });

  it("holds the username to TIDAL's rule: a-z, 0-9 and underscores", () => {
    for (const ok of ["cheese_man", "c4", "@cheese", " cheese "]) {
      expect(draftProblem({ ...draftFrom(profile), handle: ok }, profile), ok).toBeNull();
    }
    for (const bad of ["Cheese", "a b", "chée", "a-b", "a.b"]) {
      expect(draftProblem({ ...draftFrom(profile), handle: bad }, profile), bad).toBe(USERNAME_RULE);
    }
  });

  it("won't clear a username that was set", () => {
    expect(draftProblem({ ...draftFrom(profile), handle: "" }, profile)).toMatch(/needs a username/);
    const none = { ...profile, handle: null };
    expect(draftProblem({ ...draftFrom(none), handle: "" }, none)).toBeNull();
  });
});

it("linkKindFor and normalizeLink", () => {
  expect(linkKindFor("https://m.youtube.com/@c")).toBe("YOUTUBE");
  expect(linkKindFor("not a url")).toBe("OFFICIAL_HOMEPAGE");
  expect(normalizeLink(" tiktok.com/@c ")).toBe("https://tiktok.com/@c");
  expect(normalizeLink("http://a.b")).toBe("http://a.b");
});
