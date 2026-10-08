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
  let { player, meters, toasts }: { player: Player; meters: MeterStyle; toasts: Toasts } = $props();

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

<section class="rk-unit amp" aria-label="Amplifier">
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
  <div class="row">
    <Meters style={meters} />
    <div class="knobwrap">
      <VolumeKnob {player} />
      <span class="rk-silk">Volume</span>
    </div>
  </div>
  <div class="rk-tray dsp" role="group" aria-label="Sound processing">
    <button class="rk-key sm" aria-pressed={dsp?.eq.enabled ?? false} disabled={!dsp} onclick={toggleEq}>EQ</button>
    <button class="rk-key sm" aria-pressed={(dsp?.replay_gain.mode ?? "off") !== "off"} disabled={!dsp} onclick={toggleRg}>ReplayGain</button>
    <button class="rk-key sm" aria-pressed={(dsp?.crossfade.duration_ms ?? 0) > 0} disabled={!dsp} onclick={toggleCrossfade}>Crossfade</button>
  </div>
</section>

<style>
  .status {
    display: flex;
    gap: 12px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
  }
  .knobwrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .dsp {
    display: flex;
    margin-top: 12px;
  }
  .dsp .rk-key {
    flex: 1;
  }
</style>
