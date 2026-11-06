/// The list edge fade (app.css `.edge-fade`): a scroller's rows fade out at
/// its top once scrolled and at its bottom while there is more below, instead
/// of being cut off or passing under a frosted header. The mask sits on the
/// scroller itself, so it stays put while the compositor scrolls the rows
/// under it (it never trails a fast scroll). Only the fade lengths come from
/// script: each grows in with the distance left to scroll that way, so a list
/// at rest or at its end shows every row whole, and they change in the first
/// and last EDGE_FADE px of travel only (writes outside that are skipped).

export const EDGE_FADE = 48;

/// The fade lengths, in px, for a scroller's position.
export function fadeLengths(scrollTop: number, scrollHeight: number, clientHeight: number): [number, number] {
  const max = Math.max(0, scrollHeight - clientHeight);
  const top = Math.min(Math.max(0, scrollTop), EDGE_FADE);
  const bottom = Math.min(Math.max(0, max - scrollTop), EDGE_FADE);
  return [Math.round(top), Math.round(bottom)];
}

export interface EdgeFadeOptions {
  /// Anything that changes the content's height (a row count): a new value
  /// re-measures. Virtualized lists use this.
  content?: unknown;
  /// Content made of changing direct children (sections that load in, a
  /// tree that expands): watch each child's size, and children coming and
  /// going. Not for virtualized lists, whose rows churn while scrolling.
  watchChildren?: boolean;
}

/// Svelte action: keeps `--fade-top` / `--fade-bottom` on the node. Resizes
/// of the node (and of its first child, or every child with
/// `watchChildren`) re-measure too.
export function edgeFade(node: HTMLElement, options: EdgeFadeOptions = {}) {
  const watchChildren = options.watchChildren ?? false;
  let top = -1;
  let bottom = -1;
  const update = () => {
    const [nextTop, nextBottom] = fadeLengths(node.scrollTop, node.scrollHeight, node.clientHeight);
    if (nextTop !== top) node.style.setProperty("--fade-top", `${(top = nextTop)}px`);
    if (nextBottom !== bottom) node.style.setProperty("--fade-bottom", `${(bottom = nextBottom)}px`);
  };
  const resize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
  const observe = () => {
    if (!resize) return;
    resize.disconnect();
    resize.observe(node);
    const children = watchChildren ? Array.from(node.children) : [node.firstElementChild];
    for (const child of children) if (child) resize.observe(child);
  };
  const mutations =
    watchChildren && typeof MutationObserver !== "undefined"
      ? new MutationObserver(() => {
          observe();
          update();
        })
      : null;
  node.addEventListener("scroll", update, { passive: true });
  mutations?.observe(node, { childList: true });
  observe();
  update();
  return {
    // Content changed: measure once the DOM has it.
    update: () => queueMicrotask(update),
    destroy() {
      node.removeEventListener("scroll", update);
      resize?.disconnect();
      mutations?.disconnect();
    },
  };
}
