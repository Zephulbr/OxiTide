import { describe, it, expect } from "vitest";
import { shareUrl } from "./share";

describe("shareUrl", () => {
  it("builds playlist links from the uuid", () => {
    expect(shareUrl("playlist", "d8f1a3e5-1111-2222-3333-444455556666")).toBe(
      "https://tidal.com/playlist/d8f1a3e5-1111-2222-3333-444455556666",
    );
  });
  it("builds album links from the numeric id", () => {
    expect(shareUrl("album", 44590541)).toBe("https://tidal.com/album/44590541");
  });
  it("builds mix links from the id", () => {
    expect(shareUrl("mix", "0069c87839288a76ffd67d83b7cb11")).toBe(
      "https://tidal.com/mix/0069c87839288a76ffd67d83b7cb11",
    );
  });
  it("builds artist links from the numeric id", () => {
    expect(shareUrl("artist", 945)).toBe("https://tidal.com/artist/945");
  });
});
