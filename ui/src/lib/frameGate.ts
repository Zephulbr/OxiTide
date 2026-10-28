// WebKitGTK (Linux) presents every page frame through the app's GTK main
// thread. One small spinning now-playing mark held ~55% of a core at 60 fps
// and ~25% at 30 (`plans/notes/2026-10-28-linux-home-lag.md`), so the loops
// that run for as long as playback does draw at 30 fps there.

/// 4 ms of slack: WebKit rounds rAF times to whole ms, so two 60 Hz frames
/// can measure 32 ms, and a tighter step waited for a third (~21 fps).
const LINUX_STEP_MS = 1000 / 30 - 4;

const linux =
  typeof document !== "undefined" && document.documentElement.dataset.platform === "linux";

let gateAt = -Infinity;
let seenAt = NaN;
let open = false;

/// Whether an always-running rAF loop draws in the frame at `t` (its rAF
/// time), given when it last drew and its own shortest step `ms`. A loop
/// skips the frames this refuses; a skipped frame writes nothing, so it costs
/// nothing to present.
///
/// On Linux one shared gate opens every ~33 ms for every loop at once: rAF
/// callbacks of one frame share `t`, so the first to ask decides for all.
/// Loops gated on their own clocks drifted onto alternate frames and
/// presented 60 fps between them.
export function frameDue(t: number, last: number | null = null, ms = 0): boolean {
  if (!linux) return last === null || t - last >= ms;
  if (t !== seenAt) {
    seenAt = t;
    open = t - gateAt >= LINUX_STEP_MS;
    if (open) gateAt = t;
  }
  return open;
}

/// Linux stand-in for a CSS `rotate` loop of one turn per `turnMs`: WebKitGTK
/// presents every frame of a CSS animation (a `steps()` timing too), so the
/// turn is drawn on `frameDue`'s frames instead. Off Linux it does nothing and
/// the CSS animation runs; the host's CSS drops that animation under
/// `:root[data-platform="linux"]`. Stopping returns to 0°, as removing the
/// animation does.
export function gatedSpin(node: HTMLElement, options: { spinning: boolean; turnMs: number }) {
  if (!linux) return {};
  let opts = options;
  let raf = 0;
  let angle = 0;
  let last: number | null = null;
  const tick = (t: number) => {
    raf = requestAnimationFrame(tick);
    if (!frameDue(t)) return;
    if (last !== null) angle = (angle + ((t - last) / opts.turnMs) * 360) % 360;
    last = t;
    node.style.transform = `rotate(${angle}deg)`;
  };
  const apply = () => {
    cancelAnimationFrame(raf);
    raf = 0;
    last = null;
    if (opts.spinning) {
      raf = requestAnimationFrame(tick);
    } else {
      angle = 0;
      node.style.transform = "";
    }
  };
  apply();
  return {
    update(next: { spinning: boolean; turnMs: number }) {
      if (next.spinning === opts.spinning && next.turnMs === opts.turnMs) return;
      opts = next;
      apply();
    },
    destroy() {
      cancelAnimationFrame(raf);
    },
  };
}
