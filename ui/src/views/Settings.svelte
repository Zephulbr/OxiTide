<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { listen } from "@tauri-apps/api/event";
  import { tick, untrack } from "svelte";
  import {
    api,
    SCROBBLE_STATUS_EVENT,
    type AnalyzerStyle,
    type CacheStatus,
    type CatjamCoat,
    type Config,
    type CrossfadeSettings,
    type DeckFace,
    type DspSettings,
    type EqSettings,
    type LampColor,
    type MeterStyle,
    type OutputDeviceList,
    type InstrumentTone,
    type PanelStyle,
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
  import OutputDevicePicker from "../lib/components/settings/OutputDevicePicker.svelte";
  import Notice from "../lib/components/settings/Notice.svelte";
  import Segmented from "../lib/components/settings/Segmented.svelte";
  import { sortMemory, type SortMemoryMode } from "../lib/sortMemory.svelte";
  import { homeSearch } from "../lib/homeSearch.svelte";
  import Select from "../lib/components/settings/Select.svelte";
  import LanguagePicker from "../lib/components/settings/LanguagePicker.svelte";
  import SettingRow from "../lib/components/settings/SettingRow.svelte";
  import SettingsSection from "../lib/components/settings/SettingsSection.svelte";
  import SettingsSearch from "../lib/components/settings/SettingsSearch.svelte";
  import Slider from "../lib/components/settings/Slider.svelte";
  import Toggle from "../lib/components/settings/Toggle.svelte";
  import ToggleRow from "../lib/components/settings/ToggleRow.svelte";
  import CoatPicker from "../lib/components/settings/CoatPicker.svelte";
  import { CATJAM_SCALES } from "../lib/catjamSprite";
  import {
    appearance,
    clampLampBrightness,
    fontStack,
    type Density,
    type DynamicColorSetting,
    type CoverShape,
    type TypefaceSetting,
    type UiMode,
    ZOOM_MAX,
    ZOOM_MIN,
    ZOOM_STEP,
  } from "../lib/appearance.svelte";
  import { formatPercent } from "../lib/format";
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
  import { SETTINGS_INDEX, type SettingEntry } from "../lib/settingsIndex";
  import { settingsJump } from "../lib/settingsJump.svelte";
  import { outputDevices } from "../lib/outputDeviceStore.svelte";
  import { SETTINGS_TABS, nextTab, type SettingsTab } from "../lib/settingsTabs";
  import { TAB_META, formatSize, usageFraction } from "../lib/settingsView";
  import { engineNotice } from "../lib/engineNotices";
  import { copyText } from "../lib/clipboard";
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
  import { onPageResize } from "../lib/queueTransition";
  import { t, tSplit } from "../lib/i18n.svelte";
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
  let deviceList = $state<OutputDeviceList | null>(null);
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
  let confirmingLogout = $state(false);
  /// A change that turns on a fixed volume (exclusive mode, an ASIO device),
  /// held while the volume warning asks first.
  let pendingFixedVolume = $state<(() => void) | null>(null);
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
    { value: "dark", get label() { return t("settings.opt.theme.dark"); }, icon: "moon" },
    { value: "light", get label() { return t("settings.opt.theme.light"); }, icon: "sun" },
    { value: "system", get label() { return t("settings.opt.theme.system"); }, icon: "monitor" },
  ];
  const MODE_OPTIONS: { value: UiMode; label: string }[] = [
    { value: "atmosphere", get label() { return t("settings.opt.mode.atmosphere"); } },
    { value: "instrument", get label() { return t("settings.opt.mode.instrument"); } },
    { value: "rack", label: "Hi-Fi Rack" },
  ];
  const MODE_HELP: Record<UiMode, string> = {
    get atmosphere() { return t("settings.opt.modeHelp.atmosphere"); },
    get instrument() { return t("settings.opt.modeHelp.instrument"); },
    get rack() { return t("settings.opt.modeHelp.rack"); },
  };
  const TONE_OPTIONS: { value: InstrumentTone; label: string }[] = [
    { value: "warm", get label() { return t("settings.opt.tone.warm"); } },
    { value: "neutral", get label() { return t("settings.opt.tone.neutral"); } },
    { value: "black", get label() { return t("settings.opt.tone.black"); } },
  ];
  const TONE_HELP: Record<InstrumentTone, string> = {
    get warm() { return t("settings.opt.toneHelp.warm"); },
    get neutral() { return t("settings.opt.toneHelp.neutral"); },
    get black() { return t("settings.opt.toneHelp.black"); },
  };
  const DYNAMIC_OPTIONS: { value: DynamicColorSetting; label: string }[] = [
    { value: "full", get label() { return t("settings.opt.dynamic.full"); } },
    { value: "accent", get label() { return t("settings.opt.dynamic.accent"); } },
    { value: "off", get label() { return t("settings.opt.dynamic.off"); } },
  ];
  const TYPEFACE_OPTIONS: { value: TypefaceSetting; label: string; hint?: string }[] = [
    { value: "auto", get label() { return t("settings.opt.typeface.auto"); }, get hint() { return t("settings.opt.typeface.autoHint"); } },
    { value: "manrope", label: "Manrope" },
    { value: "inter", label: "Inter" },
    { value: "system", get label() { return t("settings.opt.typeface.system"); }, get hint() { return t("settings.opt.typeface.systemHint"); } },
    { value: "custom", get label() { return t("settings.opt.typeface.custom"); }, get hint() { return t("settings.opt.typeface.customHint"); } },
  ];
  const DENSITY_OPTIONS: { value: Density; label: string }[] = [
    { value: "comfortable", get label() { return t("settings.opt.density.comfortable"); } },
    { value: "compact", get label() { return t("settings.opt.density.compact"); } },
  ];
  const SORT_MEMORY_OPTIONS: { value: SortMemoryMode; label: string }[] = [
    { value: "page", get label() { return t("settings.opt.sortMemory.page"); } },
    { value: "session", get label() { return t("settings.opt.sortMemory.session"); } },
    { value: "always", get label() { return t("settings.opt.sortMemory.always"); } },
  ];
  const SORT_MEMORY_HELP: Record<SortMemoryMode, string> = {
    get page() { return t("settings.opt.sortMemoryHelp.page"); },
    get session() { return t("settings.opt.sortMemoryHelp.session"); },
    get always() { return t("settings.opt.sortMemoryHelp.always"); },
  };
  const sortMemoryStore = sortMemory();
  const homeSearchFlag = homeSearch();
  const COVER_OPTIONS: { value: CoverShape; label: string }[] = [
    { value: "disc", get label() { return t("settings.opt.cover.disc"); } },
    { value: "sleeve", get label() { return t("settings.opt.cover.sleeve"); } },
  ];
  const PANEL_STYLE_OPTIONS: { value: PanelStyle; label: string }[] = [
    { value: "docked", get label() { return t("settings.opt.panelStyle.docked"); } },
    { value: "floating", get label() { return t("settings.opt.panelStyle.floating"); } },
  ];
  const DECK_OPTIONS: { value: DeckFace; label: string }[] = [
    { value: "transport", get label() { return t("settings.opt.deck.transport"); } },
    { value: "cd", get label() { return t("settings.opt.deck.cd"); } },
    { value: "display", get label() { return t("settings.opt.deck.display"); } },
  ];
  const METER_OPTIONS: { value: MeterStyle; label: string }[] = [
    { value: "needles", get label() { return t("settings.opt.meter.needles"); } },
    { value: "bars", get label() { return t("settings.opt.meter.bars"); } },
    { value: "led", label: "LED" },
    { value: "spectrum", get label() { return t("settings.opt.meter.spectrum"); } },
  ];
  const ANALYZER_OPTIONS: { value: AnalyzerStyle; label: string }[] = [
    { value: "auto", get label() { return t("settings.opt.analyzer.auto"); } },
    { value: "spectrum", get label() { return t("settings.opt.analyzer.spectrum"); } },
    { value: "recorder", get label() { return t("settings.opt.analyzer.recorder"); } },
    { value: "tuner", get label() { return t("settings.opt.analyzer.tuner"); } },
    { value: "eq", label: "EQ" },
  ];
  const LAMP_OPTIONS: { value: LampColor; label: string }[] = [
    { value: "amber", get label() { return t("settings.opt.lamp.amber"); } },
    { value: "blue", get label() { return t("settings.opt.lamp.blue"); } },
    { value: "green", get label() { return t("settings.opt.lamp.green"); } },
    { value: "red", get label() { return t("settings.opt.lamp.red"); } },
    { value: "white", get label() { return t("settings.opt.lamp.white"); } },
    { value: "cover", get label() { return t("settings.opt.lamp.cover"); } },
  ];
  const QUALITY_OPTIONS: { value: Quality; label: string; hint: string }[] = [
    { value: "low", label: "Low", hint: "HE-AAC 96 kbps" },
    { value: "high", label: "High", hint: "AAC 320 kbps" },
    { value: "lossless", get label() { return t("settings.opt.quality.lossless"); }, get hint() { return t("settings.opt.quality.losslessHint"); } },
    { value: "max", get label() { return t("settings.opt.quality.max"); }, get hint() { return t("settings.opt.quality.maxHint"); } },
  ];
  const QUALITY_DISPLAY_OPTIONS: { value: QualityDisplay; label: string }[] = [
    { value: "compact", get label() { return t("settings.opt.qualityDisplay.compact"); } },
    { value: "detailed", get label() { return t("settings.opt.qualityDisplay.detailed"); } },
  ];
  const REPLAY_GAIN_OPTIONS: { value: ReplayGainSettings["mode"]; label: string }[] = [
    { value: "off", get label() { return t("settings.opt.replayGain.off"); } },
    { value: "track", get label() { return t("settings.opt.replayGain.track"); } },
    { value: "album", get label() { return t("settings.opt.replayGain.album"); } },
  ];
  const FALLBACK_OPTIONS: { value: Config["audio"]["exclusive_fallback"]; label: string }[] = [
    { value: "shared", get label() { return t("settings.opt.fallback.shared"); } },
    { value: "warn_only", get label() { return t("settings.opt.fallback.warnOnly"); } },
  ];

  /// Any device set to Exclusive, or an ASIO driver selected (always exclusive).
  const anyExclusive = $derived(
    deviceList !== null &&
      (deviceList.system_default.mode === "exclusive" ||
        deviceList.selected.startsWith("asio:") ||
        deviceList.devices.some((d) => d.group !== "asio" && d.prefs.mode === "exclusive")),
  );
  const selectedExclusive = $derived(
    deviceList !== null &&
      (deviceList.selected === ""
        ? deviceList.system_default.mode === "exclusive"
        : deviceList.selected.startsWith("asio:") ||
          deviceList.devices.some((d) => d.id === deviceList?.selected && d.prefs.mode === "exclusive")),
  );
  const presetOptions = $derived([
    ...EQ_PRESETS.map((p) => ({ value: p.id, label: p.label })),
    { value: "custom", label: t("settings.eqCustom"), disabled: true },
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
      lastfmAuthNote = t("settings.lastfmTimedOut");
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
  let indicator = $state<HTMLSpanElement | null>(null);
  let pill = $state({ x: 0, y: 0, w: 0, h: 0 });
  let pillReady = $state(false);

  function placePill() {
    const el = document.getElementById(`settings-tab-${tab}`);
    if (!el) return;
    pill = { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight };
  }

  // The docked queue's glide measures the final layout before the
  // ResizeObserver below fires, so re-place the pill synchronously when the
  // page takes its new width (the nav may flip to the narrow strip). Written
  // straight to the element, with the pill's own transition suspended, so the
  // glide alone carries it there; the state follows for the next render.
  $effect(() =>
    onPageResize(() => {
      const el = document.getElementById(`settings-tab-${tab}`);
      if (!el || !indicator) return;
      const next = { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight };
      const node = indicator;
      node.style.transition = "none";
      node.style.width = `${next.w}px`;
      node.style.height = `${next.h}px`;
      node.style.transform = `translate(${next.x}px, ${next.y}px)`;
      pill = next;
      requestAnimationFrame(() => requestAnimationFrame(() => (node.style.transition = "")));
    }),
  );

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
    customNote = clean && !document.fonts.check(`12px "${clean}"`) ? t("settings.fontNotFound") : "";
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
    appearance.setDynamicColor(setting).catch(apply);
  }

  function apply(failure: unknown) {
    toasts.show(errorMessage(failure), { severity: "error" });
  }

  async function copyDiagnostics() {
    try {
      const copied = await copyText(await api.getDiagnostics());
      toasts.show(copied ? t("settings.diagnosticsCopied") : t("settings.diagnosticsCopyFailed"), {
        severity: copied ? "success" : "error",
      });
    } catch (e) {
      apply(e);
    }
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
      // The output list loads in OutputDevicePicker (it re-reads on device changes).
      const [loaded, status, logs] = await Promise.all([
        api.getConfig(),
        api.getCacheStatus(),
        api.getLogsDir(),
      ]);
      cacheStatus = status;
      logsDir = logs;
      const next = loaded;
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

  /// The picker's latest list: the fallback row shows while any device plays
  /// exclusive, and the ASIO note while an ASIO driver is selected.
  function onDeviceList(list: OutputDeviceList) {
    deviceList = list;
    if (config && config.audio.output_device !== list.selected) saveAudio({ output_device: list.selected });
  }

  /// Exclusive output sets the in-app volume to 100% and leaves the level to
  /// the DAC or system: ask first when that is louder than the slider now,
  /// unless the user chose "Don't show again". The picker calls it for a
  /// switch from shared to exclusive output.
  function withVolumeWarning(change: () => void) {
    const louder = !player.state.volume_fixed && player.state.volume < 1;
    if (config?.audio.exclusive_volume_warning !== false && louder) pendingFixedVolume = change;
    else change();
  }

  function confirmFixedVolume(optedOut: boolean) {
    const change = pendingFixedVolume;
    pendingFixedVolume = null;
    if (optedOut) {
      saveAudio({ exclusive_volume_warning: false });
      api.setExclusiveVolumeWarning(false).catch(apply);
    }
    change?.();
  }

  function setDeviceIcons(enabled: boolean) {
    if (!config) return;
    config = { ...config, ui: { ...config.ui, device_icons: enabled } };
    const list = outputDevices.list;
    if (list) outputDevices.set({ ...list, icons: enabled });
    api.setDeviceIcons(enabled).catch(apply);
  }

  // "Output settings…" in the quality menu: open the row once the page is up
  // (also when Settings is already showing).
  $effect(() => {
    const id = settingsJump.pending;
    if (!id || !config) return;
    untrack(() => {
      settingsJump.take();
      const entry = SETTINGS_INDEX.find((e) => e.id === id);
      if (entry) void jumpTo(entry);
    });
  });

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

  function setCatjam(enabled: boolean) {
    if (!config) return;
    const previous = config.ui.catjam;
    config = { ...config, ui: { ...config.ui, catjam: enabled } };
    appearance.setCatjam(enabled).catch((e) => {
      if (config) config = { ...config, ui: { ...config.ui, catjam: previous } };
      apply(e);
    });
  }

  /// 1× to 2× of the 32 px sprite; the cap keeps it inside the player bar.
  const CATJAM_SIZE_OPTIONS = CATJAM_SCALES.map((percent) => ({
    value: String(percent),
    label: `${percent}%`,
  }));
  const REDUCED_MOTION_PAUSED = $derived(t("settings.reducedMotionPaused"));

  function setCatjamCoat(coat: CatjamCoat) {
    appearance.setCatjamCoat(coat).catch(apply);
  }

  function setHoverPanels(enabled: boolean) {
    appearance.setHoverPanels(enabled).catch(apply);
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
      return t("settings.hotkeyDuplicate", { keys: shown });
    }
    return t("settings.hotkeyTaken", { keys: shown });
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

  function setAutoplay(autoplay: boolean) {
    if (!config) return;
    config = { ...config, playback: { ...config.playback, autoplay } };
    api.setAutoplay(autoplay).catch(apply);
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
    <h2>{t("route.settings")}</h2>
    {#if config}
      <SettingsSearch
        scope={{
          rack: appearance.mode === "rack",
          studio: appearance.mode === "instrument",
          exclusive: anyExclusive,
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
        aria-label={t("settings.categories")}
      >
        <span
          bind:this={indicator}
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
            <SettingsSection title={t("settings.section.lookAndFeel")} icon="palette">
              <LanguagePicker onerror={apply} />
              <SettingRow
                id="theme" label={t("settings.theme.label")}
                description={appearance.theme === "system"
                  ? t("settings.theme.systemHelp")
                  : t("settings.theme.description")}
              >
                <Segmented
                  label={t("settings.theme.label")}
                  value={appearance.theme}
                  options={THEME_OPTIONS}
                  onchange={(value) => appearance.setTheme(value).catch(apply)}
                />
              </SettingRow>
              <SettingRow id="mode" label={t("settings.mode.label")} description={MODE_HELP[appearance.mode]}>
                <Segmented
                  label={t("settings.mode.label")}
                  value={appearance.mode}
                  options={MODE_OPTIONS}
                  onchange={(value) => appearance.setMode(value).catch(apply)}
                />
              </SettingRow>
              {#if appearance.mode === "instrument"}
                <SettingRow
                  id="studio-tone" label={t("settings.studioTone.label")}
                  description={appearance.resolvedTheme === "light"
                    ? t("settings.studioTone.lightHelp")
                    : TONE_HELP[appearance.instrumentTone]}
                >
                  <Segmented
                    label={t("settings.studioTone.label")}
                    value={appearance.instrumentTone}
                    options={TONE_OPTIONS}
                    onchange={(value) => appearance.setInstrumentTone(value).catch(apply)}
                  />
                </SettingRow>
              {/if}
              {#if appearance.mode !== "rack"}
                <SettingRow
                  id="dynamic-color" label={t("settings.dynamicColor.label")}
                  description={t("settings.dynamicColor.description")}
                >
                  <Segmented
                    label={t("settings.dynamicColor.label")}
                    value={appearance.dynamicColorSetting}
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
                title={t("settings.section.hiFiRack")}
                description={t("settings.section.hiFiRack.description")}
                icon="audio-lines"
              >
                <SettingRow
                  id="rack-deck" label={t("settings.rackDeck.label")}
                  description={t("settings.rackDeck.description")}
                >
                  <Segmented
                    label={t("settings.rackDeck.label")}
                    value={appearance.rack.deck_face}
                    options={DECK_OPTIONS}
                    onchange={(deck_face) => appearance.setRack({ deck_face }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow id="rack-meters" label={t("settings.rackMeters.label")}>
                  <Segmented
                    label={t("settings.rackMeters.label")}
                    value={appearance.rack.meters}
                    options={METER_OPTIONS}
                    onchange={(meters) => appearance.setRack({ meters }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow
                  id="rack-analyzer" label={t("settings.rackAnalyzer.label")}
                  description={t("settings.rackAnalyzer.description")}
                >
                  <Segmented
                    label={t("settings.rackAnalyzer.label")}
                    value={appearance.rack.analyzer}
                    options={ANALYZER_OPTIONS}
                    onchange={(analyzer) => appearance.setRack({ analyzer }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow
                  id="rack-lamp" label={t("settings.rackLamp.label")}
                  description={t("settings.rackLamp.description")}
                >
                  <Segmented
                    label={t("settings.rackLamp.label")}
                    value={appearance.rack.lamp}
                    options={LAMP_OPTIONS}
                    onchange={(lamp) => appearance.setRack({ lamp }).catch(apply)}
                  />
                </SettingRow>
                <SettingRow id="rack-brightness" label={t("settings.rackBrightness.label")}>
                  <Slider
                    label={t("settings.rackBrightness.label")}
                    min={40}
                    max={100}
                    step={5}
                    value={appearance.rack.lamp_brightness}
                    format={(v) => `${v}%`}
                    oncommit={(v) => appearance.setRack({ lamp_brightness: clampLampBrightness(v) }).catch(apply)}
                  />
                </SettingRow>
                <ToggleRow
                  id="rack-eq" label={t("settings.rackEq.label")}
                  checked={appearance.rack.eq_unit}
                  onchange={(eq_unit) => appearance.setRack({ eq_unit }).catch(apply)}
                />
              </SettingsSection>
              </div>
            {/if}

            <div class="col" style="order: 2">
            <SettingsSection title={t("settings.section.typographyAndLayout")} icon="type">
              <SettingRow id="typeface" label={t("settings.typeface.label")} description={t("settings.typeface.description")}>
                <Select
                  label={t("settings.typeface.label")}
                  value={appearance.typeface}
                  options={TYPEFACE_OPTIONS}
                  onchange={setTypeface}
                />
              </SettingRow>
              {#if appearance.typeface === "custom"}
                <SettingRow label={t("settings.customFont.label")} description={t("settings.customFont.description")} stacked>
                  <div class="custom-font">
                    <input
                      class="search-input"
                      list="font-choices"
                      placeholder={t("settings.customFont.placeholder")}
                      aria-label={t("settings.customFont.field")}
                      bind:value={customDraft}
                      oninput={() => validateFont(customDraft)}
                    />
                    <datalist id="font-choices">
                      {#each fontChoices as family (family)}<option value={family}></option>{/each}
                    </datalist>
                    {#if queryLocalFonts}
                      <Button onclick={browseFonts}>{t("settings.customFont.browse")}</Button>
                    {/if}
                    <Button variant="primary" onclick={applyCustomFont}>{t("settings.apply")}</Button>
                  </div>
                  <p style:font-family={fontStack("custom", customDraft)} class="font-preview">
                    {t("settings.customFont.preview")}
                  </p>
                  {#if customNote}<p class="muted note">{customNote}</p>{/if}
                </SettingRow>
              {/if}
              <SettingRow id="zoom" label={t("settings.zoom.label")} description={t("settings.zoom.description")}>
                <Slider
                  label={t("settings.zoom.label")}
                  min={ZOOM_MIN}
                  max={ZOOM_MAX}
                  step={ZOOM_STEP}
                  value={appearance.zoom}
                  format={formatPercent}
                  oncommit={(v) => appearance.setZoom(v).catch(apply)}
                />
              </SettingRow>
              <SettingRow id="density" label={t("settings.density.label")} description={t("settings.density.description")}>
                <Segmented
                  label={t("settings.density.label")}
                  value={appearance.density}
                  options={DENSITY_OPTIONS}
                  onchange={(value) => appearance.setDensity(value).catch(apply)}
                />
              </SettingRow>
              <ToggleRow
                id="row-separators" label={t("settings.rowSeparators.label")}
                description={t("settings.rowSeparators.description")}
                checked={appearance.rowSeparators}
                onchange={(enabled) => appearance.setRowSeparators(enabled).catch(apply)}
              />
              <SettingRow id="cover-shape" label={t("settings.coverShape.label")} description={t("settings.coverShape.description")}>
                <Segmented
                  label={t("settings.coverShape.label")}
                  value={appearance.coverShape}
                  options={COVER_OPTIONS}
                  onchange={(value) => appearance.setCoverShape(value).catch(apply)}
                />
              </SettingRow>
              {#if sortMemoryStore}
                <SettingRow
                  id="track-sort-memory"
                  label={t("settings.trackSortMemory.label")}
                  description={SORT_MEMORY_HELP[sortMemoryStore.mode]}
                >
                  <Segmented
                    label={t("settings.trackSortMemory.label")}
                    value={sortMemoryStore.mode}
                    options={SORT_MEMORY_OPTIONS}
                    onchange={(value) => sortMemoryStore.setMode(value)}
                  />
                </SettingRow>
              {/if}
              <ToggleRow
                id="home-search" label={t("settings.homeSearch.label")}
                description={t("settings.homeSearch.description")}
                checked={homeSearchFlag.shown}
                onchange={(enabled) => homeSearchFlag.setShown(enabled)}
              />
              {#if appearance.mode !== "rack"}
                <SettingRow
                  id="player-bar-style"
                  label={t("settings.playerBarStyle.label")}
                  description={t("settings.playerBarStyle.description")}
                >
                  <Segmented
                    label={t("settings.playerBarStyle.label")}
                    value={appearance.playerBarStyle}
                    options={PANEL_STYLE_OPTIONS}
                    onchange={(value) => appearance.setPlayerBarStyle(value).catch(apply)}
                  />
                </SettingRow>
                <SettingRow
                  id="queue-style"
                  label={t("settings.queueStyle.label")}
                  description={t("settings.queueStyle.description")}
                >
                  <Segmented
                    label={t("settings.queueStyle.label")}
                    value={appearance.queueStyle}
                    options={PANEL_STYLE_OPTIONS}
                    onchange={(value) => appearance.setQueueStyle(value).catch(apply)}
                  />
                </SettingRow>
                <ToggleRow
                  id="hover-panels" label={t("settings.hoverPanels.label")}
                  description={t("settings.hoverPanels.description")}
                  checked={appearance.hoverPanels}
                  onchange={setHoverPanels}
                />
              {/if}
            </SettingsSection>
            </div>

            <div class="col wide" style="order: 4">
            <SettingsSection title={t("settings.section.cat")} icon="paw-print">
              <ToggleRow
                id="cat" label={t("settings.cat.label")}
                description={appearance.reducedMotion ? REDUCED_MOTION_PAUSED : undefined}
                checked={config.ui.oneko}
                onchange={setOneko}
              />
              <ToggleRow
                id="catjam" label={t("settings.catjam.label")}
                description={appearance.reducedMotion ? REDUCED_MOTION_PAUSED : undefined}
                checked={config.ui.catjam}
                onchange={setCatjam}
              />
              {#if config.ui.catjam}
                <SettingRow id="catjam-coat" label={t("settings.catjamCoat.label")}>
                  <CoatPicker value={appearance.catjamCoat} onchange={setCatjamCoat} />
                </SettingRow>
                <SettingRow id="catjam-size" label={t("settings.catjamSize.label")}>
                  <Segmented
                    label={t("settings.catjamSizeLabel")}
                    value={String(appearance.catjamScale)}
                    options={CATJAM_SIZE_OPTIONS}
                    onchange={(value) => appearance.setCatjamScale(Number(value)).catch(apply)}
                  />
                </SettingRow>
              {/if}
              <SettingRow label={t("settings.credits.label")} stacked>
                <span class="muted credits">
                  <button class="link" type="button" onclick={() => openCredit("https://github.com/adryd325/oneko.js")}>{t("settings.creditBy", { work: "oneko.js", author: "adryd325" })}</button>
                  &amp;
                  <button class="link" type="button" onclick={() => openCredit("https://github.com/kyrie25/spicetify-oneko")}>{t("settings.creditBy", { work: "spicetify-oneko", author: "kyrie25" })}</button>
                </span>              </SettingRow>
            </SettingsSection>
            </div>
            </div>
          </div>
        {:else if tab === "playback"}
          <div id="settings-panel-playback" role="tabpanel" aria-labelledby="settings-tab-playback">
            {#if player.state.bit_perfect_notice}
              <Notice kind="warning">{engineNotice(player.state.bit_perfect_notice)}</Notice>
            {/if}
            {#if player.state.dsp_active}
              <Notice kind="info">
                {t("settings.dspActive", { stages: player.state.dsp_stages.join(", "), exclusive: selectedExclusive ? "yes" : "no" })}
              </Notice>
            {/if}

            <div class="cols">
            <div class="col">
            <SettingsSection title={t("settings.section.streaming")} icon="radio">
              <SettingRow id="quality" label={t("settings.quality.label")} description={t("settings.quality.description")}>
                <Select
                  label={t("settings.quality.label")}
                  value={config.streaming.quality}
                  options={QUALITY_OPTIONS}
                  onchange={setQuality}
                />
              </SettingRow>
              <ToggleRow
                id="allow-fallback" label={t("settings.allowFallback.label")}
                description={t("settings.allowFallback.description")}
                checked={config.streaming.allow_fallback}
                onchange={setAllowFallback}
              />
              <SettingRow
                id="quality-badge" label={t("settings.qualityBadge.label")}
                description={t("settings.qualityBadge.description")}
              >
                <Segmented
                  label={t("settings.qualityBadge.label")}
                  value={config.ui.quality_display}
                  options={QUALITY_DISPLAY_OPTIONS}
                  onchange={setDisplay}
                />
              </SettingRow>
            </SettingsSection>

            <SettingsSection title={t("settings.section.queue")} icon="list-music">
              <ToggleRow
                id="autoplay" label={t("settings.autoplay.label")}
                description={t("settings.autoplay.description")}
                checked={config.playback.autoplay}
                onchange={setAutoplay}
              />
            </SettingsSection>

            <SettingsSection title={t("settings.section.output")} icon="headphones">
              <SettingRow
                id="device" label={t("settings.device.label")} stacked
                description={t("settings.device.description")}
              >
                <OutputDevicePicker
                  icons={config.ui.device_icons ?? true}
                  onlist={onDeviceList}
                  onerror={apply}
                  beforeExclusive={withVolumeWarning}
                />
              </SettingRow>
              {#if anyExclusive}
                <SettingRow
                  id="exclusive-fallback" label={t("settings.exclusiveFallback.label")}
                  description={t("settings.exclusiveFallback.description")}
                >
                  <Segmented
                    label={t("settings.exclusiveFallback.label")}
                    value={config.audio.exclusive_fallback}
                    options={FALLBACK_OPTIONS}
                    onchange={setFallback}
                  />
                </SettingRow>
              {/if}
              {#if config.audio.output_device.startsWith("asio:")}
                <SettingRow label={t("settings.asioDriver.label")} stacked>
                  <Notice kind="info">
                    {t("settings.asioNotice")}
                  </Notice>
                </SettingRow>
              {/if}
              <ToggleRow
                id="device-icons" label={t("settings.deviceIcons.label")}
                description={t("settings.deviceIcons.description")}
                checked={config.ui.device_icons ?? true}
                onchange={setDeviceIcons}
              />
            </SettingsSection>
            {#if pendingFixedVolume}
              <ConfirmDialog
                title={t("settings.exclusiveVolume.title")}
                message={t("settings.exclusiveVolume.message")}
                confirmLabel={t("settings.exclusiveVolume.confirm")}
                destructive={false}
                optOutLabel={t("settings.dontShowAgain")}
                onconfirm={confirmFixedVolume}
                oncancel={() => (pendingFixedVolume = null)}
              />
            {/if}

            <SettingsSection
              title={t("settings.section.volumeLeveling")}
              description={t("settings.section.volumeLeveling.description")}
              icon="volume-2"
            >
              <SettingRow
                id="replaygain" label={t("settings.replaygain.label")}
                description={dsp.replay_gain.mode === "album"
                  ? t("settings.replaygain.albumHelp")
                  : t("settings.replaygain.description")}
              >
                <Segmented
                  label={t("settings.replaygain.label")}
                  value={dsp.replay_gain.mode}
                  options={REPLAY_GAIN_OPTIONS}
                  onchange={(mode) => setReplayGain({ mode })}
                />
              </SettingRow>
              <SettingRow
                id="rg-preamp" label={t("settings.rgPreamp.label")}
                description={t("settings.rgPreamp.description")}
                disabled={dsp.replay_gain.mode === "off"}
              >
                <Slider
                  label={t("settings.rgPreamp.label")}
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
                id="rg-untagged" label={t("settings.rgUntagged.label")}
                description={t("settings.rgUntagged.description")}
                disabled={dsp.replay_gain.mode === "off"}
              >
                <Slider
                  label={t("settings.rgUntagged.label")}
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
                id="rg-clipping" label={t("settings.rgClipping.label")}
                description={player.state.stream?.replay_gain_db != null && dsp.replay_gain.mode !== "off"
                  ? t("settings.rgClipping.current", { gain: formatDb(Math.round(player.state.stream.replay_gain_db * 10) / 10) })
                  : t("settings.rgClipping.description")}
                disabled={dsp.replay_gain.mode === "off"}
                checked={dsp.replay_gain.prevent_clipping}
                onchange={(prevent_clipping) => setReplayGain({ prevent_clipping })}
              />
            </SettingsSection>
            </div>
            <div class="col">
            <SettingsSection
              title={t("settings.section.equalizer")}
              description={t("settings.section.equalizer.description")}
              icon="sliders-vertical"
            >
              {#snippet actions()}
                <Toggle label={t("settings.section.equalizer")} checked={dsp.eq.enabled} onchange={(enabled) => setEq({ enabled })} />
              {/snippet}
              <SettingRow id="eq-preset" label={t("settings.eqPreset.label")} disabled={!dsp.eq.enabled}>
                <Select
                  label={t("settings.eqPresetLabel")}
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
                >{t("settings.reset")}</Button>
              </SettingRow>
              <SettingRow id="eq-bands" label={t("settings.eqBands.label")} disabled={!dsp.eq.enabled} stacked>
                <EqBands bands={dsp.eq.bands_db} disabled={!dsp.eq.enabled} onband={setBand} />
              </SettingRow>
              <ToggleRow
                id="eq-auto-preamp" label={t("settings.eqAutoPreamp.label")}
                description={t("settings.eqAutoPreamp.description")}
                disabled={!dsp.eq.enabled}
                checked={dsp.eq.auto_preamp}
                onchange={(auto_preamp) => setEq({ auto_preamp })}
              />
              <SettingRow id="eq-preamp" label={t("settings.eqPreamp.label")} disabled={!dsp.eq.enabled}>
                {#if dsp.eq.auto_preamp}
                  <span class="readout">{formatDb(effectivePreamp(dsp.eq))}</span>
                {:else}
                  <Slider
                    label={t("settings.eqPreampLabel")}
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

            <!-- Crossfade sits right above AutoMix, which builds on its length. -->
            <SettingsSection title={t("settings.section.crossfade")} icon="waves">
              <SettingRow
                id="crossfade" label={t("settings.crossfade.label")}
                description={t("settings.crossfade.description")}
              >
                <Slider
                  label={t("settings.crossfade.label")}
                  min={0}
                  max={CROSSFADE_MAX_MS}
                  step={500}
                  value={dsp.crossfade.duration_ms}
                  format={formatCrossfade}
                  oninput={(duration_ms) => setCrossfade({ duration_ms })}
                />
              </SettingRow>
              <ToggleRow
                id="gapless" label={t("settings.gapless.label")}
                description={t("settings.gapless.description")}
                disabled={dsp.crossfade.duration_ms === 0}
                checked={dsp.crossfade.skip_same_album}
                onchange={(skip_same_album) => setCrossfade({ skip_same_album })}
              />
            </SettingsSection>

            <SettingsSection
              title={t("settings.section.automixExperimental")}
              description={dsp.crossfade.duration_ms > 0
                ? t("settings.automix.description")
                : t("settings.automix.needsCrossfade")}
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
              {t("settings.contentNotice")}
            </Notice>
            <SettingsSection title={t("settings.section.filtersAndLabels")} icon="shield-check">
              <ToggleRow
                id="block-explicit" label={t("settings.blockExplicit.label")}
                description={t("settings.blockExplicit.description")}
                checked={config.content.block_explicit}
                onchange={setBlockExplicit}
              />
              <ToggleRow
                id="block-ai" label={t("settings.blockAi.label")}
                description={t("settings.blockAi.description")}
                checked={config.content.block_ai}
                onchange={setBlockAi}
              />
              <ToggleRow
                id="show-labels" label={t("settings.showLabels.label")}
                description={t("settings.showLabels.description")}
                checked={config.content.show_labels}
                onchange={setShowLabels}
              />
            </SettingsSection>
            {#if confirmingBlockExplicit}
              <ConfirmDialog
                title={t("settings.blockExplicit.label")}
                message={t("settings.blockExplicit.confirm")}
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
              title={t("settings.section.scrobbling")}
              description={t("settings.section.scrobbling.description")}
              icon="audio-lines"
            >
              {#if scrobble}
                <SettingRow
                  id="lastfm" label={t("settings.lastfm.label")}
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
                    <Loading label={t("settings.lastfmWaiting")} />
                    <Button variant="primary" onclick={() => void pollLastfm()}>{t("settings.lastfmAuthorized")}</Button>
                    <Button onclick={cancelLastfm}>{t("common.cancel")}</Button>
                  {:else if scrobble.lastfm.connected}
                    <Toggle
                      label={t("settings.scrobbleTo", { service: "Last.fm" })}
                      checked={scrobble.lastfm.enabled}
                      disabled={!canToggle(scrobble.lastfm)}
                      onchange={(enabled) => toggleService("lastfm", enabled)}
                    />
                    <Button onclick={disconnectLastfm}>{t("settings.disconnect")}</Button>
                  {:else}
                    <Button
                      variant="primary"
                      disabled={!scrobble.lastfm.configured}
                      onclick={() => void connectLastfm()}
                    >{t("settings.connect")}</Button>
                  {/if}
                </SettingRow>
                <SettingRow
                  id="listenbrainz" label={t("settings.listenbrainz.label")}
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
                      label={t("settings.scrobbleTo", { service: "ListenBrainz" })}
                      checked={scrobble.listenbrainz.enabled}
                      disabled={!canToggle(scrobble.listenbrainz)}
                      onchange={(enabled) => toggleService("listenbrainz", enabled)}
                    />
                    <Button onclick={disconnectListenbrainz}>{t("settings.disconnect")}</Button>
                  {:else}
                    <input
                      class="search-input token"
                      type="password"
                      placeholder={t("settings.listenbrainzToken")}
                      aria-label={t("settings.listenbrainzToken")}
                      bind:value={lbToken}
                    />
                    <Button
                      variant="primary"
                      disabled={lbBusy || !lbToken.trim()}
                      onclick={() => void connectListenbrainz()}
                    >{t("settings.connect")}</Button>
                  {/if}
                  {#snippet below()}
                    {#if !scrobble?.listenbrainz.connected}
                      {@const [before, after] = tSplit("settings.listenbrainzTokenHint", "profile")}
                      <span class="muted help-line">
                        {before}<button
                          class="link"
                          type="button"
                          onclick={() => openCredit("https://listenbrainz.org/profile/")}
                        >{t("settings.listenbrainzProfile")}</button>{after}
                      </span>
                    {/if}
                  {/snippet}
                </SettingRow>
              {:else}
                <SettingRow label={t("settings.loadingScrobblingStatus.label")} />
              {/if}
            </SettingsSection>

            <SettingsSection title={t("settings.section.presenceAndControls")} icon="radio">
              <ToggleRow
                id="discord" label={t("settings.discord.label")}
                description={t("settings.discord.description")}
                checked={config.integrations.discord}
                onchange={setDiscord}
              />
              <ToggleRow
                id="media-controls" label={t("settings.mediaControls.label")}
                description={t("settings.mediaControls.description")}
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
              title={t("settings.section.cache")}
              description={t("settings.section.cache.description")}
              icon="database"
            >
              {#snippet actions()}
                <Button icon="refresh-cw" onclick={refreshCacheStatus}>{t("playlist.refresh")}</Button>
                <Button variant="danger" icon="trash-2" onclick={clearCache}>{t("settings.clearCache")}</Button>
              {/snippet}
              {#if cacheStatus}
                {#if !cacheStatus.usable}
                  <div class="inset">
                    <Notice kind="warning">
                      {t("settings.cacheDisabled")}
                    </Notice>
                  </div>
                {:else if cacheStatus.configured_dir && cacheStatus.dir !== cacheStatus.configured_dir}
                  <div class="inset">
                    <Notice kind="warning">
                      {t("settings.cacheFallbackDir", { dir: cacheStatus.dir })}
                    </Notice>
                  </div>
                {/if}
                <SettingRow id="cache-usage" label={t("settings.cacheUsage.label")} stacked>
                  <div class="usage">
                    <div
                      class="meter"
                      role="meter"
                      aria-label={t("settings.cacheUsage.label")}
                      aria-valuemin="0"
                      aria-valuemax="100"
                      aria-valuenow={Math.round(cacheFraction * 100)}
                    >
                      <span class="fill" class:full={cacheFraction > 0.9} style:width="{cacheFraction * 100}%"></span>
                    </div>
                    <span class="muted">
                      {cacheStatus.pending_bytes > 0
                        ? t("settings.cacheUsedPending", { used: formatSize(cacheStatus.used_bytes), pending: formatSize(cacheStatus.pending_bytes), max: formatSize(cacheStatus.max_bytes) })
                        : t("settings.cacheUsed", { used: formatSize(cacheStatus.used_bytes), max: formatSize(cacheStatus.max_bytes) })}
                    </span>
                  </div>
                </SettingRow>
                <SettingRow id="cache-max" label={t("settings.cacheMax.label")} description={t("settings.cacheMax.description")}>
                  <NumberField
                    label={t("settings.cacheMaxLabel")}
                    min={0}
                    unit={t("settings.unitMb")}
                    value={config.cache.max_size_mb}
                    onchange={setCacheMaxMb}
                  />
                </SettingRow>
                <ToggleRow
                  id="cache-clear-exit" label={t("settings.cacheClearExit.label")}
                  checked={config.cache.clear_on_exit}
                  onchange={setClearOnExit}
                />
                <SettingRow
                  id="cache-location" label={t("settings.cacheLocation.label")}
                  description={t("settings.cacheLocation.description")}
                  stacked
                >
                  <code class="path">{cacheStatus.dir}</code>
                </SettingRow>
              {:else}
                <SettingRow label={t("settings.cacheStatusUnavailable.label")} />
              {/if}
            </SettingsSection>
            </div>
            <div class="col">
            <SettingsSection
              title={t("settings.section.listeningHistory")}
              description={t("settings.section.listeningHistory.description")}
              icon="history"
            >
              <ToggleRow
                id="history-record" label={t("settings.historyRecord.label")}
                checked={config.history.enabled}
                onchange={setHistoryEnabled}
              />
              <SettingRow id="history-max" label={t("settings.historyMax.label")} description={t("settings.historyMax.description")}>
                <NumberField
                  label={t("settings.historyMaxLabel")}
                  min={100}
                  max={100000}
                  unit={t("settings.unitPlays")}
                  value={config.history.max_entries}
                  onchange={setHistoryMaxEntries}
                />
              </SettingRow>
              <SettingRow id="history-clear" label={t("settings.historyClear.label")} description={t("settings.historyClear.description")}>
                <Button variant="danger" icon="trash-2" onclick={() => (confirmingHistoryClear = true)}>
                  {t("settings.historyClear.label")}
                </Button>
              </SettingRow>
            </SettingsSection>
            </div>
            </div>
            {#if confirmingHistoryClear}
              <ConfirmDialog
                title={t("settings.historyClear.title")}
                message={t("settings.historyClear.description")}
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
            <SettingsSection title={t("settings.section.windowAndStartup")} icon="app-window">
              <ToggleRow
                id="close-to-tray" label={t("settings.closeToTray.label")}
                description={t("settings.closeToTray.description")}
                checked={config.ui.close_to_tray}
                onchange={setCloseToTray}
              />
              <ToggleRow
                id="start-mini" label={t("settings.startMini.label")}
                description={t("settings.startMini.description")}
                checked={config.ui.start_in_mini}
                onchange={setStartInMini}
              />
            </SettingsSection>

            <SettingsSection
              title={t("settings.section.globalHotkeys")}
              description={t("settings.section.globalHotkeys.description")}
              icon="keyboard"
            >
              {#each HOTKEY_ACTIONS as entry (entry.id)}
                <SettingRow id={`hotkey-${entry.id}`} label={entry.label}>
                  <kbd class:unset={!config.hotkeys[entry.id]}>
                    {capture.recording === entry.id
                      ? t("settings.hotkeyPress")
                      : displayAccelerator(config.hotkeys[entry.id]) || t("settings.hotkeyUnset")}
                  </kbd>
                  <Button
                    id={`hotkey-${entry.id}`}
                    variant={capture.recording === entry.id ? "primary" : "secondary"}
                    onclick={() => void capture.start(entry.id)}
                  >{capture.recording === entry.id ? t("settings.hotkeyRecording") : t("settings.hotkeyRecord")}</Button>
                  <Button
                    disabled={!config.hotkeys[entry.id]}
                    onclick={() => void saveHotkey(entry.id, "")}
                  >{t("sort.clear")}</Button>
                </SettingRow>
              {/each}
            </SettingsSection>
            {#if hotkeyConflict}
              <ConfirmDialog
                title={t("settings.hotkeyConflict.title")}
                message={t("settings.hotkeyConflict.message", { keys: displayAccelerator(hotkeyConflict.accelerator), other: actionLabel(hotkeyConflict.other), action: actionLabel(hotkeyConflict.action) })}
                confirmLabel={t("settings.hotkeyConflict.confirm")}
                onconfirm={confirmReassign}
                oncancel={cancelReassign}
              />
            {/if}
            </div>
            <div class="col">
            <UpdatesSection />

            <SettingsSection
              id="shortcuts"
              title={t("settings.section.keyboardShortcuts")}
              description={t("settings.section.keyboardShortcuts.description")}
              icon="keyboard"
            >
              <ul class="shortcuts">
                {#each SHORTCUTS as shortcut (shortcut.keys)}
                  <li><kbd>{shortcut.keys}</kbd> <span>{shortcut.description}</span></li>
                {/each}
              </ul>
            </SettingsSection>

            <SettingsSection title={t("settings.section.troubleshooting")} icon="file-text">
              <SettingRow id="logs" label={t("settings.logs.label")} description={logsDir || t("settings.logsUnavailable")}>
                <Button icon="folder" onclick={() => api.openLogsFolder().catch(apply)}>
                  {t("settings.openLogs")}
                </Button>
              </SettingRow>
              <SettingRow
                id="diagnostics"
                label={t("settings.diagnostics.label")}
                description={t("settings.diagnostics.description")}
              >
                <Button icon="copy" onclick={copyDiagnostics}>{t("settings.copyDiagnostics")}</Button>
              </SettingRow>
            </SettingsSection>

            <SettingsSection title={t("settings.section.account")} icon="user">
              <SettingRow id="logout" label={t("settings.logout.label")} description={t("settings.logout.description")}>
                <Button variant="danger" icon="log-out" onclick={() => (confirmingLogout = true)}>{t("settings.logout.label")}</Button>
              </SettingRow>
            </SettingsSection>
            {#if confirmingLogout}
              <ConfirmDialog
                title={t("settings.logout.title")}
                message={t("settings.logout.message")}
                confirmLabel={t("settings.logout.label")}
                onconfirm={() => {
                  confirmingLogout = false;
                  onLogout();
                }}
                oncancel={() => (confirmingLogout = false)}
              />
            {/if}
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
  /* Windows contrast themes (A-3): the open tab is framed (its pill tint is
     dropped). */
  @media (forced-colors: active) {
    .tab.active {
      outline: 1px solid Highlight;
      outline-offset: -1px;
    }
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
    /* In flow, not sticky: `top` must go too, or it shifts the strip down
       onto the first card. It wraps rather than scrolls: a sideways scroll
       hid the last tab (System) with nothing to say it was there. */
    .nav {
      position: relative;
      top: 0;
      flex-direction: row;
      flex-wrap: wrap;
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
