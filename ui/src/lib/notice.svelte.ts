export interface Notice {
  readonly message: string | null;
  show(msg: string): void;
  dismiss(): void;
}

/// Transient user-facing message shown in the app's notice strip; a new
/// message restarts the six-second auto-dismiss timer.
export function createNotice(): Notice {
  let message = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function stopTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  return {
    get message() {
      return message;
    },
    show(msg: string) {
      message = msg;
      stopTimer();
      timer = setTimeout(() => {
        message = null;
        timer = null;
      }, 6000);
    },
    dismiss() {
      stopTimer();
      message = null;
    },
  };
}
