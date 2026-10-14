<script lang="ts">
  import type { MeterStyle, ReplayGainMode } from "../api";
  import { dspBadgeTitle } from "../dsp";
  import { dspStore } from "../dspStore.svelte";
  import type { Player } from "../player.svelte";
  import { formatQuality } from "../quality";
  import type { Toasts } from "../toasts.svelte";
  import { errorMessage } from "../errors";
  import { formatBadge } from "./deck";
  import Meters from "./meters/Meters.svelte";
  import VolumeKnob from "./VolumeKnob.svelte";

  /// Stereo amplifier (spec §6.2): meters, volume knob, status LEDs and the
  /// DSP keys (long-form settings stay in Setup).
  /// `wide` (the expanded deck's right column): DSP keys stand vertically on
  /// the left, meters centred, a larger knob on the right — a symmetric front.
  let {
    player,
    meters,
    toasts,
    wide = false,
  }: { player: Player; meters: MeterStyle; toasts: Toasts; wide?: boolean } = $props();

  $effect(() => {
    void dspStore.ensure();
  });

  const dsp = $derived(dspStore.settings);
  const quality = $derived(formatQuality(player.state.stream, "compact", player.state.preferred_quality));
  const badge = $derived(formatBadge(player.state.stream));

  let lastRg: ReplayGainMode = "album";
  let lastCrossfade = 6000;
  const report = (e: unknown) => toasts.show(errorMessage(e), { severity: "error" });

  function toggleEq() {
    if (dsp) dspStore.set({ ...dsp, eq: { ...dsp.eq, enabled: !dsp.eq.enabled } }, report);
  }
  function toggleRg() {
    if (!dsp) return;
    const on = dsp.replay_gain.mode !== "off";
    if (on) lastRg = dsp.replay_gain.mode;
    dspStore.set({ ...dsp, replay_gain: { ...dsp.replay_gain, mode: on ? "off" : lastRg } }, report);
  }
  function toggleCrossfade() {
    if (!dsp) return;
    const on = dsp.crossfade.duration_ms > 0;
    if (on) lastCrossfade = dsp.crossfade.duration_ms;
    dspStore.set({ ...dsp, crossfade: { ...dsp.crossfade, duration_ms: on ? 0 : lastCrossfade } }, report);
  }
</script>

<section class="rk-unit amp" class:wide aria-label="Amplifier">
  <div class="rk-head">
    <h2 class="rk-name">Stereo amplifier</h2>
    <div class="status" role="group" aria-label="Signal">
      <span class="rk-led" class:on={player.state.current !== null}>{quality}</span>
      {#if player.state.bit_perfect}
        <span class="rk-led on green">Bit-perfect</span>
      {:else if player.state.dsp_active}
        <span class="rk-led on" title={dspBadgeTitle(player.state.dsp_stages)}>DSP</span>
      {/if}
      {#if badge}<span class="rk-led on">{badge}</span>{/if}
    </div>
  </div>
  <div class="body">
    <div class="meterslot"><Meters style={meters} /></div>
    <div class="knobwrap">
      <VolumeKnob {player} scale={wide ? 1.35 : 1} />
      <span class="rk-silk">Volume</span>
    </div>
    <div class="rk-tray dsp" role="group" aria-label="Sound processing">
      <button class="rk-key sm" aria-pressed={dsp?.eq.enabled ?? false} disabled={!dsp} onclick={toggleEq}>EQ</button>
      <button class="rk-key sm" aria-pressed={(dsp?.replay_gain.mode ?? "off") !== "off"} disabled={!dsp} onclick={toggleRg}>ReplayGain</button>
      <button class="rk-key sm" aria-pressed={(dsp?.crossfade.duration_ms ?? 0) > 0} disabled={!dsp} onclick={toggleCrossfade}>Crossfade</button>
    </div>
  </div>
</section>

<style>
  .status {
    display: flex;
    gap: 12px;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      "meters knob"
      "dsp dsp";
    gap: 12px 14px;
    align-items: center;
  }
  .meterslot {
    grid-area: meters;
    display: flex;
    min-width: 0;
  }
  .knobwrap {
    grid-area: knob;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .dsp {
    grid-area: dsp;
    display: flex;
  }
  .dsp .rk-key {
    flex: 1;
  }
  /* Wide: [DSP keys] [meters] [knob], each column vertically centred. */
  .wide .body {
    grid-template-columns: 112px minmax(0, 1fr) 112px;
    grid-template-areas: "dsp meters knob";
    gap: 18px;
  }
  .wide .dsp {
    flex-direction: column;
    justify-self: start;
    width: 112px;
  }
  .wide .dsp .rk-key {
    flex: none;
    width: 100%;
  }
</style>
