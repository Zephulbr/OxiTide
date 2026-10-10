<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Player } from "../player.svelte";
  import { clock, currentIndex, repeatAnnunciator } from "./deck";
  import SevenSeg from "./SevenSeg.svelte";
  import VfdSeek from "./VfdSeek.svelte";

  /// The deck's VFD (DESIGN §5.4): title and sub line beside the 7-segment
  /// time, the segmented seek bar and the annunciator row. Shared by the deck,
  /// the narrow-window strip and the Rack mini player; `top` is the Display
  /// face's extra block above the title.
  let {
    player,
    tall = false,
    segHeight = 22,
    top,
  }: { player: Player; tall?: boolean; segHeight?: number; top?: Snippet } = $props();

  const state = $derived(player.state);
  const current = $derived(state.current);
  const position = $derived(player.pendingSeek ?? state.position_ms);
  const index = $derived(currentIndex(state));
  const message = $derived(state.error || (state.bit_perfect_notice ?? ""));
</script>

<div class="rk-vfd deckvfd" class:tall>
  {@render top?.()}
  <div class="row">
    <div class="titles">
      <div class="rk-vfd-text title">{current ? current.title : "No disc"}</div>
      <div class="rk-vfd-text sub" class:warn={message !== ""}>
        {message !== "" ? message : current ? `${current.artist}, ${current.album}` : "Choose something to play"}
      </div>
    </div>
    <SevenSeg value={clock(position)} height={segHeight} />
  </div>
  <VfdSeek {player} />
  <div class="row annos" aria-hidden="true">
    <div class="rk-anno">
      <span class:on={state.playing}>PLAY</span>
      <span class:on={state.buffering}>BUF</span>
      <span class:on={state.repeat !== "off"}>{repeatAnnunciator(state.repeat) ?? "RPT"}</span>
      <span class:on={state.shuffle}>SHUF</span>
      <span class="xfade" class:on={state.crossfade_ms > 0}>XFADE</span>
    </div>
    <div class="rk-anno"><span>TRK</span><span class:on={index >= 0}>{String(index + 1).padStart(2, "0")}/{String(state.queue.length).padStart(2, "0")}</span></div>
  </div>
</div>

<style>
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }
  .titles {
    min-width: 0;
  }
  .warn {
    color: var(--warning);
    text-shadow: none;
  }
  .annos {
    margin-top: 6px;
  }
  .deckvfd.tall {
    padding: 12px 14px 10px;
  }
  .tall .title {
    font-size: 16px;
  }
</style>
