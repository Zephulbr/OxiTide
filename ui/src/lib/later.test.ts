import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LaterIds } from "./api";
import { api } from "./api";
import { createLater } from "./later.svelte";
import type { ToastAction, ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: {
    getSaveForLaterIds: vi.fn(),
    setSaveForLater: vi.fn(),
  },
}));

interface Shown {
  message: string;
  severity: ToastSeverity;
  action?: ToastAction;
}

function fakeToasts(): Toasts & { calls: Shown[] } {
  const calls: Shown[] = [];
  return {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info", action: options?.action });
      return calls.length;
    },
    dismiss() {},
    calls,
  };
}

const ids = (partial: Partial<LaterIds> = {}): LaterIds => ({
  tracks: [],
  albums: [],
  artists: [],
  playlists: [],
  mixes: [],
  videos: 0,
  ...partial,
});

describe("later store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.getSaveForLaterIds).mockResolvedValue(ids());
    vi.mocked(api.setSaveForLater).mockResolvedValue(undefined);
  });

  it("hydrates every kind once and counts videos", async () => {
    vi.mocked(api.getSaveForLaterIds).mockResolvedValue(
      ids({ tracks: [7], albums: [8], artists: [9], playlists: ["p-1"], mixes: ["0019ab"], videos: 2 }),
    );
    const later = createLater();
    await Promise.all([later.hydrate(), later.hydrate()]);
    await later.hydrate();
    expect(api.getSaveForLaterIds).toHaveBeenCalledTimes(1);
    expect(later.hydrated).toBe(true);
    expect(later.has("track", 7)).toBe(true);
    expect(later.has("track", "7")).toBe(true);
    expect(later.has("album", 8)).toBe(true);
    expect(later.has("artist", 9)).toBe(true);
    expect(later.has("playlist", "p-1")).toBe(true);
    expect(later.has("mix", "0019ab")).toBe(true);
    expect(later.has("playlist", "0019ab")).toBe(false);
    expect(later.has("album", 7)).toBe(false);
    expect(later.videos).toBe(2);
  });

  it("saves optimistically, toasts with an Open action, and bumps the revision", async () => {
    const later = createLater();
    const toasts = fakeToasts();
    const open = vi.fn();
    later.connect({ toasts, open });
    const pending = later.toggle("album", 8);
    expect(later.has("album", 8)).toBe(true);
    await expect(pending).resolves.toBe(true);
    expect(api.setSaveForLater).toHaveBeenCalledWith("album", "8", true);
    expect(later.revision).toBe(1);
    expect(toasts.calls[0].message).toBe("Saved for later");
    toasts.calls[0].action?.run();
    expect(open).toHaveBeenCalled();
  });

  it("removes a saved item", async () => {
    vi.mocked(api.getSaveForLaterIds).mockResolvedValue(ids({ playlists: ["p-1"] }));
    const later = createLater();
    const toasts = fakeToasts();
    later.connect({ toasts, open: () => {} });
    await later.hydrate();
    await expect(later.toggle("playlist", "p-1")).resolves.toBe(false);
    expect(api.setSaveForLater).toHaveBeenCalledWith("playlist", "p-1", false);
    expect(later.has("playlist", "p-1")).toBe(false);
    expect(toasts.calls[0].message).toBe("Removed from Save for later");
  });

  it("reverts and shows the error when the write fails", async () => {
    vi.mocked(api.setSaveForLater).mockRejectedValue("Save for Later is full");
    const later = createLater();
    const toasts = fakeToasts();
    later.connect({ toasts, open: () => {} });
    await expect(later.toggle("track", 7)).resolves.toBeNull();
    expect(later.has("track", 7)).toBe(false);
    expect(later.revision).toBe(0);
    expect(toasts.calls[0]).toMatchObject({ message: "Save for Later is full", severity: "error" });
  });

  it("keeps a toggle made during hydration over the stale snapshot", async () => {
    let resolve!: (value: LaterIds) => void;
    vi.mocked(api.getSaveForLaterIds).mockReturnValue(new Promise((r) => (resolve = r)));
    const later = createLater();
    const hydrating = later.hydrate();
    await later.toggle("track", 7); // saved
    await later.toggle("track", 7); // and removed again
    resolve(ids({ tracks: [7] })); // a snapshot from before the removal
    await hydrating;
    expect(later.has("track", 7)).toBe(false);
  });

  it("forgets everything on reset", async () => {
    vi.mocked(api.getSaveForLaterIds).mockResolvedValue(ids({ tracks: [7], videos: 1 }));
    const later = createLater();
    await later.hydrate();
    later.reset();
    expect(later.hydrated).toBe(false);
    expect(later.has("track", 7)).toBe(false);
    expect(later.videos).toBe(0);
  });
});
