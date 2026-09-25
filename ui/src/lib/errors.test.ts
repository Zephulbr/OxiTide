import { describe, expect, it } from "vitest";
import { errorMessage } from "./errors";

describe("errorMessage", () => {
  it("maps the forbidden backend string", () => {
    expect(errorMessage("forbidden")).toBe("You can only edit your own playlists");
  });

  it("maps the rate limited backend string", () => {
    expect(errorMessage("rate limited")).toBe(
      "TIDAL is rate-limiting requests — try again in a moment",
    );
  });

  it("passes other backend strings through", () => {
    expect(errorMessage("not found: /playlists/x")).toBe("not found: /playlists/x");
    expect(errorMessage("api error 500: boom")).toBe("api error 500: boom");
  });

  it("falls back to String for non-string errors", () => {
    expect(errorMessage(new Error("offline"))).toBe("Error: offline");
    expect(errorMessage(undefined)).toBe("undefined");
  });
});
