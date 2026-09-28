import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { playMix } from "./mixPlay";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: { getMix: vi.fn(), playTracks: vi.fn() },
  trackToQueueItem: (t: { id: number }) => ({ track_id: t.id }),
}));

function fakeToasts() {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  const toasts: Toasts = {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
  };
  return { toasts, calls };
}

describe("playMix", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("queues the mapped tracks", async () => {
    vi.mocked(api.getMix).mockResolvedValue({
      id: "m1",
      title: "Mix",
      tracks: [{ id: 1 } as never, { id: 2 } as never],
    });
    const { toasts, calls } = fakeToasts();

    await playMix("m1", toasts);

    expect(api.playTracks).toHaveBeenCalledWith([{ track_id: 1 }, { track_id: 2 }], 0);
    expect(calls).toHaveLength(0);
  });

  it("informs instead of playing when the mix has no tracks", async () => {
    vi.mocked(api.getMix).mockResolvedValue({ id: "m1", title: "Mix", tracks: [] });
    const { toasts, calls } = fakeToasts();

    await playMix("m1", toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls[0]?.message).toContain("no playable tracks");
  });
});
