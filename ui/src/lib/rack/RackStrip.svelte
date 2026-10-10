<script lang="ts">
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import type { Favorites } from "../favorites.svelte";
  import type { Nav } from "../nav.svelte";
  import type { Player } from "../player.svelte";
  import DeckKeys from "./DeckKeys.svelte";
  import DeckVfd from "./DeckVfd.svelte";
  import VolumeKnob from "./VolumeKnob.svelte";

  /// Narrow-window fold (Track 9.5c): below ~1000px the rack column folds into
  /// this strip under the display — deck and amp in one row: cover window,
  /// VFD, transport and play-mode keys, a smaller volume knob. Now Playing
  /// opens from the cover or the expand key, as on the deck.
  let { player, nav, favorites }: { player: Player; nav: Nav; favorites: Favorites } = $props();

  const current = $derived(player.state.current);
</script>

<section class="rk-unit strip" aria-label="Deck">
  <button class="rk-win art" onclick={() => nav.toggleNowPlaying()} aria-label="Open now playing" disabled={!current}>
    {#if current}<Cover uuid={current.cover} size={160} radius={2} alt={`${current.title} artwork`} />{/if}
  </button>
  <div class="vfd"><DeckVfd {player} segHeight={20} /></div>
  <div class="keys">
    <DeckKeys {player} {favorites} queueOpen={nav.queueOpen} onQueue={() => nav.toggleQueue()}>
      {#snippet extra()}
        <button class="rk-key" onclick={() => nav.toggleNowPlaying()} disabled={!current} aria-label="Open now playing" title="Open now playing">
          <Icon name="chevron-up" size={13} />
        </button>
      {/snippet}
    </DeckKeys>
  </div>
  <div class="knob">
    <VolumeKnob {player} scale={0.8} />
  </div>
</section>

<style>
  section.strip {
    display: grid;
    grid-template-columns: auto minmax(220px, 1fr) auto auto;
    grid-template-areas: "art vfd keys knob";
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
  }
  .art {
    grid-area: art;
    width: 92px;
    height: 92px;
    padding: 5px;
    border: 0;
    cursor: pointer;
  }
  .art :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .vfd {
    grid-area: vfd;
    min-width: 0;
  }
  .keys {
    grid-area: keys;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
  }
  .knob {
    grid-area: knob;
  }
  /* Narrower still: the keys drop under the VFD; the cover gives way first. */
  @media (max-width: 860px) {
    section.strip {
      grid-template-columns: auto minmax(0, 1fr) auto;
      grid-template-areas:
        "art vfd knob"
        "art keys knob";
      row-gap: 8px;
    }
    .keys {
      flex-direction: row;
      justify-content: center;
    }
  }
  @media (max-width: 760px) {
    section.strip {
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas:
        "vfd knob"
        "keys knob";
    }
    .art {
      display: none;
    }
  }
</style>
