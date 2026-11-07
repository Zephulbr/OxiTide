import { describe, expect, it } from "vitest";
import type { EditableProfile } from "./api";
import {
  draftFrom,
  draftProblem,
  hasChanges,
  linkKindFor,
  missingSocials,
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

  it("keeps known link types and types new free links by host", () => {
    const free = (href: string) => ({ href, social: null });
    const draft = {
      ...draftFrom(profile),
      links: [...draftFrom(profile).links, free("x.com/cheese"), free("cheese.example"), free("youtube.com/@c"), free("")],
    };
    expect(profileChanges(profile, draft).edit.links).toEqual([
      { href: "https://www.instagram.com/c", kind: "INSTAGRAM" },
      { href: "https://x.com/cheese", kind: "TWITTER" },
      { href: "https://cheese.example", kind: "OFFICIAL_HOMEPAGE" },
      // TIDAL has no YouTube type.
      { href: "https://youtube.com/@c", kind: "OFFICIAL_HOMEPAGE" },
    ]);
  });

  it("a social field takes a username or a link, typed by the field", () => {
    const draft = {
      ...draftFrom(profile),
      links: [
        { href: "@cheese", social: "TIKTOK" as const },
        { href: "cheese.man", social: "SNAPCHAT" as const },
        { href: "facebook.com/cheese", social: "FACEBOOK" as const },
        { href: "cheese", social: "TWITTER" as const },
      ],
    };
    expect(profileChanges(profile, draft).edit.links).toEqual([
      { href: "https://www.tiktok.com/@cheese", kind: "TIKTOK" },
      { href: "https://www.snapchat.com/add/cheese.man", kind: "SNAPCHAT" },
      { href: "https://facebook.com/cheese", kind: "FACEBOOK" },
      { href: "https://x.com/cheese", kind: "TWITTER" },
    ]);
  });

  it("reads TIDAL's social links into their fields", () => {
    expect(draftFrom(profile).links).toEqual([{ href: "https://www.instagram.com/c", social: "INSTAGRAM" }]);
    const site = { ...profile, links: [{ href: "https://cheese.example", kind: "OFFICIAL_HOMEPAGE" }] };
    expect(draftFrom(site).links).toEqual([{ href: "https://cheese.example", social: null }]);
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
    expect(
      draftProblem({ ...draftFrom(profile), links: [{ href: "mailto:me@x.com", social: null }] }, profile),
    ).toMatch(/web link/);
  });

  it("holds a social field's link to its site", () => {
    const links = [{ href: "https://facebook.com/c", social: "INSTAGRAM" as const }];
    expect(draftProblem({ ...draftFrom(profile), links }, profile)).toBe(
      `"https://facebook.com/c" isn't a link to Instagram.`,
    );
    const ok = [{ href: "https://m.instagram.com/c", social: "INSTAGRAM" as const }];
    expect(draftProblem({ ...draftFrom(profile), links: ok }, profile)).toBeNull();
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

it("missingSocials offers each social field the form lacks, in TIDAL's order", () => {
  const kinds = (links: Parameters<typeof missingSocials>[0]) => missingSocials(links).map((s) => s.kind);
  expect(kinds([])).toEqual(["INSTAGRAM", "TIKTOK", "FACEBOOK", "TWITTER", "SNAPCHAT"]);
  // A free link on a social site counts as that field.
  expect(kinds([{ href: "x.com/c", social: null }, { href: "", social: "TIKTOK" }])).toEqual([
    "INSTAGRAM",
    "FACEBOOK",
    "SNAPCHAT",
  ]);
});

it("linkKindFor and normalizeLink", () => {
  expect(linkKindFor("https://m.youtube.com/@c")).toBe("OFFICIAL_HOMEPAGE");
  expect(linkKindFor("https://www.snapchat.com/add/c")).toBe("SNAPCHAT");
  expect(linkKindFor("not a url")).toBe("OFFICIAL_HOMEPAGE");
  expect(normalizeLink(" tiktok.com/@c ")).toBe("https://tiktok.com/@c");
  expect(normalizeLink("http://a.b")).toBe("http://a.b");
});
