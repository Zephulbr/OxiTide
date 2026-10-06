import type { HotkeyAction } from "./hotkeys";

/// Coordinates the recorder with the backend: capture teardown is awaited
/// strictly before a save invoke, so re-registration cannot race, and the
/// keydown listener is gated on `active` (which flips only after capture
/// engagement resolves) so a fast first press cannot double-dispatch.
export interface HotkeyCaptureOptions {
  setCapture(active: boolean): Promise<void>;
  save(action: HotkeyAction, accelerator: string, replace: HotkeyAction | null): Promise<void>;
  report(error: unknown): void;
}

export interface HotkeyCapture {
  readonly recording: HotkeyAction | null;
  readonly active: boolean;
  start(action: HotkeyAction): Promise<void>;
  cancel(): void;
  finish(
    action: HotkeyAction,
    accelerator: string,
    replace?: HotkeyAction | null,
  ): Promise<void>;
}

export function createHotkeyCapture(options: HotkeyCaptureOptions): HotkeyCapture {
  let recording = $state<HotkeyAction | null>(null);
  let active = $state(false);
  // Every backend mutation runs on this chain, in the order requested.
  let chain: Promise<void> = Promise.resolve();
  // Bumped by any newer operation; a queued engagement that sees a stale
  // token never claims `active` and never fights the newer operation.
  let generation = 0;
  let engaged = false;

  function enqueue(run: () => Promise<void>): Promise<void> {
    chain = chain.then(run, run);
    return chain;
  }

  async function disengage() {
    if (!engaged) return;
    try {
      await options.setCapture(false);
      engaged = false;
    } catch (error) {
      // Keep `engaged` set so a later cancel retries the release instead of
      // leaving the backend capturing keys forever.
      options.report(error);
    }
  }

  async function start(action: HotkeyAction) {
    recording = action;
    active = false;
    const token = ++generation;
    await enqueue(async () => {
      if (token !== generation) return;
      try {
        await options.setCapture(true);
      } catch (error) {
        if (token === generation) {
          recording = null;
          active = false;
        }
        await disengage();
        options.report(error);
        return;
      }
      engaged = true;
      if (token === generation && recording === action) active = true;
    });
  }

  function cancel() {
    if (recording === null && !active && !engaged) return;
    recording = null;
    active = false;
    generation += 1;
    void enqueue(disengage);
  }

  async function finish(
    action: HotkeyAction,
    accelerator: string,
    replace: HotkeyAction | null = null,
  ) {
    recording = null;
    active = false;
    generation += 1;
    await enqueue(disengage);
    await enqueue(() => options.save(action, accelerator, replace));
  }

  return {
    get recording() {
      return recording;
    },
    get active() {
      return active;
    },
    start,
    cancel,
    finish,
  };
}
