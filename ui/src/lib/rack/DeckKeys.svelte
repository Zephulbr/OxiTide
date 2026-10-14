<script lang="ts">
  import type { Snippet } from "svelte";
  import type { RepeatMode } from "../api";
  import Icon from "../components/Icon.svelte";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";

  /// The deck's two key trays (DESIGN §5.1): transport (46×34) and play mode
  /// (38×34: shuffle, repeat, favourite, queue). Fixed-size keys; a key is a
  /// physical part and never stretches. `queueOpen`/`onQueue` let the mini
  /// player drive its own queue sheet; `extra` appends keys to the mode tray.
  let {
    player,
    favorites,
    queueOpen,
    onQueue,
    extra,
  }: {
    player: Player;
    favorites: Favorites;
    queueOpen: boolean;
    onQueue: () => void;
    extra?: Snippet;
  } = $props();

  const state = $derived(player.state);
  const current = $derived(state.current);
  const fav = $derived(current ? favorites.hasTrack(current.track_id) : false);

  function nextRepeat(mode: RepeatMode): RepeatMode {
    return mode === "off" ? "all" : mode === "all" ? "one" : "off";
  }
</script>

<div class="rk-tray transport-keys" role="group" aria-label="Transport">
  <button class="rk-key" onclick={() => player.prev()} aria-label="Previous" disabled={state.queue.length === 0}><Icon name="skip-back-solid" size={14} /></button>
  <button class="rk-key" class:lit={state.playing} onclick={() => player.playPause()} aria-label={state.playing ? "Pause" : "Play"} disabled={!current}>
    <Icon name={state.playing ? "pause" : "play"} size={14} />
  </button>
  <button class="rk-key" onclick={() => player.next()} aria-label="Next" disabled={state.queue.length === 0}><Icon name="skip-forward-solid" size={14} /></button>
</div>
<div class="rk-tray modes" role="group" aria-label="Play mode">
  <button class="rk-key" aria-pressed={state.shuffle} onclick={() => player.setShuffle(!state.shuffle)} aria-label="Shuffle" title="Shuffle the rest of the queue">
    <Icon name="shuffle" size={13} />
  </button>
  <button
    class="rk-key"
    aria-pressed={state.repeat !== "off"}
    onclick={() => player.setRepeat(nextRepeat(state.repeat))}
    aria-label={state.repeat === "one" ? "Repeat one track" : state.repeat === "all" ? "Repeat all" : "Repeat off"}
    title={state.repeat === "one" ? "Repeat one" : state.repeat === "all" ? "Repeat all" : "Repeat off"}
  ><Icon name={state.repeat === "one" ? "repeat-1" : "repeat"} size={13} /></button>
  <button
    class="rk-key"
    aria-pressed={fav}
    disabled={!current}
    onclick={() => current && favorites.toggleTrack(current.track_id)}
    aria-label={fav ? "Unfavorite" : "Favorite"}
    title={fav ? "Unfavorite" : "Favorite"}
  ><Icon name="heart" size={13} /></button>
  <button class="rk-key" aria-pressed={queueOpen} onclick={onQueue} aria-label="Queue" title="Queue">
    <Icon name="list-music" size={13} />
  </button>
  {@render extra?.()}
</div>

<style>
  .transport-keys .rk-key {
    width: 46px;
    min-width: 0;
    padding-inline: 0;
  }
  .modes .rk-key,
  .modes :global(.rk-key) {
    width: 38px;
    min-width: 0;
    padding-inline: 0;
  }
</style>
