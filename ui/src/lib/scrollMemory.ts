/// Scroll restoration for the routed page.
///
/// Per history entry: leaving an entry remembers the scroller's offset; going
/// back to it restores the offset once the page has grown tall enough (pages
/// load their data after they mount), and a fresh entry starts at the top. The
/// restore gives up when the user scrolls, clicks or types first, or after a
/// few seconds, so a late layout never yanks the view.
///
/// Covering (Now Playing over the page): the page is `display: none` while
/// covered, which drops every scroll offset inside it, so `snapshot` records
/// the scroller's and the page's own scrollers' offsets beforehand and
/// `apply` puts them back.

const GIVE_UP_MS = 4000;
const USER_INPUT = ["wheel", "pointerdown", "keydown", "touchstart"] as const;

export type ScrollSnapshot = Array<[HTMLElement, number, number]>;

export function snapshotScroll(scroller: HTMLElement | null, page: HTMLElement | null): ScrollSnapshot {
  const out: ScrollSnapshot = [];
  const take = (el: HTMLElement) => {
    if (el.scrollTop !== 0 || el.scrollLeft !== 0) out.push([el, el.scrollTop, el.scrollLeft]);
  };
  if (scroller) take(scroller);
  if (page) for (const el of page.querySelectorAll<HTMLElement>("*")) take(el);
  return out;
}

export function applySnapshot(snapshot: ScrollSnapshot) {
  for (const [el, top, left] of snapshot) {
    if (!el.isConnected) continue;
    el.scrollTop = top;
    el.scrollLeft = left;
  }
}

export interface ScrollMemory {
  save(entry: number, top: number): void;
  restore(entry: number, scroller: HTMLElement, page: HTMLElement): void;
  cancel(): void;
}

export function createScrollMemory(): ScrollMemory {
  const saved = new Map<number, number>();
  let stop: (() => void) | null = null;

  function cancel() {
    stop?.();
    stop = null;
  }

  return {
    save(entry, top) {
      saved.set(entry, top);
    },
    restore(entry, scroller, page) {
      cancel();
      const target = saved.get(entry) ?? 0;
      scroller.scrollTop = 0;
      if (target <= 0) return;

      const reached = () => scroller.scrollHeight - scroller.clientHeight >= target - 1;
      const apply = () => {
        if (!reached()) return;
        scroller.scrollTop = target;
        cancel();
      };
      if (typeof ResizeObserver === "undefined") {
        apply();
        return;
      }
      const observer = new ResizeObserver(apply);
      const timer = setTimeout(cancel, GIVE_UP_MS);
      for (const type of USER_INPUT) scroller.addEventListener(type, cancel, { passive: true });
      stop = () => {
        observer.disconnect();
        clearTimeout(timer);
        for (const type of USER_INPUT) scroller.removeEventListener(type, cancel);
      };
      observer.observe(page);
      apply();
    },
    cancel,
  };
}
