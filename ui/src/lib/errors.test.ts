import { describe, expect, it } from "vitest";
import { API_DRIFT_MESSAGE, errorMessage } from "./errors";

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

  it("maps the stale playlist 412", () => {
    expect(
      errorMessage(
        'api error 412: {"status":412,"subStatus":7002,"userMessage":"You must send the correct Etag value in the If-None-Match header to modify a playlist"}',
      ),
    ).toBe("The playlist changed on TIDAL — refresh and try again");
  });

  it("names TIDAL API drift instead of a decode error", () => {
    expect(errorMessage("json error: missing field `id` at line 1 column 42")).toBe(API_DRIFT_MESSAGE);
    expect(errorMessage("tidal error: json error: invalid type: null, expected a string")).toBe(
      API_DRIFT_MESSAGE,
    );
    expect(errorMessage("api error 410: gone")).toBe(API_DRIFT_MESSAGE);
    expect(errorMessage("api error 4100: odd")).toBe("api error 4100: odd");
    expect(errorMessage("not found: /tracks/1")).toBe("not found: /tracks/1");
  });

  it("falls back to String for non-string errors", () => {
    expect(errorMessage(new Error("offline"))).toBe("Error: offline");
    expect(errorMessage(undefined)).toBe("undefined");
  });
});
