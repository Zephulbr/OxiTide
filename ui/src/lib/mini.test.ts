import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { createMini } from "./mini.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: { setMiniMode: vi.fn(), getConfig: vi.fn() },
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

describe("mini store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("enter sets active after the window command succeeds", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);

    await mini.enter();

    expect(api.setMiniMode).toHaveBeenCalledWith(true);
    expect(mini.active).toBe(true);
  });

  it("exit clears active and any open queue sheet", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);
    await mini.enter();
    mini.toggleQueue();

    await mini.exit();

    expect(api.setMiniMode).toHaveBeenLastCalledWith(false);
    expect(mini.active).toBe(false);
    expect(mini.queueOpen).toBe(false);
  });

  it("a rejected window command keeps the flag and toasts", async () => {
    vi.mocked(api.setMiniMode).mockRejectedValue("no window");
    const { toasts, calls } = fakeToasts();
    const mini = createMini(toasts);

    await mini.enter();

    expect(mini.active).toBe(false);
    expect(calls[0]?.severity).toBe("error");
  });

  it("toggle dispatches on the current state", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);

    await mini.toggle();
    expect(mini.active).toBe(true);

    await mini.toggle();
    expect(mini.active).toBe(false);
    expect(api.setMiniMode).toHaveBeenNthCalledWith(1, true);
    expect(api.setMiniMode).toHaveBeenNthCalledWith(2, false);
  });

  it("entering closes an open queue sheet", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);
    mini.toggleQueue();

    await mini.enter();

    expect(mini.queueOpen).toBe(false);
  });

  it("init mirrors start_in_mini without touching the window", async () => {
    vi.mocked(api.getConfig).mockResolvedValue({ ui: { start_in_mini: true } } as never);
    const mini = createMini(fakeToasts().toasts);

    await mini.init();

    expect(mini.active).toBe(true);
    expect(api.setMiniMode).not.toHaveBeenCalled();
  });

  it("a config failure leaves the default off", async () => {
    vi.mocked(api.getConfig).mockRejectedValue("offline");
    const mini = createMini(fakeToasts().toasts);

    await mini.init();

    expect(mini.active).toBe(false);
  });

  it("init does not clobber a mode command that already ran", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    let resolveConfig!: (value: unknown) => void;
    vi.mocked(api.getConfig).mockReturnValue(
      new Promise((resolve) => {
        resolveConfig = resolve;
      }) as never,
    );
    const mini = createMini(fakeToasts().toasts);

    const pending = mini.init();
    await mini.enter();
    resolveConfig({ ui: { start_in_mini: false } });
    await pending;

    expect(api.setMiniMode).toHaveBeenCalledWith(true);
    expect(mini.active).toBe(true);
  });
});
