<script lang="ts">
  import { api, type CacheStatus, type Config, type OutputDevice } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import {
    appearance,
    fontStack,
    type DynamicColorSetting,
    type TypefaceSetting,
  } from "../lib/appearance.svelte";
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
  let customDraft = $state(appearance.customTypeface);
  let customNote = $state("");
  let fontChoices = $state<string[]>([]);

  // Keep the editable draft in sync when the stored value loads/changes.
  $effect(() => {
    customDraft = appearance.customTypeface;
  });

  const queryLocalFonts = (
    window as unknown as { queryLocalFonts?: () => Promise<{ family: string }[]> }
  ).queryLocalFonts;

  async function browseFonts() {
    if (!queryLocalFonts) return;
    try {
      const fonts = await queryLocalFonts();
      fontChoices = [...new Set(fonts.map((f) => f.family))].sort();
    } catch {
      fontChoices = [];
    }
  }

  function validateFont(name: string) {
    const clean = name.trim().replace(/["\\]/g, "");
    customNote = clean && !document.fonts.check(`12px "${clean}"`) ? "not found — using fallback" : "";
  }

  async function applyCustomFont() {
    try {
      await appearance.setCustomTypeface(customDraft);
      await appearance.setTypeface("custom");
      validateFont(customDraft);
    } catch (e) {
      apply(e);
    }
  }

  function setTypeface(value: TypefaceSetting) {
    appearance.setTypeface(value).catch(apply);
    if (value === "custom") validateFont(customDraft);
  }

  function setDynamicColor(setting: DynamicColorSetting) {
    if (!config) return;
    config = { ...config, ui: { ...config.ui, dynamic_color: setting } };
    appearance.setDynamicColor(setting).catch(apply);
  }

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
  <h3>Appearance</h3>
  <div class="field">
    <span class="field-label">Mode:</span>
    <div class="segmented" role="group" aria-label="Mode">
      <button class:active={appearance.mode === "atmosphere"} onclick={() => appearance.setMode("atmosphere").catch(apply)}>Atmosphere</button>
      <button class:active={appearance.mode === "instrument"} onclick={() => appearance.setMode("instrument").catch(apply)}>Instrument</button>
    </div>
  </div>
  {#if appearance.mode === "instrument"}
    <p class="muted">Instrument minimizes animation and visual effects.</p>
  {/if}

  <div class="field">
    <span class="field-label">Dynamic color:</span>
    <div class="segmented" role="group" aria-label="Dynamic color">
      <button class:active={config.ui.dynamic_color === "auto"} onclick={() => setDynamicColor("auto")}>Default</button>
      <button class:active={config.ui.dynamic_color === "full"} onclick={() => setDynamicColor("full")}>Full</button>
      <button class:active={config.ui.dynamic_color === "accent"} onclick={() => setDynamicColor("accent")}>Accent-only</button>
      <button class:active={config.ui.dynamic_color === "off"} onclick={() => setDynamicColor("off")}>Off</button>
    </div>
  </div>

  <p>
    <label>
      Typeface:
      <select value={appearance.typeface} onchange={(e) => setTypeface((e.target as HTMLSelectElement).value as TypefaceSetting)}>
        <option value="auto">Default (per mode)</option>
        <option value="manrope">Manrope</option>
        <option value="inter">Inter</option>
        <option value="system">System</option>
        <option value="custom">Custom…</option>
      </select>
    </label>
  </p>
  {#if appearance.typeface === "custom"}
    <div class="custom-font">
      <input list="font-choices" placeholder="Font family name" bind:value={customDraft} oninput={() => validateFont(customDraft)} />
      <datalist id="font-choices">
        {#each fontChoices as family (family)}<option value={family}></option>{/each}
      </datalist>
      {#if queryLocalFonts}
        <button onclick={browseFonts}>Browse installed fonts…</button>
      {/if}
      <button onclick={applyCustomFont}>Apply</button>
      <p style:font-family={fontStack("custom", customDraft)} class="font-preview">
        The quick brown fox — 0123456789
      </p>
      {#if customNote}<p class="muted">{customNote}</p>{/if}
    </div>
  {/if}

  <div class="field">
    <span class="field-label">Density:</span>
    <div class="segmented" role="group" aria-label="Density">
      <button class:active={appearance.density === "comfortable"} onclick={() => appearance.setDensity("comfortable").catch(apply)}>Comfortable</button>
      <button class:active={appearance.density === "compact"} onclick={() => appearance.setDensity("compact").catch(apply)}>Compact</button>
    </div>
  </div>

  <div class="field">
    <span class="field-label">Cover shape:</span>
    <div class="segmented" role="group" aria-label="Cover shape">
      <button class:active={appearance.coverShape === "disc"} onclick={() => appearance.setCoverShape("disc").catch(apply)}>Disc</button>
      <button class:active={appearance.coverShape === "sleeve"} onclick={() => appearance.setCoverShape("sleeve").catch(apply)}>Sleeve</button>
    </div>
  </div>

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
      <p style="color:var(--warning)">
        Cache is disabled: the directory is not writable. Streaming is unaffected.
      </p>
    {:else if cacheStatus.configured_dir && cacheStatus.dir !== cacheStatus.configured_dir}
      <p style="color:var(--warning)">
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
  <p style="color:var(--warning)">{player.state.bit_perfect_notice}</p>
{/if}

<style>
  .segmented {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
  }
  .field-label {
    color: var(--text-muted);
  }
  .segmented button {
    padding: 4px 10px;
    border-radius: calc(var(--radius-control) - 3px);
    color: var(--text-muted);
  }
  .segmented button.active {
    background: var(--surface-3);
    color: var(--text);
  }
  .custom-font {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 4px 0 12px;
  }
  .font-preview {
    flex-basis: 100%;
    font-size: 18px;
    margin: 4px 0 0;
  }
</style>
