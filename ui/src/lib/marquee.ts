/// Hover marquee for clipped single-line text (the player bar's title, artist
/// and "From …" lines). Only while the pointer rests on the host (AR-2:
/// nothing idles), never under reduced motion. At rest each line keeps its
/// ellipsis; on hover a line that overflows slides slowly to its end, holds,
/// slides back, holds, and repeats; on leave it eases home.

/// Travel speed, slow enough to read along.
export const MARQUEE_SPEED_PX_S = 30;
/// Pause at each end.
export const MARQUEE_HOLD_MS = 1200;
/// Lines that overflow by less than this keep their ellipsis.
const MIN_OVERFLOW_PX = 3;

/// One loop: hold at the start, travel out, hold at the end, travel back.
/// Offsets are the keyframe positions of those four phases.
export function marqueeTiming(overflowPx: number) {
  const travel = (overflowPx / MARQUEE_SPEED_PX_S) * 1000;
  const duration = 2 * (MARQUEE_HOLD_MS + travel);
  return {
    duration,
    offsets: [
      MARQUEE_HOLD_MS / duration,
      (MARQUEE_HOLD_MS + travel) / duration,
      (2 * MARQUEE_HOLD_MS + travel) / duration,
    ] as const,
  };
}

/// Svelte action on the hover host. Lines are its `[data-marquee]`
/// descendants (the clipping box: overflow hidden, nowrap, ellipsis) whose
/// first element child is the text that moves.
export function marquee(host: HTMLElement) {
  const reduce = () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;

  function lines() {
    return [...host.querySelectorAll<HTMLElement>("[data-marquee]")]
      .map((box) => ({ box, text: box.firstElementChild as HTMLElement | null }))
      .filter((line): line is { box: HTMLElement; text: HTMLElement } => line.text !== null);
  }

  function reset(box: HTMLElement, text: HTMLElement) {
    box.style.textOverflow = "";
    text.style.display = "";
  }

  function start() {
    if (reduce()) return;
    for (const { box, text } of lines()) {
      if (typeof text.animate !== "function") continue;
      for (const running of text.getAnimations()) running.cancel();
      const overflow = box.scrollWidth - box.clientWidth;
      if (overflow < MIN_OVERFLOW_PX) {
        reset(box, text);
        continue;
      }
      // Transforms need a box: the text is inline (so the ellipsis works) at
      // rest and an inline-block, unclipped by the ellipsis, while it moves.
      box.style.textOverflow = "clip";
      text.style.display = "inline-block";
      const { duration, offsets } = marqueeTiming(overflow);
      const end = `translateX(${-overflow}px)`;
      text.animate(
        [
          { transform: "translateX(0)", offset: 0 },
          { transform: "translateX(0)", offset: offsets[0], easing: "ease-in-out" },
          { transform: end, offset: offsets[1] },
          { transform: end, offset: offsets[2], easing: "ease-in-out" },
          { transform: "translateX(0)", offset: 1 },
        ],
        { duration, iterations: Infinity },
      );
    }
  }

  function stop() {
    for (const { box, text } of lines()) {
      if (typeof text.animate !== "function") continue;
      const running = text.getAnimations();
      if (running.length === 0) continue;
      const from = getComputedStyle(text).transform;
      for (const animation of running) animation.cancel();
      const home = text.animate([{ transform: from === "none" ? "translateX(0)" : from }, { transform: "translateX(0)" }], {
        duration: 350,
        easing: "ease-out",
      });
      home.onfinish = () => reset(box, text);
    }
  }

  host.addEventListener("pointerenter", start);
  host.addEventListener("pointerleave", stop);
  return {
    destroy() {
      host.removeEventListener("pointerenter", start);
      host.removeEventListener("pointerleave", stop);
    },
  };
}
