<script lang="ts">
  import { api } from "../api";
  import Icon from "../components/Icon.svelte";
  import QueueRail from "../components/QueueRail.svelte";
  import type { Player } from "../player.svelte";
  import type { Route } from "../nav.svelte";
  import { changerSlots, changerStart, clampChangerStart, clock, currentIndex, remainingMs, CHANGER_SLOTS } from "./deck";
  import SevenSeg from "./SevenSeg.svelte";

  /// Queue changer (Track 9.5c): the queue as a column unit instead of a
  /// drawer. A disc changer front — a VFD with the disc number and the queue's
  /// remaining time, five disc keys with lamps (a page of the queue, paged by
  /// the arrow keys) — over the full queue in a glass window.
  let { player, onClose, go }: { player: Player; onClose: () => void; go?: (route: Route) => void } = $props();

  const ps = $derived(player.state);
  const length = $derived(ps.queue.length);
  const index = $derived(currentIndex(ps));
  const position = $derived(player.pendingSeek ?? ps.position_ms);

  /// Paged away from the current track; null follows it.
  let browse = $state<number | null>(null);
  $effect(() => {
    void index;
    browse = null;
  });
  const first = $derived(browse === null ? changerStart(index) : clampChangerStart(browse, length));
  const slots = $derived(changerSlots(length, index, first));
  const disc = $derived(String(Math.max(0, index + 1)).padStart(2, " "));
</script>

<section class="rk-unit changerunit" aria-label="Queue changer">
  <div class="rk-head"><h2 class="rk-name">Queue changer</h2>
    <button class="rk-key sm" onclick={onClose} aria-label="Close queue" title="Close"><Icon name="x" size={12} /></button>
  </div>
  <div class="rk-vfd readout" aria-hidden="true">
    <div class="cell">
      <div class="rk-anno"><span class="on">DISC</span></div>
      <SevenSeg value={index >= 0 ? disc : "--"} height={20} />
    </div>
    <div class="cell">
      <div class="rk-anno"><span class="on">OF</span></div>
      <SevenSeg value={String(length).padStart(2, " ")} height={14} />
    </div>
    <div class="cell end">
      <div class="rk-anno"><span class="on">REMAIN</span></div>
      <SevenSeg value={clock(remainingMs({ ...ps, position_ms: position }))} height={14} />
    </div>
  </div>
  <div class="discs">
    <button class="rk-key sm page" onclick={() => (browse = first - CHANGER_SLOTS)} disabled={first === 0} aria-label="Previous discs"><Icon name="chevron-left" size={12} /></button>
    <div class="rk-tray slots" role="group" aria-label="Discs">
      {#each slots as slot (slot.index)}
        {@const item = ps.queue[slot.index]}
        <button
          class="disc"
          class:lit={slot.state === "current"}
          disabled={slot.state === "empty"}
          aria-current={slot.state === "current" ? "true" : undefined}
          aria-label={item ? `Play disc ${slot.index + 1}: ${item.title}` : `Disc ${slot.index + 1}, empty`}
          title={item ? `${item.title} — ${item.artist}` : undefined}
          onclick={() => api.playTracks(ps.queue, slot.index)}
        >
          <span class="lamp"></span><span class="rk-key">{slot.index + 1}</span>
        </button>
      {/each}
    </div>
    <button class="rk-key sm page" onclick={() => (browse = first + CHANGER_SLOTS)} disabled={first + CHANGER_SLOTS >= length} aria-label="Next discs"><Icon name="chevron-right" size={12} /></button>
  </div>
  <div class="rk-win queuewin"><QueueRail {player} title={false} {go} /></div>
</section>

<style>
  .changerunit {
    flex: 1 1 0;
    min-height: 300px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .changerunit .rk-head {
    margin-bottom: 0;
  }
  .changerunit .readout {
    display: flex;
    align-items: flex-end;
    gap: 16px;
    padding: 9px 12px;
  }
  .cell {
    display: grid;
    gap: 5px;
  }
  .cell.end {
    margin-left: auto;
    justify-items: end;
  }
  .discs {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }
  .discs .slots {
    gap: 6px;
    padding: 6px 6px 4px;
  }
  /* A disc key: lamp above a numbered key, like the selector's source keys. */
  .disc {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
  }
  .disc:disabled {
    cursor: default;
  }
  .disc .rk-key {
    width: 38px;
    min-width: 0;
    padding-inline: 0;
    font-variant-numeric: tabular-nums;
  }
  .disc:disabled .rk-key {
    opacity: 0.45;
  }
  .disc .rk-key::after {
    display: none;
  }
  .disc.lit .rk-key {
    color: var(--rack-key-ink-on);
  }
  .disc:active:not(:disabled) .rk-key {
    transform: perspective(200px) rotateX(-16deg);
    transform-origin: 50% 40%;
  }
  .lamp {
    width: 14px;
    height: 4px;
    border-radius: 2px;
    background: #2a1a08;
    box-shadow: inset 0 1px 1px rgba(0, 0, 0, 0.7);
    transition: background 350ms, box-shadow 350ms;
  }
  .disc.lit .lamp {
    background: var(--rack-lamp);
    box-shadow: 0 0 8px var(--rack-lamp-glow), 0 0 2px var(--rack-lamp);
    transition-duration: 80ms;
  }
  .disc:focus-visible {
    outline: none;
  }
  .disc:focus-visible .rk-key {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .discs .page {
    width: 28px;
    padding-inline: 0;
  }
  .queuewin {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .queuewin > :global(*) {
    flex: 1;
    min-height: 0;
  }
</style>
