import { api } from "./api";
import { errorMessage } from "./errors";
import type { Toasts } from "./toasts.svelte";

export interface Mini {
  readonly active: boolean;
  readonly queueOpen: boolean;
  init(): Promise<void>;
  enter(): Promise<void>;
  exit(): Promise<void>;
  toggle(): Promise<void>;
  toggleQueue(): void;
}

/// Mini-player layout state (MP-D2): Rust owns the window; this store mirrors
/// it only after `set_mini_mode` succeeds, so the layout can never disagree
/// with the real window size. `init` trusts the launch resize Rust already did.
export function createMini(toasts: Toasts): Mini {
  let active = $state(false);
  let queueOpen = $state(false);
  // A successful mode command wins over a slower boot-time config read, so the
  // mirror can never be clobbered after the window already changed.
  let touched = false;

  async function setMode(next: boolean): Promise<void> {
    try {
      await api.setMiniMode(next);
      touched = true;
      active = next;
      queueOpen = false;
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function init(): Promise<void> {
    try {
      const config = await api.getConfig();
      if (!touched) active = config.ui.start_in_mini;
    } catch {
      if (!touched) active = false;
    }
  }

  return {
    get active() {
      return active;
    },
    get queueOpen() {
      return queueOpen;
    },
    init,
    enter: () => setMode(true),
    exit: () => setMode(false),
    toggle: () => setMode(!active),
    toggleQueue: () => {
      queueOpen = !queueOpen;
    },
  };
}
