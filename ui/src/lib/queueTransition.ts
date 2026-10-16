import type { TransitionConfig } from "svelte/transition";

/// The rail and the page content share one duration and one curve, so the
/// content leaves exactly as the rail arrives. The curve is `--ease-in-out`
/// (Material standard): a gentle start and a long settle, calmer than
/// cubic-in-out's fast middle.
export const GLIDE_MS = 380;
const GLIDE_CSS = "cubic-bezier(0.4, 0, 0.2, 1)";
const glideEasing = cubicBezier(0.4, 0, 0.2, 1);

/// Docked queue: the column grows from zero width so the main view eases
/// aside rather than snapping. The rail inside keeps its full width (app.css)
/// and is clipped, so its rows never reflow mid-glide.
export function queueDock(node: HTMLElement, { reduced }: { reduced: boolean }): TransitionConfig {
  if (reduced) return { duration: 120, css: (t) => `opacity: ${t}` };
  const width = node.getBoundingClientRect().width;
  return {
    duration: GLIDE_MS,
    easing: glideEasing,
    css: (t) => `width: ${t * width}px; overflow: hidden; opacity: ${Math.min(1, t * 1.5)}`,
  };
}

/// Floating queue (narrow windows): slides in over the content from the right.
export function queueFloat(_node: HTMLElement, { reduced }: { reduced: boolean }): TransitionConfig {
  if (reduced) return { duration: 120, css: (t) => `opacity: ${t}` };
  return {
    duration: GLIDE_MS,
    easing: glideEasing,
    css: (t) => `transform: translateX(${(1 - t) * 100}%)`,
  };
}

// ---- Page hold + glide -----------------------------------------------------
//
// While the docked rail glides, the main column would change width every
// frame and every grid on the page would re-flow per frame (auto-fill columns
// jump, covers resize): jitter. Instead the routed page is held at its final
// width for the glide, so it lays out once, and that one change is animated
// FLIP-style: each on-screen element starts at its old box and travels to its
// new one with the rail's own duration and curve. Cards and panels (anything
// with a visible box) also animate their width, so their edges glide instead
// of snapping; their insides follow by normal layout, which is continuous
// because the grids around them no longer change. Elements that the new width
// hides or clips (a card that no longer fits, a carousel card past the row's
// new edge) leave an inert copy that fades out; ones it reveals fade in.
//
// Timing: Svelte starts a transition's real animation one frame after the
// element enters (a zero-length placeholder runs first) and fires
// `introstart`/`outrostart` right before it. Closing is set up in
// `outrostart`. Opening must measure before the rail is in the DOM (by its
// intro the page has already laid out narrow), so App snapshots in a
// pre-update effect, applies the hold right after the update with the glide
// created *paused* at its first frame, and `startPendingGlide` (the rail's
// `introstart`) plays it, in step with the rail.

/// What the glide holds: Now Playing when it covers the routed page (it comes
/// first in .main), else the visible routed page.
const PAGE = ":scope > .now-playing, :scope > .page:not(.covered)";

interface Hold {
  animations: Animation[];
  release: () => void;
  fallback: ReturnType<typeof setTimeout> | null;
}

let hold: Hold | null = null;

export function releasePageHold() {
  if (!hold) return;
  if (hold.fallback) clearTimeout(hold.fallback);
  for (const animation of hold.animations) animation.cancel();
  hold.release();
  hold = null;
}

/// Closing: the rail is still laid out when its outro starts, so measure now.
/// `railTarget` is the rail's width at the end of the glide.
export function holdPage(
  main: HTMLElement | null,
  rail: HTMLElement,
  railTarget: number,
  { glide = true }: { glide?: boolean } = {},
) {
  releasePageHold();
  const page = main?.querySelector<HTMLElement>(PAGE);
  if (!main || !page) return;
  // From the rail's *current* width, not an assumed one: the event may fire
  // with the rail anywhere between zero and full width.
  const railNow = rail.getBoundingClientRect().width;
  const width = Math.max(0, page.getBoundingClientRect().width + railNow - railTarget);
  const movers = glide ? flipCandidates(main, page) : [];
  applyHold(main, page, width, movers, movers.map((el) => el.getBoundingClientRect()), false);
}

let snapshot: { page: HTMLElement; width: number; movers: HTMLElement[]; rects: DOMRect[] } | null =
  null;

