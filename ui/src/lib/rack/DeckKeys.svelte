<script lang="ts">
  import type { Snippet } from "svelte";
  import type { RepeatMode } from "../api";
  import FavoriteHeart, { toggledFx, type HeartFx } from "../components/FavoriteHeart.svelte";
  import Icon from "../components/Icon.svelte";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  import { t } from "../i18n.svelte";

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

  const ps = $derived(player.state);
  const current = $derived(ps.current);
  const fav = $derived(current ? favorites.hasTrack(current.track_id) : false);
  /// The favourite key's last toggle (FavoriteHeart pops or breaks it).
  let heartFx = $state<HeartFx | null>(null);

  function nextRepeat(mode: RepeatMode): RepeatMode {
    return mode === "off" ? "all" : mode === "all" ? "one" : "off";
  }
</script>

<div class="rk-tray transport-keys" role="group" aria-label={t("rack.transport")}>
  <button class="rk-key" onclick={() => player.prev()} aria-label={t("player.previous")} disabled={ps.queue.length === 0}><Icon name="skip-back-solid" size={14} /></button>
  <button class="rk-key" class:lit={ps.playing} onclick={() => player.playPause()} aria-label={ps.playing ? t("player.pause") : t("player.play")} disabled={!current}>
    <Icon name={ps.playing ? "pause" : "play"} size={14} />
  </button>
  <button class="rk-key" onclick={() => player.next()} aria-label={t("player.next")} disabled={ps.queue.length === 0}><Icon name="skip-forward-solid" size={14} /></button>
</div>
<div class="rk-tray modes" role="group" aria-label={t("rack.playMode")}>
  <button class="rk-key" aria-pressed={ps.shuffle} onclick={() => player.setShuffle(!ps.shuffle)} aria-label={t("player.shuffle")} title={t("player.shuffleHint")}>
    <Icon name="shuffle" size={13} />
  </button>
  <button
    class="rk-key"
    aria-pressed={ps.repeat !== "off"}
    onclick={() => player.setRepeat(nextRepeat(ps.repeat))}
    aria-label={ps.repeat === "one" ? t("player.repeatOneLabel") : ps.repeat === "all" ? t("player.repeatAll") : t("player.repeatOff")}
          title={ps.repeat === "one" ? t("player.repeatOne") : ps.repeat === "all" ? t("player.repeatAll") : t("player.repeatOff")}
  ><Icon name={ps.repeat === "one" ? "repeat-1" : "repeat"} size={13} /></button>
  <button
    class="rk-key"
    aria-pressed={fav}
    disabled={!current}
    onclick={() => {
      if (!current) return;
      heartFx = toggledFx(heartFx, fav);
      void favorites.toggleTrack(current.track_id);
    }}
    aria-label={fav ? t("menu.unfavorite") : t("menu.favorite")}
    title={fav ? t("menu.unfavorite") : t("menu.favorite")}
  ><FavoriteHeart size={13} fx={heartFx} /></button>
  <button class="rk-key" aria-pressed={queueOpen} onclick={onQueue} aria-label={t("player.queue")} title={t("player.queue")}>
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
