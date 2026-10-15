<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { listen } from "@tauri-apps/api/event";
  import { tick } from "svelte";
  import {
    api,
    SCROBBLE_STATUS_EVENT,
    type AnalyzerStyle,
    type CacheStatus,
    type Config,
    type CrossfadeSettings,
    type DeckFace,
    type DspSettings,
    type EqSettings,
    type LampColor,
    type MeterStyle,
    type OutputDevice,
    type Quality,
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
  import RemoteSettings from "../lib/components/RemoteSettings.svelte";
  import UpdatesSection from "../lib/components/UpdatesSection.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import AutoMixPanel from "../lib/components/settings/AutoMixPanel.svelte";
  import Button from "../lib/components/settings/Button.svelte";
  import EqBands from "../lib/components/settings/EqBands.svelte";
  import NumberField from "../lib/components/settings/NumberField.svelte";
  import Notice from "../lib/components/settings/Notice.svelte";
  import Segmented from "../lib/components/settings/Segmented.svelte";
  import Select from "../lib/components/settings/Select.svelte";
  import SettingRow from "../lib/components/settings/SettingRow.svelte";
  import SettingsSection from "../lib/components/settings/SettingsSection.svelte";
  import SettingsSearch from "../lib/components/settings/SettingsSearch.svelte";
  import Slider from "../lib/components/settings/Slider.svelte";
  import Toggle from "../lib/components/settings/Toggle.svelte";
  import ToggleRow from "../lib/components/settings/ToggleRow.svelte";
  import {
    appearance,
    clampLampBrightness,
    fontStack,
    type Density,
    type DynamicColorSetting,
    type CoverShape,
    type TypefaceSetting,
    type UiMode,
  } from "../lib/appearance.svelte";
  import type { ThemeSetting } from "../lib/theme";
  import {
    CROSSFADE_MAX_MS,
    DB_STEP,
    EQ_MAX_DB,
    EQ_PRESETS,
    REPLAY_GAIN_MAX_DB,
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
  import type { SettingEntry } from "../lib/settingsIndex";
  import { SETTINGS_TABS, nextTab, type SettingsTab } from "../lib/settingsTabs";
  import { TAB_META, formatSize, usageFraction } from "../lib/settingsView";
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

  // Option lists. Typed here so each control's value type is the setting's own
  // union rather than a widened string.
  const THEME_OPTIONS: { value: ThemeSetting; label: string; icon: "moon" | "sun" | "monitor" }[] = [
    { value: "dark", label: "Dark", icon: "moon" },
    { value: "light", label: "Light", icon: "sun" },
    { value: "system", label: "System", icon: "monitor" },
  ];
  const MODE_OPTIONS: { value: UiMode; label: string }[] = [
    { value: "atmosphere", label: "Atmosphere" },
    { value: "instrument", label: "Instrument" },
    { value: "rack", label: "Hi-Fi Rack" },
  ];
  const MODE_HELP: Record<UiMode, string> = {
    atmosphere: "Glass over the album art, with motion that follows the music.",
    instrument: "Minimal animation and visual effects.",
    rack: "Swaps the sidebar and player bar for a component stack. Always dark, whatever the theme.",
  };
  const DYNAMIC_OPTIONS: { value: DynamicColorSetting; label: string }[] = [
    { value: "auto", label: "Default" },
    { value: "full", label: "Full" },
    { value: "accent", label: "Accent-only" },
    { value: "off", label: "Off" },
  ];
  const TYPEFACE_OPTIONS: { value: TypefaceSetting; label: string; hint?: string }[] = [
    { value: "auto", label: "Default", hint: "Chosen by the mode" },
    { value: "manrope", label: "Manrope" },
    { value: "inter", label: "Inter" },
    { value: "system", label: "System", hint: "Your operating system's UI font" },
    { value: "custom", label: "Custom…", hint: "Any installed font" },
  ];
  const DENSITY_OPTIONS: { value: Density; label: string }[] = [
    { value: "comfortable", label: "Comfortable" },
    { value: "compact", label: "Compact" },
  ];
  const COVER_OPTIONS: { value: CoverShape; label: string }[] = [
    { value: "disc", label: "Disc" },
    { value: "sleeve", label: "Sleeve" },
  ];
  const DECK_OPTIONS: { value: DeckFace; label: string }[] = [
    { value: "transport", label: "Transport" },
    { value: "cd", label: "CD player" },
    { value: "display", label: "Display" },
  ];
  const METER_OPTIONS: { value: MeterStyle; label: string }[] = [
    { value: "needles", label: "Needles" },
    { value: "bars", label: "Bars" },
    { value: "led", label: "LED" },
    { value: "spectrum", label: "Spectrum" },
  ];
  const ANALYZER_OPTIONS: { value: AnalyzerStyle; label: string }[] = [
    { value: "auto", label: "Auto" },
    { value: "spectrum", label: "Spectrum" },
    { value: "recorder", label: "Recorder" },
    { value: "tuner", label: "Tuner" },
    { value: "eq", label: "EQ" },
  ];
  const LAMP_OPTIONS: { value: LampColor; label: string }[] = [
    { value: "amber", label: "Amber" },
    { value: "blue", label: "Blue" },
    { value: "green", label: "Green" },
    { value: "red", label: "Red" },
    { value: "white", label: "White" },
    { value: "cover", label: "Cover" },
  ];
  const QUALITY_OPTIONS: { value: Quality; label: string; hint: string }[] = [
    { value: "high", label: "High", hint: "AAC 320 kbps" },
    { value: "lossless", label: "Lossless", hint: "CD quality" },
    { value: "max", label: "Max (Hi-Res)", hint: "Highest available, up to 24-bit" },
  ];
  const QUALITY_DISPLAY_OPTIONS: { value: QualityDisplay; label: string }[] = [
    { value: "compact", label: "Compact" },
    { value: "detailed", label: "Detailed" },
  ];
  const REPLAY_GAIN_OPTIONS: { value: ReplayGainSettings["mode"]; label: string }[] = [
    { value: "off", label: "Off" },
    { value: "track", label: "Track" },
    { value: "album", label: "Album" },
  ];
  const FALLBACK_OPTIONS: { value: Config["audio"]["exclusive_fallback"]; label: string }[] = [
    { value: "shared", label: "Use shared output" },
    { value: "warn_only", label: "Stop with a message" },
  ];

  const deviceOptions = $derived([
    { value: "", label: "System default" },
    ...devices.map((d) => ({ value: d.id, label: `${d.name}${d.is_default ? " (default)" : ""}` })),
  ]);
  const presetOptions = $derived([
    ...EQ_PRESETS.map((p) => ({ value: p.id, label: p.label })),
    { value: "custom", label: "Custom", disabled: true },
  ]);

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

  /// A vertical tab list: Up/Down (and Left/Right, for the stacked narrow
  /// layout) move between categories. Handled keys never reach the global
  /// transport shortcuts, which would otherwise seek or change the volume.
  function onTabKeydown(event: KeyboardEvent, id: SettingsTab) {
    let target: SettingsTab | null = null;
    if (event.key === "ArrowDown" || event.key === "ArrowRight") target = nextTab(id, 1);
    else if (event.key === "ArrowUp" || event.key === "ArrowLeft") target = nextTab(id, -1);
    else if (event.key === "Home") target = SETTINGS_TABS[0];
    else if (event.key === "End") target = SETTINGS_TABS[SETTINGS_TABS.length - 1];
    if (!target) return;
    event.preventDefault();
    event.stopPropagation();
    tab = target;
    document.getElementById(`settings-tab-${target}`)?.focus();
  }

  /// Jump from a search result: open its tab, scroll the row to the middle of
  /// the page, ring it briefly and put focus on its control. When the tab
  /// changes the cards are still rising in, so the scroll waits for them.
  async function jumpTo(entry: SettingEntry) {
    const changed = tab !== entry.tab;
    tab = entry.tab;
    await tick();
    setTimeout(() => {
      const el = document.querySelector<HTMLElement>(`[data-setting="${entry.id}"]`);
      if (!el) return;
      el.scrollIntoView({ block: "center", behavior: appearance.reducedMotion ? "auto" : "smooth" });
      el.removeAttribute("data-found");
      void el.offsetWidth; // restart the ring if it is already showing
      el.setAttribute("data-found", "");
      setTimeout(() => el.removeAttribute("data-found"), 1700);
      el.querySelector<HTMLElement>("input, button, select, [role=slider]")?.focus({ preventScroll: true });
    }, changed ? 140 : 0);
  }

  // The active-tab highlight is one element that glides between tabs. It is
  // measured from the active tab's box (so it also fits the wrapped/scrolling
  // narrow nav) and only its transform animates; the first placement is
  // instant so it never swoops in from the corner.
  let nav = $state<HTMLDivElement | null>(null);
  let pill = $state({ x: 0, y: 0, w: 0, h: 0 });
  let pillReady = $state(false);

  function placePill() {
    const el = document.getElementById(`settings-tab-${tab}`);
    if (!el) return;
    pill = { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight };
  }

  $effect(() => {
    if (!nav) return;
    placePill();
    const frame = requestAnimationFrame(() => (pillReady = true));
    const observer = new ResizeObserver(placePill);
    observer.observe(nav);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  });

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

  function setHistoryMaxEntries(max_entries: number) {
    if (!config) return;
    config = { ...config, history: { ...config.history, max_entries } };
    api.setHistoryMaxEntries(max_entries).catch(apply);
  }

  function clearHistory() {
    confirmingHistoryClear = false;
    api.clearHistory().catch(apply);
  }

  const cacheUsed = $derived(cacheStatus ? cacheStatus.used_bytes : 0);
  const cacheFraction = $derived(cacheStatus ? usageFraction(cacheUsed, cacheStatus.max_bytes) : 0);
</script>

<div class="settings">
  <header class="page-head">
    <h2>Settings</h2>
    {#if config}
      <SettingsSearch
        scope={{
          rack: appearance.mode === "rack",
          exclusive: config.audio.exclusive || config.audio.output_device.startsWith("asio:"),
        }}
        onselect={jumpTo}
      />
    {/if}
  </header>

  {#if !config}
    {#if error}
      <ErrorState message={error} onRetry={load} />
    {:else}
      <Loading />
    {/if}
  {:else}
    <div class="layout">
      <div
        bind:this={nav}
        class="nav"
        role="tablist"
        aria-orientation="vertical"
        aria-label="Settings categories"
      >
        <span
          class="tab-indicator"
          class:ready={pillReady}
          aria-hidden="true"
          style:width="{pill.w}px"
          style:height="{pill.h}px"
          style:transform="translate({pill.x}px, {pill.y}px)"
        ></span>
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
          >
            <Icon name={TAB_META[id].icon} size={17} />
            <span>{TAB_META[id].label}</span>
          </button>
        {/each}
      </div>

      <div class="content">
        {#if tab === "appearance"}
          <div id="settings-panel-appearance" role="tabpanel" aria-labelledby="settings-tab-appearance">
            <div class="cols">
            <div class="col">
            <SettingsSection title="Look & feel" icon="palette">
              <SettingRow
                id="theme" label="Theme"
                description={appearance.theme === "system"
                  ? "Follows the operating system's app theme."
                  : "Light or dark surfaces."}
              >
                <Segmented
                  label="Theme"
                  value={appearance.theme}
                  options={THEME_OPTIONS}
                  onchange={(value) => appearance.setTheme(value).catch(apply)}
                />
              </SettingRow>
              <SettingRow id="mode" label="Mode" description={MODE_HELP[appearance.mode]}>
                <Segmented
                  label="Mode"
                  value={appearance.mode}
                  options={MODE_OPTIONS}
                  onchange={(value) => appearance.setMode(value).catch(apply)}
                />
              </SettingRow>
              {#if appearance.mode !== "rack"}
                <SettingRow
                  id="dynamic-color" label="Dynamic color"
                  description="Tints the interface with the colors of the album art. Default follows the mode."
                >
                  <Segmented
                    label="Dynamic color"
                    value={config.ui.dynamic_color}
                    options={DYNAMIC_OPTIONS}
                    onchange={setDynamicColor}
                  />
                </SettingRow>
              {/if}
            </SettingsSection>

            </div>

            {#if appearance.mode === "rack"}
              <div class="col wide" style="order: 3">
              <SettingsSection
                title="Hi-Fi Rack"
                description="The component stack's units, meters and lamp."
                icon="audio-lines"
              >
                <SettingRow
                  id="rack-deck" label="Deck face"
                  description="What the compact deck shows. Now Playing always opens the turntable."
                >
                  <Segmented
                    label="Deck face"
                    value={appearance.rack.deck_face}
                    options={DECK_OPTIONS}
                    onchange={(deck_face) => appearance.setRack({ deck_face }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow id="rack-meters" label="Amp meters">
                  <Segmented
                    label="Amp meters"
                    value={appearance.rack.meters}
                    options={METER_OPTIONS}
                    onchange={(meters) => appearance.setRack({ meters }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow
                  id="rack-analyzer" label="Now Playing analyzer"
                  description="The extra unit in Now Playing. Auto shows the level recorder when the amp's meters are already a spectrum, otherwise the spectrum."
                >
                  <Segmented
                    label="Now Playing analyzer"
                    value={appearance.rack.analyzer}
                    options={ANALYZER_OPTIONS}
                    onchange={(analyzer) => appearance.setRack({ analyzer }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow
                  id="rack-lamp" label="Lamp color"
                  description="Cover takes the hue of the album art but keeps the brightness of a lit screen; grey covers fall back to amber."
                >
                  <Segmented
                    label="Lamp color"
                    value={appearance.rack.lamp}
                    options={LAMP_OPTIONS}
                    onchange={(lamp) => appearance.setRack({ lamp }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow id="rack-brightness" label="Lamp brightness">
                  <Slider
                    label="Lamp brightness"
                    min={40}
                    max={100}
                    step={5}
                    value={appearance.rack.lamp_brightness}
                    format={(v) => `${v}%`}
                    oncommit={(v) => appearance.setRack({ lamp_brightness: clampLampBrightness(v) }).catch(apply)}
                  />
                </SettingRow>
                <ToggleRow
                  id="rack-eq" label="Show the graphic EQ unit"
                  checked={appearance.rack.eq_unit}
                  onchange={(eq_unit) => appearance.setRack({ eq_unit }).catch(apply)}
                />
              </SettingsSection>
              </div>
            {/if}

            <div class="col" style="order: 2">
            <SettingsSection title="Typography & layout" icon="type">
              <SettingRow id="typeface" label="Typeface" description="The font used across the interface.">
                <Select
                  label="Typeface"
                  value={appearance.typeface}
                  options={TYPEFACE_OPTIONS}
                  onchange={setTypeface}
                />
              </SettingRow>
              {#if appearance.typeface === "custom"}
                <SettingRow label="Custom font" description="Type an installed font family name." stacked>
                  <div class="custom-font">
                    <input
                      class="search-input"
                      list="font-choices"
                      placeholder="Font family name"
                      aria-label="Custom font family"
                      bind:value={customDraft}
                      oninput={() => validateFont(customDraft)}
                    />
                    <datalist id="font-choices">
                      {#each fontChoices as family (family)}<option value={family}></option>{/each}
                    </datalist>
                    {#if queryLocalFonts}
                      <Button onclick={browseFonts}>Browse installed fonts…</Button>
                    {/if}
                    <Button variant="primary" onclick={applyCustomFont}>Apply</Button>
                  </div>
                  <p style:font-family={fontStack("custom", customDraft)} class="font-preview">
                    The quick brown fox — 0123456789
                  </p>
                  {#if customNote}<p class="muted note">{customNote}</p>{/if}
                </SettingRow>
              {/if}
              <SettingRow id="density" label="Density" description="How tightly lists and rows are spaced.">
                <Segmented
                  label="Density"
                  value={appearance.density}
                  options={DENSITY_OPTIONS}
                  onchange={(value) => appearance.setDensity(value).catch(apply)}
                />
              </SettingRow>
              <SettingRow id="cover-shape" label="Cover shape" description="How artwork is drawn on Now Playing.">
                <Segmented
                  label="Cover shape"
                  value={appearance.coverShape}
                  options={COVER_OPTIONS}
                  onchange={(value) => appearance.setCoverShape(value).catch(apply)}
                />
              </SettingRow>
            </SettingsSection>
            </div>

            <div class="col wide" style="order: 4">
            <SettingsSection title="Extras" icon="sparkles">
              <ToggleRow
                id="cat" label="Cat"
                description={appearance.reducedMotion
                  ? "A little cat that follows your cursor. Paused while your system's reduced-motion setting is on."
                  : "A little cat that follows your cursor around the window."}
                checked={config.ui.oneko}
                onchange={setOneko}
              />
              <SettingRow label="Credits" stacked>
                <span class="muted credits">
                  <button class="link" type="button" onclick={() => openCredit("https://github.com/adryd325/oneko.js")}>oneko.js by adryd325</button>
                  &amp;
                  <button class="link" type="button" onclick={() => openCredit("https://github.com/kyrie25/spicetify-oneko")}>spicetify-oneko by kyrie25</button>
                </span>
              </SettingRow>
            </SettingsSection>
            </div>
            </div>
          </div>
        {:else if tab === "playback"}
          <div id="settings-panel-playback" role="tabpanel" aria-labelledby="settings-tab-playback">
            {#if player.state.bit_perfect_notice}
              <Notice kind="warning">{player.state.bit_perfect_notice}</Notice>
            {/if}
            {#if player.state.dsp_active}
              <Notice kind="info">
                Sound processing is active ({player.state.dsp_stages.join(", ")}), so bit-perfect
                output is off{config.audio.exclusive
                  ? "; exclusive mode still keeps the device to itself"
                  : ""}.
              </Notice>
            {/if}

            <div class="cols">
            <div class="col">
            <SettingsSection title="Streaming" icon="radio">
              <SettingRow id="quality" label="Streaming quality" description="The quality OxiTide asks TIDAL for.">
                <Select
                  label="Streaming quality"
                  value={config.streaming.quality}
                  options={QUALITY_OPTIONS}
                  onchange={setQuality}
                />
              </SettingRow>
              <ToggleRow
                id="allow-fallback" label="Use the best available quality"
                description="When a track isn't offered at the quality you chose, play the best available and report it."
                checked={config.streaming.allow_fallback}
                onchange={setAllowFallback}
              />
              <SettingRow
                id="quality-badge" label="Quality badge"
                description="Compact shows MAX / LOSSLESS / AAC; Detailed shows bit depth, sample rate and bitrate."
              >
                <Segmented
                  label="Quality display"
                  value={config.ui.quality_display}
                  options={QUALITY_DISPLAY_OPTIONS}
                  onchange={setDisplay}
                />
              </SettingRow>
            </SettingsSection>

            <SettingsSection title="Output" icon="headphones">
              <SettingRow id="device" label="Audio device" description="Where OxiTide plays.">
                <Select
                  label="Audio output"
                  width={300}
                  value={config.audio.output_device}
                  options={deviceOptions}
                  onchange={setDevice}
                />
              </SettingRow>
              <ToggleRow
                id="exclusive" label="Bit-perfect output (exclusive mode)"
                description="Bypasses the system mixer so samples reach the device unchanged. Other apps cannot use the device while it plays."
                checked={config.audio.exclusive}
                onchange={setExclusive}
              />
              {#if config.audio.exclusive || config.audio.output_device.startsWith("asio:")}
                <SettingRow
                  id="exclusive-fallback" label="If unsupported"
                  description="What to do when the device refuses exclusive mode."
                >
                  <Segmented
                    label="If unsupported"
                    value={config.audio.exclusive_fallback}
                    options={FALLBACK_OPTIONS}
                    onchange={setFallback}
                  />
                </SettingRow>
              {/if}
              {#if config.audio.output_device.startsWith("asio:")}
                <SettingRow label="ASIO driver" stacked>
                  <Notice kind="info">
                    ASIO drivers are always exclusive and bit-perfect at 100% volume; lower volume is
                    applied in software. The buffer size follows the driver's own control panel.
                  </Notice>
                </SettingRow>
              {/if}
            </SettingsSection>

            <SettingsSection
              title="Volume leveling"
              description="ReplayGain evens out loudness between tracks using TIDAL's gain data."
              icon="volume-2"
            >
              <SettingRow
                id="replaygain" label="ReplayGain"
                description={dsp.replay_gain.mode === "album"
                  ? "Album keeps the dynamics between tracks of one album and falls back to the track gain."
                  : "Off by default; any leveling changes the samples."}
              >
                <Segmented
                  label="ReplayGain"
                  value={dsp.replay_gain.mode}
                  options={REPLAY_GAIN_OPTIONS}
                  onchange={(mode) => setReplayGain({ mode })}
                />
              </SettingRow>
              <SettingRow
                id="rg-preamp" label="Preamp"
                description="Raises or lowers every track after leveling."
                disabled={dsp.replay_gain.mode === "off"}
              >
                <Slider
                  label="Preamp"
                  origin="center"
                  disabled={dsp.replay_gain.mode === "off"}
                  min={-REPLAY_GAIN_MAX_DB}
                  max={REPLAY_GAIN_MAX_DB}
                  step={DB_STEP}
                  value={dsp.replay_gain.preamp_db}
                  format={formatDb}
                  oninput={(v) => setReplayGain({ preamp_db: clampDb(v, REPLAY_GAIN_MAX_DB) })}
                />
              </SettingRow>
              <SettingRow
                id="rg-untagged" label="Tracks without data"
                description="The gain applied to tracks TIDAL has no ReplayGain data for."
                disabled={dsp.replay_gain.mode === "off"}
              >
                <Slider
                  label="Tracks without data"
                  origin="center"
                  disabled={dsp.replay_gain.mode === "off"}
                  min={-REPLAY_GAIN_MAX_DB}
                  max={REPLAY_GAIN_MAX_DB}
                  step={DB_STEP}
                  value={dsp.replay_gain.untagged_db}
                  format={formatDb}
                  oninput={(v) => setReplayGain({ untagged_db: clampDb(v, REPLAY_GAIN_MAX_DB) })}
                />
              </SettingRow>
              <ToggleRow
                id="rg-clipping" label="Prevent clipping"
                description={player.state.stream?.replay_gain_db != null && dsp.replay_gain.mode !== "off"
                  ? `Never raise a track past its peak. Current track: ${formatDb(Math.round(player.state.stream.replay_gain_db * 10) / 10)}.`
                  : "Never raise a track past its peak."}
                disabled={dsp.replay_gain.mode === "off"}
                checked={dsp.replay_gain.prevent_clipping}
                onchange={(prevent_clipping) => setReplayGain({ prevent_clipping })}
              />
            </SettingsSection>

            <SettingsSection title="Crossfade" icon="waves">
              <SettingRow
                id="crossfade" label="Crossfade"
                description="Crossfades when a track ends on its own; skipping and seeking cut straight over."
              >
                <Slider
                  label="Crossfade"
                  min={0}
                  max={CROSSFADE_MAX_MS}
                  step={500}
                  value={dsp.crossfade.duration_ms}
                  format={formatCrossfade}
                  oninput={(duration_ms) => setCrossfade({ duration_ms })}
                />
              </SettingRow>
              <ToggleRow
                id="gapless" label="Keep albums gapless"
                description="No crossfade between consecutive tracks of one album."
                disabled={dsp.crossfade.duration_ms === 0}
                checked={dsp.crossfade.skip_same_album}
                onchange={(skip_same_album) => setCrossfade({ skip_same_album })}
              />
            </SettingsSection>
            </div>
            <div class="col">
            <SettingsSection
              title="Equalizer"
              description="Ten bands. Active processing turns bit-perfect off."
              icon="sliders-vertical"
            >
              {#snippet actions()}
                <Toggle label="Equalizer" checked={dsp.eq.enabled} onchange={(enabled) => setEq({ enabled })} />
              {/snippet}
              <SettingRow id="eq-preset" label="Preset" disabled={!dsp.eq.enabled}>
                <Select
                  label="Equalizer preset"
                  disabled={!dsp.eq.enabled}
                  value={matchPreset(dsp.eq.bands_db)}
                  options={presetOptions}
                  width={180}
                  onchange={setPreset}
                />
                <Button
                  icon="undo-2"
                  disabled={!dsp.eq.enabled || matchPreset(dsp.eq.bands_db) === "flat"}
                  onclick={() => setPreset("flat")}
                >Reset</Button>
              </SettingRow>
              <SettingRow id="eq-bands" label="Bands" disabled={!dsp.eq.enabled} stacked>
                <EqBands bands={dsp.eq.bands_db} disabled={!dsp.eq.enabled} onband={setBand} />
              </SettingRow>
              <ToggleRow
                id="eq-auto-preamp" label="Automatic preamp"
                description="Lowers the level by the largest boost so it cannot clip."
                disabled={!dsp.eq.enabled}
                checked={dsp.eq.auto_preamp}
                onchange={(auto_preamp) => setEq({ auto_preamp })}
              />
              <SettingRow id="eq-preamp" label="Preamp" disabled={!dsp.eq.enabled}>
                {#if dsp.eq.auto_preamp}
                  <span class="readout">{formatDb(effectivePreamp(dsp.eq))}</span>
                {:else}
                  <Slider
                    label="Equalizer preamp"
                    origin="center"
                    disabled={!dsp.eq.enabled}
                    min={-EQ_MAX_DB}
                    max={EQ_MAX_DB}
                    step={DB_STEP}
                    value={dsp.eq.preamp_db}
                    format={formatDb}
                    oninput={(v) => setEq({ preamp_db: clampDb(v, EQ_MAX_DB) })}
                  />
                {/if}
              </SettingRow>
            </SettingsSection>

            <SettingsSection
              title="AutoMix (experimental)"
              description={dsp.crossfade.duration_ms > 0
                ? "Smarter transitions on top of the crossfade. Each option works on its own so you can A/B them."
                : "Smarter transitions on top of the crossfade. Set a crossfade length to use it."}
              icon="sparkles"
            >
              <AutoMixPanel
                crossfade={dsp.crossfade}
                {player}
                disabled={dsp.crossfade.duration_ms === 0}
                onchange={setCrossfade}
              />
            </SettingsSection>
            </div>
            </div>
          </div>
        {:else if tab === "content"}
          <div id="settings-panel-content" role="tabpanel" aria-labelledby="settings-tab-content">
            <Notice kind="info">
              OxiTide enforces these client-side: TIDAL does not expose its own filters to
              third-party clients, and blocked tracks can still appear in TIDAL recommendations.
              Flagged rows are grayed out and skipped; unknown flags always play.
            </Notice>
            <SettingsSection title="Filters & labels" icon="shield-check">
              <ToggleRow
                id="block-explicit" label="Block explicit content"
                description="Hides explicit tracks and blocks them from playing. Enabling clears the current queue, matching the official app."
                checked={config.content.block_explicit}
                onchange={setBlockExplicit}
              />
              <ToggleRow
                id="block-ai" label="Block AI-generated content"
                description="Hides tracks TIDAL labels as AI-generated and removes them from the queue. Labels may be missing for some tracks or regions; those play normally."
                checked={config.content.block_ai}
                onchange={setBlockAi}
              />
              <ToggleRow
                id="show-labels" label="Show content labels"
                description="Tags explicit tracks with E and AI-generated tracks with AI, even when not blocking them. Turning this off stops the background label lookups too."
                checked={config.content.show_labels}
                onchange={setShowLabels}
              />
            </SettingsSection>
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
            <div class="cols">
            <div class="col">
            <SettingsSection
              title="Scrobbling"
              description="Send what you play to your listening profile."
              icon="audio-lines"
            >
              {#if scrobble}
                <SettingRow
                  id="lastfm" label="Last.fm"
                  description={[
                    serviceLabel(scrobble.lastfm),
                    queuedLabel(scrobble.lastfm.queued),
                    scrobble.lastfm.last_error ?? "",
                    lastfmAuthNote,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                >
                  {#if lastfmAuth === "waiting"}
                    <Loading label="Waiting for authorization in your browser…" />
                    <Button variant="primary" onclick={() => void pollLastfm()}>I've authorized</Button>
                    <Button onclick={cancelLastfm}>Cancel</Button>
                  {:else if scrobble.lastfm.connected}
                    <Toggle
                      label="Scrobble to Last.fm"
                      checked={scrobble.lastfm.enabled}
                      disabled={!canToggle(scrobble.lastfm)}
                      onchange={(enabled) => toggleService("lastfm", enabled)}
                    />
                    <Button onclick={disconnectLastfm}>Disconnect</Button>
                  {:else}
                    <Button
                      variant="primary"
                      disabled={!scrobble.lastfm.configured}
                      onclick={() => void connectLastfm()}
                    >Connect</Button>
                  {/if}
                </SettingRow>
                <SettingRow
                  id="listenbrainz" label="ListenBrainz"
                  description={[
                    serviceLabel(scrobble.listenbrainz),
                    queuedLabel(scrobble.listenbrainz.queued),
                    scrobble.listenbrainz.last_error ?? "",
                    lbError,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                >
                  {#if scrobble.listenbrainz.connected}
                    <Toggle
                      label="Scrobble to ListenBrainz"
                      checked={scrobble.listenbrainz.enabled}
                      disabled={!canToggle(scrobble.listenbrainz)}
                      onchange={(enabled) => toggleService("listenbrainz", enabled)}
                    />
                    <Button onclick={disconnectListenbrainz}>Disconnect</Button>
                  {:else}
                    <input
                      class="search-input token"
                      type="password"
                      placeholder="ListenBrainz token"
                      aria-label="ListenBrainz token"
                      bind:value={lbToken}
                    />
                    <Button
                      variant="primary"
                      disabled={lbBusy || !lbToken.trim()}
                      onclick={() => void connectListenbrainz()}
                    >Connect</Button>
                  {/if}
                  {#snippet below()}
                    {#if !scrobble?.listenbrainz.connected}
                      <span class="muted help-line">
                        Get a token from
                        <button
                          class="link"
                          type="button"
                          onclick={() => openCredit("https://listenbrainz.org/profile/")}
                        >your ListenBrainz profile</button>.
                      </span>
                    {/if}
                  {/snippet}
                </SettingRow>
              {:else}
                <SettingRow label="Loading scrobbling status…" />
              {/if}
            </SettingsSection>

            <SettingsSection title="Presence & controls" icon="radio">
              <ToggleRow
                id="discord" label="Discord Rich Presence"
                description="Shows the current track on your Discord profile. Requires the Discord desktop client."
                checked={config.integrations.discord}
                onchange={setDiscord}
              />
              <ToggleRow
                id="media-controls" label="Media controls"
                description="Hardware media keys and the system media overlay."
                checked={config.integrations.smtc}
                onchange={setSmtc}
              />
            </SettingsSection>
            </div>
            <div class="col">
              <RemoteSettings />
            </div>
            </div>
          </div>
        {:else if tab === "storage"}
          <div id="settings-panel-storage" role="tabpanel" aria-labelledby="settings-tab-storage">
            <div class="cols">
            <div class="col">
            <SettingsSection
              title="Cache"
              description="Downloaded audio kept on disk so replays and offline playback start instantly."
              icon="database"
            >
              {#snippet actions()}
                <Button icon="refresh-cw" onclick={refreshCacheStatus}>Refresh</Button>
                <Button variant="danger" icon="trash-2" onclick={clearCache}>Clear cache</Button>
              {/snippet}
              {#if cacheStatus}
                {#if !cacheStatus.usable}
                  <div class="inset">
                    <Notice kind="warning">
                      Cache is disabled: the directory is not writable. Streaming is unaffected.
                    </Notice>
                  </div>
                {:else if cacheStatus.configured_dir && cacheStatus.dir !== cacheStatus.configured_dir}
                  <div class="inset">
                    <Notice kind="warning">
                      The configured cache directory is unusable; using {cacheStatus.dir}.
                    </Notice>
                  </div>
                {/if}
                <SettingRow id="cache-usage" label="Disk usage" stacked>
                  <div class="usage">
                    <div
                      class="meter"
                      role="meter"
                      aria-label="Cache usage"
                      aria-valuemin="0"
                      aria-valuemax="100"
                      aria-valuenow={Math.round(cacheFraction * 100)}
                    >
                      <span class="fill" class:full={cacheFraction > 0.9} style:width="{cacheFraction * 100}%"></span>
                    </div>
                    <span class="muted">
                      {formatSize(cacheStatus.used_bytes)} used{#if cacheStatus.pending_bytes > 0}
                        (+{formatSize(cacheStatus.pending_bytes)} downloading){/if}
                      of {formatSize(cacheStatus.max_bytes)}
                    </span>
                  </div>
                </SettingRow>
                <SettingRow id="cache-max" label="Maximum size" description="The oldest tracks are removed when the cache grows past this.">
                  <NumberField
                    label="Maximum cache size"
                    min={0}
                    unit="MB"
                    value={config.cache.max_size_mb}
                    onchange={setCacheMaxMb}
                  />
                </SettingRow>
                <ToggleRow
                  id="cache-clear-exit" label="Clear cache on exit"
                  checked={config.cache.clear_on_exit}
                  onchange={setClearOnExit}
                />
                <SettingRow
                  id="cache-location" label="Location"
                  description="To move the cache, edit cache.dir in config.toml."
                  stacked
                >
                  <code class="path">{cacheStatus.dir}</code>
                </SettingRow>
              {:else}
                <SettingRow label="Cache status unavailable" />
              {/if}
            </SettingsSection>
            </div>
            <div class="col">
            <SettingsSection
              title="Listening history"
              description="Recorded locally in history.jsonl and used by the History view and stats."
              icon="history"
            >
              <ToggleRow
                id="history-record" label="Record listening history"
                checked={config.history.enabled}
                onchange={setHistoryEnabled}
              />
              <SettingRow id="history-max" label="Maximum entries" description="Older plays are dropped beyond this.">
                <NumberField
                  label="Maximum history entries"
                  min={100}
                  max={100000}
                  unit="plays"
                  value={config.history.max_entries}
                  onchange={setHistoryMaxEntries}
                />
              </SettingRow>
              <SettingRow id="history-clear" label="Clear history" description="Permanently deletes your local play history and stats.">
                <Button variant="danger" icon="trash-2" onclick={() => (confirmingHistoryClear = true)}>
                  Clear history
                </Button>
              </SettingRow>
            </SettingsSection>
            </div>
            </div>
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
            <KeyringNotice />

            <div class="cols">
            <div class="col">
            <SettingsSection title="Window & startup" icon="app-window">
              <ToggleRow
                id="close-to-tray" label="Close to tray"
                description="Closing the window hides it and keeps playback running; Quit lives in the tray menu."
                checked={config.ui.close_to_tray}
                onchange={setCloseToTray}
              />
              <ToggleRow
                id="start-mini" label="Start in mini player"
                description="Open as the compact mini player instead of the full window."
                checked={config.ui.start_in_mini}
                onchange={setStartInMini}
              />
            </SettingsSection>

            <SettingsSection
              title="Global hotkeys"
              description="Work while OxiTide is in the background. Press Backspace or Delete while recording to clear one; a shortcut already used by another action asks before reassigning, and one another app owns keeps the old binding."
              icon="keyboard"
            >
              {#each HOTKEY_ACTIONS as entry (entry.id)}
                <SettingRow id={`hotkey-${entry.id}`} label={entry.label}>
                  <kbd class:unset={!config.hotkeys[entry.id]}>
                    {capture.recording === entry.id
                      ? "Press keys…"
                      : displayAccelerator(config.hotkeys[entry.id]) || "Not set"}
                  </kbd>
                  <Button
                    id={`hotkey-${entry.id}`}
                    variant={capture.recording === entry.id ? "primary" : "secondary"}
                    onclick={() => void capture.start(entry.id)}
                  >{capture.recording === entry.id ? "Recording…" : "Record"}</Button>
                  <Button
                    disabled={!config.hotkeys[entry.id]}
                    onclick={() => void saveHotkey(entry.id, "")}
                  >Clear</Button>
                </SettingRow>
              {/each}
            </SettingsSection>
            {#if hotkeyConflict}
              <ConfirmDialog
                title="Shortcut already in use"
                message={`"${displayAccelerator(hotkeyConflict.accelerator)}" is already used by "${actionLabel(hotkeyConflict.other)}". Reassign it to "${actionLabel(hotkeyConflict.action)}"?`}
                confirmLabel="Reassign"
                onconfirm={confirmReassign}
                oncancel={cancelReassign}
              />
            {/if}
            </div>
            <div class="col">
            <UpdatesSection />

            <SettingsSection
              id="shortcuts"
              title="Keyboard shortcuts"
              description="Available while OxiTide is focused."
              icon="keyboard"
            >
              <ul class="shortcuts">
                {#each SHORTCUTS as shortcut (shortcut.keys)}
                  <li><kbd>{shortcut.keys}</kbd> <span>{shortcut.description}</span></li>
                {/each}
              </ul>
            </SettingsSection>

            <SettingsSection title="Troubleshooting" icon="file-text">
              <SettingRow id="logs" label="Logs" description={logsDir || "Directory unavailable"}>
                <Button icon="folder" onclick={() => api.openLogsFolder().catch(apply)}>
                  Open logs folder
                </Button>
              </SettingRow>
            </SettingsSection>

            <SettingsSection title="Account" icon="user">
              <SettingRow id="logout" label="Log out" description="Signs out of TIDAL on this device. Your settings are kept.">
                <Button variant="danger" icon="log-out" onclick={onLogout}>Log out</Button>
              </SettingRow>
            </SettingsSection>
            </div>
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* Container queries (not viewport ones): the sidebar and queue change how
     much room the page really has. */
  .settings {
    container: settings / inline-size;
  }
  /* The header shares the page's two-column grid, so the search sits centred
     over the cards (not the whole window); it stacks under the title when the
     page is narrow. */
  .settings > .page-head {
    display: grid;
    grid-template-columns: 200px minmax(0, 1320px);
    column-gap: 36px;
    align-items: center;
  }
  .layout {
    display: grid;
    grid-template-columns: 200px minmax(0, 1320px);
    gap: 36px;
    align-items: start;
  }
  .nav {
    position: sticky;
    top: calc(var(--titlebar-h) + 4px);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tab-indicator {
    position: absolute;
    top: 0;
    left: 0;
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    pointer-events: none;
  }
  .tab-indicator.ready {
    transition: transform var(--dur-view) var(--ease-in-out);
  }
  .tab {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 12px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    font-weight: 550;
    text-align: left;
    white-space: nowrap;
  }
  .tab :global(svg) {
    flex: none;
  }
  .tab:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .tab.active {
    color: var(--text);
  }
  .tab.active :global(svg) {
    color: var(--accent);
  }
  .content {
    min-width: 0;
  }
  /* Each tab is a grid of column stacks: two where 520px each fit (about
     1060px of content), one below that. The stacks are balanced by hand per
     tab and kept at a constant height (controls that do not apply are
     disabled, not removed). The shorter stack's cards share the leftover
     height evenly, each spreading it across its rows, so both columns
     always end on the same line. `.wide` stacks span every column. */
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(520px, 100%), 1fr));
    gap: 20px;
    align-items: stretch;
    margin-bottom: 20px;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .col.wide {
    grid-column: 1 / -1;
  }
  .col > :global(.set-section) {
    margin: 0;
  }
  .col > :global(.set-section) {
    flex: 1 1 auto;
  }

  /* Switching tabs remounts the panel, so each card rises in (opacity + a
     10px lift) with a short stagger down the page: the first card of each
     stack leads, later ones follow by 45ms steps. `backwards` fill keeps the
     finished card transform-free (dropdowns and dialogs anchor normally). */
  .content :global(.set-section) {
    animation: card-in var(--dur-view) var(--ease-enter) backwards;
    animation-delay: calc(var(--n, 0) * 45ms);
  }
  .col:nth-child(2) > :global(.set-section:nth-child(1)) { --n: 1; }
  .col > :global(.set-section:nth-child(2)) { --n: 1; }
  .col:nth-child(2) > :global(.set-section:nth-child(2)) { --n: 2; }
  .col > :global(.set-section:nth-child(3)) { --n: 2; }
  .col:nth-child(2) > :global(.set-section:nth-child(3)) { --n: 3; }
  .col > :global(.set-section:nth-child(n + 4)) { --n: 3; }
  @keyframes card-in {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
  /* Reduced motion is a hard floor: no movement, only a quick fade. */
  @media (prefers-reduced-motion: reduce) {
    .content :global(.set-section) {
      animation-name: card-fade;
      animation-delay: 0ms;
    }
    .tab-indicator.ready {
      transition: none;
    }
    @keyframes card-fade {
      from {
        opacity: 0;
      }
      to {
        opacity: 1;
      }
    }
  }
  @container settings (max-width: 760px) {
    .settings > .page-head {
      grid-template-columns: minmax(0, 1fr);
      row-gap: 12px;
    }
    .layout {
      grid-template-columns: minmax(0, 1fr);
      gap: 16px;
    }
    .nav {
      position: relative;
      flex-direction: row;
      overflow-x: auto;
      padding-bottom: 4px;
    }
  }

  .link {
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }
  .credits,
  .help-line {
    font-size: 12.5px;
  }
  .note {
    margin: 4px 0 0;
    font-size: 12.5px;
  }
  .custom-font {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .custom-font input {
    flex: 1 1 220px;
    min-width: 0;
    padding: 8px 12px;
  }
  .font-preview {
    margin: 10px 0 0;
    font-size: 18px;
  }
  .token {
    width: 220px;
    max-width: 100%;
    padding: 8px 12px;
  }
  .readout {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .inset {
    padding: 14px 18px 0;
  }
  .usage {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }
  .meter {
    height: 8px;
    overflow: hidden;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 14%, transparent);
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
  }
  .fill.full {
    background: var(--warning);
  }
  .path {
    display: block;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
    color: var(--text-muted);
    font-size: 12.5px;
    overflow-wrap: anywhere;
    user-select: all;
  }
  kbd {
    display: inline-block;
    min-width: 14ch;
    padding: 4px 10px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--text);
    font: 600 12.5px/1.3 var(--font-ui);
    text-align: center;
  }
  kbd.unset {
    color: var(--text-muted);
    font-weight: 500;
  }
  .shortcuts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 2px 24px;
    margin: 0;
    padding: 10px 18px 14px;
    list-style: none;
  }
  .shortcuts li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 0;
  }
  .shortcuts kbd {
    flex: none;
    min-width: 9ch;
    padding: 4px 8px;
  }
  .shortcuts span {
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