/// Opening, step 1 (before the rail is inserted): remember the page.
export function prepareOpenGlide(main: HTMLElement | null, { glide = true }: { glide?: boolean } = {}) {
  snapshot = null;
  const page = main?.querySelector<HTMLElement>(PAGE);
  if (!main || !page) return;
  const movers = glide ? flipCandidates(main, page) : [];
  snapshot = {
    page,
    width: page.getBoundingClientRect().width,
    movers,
    rects: movers.map((el) => el.getBoundingClientRect()),
  };
}

/// Opening, step 2 (right after the rail is inserted, before paint): hold the
/// page at its final width with the glide parked on its first frame.
export function commitOpenGlide(main: HTMLElement | null, railTarget: number) {
  const taken = snapshot;
  snapshot = null;
  if (!main || !taken || !taken.page.isConnected) return;
  releasePageHold();
  applyHold(main, taken.page, Math.max(0, taken.width - railTarget), taken.movers, taken.rects, true);
}

/// Opening, step 3 (the rail's `introstart`): run the parked glide.
export function startPendingGlide() {
  if (!hold) return;
  if (hold.fallback) {
    clearTimeout(hold.fallback);
    hold.fallback = null;
  }
  for (const animation of hold.animations) {
    if (animation.playState === "paused") animation.play();
  }
}

/// Synchronous re-placement hooks for anything a page positions from its own
/// measurements (e.g. the Settings tab pill). A ResizeObserver reacts a frame
/// after the glide measures the final layout, so such elements would glide to
/// a stale spot and then jump; a hook runs right after the page takes its
/// final width and before that measurement. Hooks must write the DOM directly:
/// Svelte state is not flushed in between. Returns the unregister function.
const pageResizeHooks = new Set<() => void>();

export function onPageResize(hook: () => void): () => void {
  pageResizeHooks.add(hook);
  return () => pageResizeHooks.delete(hook);
}

function applyHold(
  main: HTMLElement,
  page: HTMLElement,
  width: number,
  movers: HTMLElement[],
  before: DOMRect[],
  parked: boolean,
) {
  page.style.width = `${width}px`;
  main.style.overflowX = "hidden";
  main.scrollLeft = 0;
  for (const hook of pageResizeHooks) hook();

  const ghosts: HTMLElement[] = [];
  const animations =
    movers.length > 0
      ? flip(movers, before, movers.map((el) => el.getBoundingClientRect()), main, page, ghosts)
      : [];
  if (parked) for (const animation of animations) animation.pause();

  hold = {
    animations,
    release: () => {
      for (const ghost of ghosts) ghost.remove();
      page.style.width = "";
      main.style.overflowX = "";
      main.scrollLeft = 0;
    },
    // Never leave content parked if the rail's start event goes missing.
    fallback: parked ? setTimeout(startPendingGlide, 120) : null,
  };
}

const MAX_MOVERS = 2000;

/// Every element is a candidate, so anything that moves, resizes, appears or
/// hides is caught whatever its tag or class. Most cost nothing: a nested
/// element animates only its own share of its ancestor's motion (usually
/// none), and anything inside a resizing card or a hiding element is skipped.
/// SVG internals ride along with their `<svg>`.
const MOVER_SELECTOR = "*:not(svg *)";

/// Movers on or near the screen, capped so a huge page never stalls a frame.
function flipCandidates(main: HTMLElement, page: HTMLElement): HTMLElement[] {
  const view = main.getBoundingClientRect();
  const out: HTMLElement[] = [];
  for (const el of page.querySelectorAll<HTMLElement>(MOVER_SELECTOR)) {
    const rect = el.getBoundingClientRect();
    if (rect.bottom < view.top - 200 || rect.top > view.bottom + 200) continue;
    out.push(el);
    if (out.length >= MAX_MOVERS) break;
  }
  return out;
}

/// Paint of the element's own box: fill, shadow or border.
function paintsOwnBox(style: CSSStyleDeclaration): boolean {
  const border =
    parseFloat(style.borderTopWidth) > 0 &&
    !/rgba\(0, 0, 0, 0\)|transparent/.test(style.borderTopColor);
  return (
    style.backgroundImage !== "none" ||
    style.boxShadow !== "none" ||
    border ||
    !/rgba\(0, 0, 0, 0\)|transparent/.test(style.backgroundColor)
  );
}

