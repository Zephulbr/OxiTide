import { describe, expect, it } from "vitest";
import { countdownLabel, firewallHint, lastSeenLabel, parsePort } from "./remoteSettings";

describe("remote settings helpers", () => {
  it("labels last-seen ages", () => {
    const now = 1_000_000_000;
    expect(lastSeenLabel(now - 10_000, now)).toBe("just now");
    expect(lastSeenLabel(now - 5 * 60_000, now)).toBe("5 min ago");
    expect(lastSeenLabel(now - 3 * 3_600_000, now)).toBe("3 h ago");
    expect(lastSeenLabel(now - 30 * 3_600_000, now)).toBe("yesterday");
    expect(lastSeenLabel(now - 72 * 3_600_000, now)).toBe("3 days ago");
    expect(lastSeenLabel(now + 60_000, now)).toBe("just now");
  });

  it("formats the pairing countdown", () => {
    expect(countdownLabel(300)).toBe("5:00");
    expect(countdownLabel(65.9)).toBe("1:05");
    expect(countdownLabel(-3)).toBe("0:00");
  });

  it("accepts only unprivileged ports with fallback room", () => {
    expect(parsePort("47820")).toBe(47820);
    expect(parsePort(" 1024 ")).toBe(1024);
    expect(parsePort("80")).toBeNull();
    expect(parsePort("65530")).toBeNull();
    expect(parsePort("abc")).toBeNull();
    expect(parsePort("4782.5")).toBeNull();
  });

  it("names the right firewall", () => {
    expect(firewallHint("Mozilla/5.0 (Windows NT 10.0; Win64; x64)", 47820)).toContain(
      "Private networks",
    );
    expect(firewallHint("Mozilla/5.0 (X11; Linux x86_64)", 47821)).toContain("ufw allow 47821/tcp");
  });
});
