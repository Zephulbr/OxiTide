<script lang="ts">
  import { api, type OutputDevice } from "../lib/api";
  import type { Player } from "../lib/player.svelte";
  let { player }: { player: Player } = $props();
  let devices = $state<OutputDevice[]>([]);
  let quality = $state<"low" | "high" | "lossless" | "max">("lossless");
  let exclusive = $state(false);
  let fallback = $state<"shared" | "warn_only">("shared");
  let error = $state("");

  $effect(() => {
    api.getOutputDevices().then((d) => (devices = d)).catch((e) => (error = String(e)));
  });

  $effect(() => {
    quality = player.state.preferred_quality;
  });

  $effect(() => {
    exclusive = player.state.output_mode === "exclusive";
  });
</script>

<h2>Settings</h2>
{#if error}<p style="color:#ff6b6b">{error}</p>{/if}
<h3>Audio output</h3>
<select onchange={(e) => {
  const value = (e.target as HTMLSelectElement).value;
  api.setOutputDevice(value).catch((err) => (error = String(err)));
}}>
  <option value="">System default</option>
  {#each devices as d (d.id)}
    <option value={d.id}>{d.name}{d.is_default ? " (default)" : ""}</option>
  {/each}
</select>
<h3>Streaming quality</h3>
<select bind:value={quality} onchange={() => api.setPreferredQuality(quality)}>
  <option value="low">Low</option>
  <option value="high">High</option>
  <option value="lossless">Lossless</option>
  <option value="max">Max (Hi-Res)</option>
</select>
<h3>Bit-perfect output</h3>
<label>
  <input
    type="checkbox"
    bind:checked={exclusive}
    onchange={() => api.setExclusive(exclusive).catch((e) => (error = String(e)))}
  />
  Bit-perfect (exclusive mode)
</label>
<p>
  <label>
    If unsupported:
    <select
      bind:value={fallback}
      onchange={() => api.setExclusiveFallback(fallback).catch((e) => (error = String(e)))}
    >
      <option value="shared">Fall back to shared output</option>
      <option value="warn_only">Stop with a message</option>
    </select>
  </label>
</p>
{#if player.state.bit_perfect_notice}
  <p style="color:#e0a030">{player.state.bit_perfect_notice}</p>
{/if}
