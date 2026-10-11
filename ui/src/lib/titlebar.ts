/// Custom window chrome (Windows): the window is undecorated, so the top band
/// of every shell doubles as the title bar. A press there on anything that is
/// not a control drags the window; a double-click toggles maximize. Rust owns
/// whether the chrome is active (`get_custom_titlebar`).

/// Height of the drag band and the caption buttons, in CSS px. Mirrors
/// `--titlebar-h` in app.css.
export const TITLEBAR_HEIGHT = 32;

/// Elements that keep their own pointer behaviour inside the band.
const INTERACTIVE = [
  "button",
  "a[href]",
  "input",
  "select",
  "textarea",
  "label",
  "summary",
  "[contenteditable='']",
  "[contenteditable='true']",
  "[role='button']",
  "[role='slider']",
  "[role='tab']",
  "[role='treeitem']",
  "[role='option']",
  "[role='menuitem']",
  "[role='checkbox']",
  "[role='switch']",
  "[tabindex]:not([tabindex='-1'])",
  "[draggable='true']",
  "[data-no-drag]",
].join(",");

/// Whether a primary press at `clientY` on `target` should move the window.
export function isDragTarget(target: EventTarget | null, clientY: number): boolean {
  if (clientY < 0 || clientY >= TITLEBAR_HEIGHT) return false;
  if (!(target instanceof Element)) return false;
  return target.closest(INTERACTIVE) === null;
}
