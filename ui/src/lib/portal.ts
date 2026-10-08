/// Svelte action: render the node at the document body so ancestors with
/// transforms/animations or `inert` cannot clip or disable it. The overlay is
/// `position: fixed`, so leaving its Svelte parent visually changes nothing.
/// Moving a subtree that holds focus disconnects the focused node (browsers
/// reset focus to body), so re-apply it after the move — the Modal's
/// focusTrap action runs before this one.
export function portal(node: HTMLElement) {
  const active = document.activeElement;
  document.body.appendChild(node);
  if (active instanceof HTMLElement && node.contains(active)) active.focus();
  return {
    destroy() {
      node.remove();
    },
  };
}
