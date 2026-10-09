<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { listen } from "@tauri-apps/api/event";
  import {
    api,
    SCROBBLE_STATUS_EVENT,
    type CacheStatus,
    type Config,
    type CrossfadeSettings,
    type DspSettings,
    type EqSettings,
    type OutputDevice,
    type ReplayGainSettings,
    type ScrobbleServiceId,
    type ScrobbleStatusDto,
  } from "../lib/api";
  import { canToggle, queuedLabel, serviceLabel } from "../lib/scrobble";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import KeyringNotice from "../lib/components/KeyringNotice.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import {
    appearance,
    fontStack,
    type DynamicColorSetting,
    type TypefaceSetting,
  } from "../lib/appearance.svelte";
  import {
    CROSSFADE_MAX_MS,
    DB_STEP,
    EQ_BANDS_HZ,
    EQ_MAX_DB,
    EQ_PRESETS,
    REPLAY_GAIN_MAX_DB,
    bandLabel,
    clampDb,
    defaultDspSettings,
    effectivePreamp,
    formatCrossfade,
    formatDb,
    matchPreset,
    presetBands,
    withBand,
  } from "../lib/dsp";
  import { dspStore } from "../lib/dspStore.svelte";
  import { errorMessage } from "../lib/errors";
  import { optimistic } from "../lib/optimistic";
  import type { QualityDisplay } from "../lib/quality";
  import { SETTINGS_TABS, nextTab, type SettingsTab } from "../lib/settingsTabs";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { Player } from "../lib/player.svelte";
  import { SHORTCUTS } from "../lib/shortcuts";
  import {
    HOTKEY_ACTIONS,
    acceleratorFromEvent,
    actionLabel,
    conflictFor,
    displayAccelerator,
    type HotkeyAction,
  } from "../lib/hotkeys";
  import { createHotkeyCapture } from "../lib/hotkeyCapture.svelte";
  let {
    player,
    toasts,
    onLogout,
    onQualityDisplay,
  }: {
    player: Player;
    toasts: Toasts;
    onLogout: () => void;
    onQualityDisplay: (mode: QualityDisplay) => void;
  } = $props();
  let tab = $state<SettingsTab>("appearance");
  let devices = $state<OutputDevice[]>([]);
  let config = $state<Config | null>(null);
  let cacheStatus = $state<CacheStatus | null>(null);
  let logsDir = $state("");
  let error = $state("");
  let customDraft = $state(appearance.customTypeface);
  let customNote = $state("");
  let fontChoices = $state<string[]>([]);
  let scrobble = $state<ScrobbleStatusDto | null>(null);
  let lastfmAuth = $state<"idle" | "waiting">("idle");
  let lastfmAuthNote = $state("");
  let lbToken = $state("");
  let lbBusy = $state(false);
  let lbError = $state("");
  let confirmingHistoryClear = $state(false);
  let confirmingBlockExplicit = $state(false);
  let hotkeyConflict = $state<
    { action: HotkeyAction; accelerator: string; other: HotkeyAction } | null
  >(null);
  const capture = createHotkeyCapture({
    setCapture: (active) => api.setHotkeyCapture(active),
    save: saveHotkey,
    report: apply,
  });

  async function refreshScrobble() {
    try {
      scrobble = await api.getScrobbleStatus();
    } catch (e) {
      apply(e);
    }
  }

  $effect(() => {
    refreshScrobble();
  });

  $effect(() => {
    const unlisten = listen<ScrobbleStatusDto>(SCROBBLE_STATUS_EVENT, (event) => {
      scrobble = event.payload;
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  function toggleService(service: ScrobbleServiceId, enabled: boolean) {
    if (!scrobble) return;
    const previous = scrobble[service].enabled;
    scrobble = { ...scrobble, [service]: { ...scrobble[service], enabled } };
    api.setScrobbleEnabled(service, enabled).catch((e) => {
      if (scrobble) {
        scrobble = { ...scrobble, [service]: { ...scrobble[service], enabled: previous } };
      }
      apply(e);
    });
  }

  async function connectLastfm() {
    lastfmAuthNote = "";
    try {
      const url = await api.lastfmBeginAuth();
      lastfmAuth = "waiting";
      await openUrl(url);
      void pollLastfm();
    } catch (e) {
      lastfmAuth = "idle";
      lastfmAuthNote = errorMessage(e);
    }
  }

  async function pollLastfm() {
    for (let attempt = 0; attempt < 40 && lastfmAuth === "waiting"; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 3000));
      if (lastfmAuth !== "waiting") return;
      try {
        scrobble = await api.lastfmCompleteAuth();
        lastfmAuth = "idle";
        return;
      } catch (e) {
        const message = errorMessage(e);
        if (!message.includes("not complete")) {
          lastfmAuth = "idle";
          lastfmAuthNote = message;
          return;
        }
      }
    }
    if (lastfmAuth === "waiting") {
      lastfmAuth = "idle";
      lastfmAuthNote = "Authorization timed out — try again.";
    }
  }

  function cancelLastfm() {
    lastfmAuth = "idle";
    api.lastfmCancelAuth().catch(apply);
  }

  function disconnectLastfm() {
    api.lastfmDisconnect().then(refreshScrobble).catch(apply);
  }

  async function connectListenbrainz() {
    lbError = "";
    lbBusy = true;
    try {
      scrobble = await api.listenbrainzConnect(lbToken);
      lbToken = "";
    } catch (e) {
      lbError = errorMessage(e);
    } finally {
      lbBusy = false;
    }
  }

  function disconnectListenbrainz() {
    api.listenbrainzDisconnect().then(refreshScrobble).catch(apply);
  }

  const TAB_LABELS: Record<SettingsTab, string> = {
    appearance: "Appearance",
    playback: "Playback",
    content: "Content",
    connections: "Connections",
    storage: "Storage",
    system: "System",
  };

  function onTabKeydown(event: KeyboardEvent, id: SettingsTab) {
    let target: SettingsTab | null = null;
    if (event.key === "ArrowRight") target = nextTab(id, 1);
    else if (event.key === "ArrowLeft") target = nextTab(id, -1);
    else if (event.key === "Home") target = SETTINGS_TABS[0];
    else if (event.key === "End") target = SETTINGS_TABS[SETTINGS_TABS.length - 1];
    if (!target) return;
    event.preventDefault();
    tab = target;
    document.getElementById(`settings-tab-${target}`)?.focus();
  }

  // Keep the editable draft in sync when the stored value loads/changes.
  $effect(() => {
    customDraft = appearance.customTypeface;
  });

  // While capture is active, a capture-phase listener owns the keyboard; the
  // controller attaches it only after the backend suspended global hotkeys.
  // Cleanup restores both (Escape, save, cancel, unmount).
  $effect(() => {
    if (!capture.active || capture.recording === null) return;
    const action = capture.recording;
    const handler = (event: KeyboardEvent) => onHotkeyKeydown(event);
    window.addEventListener("keydown", handler, true);
    document.getElementById(`hotkey-${action}`)?.focus();
    return () => {
      window.removeEventListener("keydown", handler, true);
    };
  });

  // Recording only makes sense on the System tab; leaving it cancels capture.
  $effect(() => {
    if (tab !== "system") capture.cancel();
  });

  // Navigating away unmounts Settings while capture may still be engaged;
  // cancel is idempotent and restores global delivery.
  $effect(() => {
    return () => capture.cancel();
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
      dspStore.hydrate(next.dsp);
      trackFlags.setContentFlags(next.content.block_explicit, next.content.block_ai);
      trackFlags.setShowLabels(next.content.show_labels);
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    load();
  });

  /// DSP lives in the shared store (the Hi-Fi Rack's amp and EQ units write it
  /// too); it debounces persistence because sliders fire on every step.
  const dsp = $derived(dspStore.settings ?? config?.dsp ?? defaultDspSettings());

  $effect(() => () => dspStore.flush());

  function setDsp(next: DspSettings) {
    if (!config) return;
    dspStore.set(next, apply);
  }

  function setEq(patch: Partial<EqSettings>) {
    if (config) setDsp({ ...dsp, eq: { ...dsp.eq, ...patch } });
  }

  function setBand(index: number, db: number) {
    if (config) setDsp({ ...dsp, eq: withBand(dsp.eq, index, db) });
  }

  function setPreset(id: string) {
    const bands = presetBands(id);
    if (bands) setEq({ bands_db: bands });
  }

  function setReplayGain(patch: Partial<ReplayGainSettings>) {
    if (config) {
      setDsp({ ...dsp, replay_gain: { ...dsp.replay_gain, ...patch } });
    }
  }

  function setCrossfade(patch: Partial<CrossfadeSettings>) {
    if (config) {
      setDsp({ ...dsp, crossfade: { ...dsp.crossfade, ...patch } });
    }
  }

  function rangeValue(event: Event): number {
    return Number((event.target as HTMLInputElement).value);
  }

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

  function setStartInMini(enabled: boolean) {
    if (!config) return;
    config = { ...config, ui: { ...config.ui, start_in_mini: enabled } };
    api.setStartInMini(enabled).catch(apply);
  }

  function setOneko(enabled: boolean) {
    if (!config) return;
    const previous = config.ui.oneko;
    config = { ...config, ui: { ...config.ui, oneko: enabled } };
    appearance.setOneko(enabled).catch((e) => {
      if (config) config = { ...config, ui: { ...config.ui, oneko: previous } };
      apply(e);
    });
  }

  function setSmtc(enabled: boolean) {
    if (!config) return;
    void optimistic(
      config.integrations.smtc,
      enabled,
      (value) => {
        if (config) {
          config = { ...config, integrations: { ...config.integrations, smtc: value } };
        }
      },
      () => api.setSmtc(enabled),
      apply,
    );
  }

  function setCloseToTray(enabled: boolean) {
    if (!config) return;
    void optimistic(
      config.ui.close_to_tray,
      enabled,
      (value) => {
        if (config) config = { ...config, ui: { ...config.ui, close_to_tray: value } };
      },
      () => api.setCloseToTray(enabled),
      apply,
    );
  }

  function onHotkeyKeydown(event: KeyboardEvent) {
    const action = capture.recording;
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      capture.cancel();
      return;
    }
    if (event.key === "Backspace" || event.key === "Delete") {
      void capture.finish(action, "");
      return;
    }
    const accelerator = acceleratorFromEvent(event);
    if (!accelerator) return;
    const other = config ? conflictFor(config.hotkeys, action, accelerator) : null;
    if (other) {
      capture.cancel();
      hotkeyConflict = { action, accelerator, other };
      return;
    }
    void capture.finish(action, accelerator);
  }

  function hotkeyErrorMessage(failure: unknown, accelerator: string): string {
    const shown = displayAccelerator(accelerator);
    if (errorMessage(failure).includes("duplicates")) {
      return `"${shown}" is already in use by another shortcut.`;
    }
    return `Couldn't register "${shown}" — another app may be using it.`;
  }

  /// Optimistic update; passing `replace` clears that action's binding in the
  /// same mutation. The Rust command rolls back its own config on failure, so
  /// a failed save restores both shown bindings too.
  async function saveHotkey(
    action: HotkeyAction,
    accelerator: string,
    replace: HotkeyAction | null = null,
  ) {
    if (!config) return;
    const previousAction = config.hotkeys[action];
    const previousOther = replace ? config.hotkeys[replace] : null;
    config = {
      ...config,
      hotkeys: {
        ...config.hotkeys,
        [action]: accelerator,
        ...(replace ? { [replace]: "" } : {}),
      },
    };
    try {
      await api.setHotkey(action, accelerator, replace);
    } catch (e) {
      config = {
        ...config,
        hotkeys: {
          ...config.hotkeys,
          [action]: previousAction,
          ...(replace ? { [replace]: previousOther ?? "" } : {}),
        },
      };
      toasts.show(hotkeyErrorMessage(e, accelerator), { severity: "error" });
    }
  }

  function confirmReassign() {
    const conflict = hotkeyConflict;
    hotkeyConflict = null;
    if (!conflict) return;
    void saveHotkey(conflict.action, conflict.accelerator, conflict.other);
  }

  function cancelReassign() {
    hotkeyConflict = null;
  }

  function setDiscord(enabled: boolean) {
    if (!config) return;
    config = { ...config, integrations: { ...config.integrations, discord: enabled } };
    api.setDiscord(enabled).catch(apply);
  }

  function openCredit(url: string) {
    openUrl(url).catch(apply);
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

  function setHistoryEnabled(enabled: boolean) {
    if (!config) return;
    config = { ...config, history: { ...config.history, enabled } };
    api.setHistoryEnabled(enabled).catch(apply);
  }

  function applyBlockExplicit(enabled: boolean) {
    confirmingBlockExplicit = false;
    if (!config) return;
    const next = { ...config, content: { ...config.content, block_explicit: enabled } };
    config = next;
    api
      .setBlockExplicit(enabled)
      .then(async () => {
        trackFlags.setContentFlags(enabled, next.content.block_ai);
        if (enabled) await api.applyContentFilters("explicit");
      })
      .catch(apply);
  }

  function setBlockExplicit(enabled: boolean) {
    if (!config) return;
    if (enabled && !config.content.block_explicit) {
      confirmingBlockExplicit = true;
      return;
    }
    applyBlockExplicit(enabled);
  }

  function setBlockAi(enabled: boolean) {
    if (!config) return;
    const next = { ...config, content: { ...config.content, block_ai: enabled } };
    config = next;
    api
      .setBlockAi(enabled)
      .then(async () => {
        trackFlags.setContentFlags(next.content.block_explicit, enabled);
        if (enabled) await api.applyContentFilters("ai");
      })
      .catch(apply);
  }

  function setShowLabels(enabled: boolean) {
    if (!config) return;
    config = { ...config, content: { ...config.content, show_labels: enabled } };
    trackFlags.setShowLabels(enabled);
    api.setShowLabels(enabled).catch(apply);
  }

  function setHistoryMaxEntries(value: number) {
    if (!config) return;
    const max_entries = Math.min(100_000, Math.max(100, Math.round(value)));
    config = { ...config, history: { ...config.history, max_entries } };
    api.setHistoryMaxEntries(max_entries).catch(apply);
  }

  function clearHistory() {
    confirmingHistoryClear = false;
    api.clearHistory().catch(apply);
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
  <div class="tabs" role="tablist" aria-label="Settings categories">
    {#each SETTINGS_TABS as id (id)}
      <button
        class="tab"
        class:active={tab === id}
        id={`settings-tab-${id}`}
        role="tab"
        aria-selected={tab === id}
        aria-controls={`settings-panel-${id}`}
        tabindex={tab === id ? 0 : -1}
        onclick={() => (tab = id)}
        onkeydown={(event) => onTabKeydown(event, id)}
      >{TAB_LABELS[id]}</button>
    {/each}
  </div>

  {#if tab === "appearance"}
    <div
      id="settings-panel-appearance"
      role="tabpanel"
      aria-labelledby="settings-tab-appearance"
    >
      <h3>Appearance</h3>
      <div class="field">
        <span class="field-label">Theme:</span>
        <div class="segmented" role="group" aria-label="Theme">
          <button class:active={appearance.theme === "dark"} onclick={() => appearance.setTheme("dark").catch(apply)}>Dark</button>
          <button class:active={appearance.theme === "light"} onclick={() => appearance.setTheme("light").catch(apply)}>Light</button>
          <button class:active={appearance.theme === "system"} onclick={() => appearance.setTheme("system").catch(apply)}>System</button>
        </div>
      </div>
      {#if appearance.theme === "system"}
        <p class="muted">Follows the operating system's app theme.</p>
      {/if}

      <div class="field">
        <span class="field-label">Mode:</span>
        <div class="segmented" role="group" aria-label="Mode">
          <button class:active={appearance.mode === "atmosphere"} onclick={() => appearance.setMode("atmosphere").catch(apply)}>Atmosphere</button>
          <button class:active={appearance.mode === "instrument"} onclick={() => appearance.setMode("instrument").catch(apply)}>Instrument</button>
          <button class:active={appearance.mode === "rack"} onclick={() => appearance.setMode("rack").catch(apply)}>Hi-Fi Rack</button>
        </div>
      </div>
      {#if appearance.mode === "instrument"}
        <p class="muted">Instrument minimizes animation and visual effects.</p>
      {:else if appearance.mode === "rack"}
        <p class="muted">Hi-Fi Rack swaps the sidebar and player bar for a component stack. It is always dark, whatever the theme.</p>
      {/if}

      {#if appearance.mode === "rack"}
        <h3>Rack</h3>
        <div class="field">
          <span class="field-label">Deck face:</span>
          <div class="segmented" role="group" aria-label="Deck face">
            <button class:active={appearance.rack.deck_face === "transport"} onclick={() => appearance.setRack({ deck_face: "transport" }).catch(apply)}>Transport</button>
            <button class:active={appearance.rack.deck_face === "cd"} onclick={() => appearance.setRack({ deck_face: "cd" }).catch(apply)}>CD player</button>
            <button class:active={appearance.rack.deck_face === "display"} onclick={() => appearance.setRack({ deck_face: "display" }).catch(apply)}>Display</button>
          </div>
        </div>
        <p class="muted">What the compact deck shows. Now Playing always opens the turntable.</p>
        <div class="field">
          <span class="field-label">Amp meters:</span>
          <div class="segmented" role="group" aria-label="Amp meters">
            <button class:active={appearance.rack.meters === "needles"} onclick={() => appearance.setRack({ meters: "needles" }).catch(apply)}>Needles</button>
            <button class:active={appearance.rack.meters === "bars"} onclick={() => appearance.setRack({ meters: "bars" }).catch(apply)}>Bars</button>
            <button class:active={appearance.rack.meters === "led"} onclick={() => appearance.setRack({ meters: "led" }).catch(apply)}>LED</button>
            <button class:active={appearance.rack.meters === "spectrum"} onclick={() => appearance.setRack({ meters: "spectrum" }).catch(apply)}>Spectrum</button>
          </div>
        </div>
        <div class="field">
          <span class="field-label">Now Playing analyzer:</span>
          <div class="segmented" role="group" aria-label="Now Playing analyzer">
            <button class:active={appearance.rack.analyzer === "auto"} onclick={() => appearance.setRack({ analyzer: "auto" }).catch(apply)}>Auto</button>
            <button class:active={appearance.rack.analyzer === "spectrum"} onclick={() => appearance.setRack({ analyzer: "spectrum" }).catch(apply)}>Spectrum</button>
            <button class:active={appearance.rack.analyzer === "recorder"} onclick={() => appearance.setRack({ analyzer: "recorder" }).catch(apply)}>Level recorder</button>
          </div>
        </div>
        <p class="muted">Auto shows the level recorder when the amp's meters are already a spectrum.</p>
        <div class="field">
          <span class="field-label">Lamp color:</span>
          <div class="segmented" role="group" aria-label="Lamp color">
            {#each [["amber", "Amber"], ["blue", "Blue"], ["green", "Green"], ["red", "Red"], ["white", "White"], ["cover", "Cover"]] as const as [id, label] (id)}
              <button class:active={appearance.rack.lamp === id} onclick={() => appearance.setRack({ lamp: id }).catch(apply)}>{label}</button>
            {/each}
          </div>
        </div>
        <p class="muted">Cover takes the hue of the album art but keeps the brightness of a lit screen; grey covers fall back to amber.</p>
        <div class="field">
          <label>
            <input
              type="checkbox"
              checked={appearance.rack.eq_unit}
              onchange={(e) => appearance.setRack({ eq_unit: (e.target as HTMLInputElement).checked }).catch(apply)}
            />
            Show the graphic EQ unit
          </label>
        </div>
        <label class="db-row">
          <span>Lamp brightness</span>
          <input
            type="range"
            min="40"
            max="100"
            step="5"
            value={appearance.rack.lamp_brightness}
            aria-valuetext={`${appearance.rack.lamp_brightness} percent`}
            onchange={(e) => appearance.setRack({ lamp_brightness: Number((e.target as HTMLInputElement).value) }).catch(apply)}
          />
          <output>{appearance.rack.lamp_brightness}%</output>
        </label>
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

      <div class="field">
        <span class="field-label">Mini player:</span>
        <label>
          <input
            type="checkbox"
            checked={config.ui.start_in_mini}
            onchange={(e) => setStartInMini((e.target as HTMLInputElement).checked)}
          />
          Start in mini player
        </label>
      </div>

      <div class="field">
        <label>
          <input
            type="checkbox"
            checked={config.ui.oneko}
            onchange={(e) => setOneko((e.target as HTMLInputElement).checked)}
          />
          Cat
        </label>
      </div>
      {#if appearance.reducedMotion}
        <p class="muted">Paused while your system's reduced-motion setting is on.</p>
      {/if}
      <p class="muted">
        Credits:
        <button class="link" type="button" onclick={() => openCredit("https://github.com/adryd325/oneko.js")}>oneko.js by adryd325</button>
        &amp;
        <button class="link" type="button" onclick={() => openCredit("https://github.com/kyrie25/spicetify-oneko")}>spicetify-oneko by kyrie25</button>
      </p>
    </div>
  {:else if tab === "playback"}
    <div id="settings-panel-playback" role="tabpanel" aria-labelledby="settings-tab-playback">
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
      {#if player.state.bit_perfect_notice}
        <p style="color:var(--warning)">{player.state.bit_perfect_notice}</p>
      {/if}

      <h3>Sound</h3>
      <p class="muted">
        Off by default. Any active processing changes the samples, so bit-perfect output is
        off while it is in effect{config.audio.exclusive
          ? "; exclusive mode keeps the device to itself"
          : ""}. Settings that leave the sound untouched keep bit-perfect.
      </p>
      {#if player.state.dsp_active}
        <p class="muted">Active now: {player.state.dsp_stages.join(", ")}</p>
      {/if}

      <div class="field">
        <span class="field-label">ReplayGain:</span>
        <div class="segmented" role="group" aria-label="ReplayGain">
          <button aria-pressed={dsp.replay_gain.mode === "off"} class:active={dsp.replay_gain.mode === "off"} onclick={() => setReplayGain({ mode: "off" })}>Off</button>
          <button aria-pressed={dsp.replay_gain.mode === "track"} class:active={dsp.replay_gain.mode === "track"} onclick={() => setReplayGain({ mode: "track" })}>Track</button>
          <button aria-pressed={dsp.replay_gain.mode === "album"} class:active={dsp.replay_gain.mode === "album"} onclick={() => setReplayGain({ mode: "album" })}>Album</button>
        </div>
      </div>
      {#if dsp.replay_gain.mode !== "off"}
        <p class="muted">
          Levels loudness with TIDAL's ReplayGain data. Album mode keeps the dynamics between
          tracks of one album and falls back to the track gain.
        </p>
        <label class="db-row">
          <span>Preamp</span>
          <input
            type="range"
            min={-REPLAY_GAIN_MAX_DB}
            max={REPLAY_GAIN_MAX_DB}
            step={DB_STEP}
            value={dsp.replay_gain.preamp_db}
            aria-valuetext={formatDb(dsp.replay_gain.preamp_db)}
            oninput={(e) => setReplayGain({ preamp_db: clampDb(rangeValue(e), REPLAY_GAIN_MAX_DB) })}
          />
          <output>{formatDb(dsp.replay_gain.preamp_db)}</output>
        </label>
        <label class="db-row">
          <span>Tracks without data</span>
          <input
            type="range"
            min={-REPLAY_GAIN_MAX_DB}
            max={REPLAY_GAIN_MAX_DB}
            step={DB_STEP}
            value={dsp.replay_gain.untagged_db}
            aria-valuetext={formatDb(dsp.replay_gain.untagged_db)}
            oninput={(e) => setReplayGain({ untagged_db: clampDb(rangeValue(e), REPLAY_GAIN_MAX_DB) })}
          />
          <output>{formatDb(dsp.replay_gain.untagged_db)}</output>
        </label>
        <p>
          <label>
            <input
              type="checkbox"
              checked={dsp.replay_gain.prevent_clipping}
              onchange={(e) => setReplayGain({ prevent_clipping: (e.target as HTMLInputElement).checked })}
            />
            Prevent clipping (never raise a track past its peak)
          </label>
        </p>
        {#if player.state.stream?.replay_gain_db != null}
          <p class="muted">Current track: {formatDb(Math.round(player.state.stream.replay_gain_db * 10) / 10)}</p>
        {/if}
      {/if}

      <p>
        <label>
          <input
            type="checkbox"
            checked={dsp.eq.enabled}
            onchange={(e) => setEq({ enabled: (e.target as HTMLInputElement).checked })}
          />
          Equalizer
        </label>
      </p>
      {#if dsp.eq.enabled}
        <div class="field">
          <label>
            Preset:
            <select
              value={matchPreset(dsp.eq.bands_db)}
              onchange={(e) => setPreset((e.target as HTMLSelectElement).value)}
            >
              {#each EQ_PRESETS as preset (preset.id)}
                <option value={preset.id}>{preset.label}</option>
              {/each}
              <option value="custom" disabled>Custom</option>
            </select>
          </label>
        </div>
        <div class="eq" role="group" aria-label="Equalizer bands">
          {#each EQ_BANDS_HZ as hz, index (hz)}
            <label class="db-row">
              <span>{bandLabel(hz)} Hz</span>
              <input
                type="range"
                min={-EQ_MAX_DB}
                max={EQ_MAX_DB}
                step={DB_STEP}
                value={dsp.eq.bands_db[index] ?? 0}
                aria-valuetext={formatDb(dsp.eq.bands_db[index] ?? 0)}
                oninput={(e) => setBand(index, rangeValue(e))}
              />
              <output>{formatDb(dsp.eq.bands_db[index] ?? 0)}</output>
            </label>
          {/each}
        </div>
        <p>
          <label>
            <input
              type="checkbox"
              checked={dsp.eq.auto_preamp}
              onchange={(e) => setEq({ auto_preamp: (e.target as HTMLInputElement).checked })}
            />
            Automatic preamp (lowers the level by the largest boost so it cannot clip)
          </label>
        </p>
        {#if dsp.eq.auto_preamp}
          <p class="muted">Preamp: {formatDb(effectivePreamp(dsp.eq))}</p>
        {:else}
          <label class="db-row">
            <span>Preamp</span>
            <input
              type="range"
              min={-EQ_MAX_DB}
              max={EQ_MAX_DB}
              step={DB_STEP}
              value={dsp.eq.preamp_db}
              aria-valuetext={formatDb(dsp.eq.preamp_db)}
              oninput={(e) => setEq({ preamp_db: clampDb(rangeValue(e), EQ_MAX_DB) })}
            />
            <output>{formatDb(dsp.eq.preamp_db)}</output>
          </label>
        {/if}
      {/if}

      <label class="db-row">
        <span>Crossfade</span>
        <input
          type="range"
          min="0"
          max={CROSSFADE_MAX_MS}
          step="500"
          value={dsp.crossfade.duration_ms}
          aria-valuetext={formatCrossfade(dsp.crossfade.duration_ms)}
          oninput={(e) => setCrossfade({ duration_ms: rangeValue(e) })}
        />
        <output>{formatCrossfade(dsp.crossfade.duration_ms)}</output>
      </label>
      {#if dsp.crossfade.duration_ms > 0}
        <p>
          <label>
            <input
              type="checkbox"
              checked={dsp.crossfade.skip_same_album}
              onchange={(e) => setCrossfade({ skip_same_album: (e.target as HTMLInputElement).checked })}
            />
            Keep albums gapless (no crossfade between consecutive tracks of one album)
          </label>
        </p>
        <p class="muted">
          Crossfades when a track ends on its own; skipping and seeking cut straight over.
          Tracks with different sample rates fade out instead of overlapping.
        </p>
      {/if}

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

      <h3>Quality display</h3>
      <select
        aria-label="Quality display"
        value={config.ui.quality_display}
        onchange={(e) => setDisplay((e.target as HTMLSelectElement).value as QualityDisplay)}
      >
        <option value="compact">Compact (MAX / LOSSLESS / AAC)</option>
        <option value="detailed">Detailed (bit depth / rate / bitrate)</option>
      </select>
    </div>
  {:else if tab === "content"}
    <div id="settings-panel-content" role="tabpanel" aria-labelledby="settings-tab-content">
      <h3>Content filters</h3>
      <p class="muted">
        OxiTide enforces these client-side: TIDAL does not expose its own filters to
        third-party clients, and blocked tracks can still appear in TIDAL recommendations.
        Flagged rows are grayed out and skipped; unknown flags always play.
      </p>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.content.block_explicit}
              onchange={(e) => setBlockExplicit((e.target as HTMLInputElement).checked)}
            />
            <span>Block explicit content</span>
          </label>
          <span class="muted">
            Hides explicit tracks and blocks them from playing. Enabling clears the current
            queue, matching the official app.
          </span>
        </div>
      </div>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.content.block_ai}
              onchange={(e) => setBlockAi((e.target as HTMLInputElement).checked)}
            />
            <span>Block AI-generated content</span>
          </label>
          <span class="muted">
            Hides tracks TIDAL labels as AI-generated and removes them from the queue.
            Labels may be missing for some tracks or regions; those play normally.
          </span>
        </div>
      </div>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.content.show_labels}
              onchange={(e) => setShowLabels((e.target as HTMLInputElement).checked)}
            />
            <span>Show content labels</span>
          </label>
          <span class="muted">
            Tags explicit tracks with <strong>E</strong> and AI-generated tracks with
            <strong>AI</strong>, even when not blocking them. Turning this off stops the
            background label lookups too.
          </span>
        </div>
      </div>
      {#if confirmingBlockExplicit}
        <ConfirmDialog
          title="Block explicit content"
          message="This clears the current queue, matching the TIDAL app. Playback will stop."
          onconfirm={() => applyBlockExplicit(true)}
          oncancel={() => (confirmingBlockExplicit = false)}
        />
      {/if}
    </div>
  {:else if tab === "connections"}
    <div
      id="settings-panel-connections"
      role="tabpanel"
      aria-labelledby="settings-tab-connections"
    >
      <p class="muted">Scrobbling, media controls, and Discord Rich Presence are configurable below.</p>
      <h3>Scrobbling</h3>
      {#if scrobble}
        <div class="scrobble">
          <div class="scrobble-main">
            <label class="scrobble-head">
              <input
                type="checkbox"
                checked={scrobble.lastfm.enabled}
                disabled={!canToggle(scrobble.lastfm)}
                onchange={(e) => toggleService("lastfm", (e.target as HTMLInputElement).checked)}
              />
              <span>Last.fm</span>
            </label>
            <span class="muted">{serviceLabel(scrobble.lastfm)}</span>
            {#if queuedLabel(scrobble.lastfm.queued)}
              <span class="muted">{queuedLabel(scrobble.lastfm.queued)}</span>
            {/if}
            {#if scrobble.lastfm.last_error}
              <span class="muted">{scrobble.lastfm.last_error}</span>
            {/if}
            {#if lastfmAuthNote}<span class="muted">{lastfmAuthNote}</span>{/if}
          </div>
          {#if lastfmAuth === "waiting"}
            <div class="scrobble-actions">
              <Loading label="Waiting for authorization in your browser…" />
              <button onclick={() => void pollLastfm()}>I've authorized</button>
              <button onclick={cancelLastfm}>Cancel</button>
            </div>
          {:else if scrobble.lastfm.connected}
            <button onclick={disconnectLastfm}>Disconnect</button>
          {:else}
            <button disabled={!scrobble.lastfm.configured} onclick={() => void connectLastfm()}>
              Connect
            </button>
          {/if}
        </div>

        <div class="scrobble">
          <div class="scrobble-main">
            <label class="scrobble-head">
              <input
                type="checkbox"
                checked={scrobble.listenbrainz.enabled}
                disabled={!canToggle(scrobble.listenbrainz)}
                onchange={(e) =>
                  toggleService("listenbrainz", (e.target as HTMLInputElement).checked)}
              />
              <span>ListenBrainz</span>
            </label>
            <span class="muted">{serviceLabel(scrobble.listenbrainz)}</span>
            {#if queuedLabel(scrobble.listenbrainz.queued)}
              <span class="muted">{queuedLabel(scrobble.listenbrainz.queued)}</span>
            {/if}
            {#if scrobble.listenbrainz.last_error}
              <span class="muted">{scrobble.listenbrainz.last_error}</span>
            {/if}
            {#if lbError}<span class="muted">{lbError}</span>{/if}
          </div>
          {#if scrobble.listenbrainz.connected}
            <button onclick={disconnectListenbrainz}>Disconnect</button>
          {:else}
            <input type="password" placeholder="ListenBrainz token" bind:value={lbToken} />
            <button disabled={lbBusy || !lbToken.trim()} onclick={() => void connectListenbrainz()}>
              Connect
            </button>
          {/if}
        </div>
      {:else}
        <p class="muted">Loading scrobbling status…</p>
      {/if}
      <p class="muted">
        Get a ListenBrainz token from
        <button
          class="link"
          type="button"
          onclick={() => openCredit("https://listenbrainz.org/profile/")}
        >your profile</button>.
      </p>
      <h3>Presence & controls</h3>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.integrations.discord}
              onchange={(e) => setDiscord((e.target as HTMLInputElement).checked)}
            />
            <span>Discord Rich Presence</span>
          </label>
          <span class="muted">
            Shows the current track on your Discord profile. Requires the Discord desktop client.
          </span>
        </div>
      </div>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.integrations.smtc}
              onchange={(e) => setSmtc((e.target as HTMLInputElement).checked)}
            />
            <span>Media controls</span>
          </label>
          <span class="muted">Hardware media keys and the system media overlay.</span>
        </div>
      </div>
    </div>
  {:else if tab === "storage"}
    <div id="settings-panel-storage" role="tabpanel" aria-labelledby="settings-tab-storage">
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
      <h3>Listening history</h3>
      <p class="muted">Recorded locally in history.jsonl and used by the History view and stats.</p>
      <p>
        <label>
          <input
            type="checkbox"
            checked={config.history.enabled}
            onchange={(e) => setHistoryEnabled((e.target as HTMLInputElement).checked)}
          />
          Record listening history
        </label>
      </p>
      <p>
        <label>
          Max entries:
          <input
            type="number"
            min="100"
            max="100000"
            value={config.history.max_entries}
            onchange={(e) => setHistoryMaxEntries(Number((e.target as HTMLInputElement).value))}
          />
        </label>
      </p>
      <p><button onclick={() => (confirmingHistoryClear = true)}>Clear history</button></p>
      {#if confirmingHistoryClear}
        <ConfirmDialog
          title="Clear listening history"
          message="This permanently deletes your local play history and stats."
          onconfirm={clearHistory}
          oncancel={() => (confirmingHistoryClear = false)}
        />
      {/if}
    </div>
  {:else}
    <div id="settings-panel-system" role="tabpanel" aria-labelledby="settings-tab-system">
      <h3>Account</h3>
      <KeyringNotice />
      <p>
        <button class="logout" onclick={onLogout}>
          <Icon name="log-out" size={16} />
          Log out
        </button>
      </p>
      <h3>Desktop</h3>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.ui.close_to_tray}
              onchange={(e) => setCloseToTray((e.target as HTMLInputElement).checked)}
            />
            <span>Close to tray</span>
          </label>
          <span class="muted">
            Closing the window hides it and keeps playback running; Quit lives in
            the tray menu.
          </span>
        </div>
      </div>

      <h3>Global hotkeys</h3>
      <p class="muted">
        Work while OxiTide is in the background. Record a shortcut, or press
        Backspace/Delete while recording to clear it. A shortcut already used by
        another action asks whether to reassign it; one another app owns shows a
        message and keeps the old binding.
      </p>
      {#each HOTKEY_ACTIONS as entry (entry.id)}
        <div class="hotkey-row">
          <span class="hotkey-label">{entry.label}</span>
          <kbd>{displayAccelerator(config.hotkeys[entry.id]) || "Not set"}</kbd>
          <button
            id={`hotkey-${entry.id}`}
            class:recording={capture.recording === entry.id}
            onclick={() => void capture.start(entry.id)}
          >{capture.recording === entry.id ? "Press keys…" : "Record"}</button>
          {#if config.hotkeys[entry.id]}
            <button onclick={() => void saveHotkey(entry.id, "")}>Clear</button>
          {/if}
        </div>
      {/each}
      {#if hotkeyConflict}
        <ConfirmDialog
          title="Shortcut already in use"
          message={`"${displayAccelerator(hotkeyConflict.accelerator)}" is already used by "${actionLabel(hotkeyConflict.other)}". Reassign it to "${actionLabel(hotkeyConflict.action)}"?`}
          confirmLabel="Reassign"
          onconfirm={confirmReassign}
          oncancel={cancelReassign}
        />
      {/if}
      <h3>Keyboard shortcuts</h3>
      <ul>
        {#each SHORTCUTS as shortcut (shortcut.keys)}
          <li><kbd>{shortcut.keys}</kbd> <span class="muted">{shortcut.description}</span></li>
        {/each}
      </ul>
      <h3>Logs</h3>
      <p class="muted">Directory: {logsDir || "unavailable"}</p>
      <p><button onclick={() => api.openLogsFolder().catch(apply)}>Open logs folder</button></p>
    </div>
  {/if}
{/if}

<style>
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--line);
    margin: 8px 0 16px;
  }
  .tab {
    margin-bottom: -1px;
    padding: 8px 12px;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
  }
  .tab:hover,
  .tab.active {
    color: var(--text);
  }
  .tab.active {
    border-bottom-color: var(--accent);
  }
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
  .planned {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 8px 0;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  .planned-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .planned-head {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
  }
  .scrobble {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 8px 0;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  .scrobble-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .scrobble-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .scrobble-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
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
  .link {
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }
  .logout {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 9px 18px;
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    color: var(--danger);
    font-weight: 600;
    transition:
      background var(--dur-state) var(--ease-in),
      border-color var(--dur-state) var(--ease-in),
      box-shadow var(--dur-state) var(--ease-in),
      transform var(--dur-micro) var(--ease-in);
  }
  .logout:hover {
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    border-color: var(--danger);
    box-shadow: 0 0 9px color-mix(in srgb, var(--danger) 80%, transparent);
  }
  .logout:active {
    transform: scale(0.97);
  }
  .db-row {
    display: grid;
    grid-template-columns: 150px minmax(120px, 280px) 72px;
    align-items: center;
    gap: 12px;
    margin: 4px 0;
  }
  .db-row output {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .eq {
    margin: 4px 0 8px;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  .hotkey-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 6px 0;
  }
  .hotkey-label {
    min-width: 150px;
  }
  .hotkey-row kbd {
    min-width: 140px;
    color: var(--text-muted);
  }
  .hotkey-row button.recording {
    border: 1px solid var(--accent);
    color: var(--accent);
  }
</style>
