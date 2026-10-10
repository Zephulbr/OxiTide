<script lang="ts">
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import QueueRail from "../components/QueueRail.svelte";
  import type { Favorites } from "../favorites.svelte";
  import { focusTrap } from "../focusTrap";
  import type { Mini } from "../mini.svelte";
  import type { Player } from "../player.svelte";
  import { formatQuality } from "../quality";
  import DeckKeys from "./DeckKeys.svelte";
  import DeckVfd from "./DeckVfd.svelte";
  import { levels } from "./levels.svelte";
  import BarMeter from "./meters/BarMeter.svelte";
  import VolumeKnob from "./VolumeKnob.svelte";

  /// Rack mini player (Track 9.5c): a compact deck over a compact amp, in the
  /// same hardware language as the full rack — cover in a glass window, the
  /// deck VFD, the transport and play-mode key trays, a bar meter beside the
  /// volume knob. The queue opens as a unit sliding up over the stack.
  let { player, favorites, mini }: { player: Player; favorites: Favorites; mini: Mini } = $props();

  const current = $derived(player.state.current);
  const quality = $derived(formatQuality(player.state.stream, "compact", player.state.preferred_quality));

  $effect(() => levels.use());
</script>

<div class="rmini">
  <section class="rk-unit deck" aria-label="Deck">
    <div class="rk-head">
      <h2 class="rk-name">Network transport</h2>
      <div class="headright">
        <span class="rk-silk">{quality}</span>
        <button class="rk-key sm" onclick={() => void mini.exit()} aria-label="Restore full window" title="Restore full window">
          <Icon name="maximize-2" size={12} />
        </button>
      </div>
    </div>
    <div class="art">
      <div class="rk-win window">
        {#if current}<Cover uuid={current.cover} size={320} radius={2} alt={`${current.title} artwork`} />{/if}
      </div>
    </div>
    <DeckVfd {player} segHeight={18} />
    <div class="keys">
      <DeckKeys {player} {favorites} queueOpen={mini.queueOpen} onQueue={() => mini.toggleQueue()} />
    </div>
  </section>
  <section class="rk-unit amp" aria-label="Amplifier">
    <div class="rk-vfd meter" aria-hidden="true"><BarMeter snapshot={levels.snapshot} header={false} /></div>
    <VolumeKnob {player} scale={0.7} />
  </section>

  {#if mini.queueOpen}
    <div class="sheet-backdrop" onclick={() => mini.toggleQueue()} aria-hidden="true"></div>
    <!-- Escape is routed by App.currentEscapeTarget ("mini-queue"); the trap
         restores focus to the Queue key on close. -->
    <div class="rk-unit sheet" role="dialog" aria-modal="true" aria-label="Queue" tabindex="-1" use:focusTrap>
      <div class="rk-head"><h2 class="rk-name">Queue</h2>
        <button class="rk-key sm" onclick={() => mini.toggleQueue()} aria-label="Close queue" title="Close"><Icon name="x" size={12} /></button>
      </div>
      <div class="rk-win queuewin"><QueueRail {player} /></div>
    </div>
  {/if}
</div>

<style>
  .rmini {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    height: 100%;
    padding: 8px;
    box-sizing: border-box;
    background: var(--rack-room);
    overflow: hidden;
  }
  .rmini .rk-unit {
    padding: 10px 12px;
  }
  .rmini .rk-head {
    margin-bottom: 8px;
  }
  .headright {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .deck {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .deck .rk-head {
    margin-bottom: 0;
  }
  /* The cover takes the deck's slack and stays square; it gives way first. */
  .art {
    flex: 1;
    min-height: 0;
    display: flex;
    justify-content: center;
  }
  .window {
    height: 100%;
    max-width: 100%;
    aspect-ratio: 1;
    padding: 5px;
    box-sizing: border-box;
  }
  .window :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
  }
  .amp {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .amp .meter {
    flex: 1;
    min-width: 0;
    padding: 8px 10px 6px;
  }
  .sheet-backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    background: rgba(0, 0, 0, 0.55);
  }
  .rmini .sheet {
    position: fixed;
    left: 8px;
    right: 8px;
    bottom: 8px;
    z-index: 11;
    display: flex;
    flex-direction: column;
    max-height: 85vh;
    min-height: 50vh;
    box-shadow: var(--rack-unit-shadow), 0 -12px 30px rgba(0, 0, 0, 0.7);
    animation: rk-sheet var(--dur-view) var(--ease-in) both;
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
  @keyframes rk-sheet {
    from {
      transform: translateY(100%);
    }
  }
  @media (max-height: 400px) {
    .art {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .rmini .sheet {
      animation-name: rk-sheet-fade;
    }
    @keyframes rk-sheet-fade {
      from {
        opacity: 0;
      }
    }
  }
</style>
