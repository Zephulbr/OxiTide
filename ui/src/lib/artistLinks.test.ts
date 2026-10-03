import { describe, expect, it } from "vitest";
import { isHttpUrl, linkLabel } from "./artistLinks";

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
