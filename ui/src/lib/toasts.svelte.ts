export type ToastSeverity = "success" | "info" | "error";

export interface ToastAction {
  label: string;
  run(): void;
}

export interface Toast {
  id: number;
  message: string;
  severity: ToastSeverity;
  action?: ToastAction;
}

export interface Toasts {
  readonly toasts: readonly Toast[];
  show(message: string, options?: { severity?: ToastSeverity; action?: ToastAction }): number;
  dismiss(id: number): void;
}

/// Auto-dismiss delay for success/info toasts; errors persist until dismissed.
const AUTO_DISMISS_MS = 6000;
/// Visible toasts after which the oldest is evicted.
const MAX_TOASTS = 3;

/// Outcome messages shown in the toast host. Errors never auto-dismiss; when
/// the cap is hit the oldest auto-dismissing toast is evicted before any error.
export function createToasts(): Toasts {
  let toasts = $state<Toast[]>([]);
  let nextId = 1;
  const timers = new Map<number, ReturnType<typeof setTimeout>>();

  function clearTimer(id: number) {
    const timer = timers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timers.delete(id);
    }
  }

  function dismiss(id: number) {
    clearTimer(id);
    toasts = toasts.filter((t) => t.id !== id);
  }

  function startTimer(id: number) {
    timers.set(id, setTimeout(() => dismiss(id), AUTO_DISMISS_MS));
  }

  function evictOne() {
    const victim = toasts.find((t) => t.severity !== "error") ?? toasts[0];
    if (victim) dismiss(victim.id);
  }

  return {
    get toasts() {
      return toasts;
    },
    show(message, options = {}) {
      const severity = options.severity ?? "info";
      if (toasts.length >= MAX_TOASTS) evictOne();
      const id = nextId++;
      toasts = [...toasts, { id, message, severity, action: options.action }];
      if (severity !== "error") startTimer(id);
      return id;
    },
    dismiss,
  };
}
