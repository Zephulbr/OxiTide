import { cubicInOut } from "svelte/easing";
import type { TransitionConfig } from "svelte/transition";

/// Docked queue: the column grows from zero width so the main view eases
/// aside rather than snapping. The rail inside keeps its full width (app.css)
/// and is clipped, so its rows never reflow mid-glide.
export function queueDock(node: HTMLElement, { reduced }: { reduced: boolean }): TransitionConfig {
  if (reduced) return { duration: 120, css: (t) => `opacity: ${t}` };
  const width = node.getBoundingClientRect().width;
  return {
    duration: 260,
    easing: cubicInOut,
    css: (t) => `width: ${t * width}px; overflow: hidden; opacity: ${Math.min(1, t * 1.5)}`,
  };
}

/// Floating queue (narrow windows): slides in over the content from the right.
export function queueFloat(_node: HTMLElement, { reduced }: { reduced: boolean }): TransitionConfig {
  if (reduced) return { duration: 120, css: (t) => `opacity: ${t}` };
  return {
    duration: 260,
    easing: cubicInOut,
    css: (t) => `transform: translateX(${(1 - t) * 100}%)`,
  };
}
