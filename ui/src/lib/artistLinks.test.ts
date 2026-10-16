import { describe, expect, it } from "vitest";
import { isHttpUrl, linkBrand, linkLabel } from "./artistLinks";

describe("linkLabel", () => {
  it("maps known site keys", () => {
    expect(linkLabel("TWITTER", "https://twitter.com/acdc")).toBe("Twitter");
    expect(linkLabel("OFFICIAL_HOMEPAGE", "https://acdc.com")).toBe("Website");
    expect(linkLabel("LAST_FM", "https://last.fm/music/AC+DC")).toBe("Last.fm");
  });
  it("falls back to the host for unknown sites", () => {
    expect(linkLabel("SOMETHING_NEW", "https://www.example.com/x")).toBe("example.com");
  });
  it("falls back to the site key for unparseable urls", () => {
    expect(linkLabel("MYSTERY", "not a url")).toBe("MYSTERY");
  });
});

describe("linkBrand", () => {
  it("maps known site keys", () => {
    expect(linkBrand("TWITTER", "https://twitter.com/acdc")).toBe("x");
    expect(linkBrand("instagram", "https://instagram.com/acdc")).toBe("instagram");
  });
  it("falls back to the host, including subdomains", () => {
    expect(linkBrand("SOMETHING_NEW", "https://en.wikipedia.org/wiki/AC/DC")).toBe("wikipedia");
    expect(linkBrand("", "https://acdc.bandcamp.com")).toBe("bandcamp");
  });
  it("returns null for unknown hosts and junk", () => {
    expect(linkBrand("OFFICIAL_HOMEPAGE", "https://acdc.com")).toBeNull();
    expect(linkBrand("", "https://notx.com")).toBeNull();
    expect(linkBrand("", "not a url")).toBeNull();
  });
});

describe("isHttpUrl", () => {
  it("accepts http and https", () => {
    expect(isHttpUrl("https://example.com")).toBe(true);
    expect(isHttpUrl("http://example.com")).toBe(true);
  });
  it("rejects other schemes and junk", () => {
    expect(isHttpUrl("javascript:alert(1)")).toBe(false);
    expect(isHttpUrl("file:///C:/secret")).toBe(false);
    expect(isHttpUrl("")).toBe(false);
  });
});
