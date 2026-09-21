<script lang="ts">
  import { api, type OutputDevice } from "../lib/api";
  import type { Player } from "../lib/player.svelte";
  let { player }: { player: Player } = $props();
  let devices = $state<OutputDevice[]>([]);
  let quality = $state<"low" | "high" | "lossless" | "max">("lossless");
  let error = $state("");

  $effect(() => {
    api.getOutputDevices().then((d) => (devices = d)).catch((e) => (error = String(e)));
  });

  $effect(() => {
    quality = player.state.preferred_quality;
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
