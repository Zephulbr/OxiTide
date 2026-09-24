<script lang="ts">
  import { api, type CacheStatus, type Config, type OutputDevice } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import { errorMessage } from "../lib/errors";
  import type { QualityDisplay } from "../lib/quality";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { Player } from "../lib/player.svelte";
  import { SHORTCUTS } from "../lib/shortcuts";
  let {
    player,
    toasts,
    onQualityDisplay,
  }: { player: Player; toasts: Toasts; onQualityDisplay: (mode: QualityDisplay) => void } =
    $props();
  let devices = $state<OutputDevice[]>([]);
  let config = $state<Config | null>(null);
  let cacheStatus = $state<CacheStatus | null>(null);
  let logsDir = $state("");
  let error = $state("");

  function apply(failure: unknown) {
    toasts.show(errorMessage(failure), { severity: "error" });
  }

  function refreshCacheStatus() {
    api.getCacheStatus().then((status) => (cacheStatus = status)).catch(apply);
  }

  function saveAudio(patch: Partial<Config["audio"]>) {
    if (!config) return;
    config = { ...config, audio: { ...config.audio, ...patch } };
  }

  async function load() {
    error = "";
    try {
      const [loaded, list, status, logs] = await Promise.all([
        api.getConfig(),
        api.getOutputDevices(),
        api.getCacheStatus(),
        api.getLogsDir(),
      ]);
      devices = list;
      cacheStatus = status;
      logsDir = logs;
      // The stored device may be gone; fall back to the system default and
      // persist that so the next start does not retry it.
      let next = loaded;
      if (next.audio.output_device && !list.some((d) => d.id === next.audio.output_device)) {
        next = { ...next, audio: { ...next.audio, output_device: "" } };
        api.setOutputDevice("").catch(apply);
      }
      config = next;
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    load();
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

  function setAllowFallback(enabled: boolean) {
    if (!config) return;
    config = { ...config, streaming: { ...config.streaming, allow_fallback: enabled } };
    api.setAllowFallback(enabled).catch(apply);
  }

  function setDisplay(mode: QualityDisplay) {
    if (!config) return;
    config = { ...config, ui: { ...config.ui, quality_display: mode } };
    onQualityDisplay(mode);
    api.setQualityDisplay(mode).catch(apply);
  }

  function setCacheMaxMb(mb: number) {
    if (!config) return;
    const clamped = Math.max(0, Math.floor(mb));
    config = { ...config, cache: { ...config.cache, max_size_mb: clamped } };
    api.setCacheMaxSizeMb(clamped).then(refreshCacheStatus).catch(apply);
  }

  function setClearOnExit(enabled: boolean) {
    if (!config) return;
    config = { ...config, cache: { ...config.cache, clear_on_exit: enabled } };
    api.setCacheClearOnExit(enabled).catch(apply);
  }

  function clearCache() {
    api.clearCache().then(refreshCacheStatus).catch(apply);
  }

  function formatMb(bytes: number): string {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
</script>

<h2>Settings</h2>
{#if !config}
  {#if error}
    <ErrorState message={error} onRetry={load} />
  {:else}
    <Loading />
  {/if}
{:else}
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
    <option value="high">High (AAC 320 kbps)</option>
    <option value="lossless">Lossless</option>
    <option value="max">Max (Hi-Res)</option>
  </select>

  <p>
    <label>
      <input
        type="checkbox"
        checked={config.streaming.allow_fallback}
        onchange={(e) => setAllowFallback((e.target as HTMLInputElement).checked)}
      />
      Play the best available quality and report it
    </label>
  </p>

  <h3>Display</h3>
  <p>
    <label>
      Quality display:
      <select
        value={config.ui.quality_display}
        onchange={(e) => setDisplay((e.target as HTMLSelectElement).value as QualityDisplay)}
      >
        <option value="compact">Compact (MAX / LOSSLESS / AAC)</option>
        <option value="detailed">Detailed (bit depth / rate / bitrate)</option>
      </select>
    </label>
  </p>

  <h3>Cache</h3>
  {#if cacheStatus}
    {#if !cacheStatus.usable}
      <p style="color:#e0a030">
        Cache is disabled: the directory is not writable. Streaming is unaffected.
      </p>
    {:else if cacheStatus.configured_dir && cacheStatus.dir !== cacheStatus.configured_dir}
      <p style="color:#e0a030">
        The configured cache directory is unusable; using {cacheStatus.dir}.
      </p>
    {/if}
    <p class="muted">
      {formatMb(cacheStatus.used_bytes)} used{#if cacheStatus.pending_bytes > 0}
        (+{formatMb(cacheStatus.pending_bytes)} downloading){/if}
      of {formatMb(cacheStatus.max_bytes)}
    </p>
    <p>
      <label>
        Max size (MB):
        <input
          type="number"
          min="0"
          value={config.cache.max_size_mb}
          onchange={(e) => setCacheMaxMb(Number((e.target as HTMLInputElement).value))}
        />
      </label>
    </p>
    <p>
      <label>
        <input
          type="checkbox"
          checked={config.cache.clear_on_exit}
          onchange={(e) => setClearOnExit((e.target as HTMLInputElement).checked)}
        />
        Clear cache on exit
      </label>
    </p>
    <p>
      <button onclick={refreshCacheStatus}>Refresh</button>
      <button onclick={clearCache}>Clear cache</button>
    </p>
    <p class="muted">Directory: {cacheStatus.dir} (edit cache.dir in config.toml)</p>
  {/if}
  <h3>Logs</h3>
  <p class="muted">Directory: {logsDir || "unavailable"}</p>
  <p><button onclick={() => api.openLogsFolder().catch(apply)}>Open logs folder</button></p>
{/if}
<h3>Keyboard shortcuts</h3>
<ul>
  {#each SHORTCUTS as shortcut (shortcut.keys)}
    <li><kbd>{shortcut.keys}</kbd> <span class="muted">{shortcut.description}</span></li>
  {/each}
</ul>
{#if player.state.bit_perfect_notice}
  <p style="color:#e0a030">{player.state.bit_perfect_notice}</p>
{/if}
