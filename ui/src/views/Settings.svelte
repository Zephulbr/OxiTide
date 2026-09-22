<script lang="ts">
  import { api, type Config, type OutputDevice } from "../lib/api";
  import type { Player } from "../lib/player.svelte";
  let { player }: { player: Player } = $props();
  let devices = $state<OutputDevice[]>([]);
  let config = $state<Config | null>(null);
  let error = $state("");

  function apply(failure: unknown) {
    error = String(failure);
  }

  function saveAudio(patch: Partial<Config["audio"]>) {
    if (!config) return;
    config = { ...config, audio: { ...config.audio, ...patch } };
  }

  $effect(() => {
    Promise.all([api.getConfig(), api.getOutputDevices()])
      .then(([loaded, list]) => {
        devices = list;
        // The stored device may be gone; fall back to the system default and
        // persist that so the next start does not retry it.
        let next = loaded;
        if (next.audio.output_device && !list.some((d) => d.id === next.audio.output_device)) {
          next = { ...next, audio: { ...next.audio, output_device: "" } };
          api.setOutputDevice("").catch(apply);
        }
        config = next;
      })
      .catch(apply);
  });

  function setDevice(deviceId: string) {
    saveAudio({ output_device: deviceId });
    api.setOutputDevice(deviceId).catch(apply);
  }

  function setExclusive(enabled: boolean) {
    saveAudio({ exclusive: enabled });
    api.setExclusive(enabled).catch(apply);
  }

  function setFallback(mode: "shared" | "warn_only") {
    saveAudio({ exclusive_fallback: mode });
    api.setExclusiveFallback(mode).catch(apply);
  }

  function setQuality(quality: Config["streaming"]["quality"]) {
    if (!config) return;
    config = { ...config, streaming: { ...config.streaming, quality } };
    api.setPreferredQuality(quality).catch(apply);
  }
</script>

<h2>Settings</h2>
{#if error}<p style="color:#ff6b6b">{error}</p>{/if}
{#if config}
  <h3>Audio output</h3>
  <select
    value={config.audio.output_device}
    onchange={(e) => setDevice((e.target as HTMLSelectElement).value)}
  >
    <option value="">System default</option>
    {#each devices as d (d.id)}
      <option value={d.id}>{d.name}{d.is_default ? " (default)" : ""}</option>
    {/each}
  </select>

  <h3>Bit-perfect output</h3>
  <label>
    <input
      type="checkbox"
      checked={config.audio.exclusive}
      onchange={(e) => setExclusive((e.target as HTMLInputElement).checked)}
    />
    Bit-perfect (exclusive mode)
  </label>
  <p>
    <label>
      If unsupported:
      <select
        value={config.audio.exclusive_fallback}
        onchange={(e) =>
          setFallback((e.target as HTMLSelectElement).value as "shared" | "warn_only")}
      >
        <option value="shared">Fall back to shared output</option>
        <option value="warn_only">Stop with a message</option>
      </select>
    </label>
  </p>

  <h3>Streaming quality</h3>
  <select
    value={config.streaming.quality}
    onchange={(e) =>
      setQuality((e.target as HTMLSelectElement).value as Config["streaming"]["quality"])}
  >
    <option value="low">Low</option>
    <option value="high">High</option>
    <option value="lossless">Lossless</option>
    <option value="max">Max (Hi-Res)</option>
  </select>
{/if}
{#if player.state.bit_perfect_notice}
  <p style="color:#e0a030">{player.state.bit_perfect_notice}</p>
{/if}
