import { describe, expect, it, vi } from "vitest";
import { createHotkeyCapture } from "./hotkeyCapture.svelte";

function setup() {
  const calls: string[] = [];
  const capture = createHotkeyCapture({
    setCapture: vi.fn(async (active: boolean) => {
      calls.push(`capture:${active}`);
    }),
    save: vi.fn(async () => {
      calls.push("save");
    }),
    report: vi.fn(),
  });
  return { capture, calls };
}

function setupPendingCapture() {
  const calls: string[] = [];
  let releaseCapture: () => void = () => {};
  const capture = createHotkeyCapture({
    setCapture: vi.fn((active: boolean) => {
      calls.push(`capture:${active}`);
      if (!active) return Promise.resolve();
      return new Promise<void>((resolve) => (releaseCapture = resolve));
    }),
    save: vi.fn(async () => {
      calls.push("save");
    }),
    report: vi.fn(),
  });
  return { capture, calls, release: () => releaseCapture() };
}

describe("hotkey capture", () => {
  it("engages capture before reporting active", async () => {
    const { capture, calls } = setup();
    await capture.start("play_pause");
    expect(calls).toEqual(["capture:true"]);
    expect(capture.active).toBe(true);
    expect(capture.recording).toBe("play_pause");
  });

  it("orders capture teardown strictly before save", async () => {
    const { capture, calls } = setup();
    await capture.start("play_pause");
    await capture.finish("play_pause", "Ctrl+Alt+KeyP");
    expect(calls).toEqual(["capture:true", "capture:false", "save"]);
    expect(capture.recording).toBeNull();
  });

  it("finishes while capture engagement is still in flight", async () => {
    const { capture, calls, release } = setupPendingCapture();
    const starting = capture.start("next");
    await Promise.resolve(); // engagement reached the backend and is pending
    const finishing = capture.finish("next", "Ctrl+Alt+KeyN");
    release();
    await starting;
    await finishing;
    expect(calls).toEqual(["capture:true", "capture:false", "save"]);
  });

  it("cancels an in-flight engagement without leaving capture on", async () => {
    const { capture, calls, release } = setupPendingCapture();
    const starting = capture.start("prev");
    await Promise.resolve(); // engagement reached the backend and is pending
    capture.cancel();
    release();
    await starting;
    await Promise.resolve();
    expect(calls).toEqual(["capture:true", "capture:false"]);
    expect(capture.recording).toBeNull();
    expect(capture.active).toBe(false);
  });

  it("reports capture failures", async () => {
    const report = vi.fn();
    const capture = createHotkeyCapture({
      setCapture: vi.fn(async () => {
        throw new Error("plugin down");
      }),
      save: vi.fn(),
      report,
    });
    await capture.start("play_pause");
    expect(report).toHaveBeenCalled();
    expect(capture.recording).toBeNull();
  });

  it("keeps a superseding start behind an in-flight save", async () => {
    const calls: string[] = [];
    let releaseSave: () => void = () => {};
    const capture = createHotkeyCapture({
      setCapture: vi.fn(async (active: boolean) => {
        calls.push(`capture:${active}`);
      }),
      save: vi.fn(() => {
        calls.push("save");
        return new Promise<void>((resolve) => (releaseSave = resolve));
      }),
      report: vi.fn(),
    });
    await capture.start("next");
    const finishing = capture.finish("next", "Ctrl+Alt+KeyN");
    for (let i = 0; i < 5 && calls.length < 3; i += 1) {
      await Promise.resolve(); // save reached the backend and is pending
    }
    const restarting = capture.start("prev");
    await Promise.resolve();
    expect(calls).toEqual(["capture:true", "capture:false", "save"]);
    releaseSave();
    await finishing;
    await restarting;
    expect(calls).toEqual(["capture:true", "capture:false", "save", "capture:true"]);
  });

  it("disengages when a re-engage fails after a prior engagement", async () => {
    const calls: string[] = [];
    let engage = 0;
    const capture = createHotkeyCapture({
      setCapture: vi.fn(async (active: boolean) => {
        calls.push(`capture:${active}`);
        if (active) {
          engage += 1;
          if (engage === 2) throw new Error("plugin down");
        }
      }),
      save: vi.fn(),
      report: vi.fn(),
    });
    await capture.start("next");
    await capture.start("prev");
    expect(calls).toEqual(["capture:true", "capture:true", "capture:false"]);
    expect(capture.recording).toBeNull();
    expect(capture.active).toBe(false);
  });

  it("retries a failed release on a later cancel", async () => {
    const calls: string[] = [];
    const report = vi.fn();
    let releases = 0;
    const capture = createHotkeyCapture({
      setCapture: vi.fn(async (active: boolean) => {
        calls.push(`capture:${active}`);
        if (!active) {
          releases += 1;
          if (releases === 1) throw new Error("plugin down");
        }
      }),
      save: vi.fn(),
      report,
    });
    await capture.start("play_pause");
    capture.cancel();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(report).toHaveBeenCalled();
    expect(calls).toEqual(["capture:true", "capture:false"]);

    capture.cancel();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(calls).toEqual(["capture:true", "capture:false", "capture:false"]);
  });
});
