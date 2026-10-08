const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), ' +
  'textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function focusables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE));
}

/// Svelte action for dialog-like surfaces: focus the first control, keep
/// Tab/Shift+Tab cycling inside `node`, and restore the mount-time focus on
/// destroy (guarded so an unmounted opener is not resurrected).
export function focusTrap(node: HTMLElement, options: { restore?: boolean } = {}) {
  const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  (focusables(node)[0] ?? node).focus();

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Tab") return;
    const list = focusables(node);
    if (list.length === 0) {
      event.preventDefault();
      node.focus();
      return;
    }
    const current =
      document.activeElement instanceof HTMLElement ? list.indexOf(document.activeElement) : -1;
    const next = event.shiftKey
      ? current <= 0
        ? list.length - 1
        : current - 1
      : current === -1 || current === list.length - 1
        ? 0
        : current + 1;
    event.preventDefault();
    list[next].focus();
  }

  node.addEventListener("keydown", onKeydown);
  return {
    destroy() {
      node.removeEventListener("keydown", onKeydown);
      if (options.restore !== false && previous?.isConnected) previous.focus();
    },
  };
}
