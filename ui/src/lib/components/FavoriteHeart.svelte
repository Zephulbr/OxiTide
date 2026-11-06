<script lang="ts" module>
  /// A favorite toggle the user just made: `n` counts toggles (a new value
  /// replays the animation), `kind` is which way it went.
  export interface HeartFx {
    n: number;
    kind: "pop" | "break";
  }

  /// The fx for a toggle the user just made from `wasOn`.
  export function toggledFx(previous: HeartFx | null, wasOn: boolean): HeartFx {
    return { n: (previous?.n ?? 0) + 1, kind: wasOn ? "break" : "pop" };
  }
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import Icon from "./Icon.svelte";

  /// The favorite heart's glyph and motion, shared by every favorite toggle
  /// (the player bar, track rows, album and artist headers, the mini player,
  /// Rack's deck key and the phone remote). The host button owns the click, the label and the colour
  /// (`.on` fills it). Hovering the host button beats the heart twice.
  /// Favoriting pops it. Unfavoriting cracks it: the filled heart swells,
  /// splits along a jagged line, and the halves tip apart and fall while the
  /// outline fades back in. Only a toggle (`fx`) animates: a mount, a remount
  /// (a virtualized row) or a new track never does.
  let { size = 18, fx = null }: { size?: number; fx?: HeartFx | null } = $props();

  const mountedAt = untrack(() => fx?.n);
  const kind = $derived(fx && fx.n !== mountedAt ? fx.kind : null);
</script>

<span class="fav-heart">
  {#key fx?.n}
    <span class="heart-ico" class:pop={kind === "pop"} class:mend={kind === "break"}><Icon name="heart" {size} /></span>
  {/key}
  {#if kind === "break"}
    {#key fx?.n}
      <span class="heart-break" aria-hidden="true">
        <span class="half left"><Icon name="heart" {size} /></span>
        <span class="half right"><Icon name="heart" {size} /></span>
      </span>
    {/key}
  {/if}
</span>

<style>
  .fav-heart {
    position: relative;
    display: inline-grid;
    place-items: center;
  }
  .heart-ico {
    display: inline-grid;
    place-items: center;
  }
  :global(button:hover) > .fav-heart > .heart-ico :global(svg) {
    animation: heartbeat 640ms var(--ease-in-out);
  }
  @keyframes heartbeat {
    0%,
    100% {
      transform: scale(1);
    }
    22% {
      transform: scale(1.18);
    }
    40% {
      transform: scale(1.02);
    }
    60% {
      transform: scale(1.12);
    }
  }
  .heart-ico.pop {
    animation: heart-pop var(--dur-state) var(--ease-enter);
  }
  @keyframes heart-pop {
    0% {
      transform: scale(1);
    }
    40% {
      transform: scale(1.35);
    }
    100% {
      transform: scale(1);
    }
  }
  .heart-ico.mend {
    animation: heart-mend 700ms var(--ease-in-out) both;
  }
  @keyframes heart-mend {
    0%,
    55% {
      opacity: 0;
    }
    100% {
      opacity: 1;
    }
  }
  .heart-break {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    animation: heart-crack 700ms var(--ease-enter) both;
  }
  @keyframes heart-crack {
    0% {
      transform: scale(1);
    }
    15% {
      transform: scale(1.12);
    }
    30%,
    100% {
      transform: scale(1);
    }
  }
  .half {
    grid-area: 1 / 1;
    display: inline-grid;
    place-items: center;
    color: var(--accent);
    transform-origin: 50% 85%;
  }
  .half :global(svg) {
    fill: currentColor;
  }
  .half.left {
    clip-path: polygon(0 0, 52% 0, 44% 28%, 56% 46%, 42% 64%, 50% 100%, 0 100%);
    animation: heart-half-left 700ms ease-in both;
  }
  .half.right {
    clip-path: polygon(52% 0, 100% 0, 100% 100%, 50% 100%, 42% 64%, 56% 46%, 44% 28%);
    animation: heart-half-right 700ms ease-in both;
  }
  @keyframes heart-half-left {
    0%,
    25% {
      transform: none;
      opacity: 1;
    }
    100% {
      transform: translate(-5px, 9px) rotate(-28deg);
      opacity: 0;
    }
  }
  @keyframes heart-half-right {
    0%,
    25% {
      transform: none;
      opacity: 1;
    }
    100% {
      transform: translate(5px, 9px) rotate(28deg);
      opacity: 0;
    }
  }
  /* Reduced motion: no movement; the broken heart just fades and the
     outline is there at once. */
  @media (prefers-reduced-motion: reduce) {
    :global(button:hover) > .fav-heart > .heart-ico :global(svg),
    .heart-break,
    .heart-ico.mend {
      animation: none;
    }
    .half.left,
    .half.right {
      animation: heart-fade var(--dur-state) ease-out both;
    }
  }
  @keyframes heart-fade {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }
</style>
