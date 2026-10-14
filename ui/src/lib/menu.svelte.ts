import type { IconName } from "./components/Icon.svelte";

export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  danger?: boolean;
  /// Heading rendered above this row; consecutive rows sharing a section get
  /// one heading. Headings are decorative — keyboard nav skips them.
  section?: string;
  /// Filled treatment for the active option (radio-style).
  selected?: boolean;
  /// Small icon rendered at the trailing edge, e.g. the sort direction arrow.
  trailing?: IconName;
  run(): void;
}

export interface MenuState {
  x: number;
  y: number;
  items: MenuItem[];
  /// The element that opened the menu; closing restores focus to it.
  opener: HTMLElement | null;
}

export interface Menu {
  readonly open: MenuState | null;
  show(x: number, y: number, items: MenuItem[], opener?: HTMLElement | null): void;
  /// Close; with `restoreFocus` (default) focus returns to a connected opener.
  close(restoreFocus?: boolean): void;
}

export function createMenu(): Menu {
  // Replaced whole, never mutated: raw keeps item and opener identity.
  let open = $state.raw<MenuState | null>(null);
  return {
    get open() {
      return open;
    },
    show(x, y, items, opener = null) {
      open = { x, y, items, opener };
    },
    close(restoreFocus = true) {
      const state = open;
      open = null;
      if (restoreFocus && state?.opener?.isConnected) state.opener.focus();
    },
  };
}

/// One app-wide menu: rows across every view attach through `menuFor`, and
/// App renders the single ContextMenu host for this instance.
export const menu: Menu = createMenu();

/// Svelte action: `contextmenu` -> preventDefault + show at the pointer;
/// Shift+F10 / the ContextMenu key -> show at the node's box. The node is
/// recorded as the opener so closing restores focus to it. Svelte calls
/// actions with `(node, param)`, so the item closure is the action's
/// parameter, not a curried factory argument.
export function menuFor(node: HTMLElement, items: () => MenuItem[]) {
  function onContextMenu(event: MouseEvent) {
    event.preventDefault();
    menu.show(event.clientX, event.clientY, items(), node);
  }
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "ContextMenu" && !(event.key === "F10" && event.shiftKey)) return;
    event.preventDefault();
    const rect = node.getBoundingClientRect();
    menu.show(rect.left, rect.bottom || rect.top, items(), node);
  }
  node.addEventListener("contextmenu", onContextMenu);
  node.addEventListener("keydown", onKeydown);
  return {
    destroy() {
      node.removeEventListener("contextmenu", onContextMenu);
      node.removeEventListener("keydown", onKeydown);
    },
  };
}
