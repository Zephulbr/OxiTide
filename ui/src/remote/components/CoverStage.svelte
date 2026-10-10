<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { art } from "../convert";
  import { stepVolume, SWIPE_THRESHOLD_PX, volumeFromDrag } from "../nowPlaying";

  /// Now Playing artwork. Covers crossfade on track change and settle back
  /// slightly while paused. Swiping up/down on it sets the PC's volume (phone
  /// hardware keys cannot reach a web page); it is also a keyboard slider.
  let {
    cover,
    playing,
    volume,
    onvolume,
  }: {
    cover: string | null;
    playing: boolean;
    volume: number;
    /// `final` marks the end of a gesture (commit) vs. a live update.
    onvolume: (volume: number, final: boolean) => void;
  } = $props();

  let layers = $state<[string, string]>(["", ""]);
  let active = $state<0 | 1>(0);
  let token = 0;

  $effect(() => {
    const url = art(cover, 640);
    if (url === layers[active]) return;
    const next = active === 0 ? 1 : 0;
    const mine = ++token;
    const apply = () => {
      if (mine !== token) return;
      layers[next] = url;
      active = next;
    };
    if (!url) {
      apply();
      return;
    }
    const img = new Image();
    img.onload = apply;
    img.onerror = apply;
    img.src = url;
    return () => {
      img.onload = null;
      img.onerror = null;
    };
  });

  let stage = $state<HTMLDivElement | null>(null);
  let drag: { id: number; y: number; start: number; swiping: boolean } | null = null;
  let level = $state<number | null>(null);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  const shown = $derived(level ?? volume);
  const percent = $derived(Math.round(shown * 100));

  function show(value: number) {
    level = value;
    if (hideTimer) clearTimeout(hideTimer);
  }

  function hideSoon() {
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (level = null), 900);
  }

  function down(event: PointerEvent) {
    if (!event.isPrimary || drag) return;
    drag = { id: event.pointerId, y: event.clientY, start: volume, swiping: false };
  }

  function move(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.id || !stage) return;
    const delta = event.clientY - drag.y;
    if (!drag.swiping) {
      if (Math.abs(delta) < SWIPE_THRESHOLD_PX) return;
      drag.swiping = true;
      stage.setPointerCapture(event.pointerId);
    }
    const value = volumeFromDrag(drag.start, delta, stage.clientHeight);
    show(value);
    onvolume(value, false);
  }

  function up(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.id) return;
    const swiping = drag.swiping;
    drag = null;
    if (swiping && level !== null) {
      onvolume(level, true);
      hideSoon();
    }
  }

  function key(event: KeyboardEvent) {
    const direction =
      event.key === "ArrowUp" || event.key === "ArrowRight"
        ? 1
        : event.key === "ArrowDown" || event.key === "ArrowLeft"
          ? -1
          : 0;
    if (!direction) return;
    event.preventDefault();
    const value = stepVolume(level ?? volume, direction);
    show(value);
    onvolume(value, true);
    hideSoon();
  }
</script>

<div
  class="stage"
  class:paused={!playing}
  bind:this={stage}
  role="slider"
  tabindex="0"
  aria-label="Volume — swipe up or down on the artwork"
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={percent}
  aria-valuetext={`${percent}%`}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
>
  <div class="frame">
    {#each [0, 1] as i (i)}
      {#if layers[i]}
        <img class:active={active === i} src={layers[i]} alt="" draggable="false" />
      {/if}
    {/each}
    {#if !layers[0] && !layers[1]}
      <span class="empty"><Icon name="music" size={48} /></span>
    {/if}
  </div>
  {#if level !== null}
    <div class="level" aria-hidden="true">
      <Icon name={level === 0 ? "volume-x" : level < 0.5 ? "volume-1" : "volume-2"} size={22} />
      <span class="bar"><span class="fill" style:transform={`scaleY(${level})`}></span></span>
      <strong>{percent}%</strong>
    </div>
  {/if}
</div>

<style>
  .stage {
    position: relative;
    width: min(100%, 52vh);
    aspect-ratio: 1;
    margin: 0 auto;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
    border-radius: var(--radius-card);
  }
  .frame {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--surface-2);
    box-shadow:
      0 24px 48px color-mix(in srgb, var(--accent) 18%, rgba(0, 0, 0, 0.35)),
      0 2px 6px rgba(0, 0, 0, 0.25);
    transform: scale(1);
    transition:
      transform var(--dur-view) var(--ease-in),
      box-shadow var(--dur-mood) linear;
  }
  .paused .frame {
    transform: scale(0.9);
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity var(--dur-view) linear;
  }
  img.active {
    opacity: 1;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  .level {
    position: absolute;
    left: 50%;
    top: 50%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 16px 18px;
    border-radius: var(--radius-sheet);
    background: var(--surface-float);
    backdrop-filter: blur(var(--overlay-blur));
    -webkit-backdrop-filter: blur(var(--overlay-blur));
    transform: translate(-50%, -50%);
    animation: pop var(--dur-state) var(--ease-in);
  }
  .bar {
    position: relative;
    width: 8px;
    height: 120px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }
  .fill {
    position: absolute;
    inset: 0;
    border-radius: 999px;
    background: var(--accent);
    transform-origin: bottom;
  }
  strong {
    font-variant-numeric: tabular-nums;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translate(-50%, -50%) scale(0.92);
    }
  }
</style>
