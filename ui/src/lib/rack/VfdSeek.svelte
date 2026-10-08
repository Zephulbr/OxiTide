<script lang="ts">
  import type { Player } from "../player.svelte";
  import { seekByKey, seekRatio } from "../seek";
  import { clock } from "./deck";

  /// Segmented VFD seek bar (DESIGN §5.4): dim segments, the elapsed part
  /// glows. A real slider — drag, arrows ±5 s, Page keys ±10 %, Home/End.
  let { player }: { player: Player } = $props();

  let el: HTMLDivElement;
  let dragMs = $state<number | null>(null);
  const duration = $derived(player.state.duration_ms);
  const position = $derived(dragMs ?? player.pendingSeek ?? player.state.position_ms);
  const fraction = $derived(duration > 0 ? Math.min(1, position / duration) : 0);
  const disabled = $derived(!player.state.current || duration <= 0);

  function at(event: PointerEvent): number {
    const box = el.getBoundingClientRect();
    return seekRatio(event.clientX, box.left, box.width) * duration;
  }

  function onPointerDown(event: PointerEvent) {
    if (disabled) return;
    el.setPointerCapture(event.pointerId);
    dragMs = at(event);
  }
  function onPointerMove(event: PointerEvent) {
    if (dragMs !== null) dragMs = at(event);
  }
  function onPointerUp() {
    if (dragMs === null) return;
    void player.seek(dragMs);
    dragMs = null;
  }
  function onKeydown(event: KeyboardEvent) {
    if (disabled) return;
    const target = seekByKey(event.key, position, duration);
    if (target === null) return;
    event.preventDefault();
    void player.seek(target);
  }
</script>

<div
  class="vseek"
  class:disabled
  bind:this={el}
  role="slider"
  tabindex={disabled ? -1 : 0}
  aria-label="Seek"
  aria-valuemin={0}
  aria-valuemax={Math.round(duration / 1000)}
  aria-valuenow={Math.round(position / 1000)}
  aria-valuetext={`${clock(position)} of ${clock(duration)}`}
  aria-disabled={disabled}
  style:--p={fraction}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={() => (dragMs = null)}
  onkeydown={onKeydown}
>
  <i></i>
</div>

<style>
  .vseek {
    position: relative;
    height: 12px;
    margin-top: 8px;
    cursor: pointer;
    touch-action: none;
    color: var(--rack-vfd);
    --seg: repeating-linear-gradient(90deg, currentColor 0 3px, transparent 3px 5px);
  }
  .vseek.disabled {
    cursor: default;
  }
  .vseek::before,
  .vseek i {
    content: "";
    position: absolute;
    left: 0;
    top: 4px;
    height: 4px;
    background: var(--seg);
  }
  .vseek::before {
    right: 0;
    opacity: 0.12;
  }
  .vseek i {
    width: calc(var(--p) * 100%);
    filter: drop-shadow(0 0 3px var(--rack-vfd));
  }
  .vseek:focus-visible {
    outline: 1px solid var(--rack-vfd);
    outline-offset: 2px;
    border-radius: 2px;
  }
</style>
