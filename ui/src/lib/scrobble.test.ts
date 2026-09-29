import { describe, expect, it } from "vitest";
import type { ScrobbleServiceStatus } from "./api";
import { canToggle, queuedLabel, serviceLabel } from "./scrobble";

function status(patch: Partial<ScrobbleServiceStatus>): ScrobbleServiceStatus {
  return {
    id: "lastfm",
    enabled: false,
    connected: false,
    username: null,
    queued: 0,
    needs_reconnect: false,
    last_error: null,
    configured: true,
    ...patch,
  };
}

describe("scrobble helpers", () => {
  it("reports a missing key", () => {
    expect(serviceLabel(status({ configured: false }))).toBe("API key not configured");
  });

  it("prioritizes reconnect over connected", () => {
    expect(
      serviceLabel(status({ connected: true, username: "v", needs_reconnect: true })),
    ).toBe("Reconnect needed");
  });

  it("names the connected user", () => {
    expect(serviceLabel(status({ connected: true, username: "v" }))).toBe("Connected as v");
  });

  it("defaults to not connected", () => {
    expect(serviceLabel(status({}))).toBe("Not connected");
  });

  it("only allows toggling a connected service", () => {
    expect(canToggle(status({ connected: false }))).toBe(false);
    expect(canToggle(status({ connected: true }))).toBe(true);
  });

  it("describes the queue only when non-empty", () => {
    expect(queuedLabel(0)).toBe("");
    expect(queuedLabel(3)).toBe("3 queued");
  });
});
