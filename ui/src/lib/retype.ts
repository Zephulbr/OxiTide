/// The language-switch flourish: every visible piece of text the switch
/// changed is erased and typed again in the new language (owner request,
/// v1.1 smoke). It works on the text nodes Svelte already rendered: a
/// snapshot before the switch, a diff after it, then one animation frame
/// loop that writes intermediate strings and ends on the node's real text.
/// A node something else rewrites mid-animation (a clock, a live region) is
/// dropped and keeps that text, so the UI never ends on a stale string.

/// Erase and type each take this long, whatever the length: long strings
/// move faster, so everything lands together.
export const ERASE_MS = 220;
export const TYPE_MS = 340;
/// A ceiling for very busy screens; the rest switch instantly.
export const MAX_NODES = 600;

export interface Snapshot {
  readonly texts: ReadonlyMap<Text, string>;
}

/// Containers whose text is not animated: live regions (a screen reader
/// would read every frame), form fields, code, and anything opted out.
const SKIP = "[aria-live],[role=status],[role=alert],script,style,textarea,input,select,code,pre,[data-no-retype]";

function onScreen(node: Text, view: { width: number; height: number }): boolean {
  const parent = node.parentElement;
  if (!parent || parent.closest(SKIP)) return false;
  const rect = parent.getBoundingClientRect();
  if (rect.width === 0 && rect.height === 0) return false;
  return rect.bottom > 0 && rect.right > 0 && rect.top < view.height && rect.left < view.width;
}

/// Records the visible text before a switch.
export function snapshotText(root: Node = document.body, view = viewport()): Snapshot {
  const texts = new Map<Text, string>();
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node && texts.size < MAX_NODES; node = walker.nextNode()) {
    const text = node as Text;
    const value = text.nodeValue ?? "";
    if (value.trim() === "") continue;
    if (onScreen(text, view)) texts.set(text, value);
  }
  return { texts };
}

function viewport() {
  return { width: window.innerWidth, height: window.innerHeight };
}

/// Splits into user-perceived characters, so a CJK ideograph, an accent or
/// an emoji is erased in one step.
export function graphemes(text: string): string[] {
  const Segmenter = (Intl as unknown as { Segmenter?: SegmenterCtor }).Segmenter;
  if (Segmenter) {
    const segmenter = new Segmenter(undefined, { granularity: "grapheme" });
    return Array.from(segmenter.segment(text), (s) => s.segment);
  }
  return Array.from(text);
}

/// `Intl.Segmenter` (ES2022 Intl; WebView2 has it, the TS lib here does not).
type SegmenterCtor = new (
  locale: undefined,
  options: { granularity: "grapheme" },
) => { segment(text: string): Iterable<{ segment: string }> };

/// The string shown `elapsed` ms into the animation: `from` shrinking to
/// its common prefix with `to`, then `to` growing from that prefix.
export function frameText(from: string[], to: string[], elapsed: number): string {
  let common = 0;
  while (common < from.length && common < to.length && from[common] === to[common]) common += 1;
  if (elapsed < ERASE_MS) {
    const keep = Math.round(from.length - (from.length - common) * (elapsed / ERASE_MS));
    return from.slice(0, Math.max(common, keep)).join("");
  }
  const t = Math.min(1, (elapsed - ERASE_MS) / TYPE_MS);
  return to.slice(0, common + Math.round((to.length - common) * t)).join("");
}

interface Job {
  node: Text;
  from: string[];
  to: string[];
  target: string;
  written: string;
}

let running: { cancel: () => void } | null = null;

/// Animates every snapshotted node whose text changed. Returns when done.
export function retype(snapshot: Snapshot): Promise<void> {
  running?.cancel();
  const jobs: Job[] = [];
  for (const [node, before] of snapshot.texts) {
    const after = node.nodeValue ?? "";
    if (!node.isConnected || after === before) continue;
    // Put the old text back now, in the same task as the re-render, so the
    // new text never paints before its erase.
    node.nodeValue = before;
    jobs.push({ node, from: graphemes(before), to: graphemes(after), target: after, written: before });
  }
  if (jobs.length === 0) return Promise.resolve();

  return new Promise((resolve) => {
    let frame = 0;
    const start = performance.now();
    const finish = () => {
      cancelAnimationFrame(frame);
      // Land every node still ours on its real text.
      for (const job of jobs) if (job.node.nodeValue === job.written) job.node.nodeValue = job.target;
      running = null;
      resolve();
    };
    running = { cancel: finish };
    const step = (now: number) => {
      const elapsed = now - start;
      for (let i = jobs.length - 1; i >= 0; i -= 1) {
        const job = jobs[i];
        // Rewritten by its owner (a timer, a new track): keep that text.
        if (job.node.nodeValue !== job.written) {
          jobs.splice(i, 1);
          continue;
        }
        const text = frameText(job.from, job.to, elapsed);
        if (text !== job.written) {
          job.node.nodeValue = text;
          job.written = text;
        }
      }
      if (elapsed >= ERASE_MS + TYPE_MS || jobs.length === 0) finish();
      else frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
  });
}
