import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getPlaybackState: vi.fn(),
  retry: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("./api", () => ({ api: mocks }));

import { createPlayer, emptyState } from "./player.svelte";
import { createToasts } from "./toasts.svelte";

type Handler = (event: { payload: unknown }) => void;

async function wire() {
  const toasts = createToasts();
  const player = createPlayer(toasts);
  await player.init();
  const handler = mocks.listen.mock.calls[0][1] as Handler;
  return { toasts, player, handler };
}

/// A Rust `state` event payload; unlike `emptyState` it carries no UI-only
/// error fields, so it mirrors what the engine actually sends.
function snapshot(overrides: Record<string, unknown> = {}) {
  return {
    type: "state",
    playing: false,
    buffering: false,
    current: null,
    position_ms: 0,
    duration_ms: 0,
    volume: 1,
    shuffle: false,
    repeat: "off",
    preferred_quality: "lossless",
    output_mode: "shared",
    bit_perfect: false,
    bit_perfect_notice: null,
    dsp_active: false,
    dsp_stages: [],
    queue: [],
    stream: null,
    ...overrides,
  };
}

beforeEach(() => {
  mocks.getPlaybackState.mockReset().mockResolvedValue({ ...emptyState() });
  mocks.retry.mockReset().mockResolvedValue(undefined);
  mocks.listen.mockReset().mockResolvedValue(() => {});
});

describe("player error toasts", () => {
  it("shows a persistent error toast with a retry action on an engine error", async () => {
    const { toasts, handler } = await wire();
    handler({
      payload: { type: "error", code: "output", message: "Audio device has been disconnected or stalled" },
    });
    expect(toasts.toasts).toHaveLength(1);
    expect(toasts.toasts[0]).toMatchObject({
      severity: "error",
      message: "Audio device has been disconnected or stalled",
    });
    expect(toasts.toasts[0].action?.label).toBe("Retry");
  });

  it("replaces the previous error toast", async () => {
    const { toasts, handler } = await wire();
    handler({ payload: { type: "error", code: "output", message: "first" } });
    handler({ payload: { type: "error", code: "output", message: "second" } });
    expect(toasts.toasts).toHaveLength(1);
    expect(toasts.toasts[0].message).toBe("second");
  });

  it("dismisses the toast on now_playing", async () => {
    const { toasts, handler } = await wire();
    handler({ payload: { type: "error", code: "output", message: "boom" } });
    handler({ payload: { type: "now_playing", item: null } });
    expect(toasts.toasts).toHaveLength(0);
  });

  it("dismisses the toast when retry clears a non-output error", async () => {
    const { toasts, player, handler } = await wire();
    handler({ payload: { type: "error", code: "playback", message: "boom" } });
    toasts.toasts[0].action?.run();
    await vi.waitFor(() => expect(toasts.toasts).toHaveLength(0));
    expect(mocks.retry).toHaveBeenCalledTimes(1);
    expect(player.state.error).toBe("");
  });

  it("keeps an output error until the engine reports recovery", async () => {
    const { toasts, player, handler } = await wire();
    const message = "Audio device has been disconnected or stalled";
    handler({ payload: { type: "error", code: "output", message } });
    handler({ payload: snapshot({ bit_perfect_notice: message }) });
    await player.retry();
    expect(player.state.error).toBe(message);
    expect(player.state.error_code).toBe("output");

    handler({ payload: snapshot({ bit_perfect_notice: null }) });
    expect(player.state.error).toBe("");
    expect(player.state.error_code).toBe("");
    expect(toasts.toasts).toHaveLength(0);
  });

  it("keeps the error state without a toast host", async () => {
    const player = createPlayer();
    await player.init();
    const handler = mocks.listen.mock.calls[0][1] as Handler;
    handler({ payload: { type: "error", code: "output", message: "boom" } });
    expect(player.state.error).toBe("boom");
  });
});
