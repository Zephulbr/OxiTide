import type { Snippet } from "svelte";
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
  /// Icon rendered before the label.
  icon?: IconName;
  /// A second, muted line under the label (e.g. what a quality tier means).
  detail?: string;
  /// Small icon rendered at the trailing edge, e.g. the sort direction arrow.
  trailing?: IconName;
  /// Draws a hairline divider above this row, grouping it apart from the
  /// rows before it (ignored on the first row and under a section heading).
  divider?: boolean;
  /// A status line, not an action: drawn at full text colour (not dimmed like
  /// a disabled row), never runs, and keyboard nav skips it.
  note?: boolean;
  /// A note's own body in place of label/detail (e.g. the quality menu's
  /// stream readout); `label` stays its plain-text name.
  content?: Snippet;
  run(): void;
}

export type MenuPlacement = "below" | "above";
export type MenuAlign = "start" | "end";

export interface MenuOptions {
  /// "above" grows the menu upward from `y` (openers at the bottom edge,
  /// e.g. the player bar); the default hangs it below `y`.
  placement?: MenuPlacement;
  /// "end" puts the menu's right edge at `x` (openers on the right); the
  /// default puts its left edge there.
  align?: MenuAlign;
}

export interface MenuState {
  x: number;
  y: number;
  items: MenuItem[];
  /// The element that opened the menu; closing restores focus to it.
  opener: HTMLElement | null;
  placement: MenuPlacement;
  align: MenuAlign;
}

export interface Menu {
  readonly open: MenuState | null;
  show(
    x: number,
    y: number,
    items: MenuItem[],
    opener?: HTMLElement | null,
    options?: MenuOptions,
  ): void;
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
    show(x, y, items, opener = null, { placement = "below", align = "start" } = {}) {
      open = { x, y, items, opener, placement, align };
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

/// Svelte action for a button that opens a menu (`open` shows it): pressing
/// the button while its menu is open closes the menu instead of reopening it.
/// The host closes an open menu on the window's mousedown, before the
/// button's click, so the button notes on pointerdown whether its own menu
/// was the one open. Keyboard clicks (`detail` 0) always open: focus is inside
/// an open menu, so they cannot be a second press.
export function menuButton(node: HTMLElement, open: (button: HTMLElement) => void) {
  let show = open;
  let wasOpen = false;
  function onPointerDown() {
    wasOpen = menu.open?.opener === node;
  }
  function onClick(event: MouseEvent) {
    const dismissed = wasOpen && event.detail > 0;
    wasOpen = false;
    if (!dismissed) show(node);
  }
  node.addEventListener("pointerdown", onPointerDown);
  node.addEventListener("click", onClick);
  return {
    update(next: (button: HTMLElement) => void) {
      show = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", onPointerDown);
      node.removeEventListener("click", onClick);
    },
  };
}
