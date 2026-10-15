/// The webview's own right-click menu ("Save image as", "Inspect", …) is not
/// part of the app. Suppress it, except where it does real work: editable
/// fields (cut/copy/paste/spellcheck) and a text selection (copy). Rows with
/// their own menu already call preventDefault through `menuFor`.
export function shouldSuppressNativeMenu(target: EventTarget | null, selection: string): boolean {
  if (selection.trim() !== "") return false;
  if (!(target instanceof Element)) return true;
  return !target.closest("input, textarea, select, [contenteditable]:not([contenteditable='false'])");
}

/// Installs the suppression; `enabled` is false in dev builds so Inspect stays
/// reachable. Returns the uninstall function.
export function installNativeMenuSuppression(enabled: boolean = !import.meta.env.DEV): () => void {
  if (!enabled) return () => {};
  const onContextMenu = (event: MouseEvent) => {
    if (shouldSuppressNativeMenu(event.target, window.getSelection()?.toString() ?? "")) {
      event.preventDefault();
    }
  };
  window.addEventListener("contextmenu", onContextMenu);
  return () => window.removeEventListener("contextmenu", onContextMenu);
}
