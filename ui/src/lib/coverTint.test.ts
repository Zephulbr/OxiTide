import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({ api: { analyzeCover: vi.fn() } }));

import { clearCoverTints, coverTint } from "./coverTint";

beforeEach(() => clearCoverTints());

describe("coverTint", () => {
  it("returns the palette primary and caches it per theme", async () => {
    const analyze = vi.fn(async () => ({ primary: "#aabbcc" }));
    expect(await coverTint("a-b", "dark", { analyze })).toBe("#aabbcc");
    expect(await coverTint("a-b", "dark", { analyze })).toBe("#aabbcc");
    expect(analyze).toHaveBeenCalledTimes(1);
    await coverTint("a-b", "light", { analyze });
    expect(analyze).toHaveBeenCalledTimes(2);
  });

  it("shares one in-flight request between concurrent callers", async () => {
    const analyze = vi.fn(async () => ({ primary: "#123456" }));
    const [a, b] = await Promise.all([
      coverTint("x-y", "dark", { analyze }),
      coverTint("x-y", "dark", { analyze }),
    ]);
    expect([a, b]).toEqual(["#123456", "#123456"]);
    expect(analyze).toHaveBeenCalledTimes(1);
  });

  it("skips URLs and empty ids without calling the backend", async () => {
    const analyze = vi.fn(async () => ({ primary: "#000000" }));
    expect(await coverTint("https://example.com/a.jpg", "dark", { analyze })).toBeNull();
    expect(await coverTint(null, "dark", { analyze })).toBeNull();
    expect(analyze).not.toHaveBeenCalled();
  });

  it("returns null on failure and retries next time", async () => {
    const analyze = vi
      .fn<() => Promise<{ primary: string }>>()
      .mockRejectedValueOnce(new Error("nope"))
      .mockResolvedValueOnce({ primary: "#ffffff" });
    expect(await coverTint("c-d", "dark", { analyze })).toBeNull();
    expect(await coverTint("c-d", "dark", { analyze })).toBe("#ffffff");
  });

  it("runs at most three prefetches at once, but an urgent request skips the queue", async () => {
    const releases: (() => void)[] = [];
    let running = 0;
    let peak = 0;
    const slow = (uuid: string) => {
      running += 1;
      peak = Math.max(peak, running);
      return new Promise<{ primary: string }>((resolve) => {
        releases.push(() => {
          running -= 1;
          resolve({ primary: `#${uuid}` });
        });
      });
    };
    const analyze = vi.fn(slow);

    const batch = ["a-1", "a-2", "a-3", "a-4", "a-5"].map((id) => coverTint(id, "dark", { analyze }));
    await Promise.resolve();
    expect(analyze).toHaveBeenCalledTimes(3);

    // A hover while the queue is full still starts immediately.
    const urgent = coverTint("u-1", "dark", { analyze, urgent: true });
    expect(analyze).toHaveBeenCalledTimes(4);

    while (releases.length > 0) {
      releases.shift()!();
      await Promise.resolve();
      await Promise.resolve();
    }
    await Promise.all([...batch, urgent]);
    expect(analyze).toHaveBeenCalledTimes(6);
    expect(peak).toBeLessThanOrEqual(4);
  });
});
