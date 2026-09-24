export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  danger?: boolean;
  run(): void;
}

export interface MenuState {
  x: number;
  y: number;
  items: MenuItem[];
}

export interface Menu {
  readonly open: MenuState | null;
  show(x: number, y: number, items: MenuItem[]): void;
  close(): void;
}

export function createMenu(): Menu {
  let open = $state<MenuState | null>(null);
  return {
    get open() {
      return open;
    },
    show(x, y, items) {
      open = { x, y, items };
    },
    close() {
      open = null;
    },
  };
}

/// One app-wide menu: rows across every view attach through `menuFor`, and
/// App renders the single ContextMenu host for this instance.
export const menu: Menu = createMenu();

/// Svelte action: `contextmenu` -> preventDefault + show at the pointer.
export function menuFor(items: () => MenuItem[]) {
  return (node: HTMLElement) => {
    function onContextMenu(event: MouseEvent) {
      event.preventDefault();
      menu.show(event.clientX, event.clientY, items());
    }
    node.addEventListener("contextmenu", onContextMenu);
    return {
      destroy() {
        node.removeEventListener("contextmenu", onContextMenu);
      },
    };
  };
}