/// Boxes that must resize with the glide rather than snap: cards and panels
/// you can see the edges of, and clipping boxes whose edge is the clip.
///
/// A clipping box whose content fits its width (a vertical scroller such as
/// TrackTable's `.table-scroll`, or ArtistHero's `overflow: hidden` header
/// painted by absolute layers) resizes, and its contents follow by layout.
/// Left to snap, its in-flight children would overflow it: a horizontal
/// scrollbar appears and the right-hand columns are cut off mid-glide.
/// A box that already overflows sideways (a carousel row) keeps its snap: its
/// cards are handled by the clipped-card fade in `flip`.
function hasVisibleBox(el: HTMLElement): boolean {
  if (el.matches(".media")) return true;
  const style = getComputedStyle(el);
  if (paintsOwnBox(style)) return true;
  if (style.overflowX === "visible" && style.overflowY === "visible") return false;
  return el.scrollWidth <= el.clientWidth + 1;
}

/// A non-interactive, screen-reader-hidden copy of `el` placed over its old
/// box inside `.main`. `clip` (the old visible area of a scroller it sat in)
/// trims the copy to what was actually on screen.
function ghostOf(el: HTMLElement, rect: DOMRect, main: HTMLElement, clip?: DOMRect): HTMLElement {
  const box = main.getBoundingClientRect();
  const ghost = el.cloneNode(true) as HTMLElement;
  ghost.removeAttribute("id");
  ghost.setAttribute("aria-hidden", "true");
  ghost.inert = true;
  Object.assign(ghost.style, {
    position: "absolute",
    left: `${rect.left - box.left + main.scrollLeft}px`,
    top: `${rect.top - box.top + main.scrollTop}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
    margin: "0",
    pointerEvents: "none",
  });
  if (clip) {
    const top = Math.max(0, clip.top - rect.top);
    const right = Math.max(0, rect.right - clip.right);
    const bottom = Math.max(0, rect.bottom - clip.bottom);
    const left = Math.max(0, clip.left - rect.left);
    ghost.style.clipPath = `inset(${top}px ${right}px ${bottom}px ${left}px)`;
  }
  main.append(ghost);
  return ghost;
}

/// Share of `rect` inside `clip` (0..1).
function visibleShare(rect: DOMRect, clip: DOMRect): number {
  const area = rect.width * rect.height;
  if (area <= 0) return 0;
  const w = Math.max(0, Math.min(rect.right, clip.right) - Math.max(rect.left, clip.left));
  const h = Math.max(0, Math.min(rect.bottom, clip.bottom) - Math.max(rect.top, clip.top));
  return (w * h) / area;
}

/// Nearest ancestor (inside the page) that clips its content: carousels and
/// other scrollers. Memoised per element for one glide.
function clipAncestor(
  el: HTMLElement,
  page: HTMLElement,
  memo: Map<HTMLElement, boolean>,
): HTMLElement | null {
  for (let p = el.parentElement; p && p !== page; p = p.parentElement) {
    let clips = memo.get(p);
    if (clips === undefined) {
      const style = getComputedStyle(p);
      clips = style.overflowX !== "visible" || style.overflowY !== "visible";
      memo.set(p, clips);
    }
    if (clips) return p;
  }
  return null;
}

function flip(
  movers: HTMLElement[],
  before: DOMRect[],
  after: DOMRect[],
  main: HTMLElement,
  page: HTMLElement,
  ghosts: HTMLElement[],
): Animation[] {
  const raw = new Map<HTMLElement, { x: number; y: number }>();
  const resizing = new Set<HTMLElement>();
  const animations: Animation[] = [];
  const timing: KeyframeAnimationOptions = { duration: GLIDE_MS, easing: GLIDE_CSS };
  const index = new Map(movers.map((el, i) => [el, i]));
  const clipMemo = new Map<HTMLElement, boolean>();
  const fadeOut = (el: HTMLElement, rect: DOMRect, clip?: DOMRect) => {
    const ghost = ghostOf(el, rect, main, clip);
    ghosts.push(ghost);
    animations.push(
      ghost.animate([{ opacity: 1 }, { opacity: 0 }], {
        ...timing,
        duration: GLIDE_MS * 0.6,
        fill: "forwards",
      }),
    );
  };
  const fadeIn = (el: HTMLElement) => {
    animations.push(el.animate([{ opacity: 0 }, { opacity: 1 }], timing));
  };

  movers.forEach((el, i) => {
    // Inside a resizing card, layout already carries the element along.
    let parent = el.parentElement;
    let inherited: { x: number; y: number } | undefined;
    for (; parent; parent = parent.parentElement) {
      if (resizing.has(parent)) return;
      inherited ??= raw.get(parent);
    }

    // An element hidden on one side of the change (e.g. a Jump back in card
    // that only fits once the rail is gone) reports an empty box at 0,0; a FLIP
    // from there would fly it in from the corner. Appearing elements fade in
    // in place. Disappearing ones are already `display: none`, so an inert
    // copy is left at the old spot inside `.main` and fades out there (clipped
    // by `.main`, so the arriving rail still covers it).
    const hiddenBefore = before[i].width === 0 && before[i].height === 0;
    const hiddenAfter = after[i].width === 0 && after[i].height === 0;
    // Boxless on both sides (empty spans, `display: contents` wrappers): nothing
    // to show, but its children may still move, so don't claim them.
    if (hiddenBefore && hiddenAfter) return;
    if (hiddenBefore || hiddenAfter) {
      if (hiddenBefore) fadeIn(el);
      else fadeOut(el, before[i]);
      // Its contents come and go with it; no separate motion or fade.
      resizing.add(el);
      return;
    }

    // Clipped rather than hidden: a carousel narrows at once, so cards past its
    // new edge would be cut off instantly. Treat that like hiding — a card that
    // loses visible area leaves a copy (trimmed to what was on screen) that
    // fades out; a card that was fully clipped and comes into view fades in.
    const clipper = clipAncestor(el, page, clipMemo);
    const ci = clipper ? index.get(clipper) : undefined;
    if (ci !== undefined) {
      const shareBefore = visibleShare(before[i], before[ci]);
      const shareAfter = visibleShare(after[i], after[ci]);
      if (shareBefore > 0.01 && shareAfter < shareBefore - 0.01) {
        fadeOut(el, before[i], before[ci]);
        resizing.add(el);
        return;
      }
      if (shareBefore <= 0.01 && shareAfter > 0.01) {
        fadeIn(el);
        resizing.add(el);
        return;
      }
    }

    const dxRaw = before[i].left - after[i].left;
    const dyRaw = before[i].top - after[i].top;
    raw.set(el, { x: dxRaw, y: dyRaw });
    const x = dxRaw - (inherited?.x ?? 0);
    const y = dyRaw - (inherited?.y ?? 0);

    const resize = Math.abs(before[i].width - after[i].width) > 1 && hasVisibleBox(el);
    if (resize) {
      resizing.add(el);
      animations.push(
        el.animate(
          [{ width: `${before[i].width}px` }, { width: `${after[i].width}px` }],
          timing,
        ),
      );
    }
    if (Math.abs(x) >= 0.5 || Math.abs(y) >= 0.5) {
      animations.push(
        el.animate(
          [{ transform: `translate(${x}px, ${y}px)` }, { transform: "translate(0, 0)" }],
          // `add` composes with any transform the element already has.
          { ...timing, composite: "add" },
        ),
      );
    }
  });
  return animations;
}

/// CSS `cubic-bezier()` as an easing function, so the Svelte-driven rail and
/// the WAAPI-driven content follow the identical curve.
function cubicBezier(x1: number, y1: number, x2: number, y2: number): (t: number) => number {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const sampleX = (t: number) => ((ax * t + bx) * t + cx) * t;
  const sampleY = (t: number) => ((ay * t + by) * t + cy) * t;
  const slopeX = (t: number) => (3 * ax * t + 2 * bx) * t + cx;
  return (x: number) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let t = x;
    for (let i = 0; i < 8; i++) {
      const error = sampleX(t) - x;
      if (Math.abs(error) < 1e-5) return sampleY(t);
      const slope = slopeX(t);
      if (Math.abs(slope) < 1e-6) break;
      t -= error / slope;
    }
    // Bisection fallback for flat slopes.
    let lo = 0;
    let hi = 1;
    t = x;
    while (hi - lo > 1e-5) {
      if (sampleX(t) < x) lo = t;
      else hi = t;
      t = (lo + hi) / 2;
    }
    return sampleY(t);
  };
}
