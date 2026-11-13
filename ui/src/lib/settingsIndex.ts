import { HOTKEY_ACTIONS } from "./hotkeys";
import { i18n, t, tEn, type MessageKey } from "./i18n.svelte";
import type { SettingsTab } from "./settingsTabs";

/// One setting's searchable text in one language.
export interface EntryText {
  section: string;
  label: string;
  keywords: string;
}

/// One searchable setting. `id` is the row's `data-setting` (or a card's, for
/// `shortcuts`); `needs` hides entries whose control is not on the page in the
/// current state, so a result never lands on a row that is not there.
/// `section` / `label` / `keywords` follow the language; `en` is the English,
/// which search also matches (spec §4.5).
export interface SettingEntry extends EntryText {
  id: string;
  tab: SettingsTab;
  en: EntryText;
  needs?: "rack" | "notRack" | "studio" | "exclusive";
}

export interface SearchContext {
  rack: boolean;
  /// Studio mode (`instrument`): its Tone row is on the page.
  studio: boolean;
  exclusive: boolean;
}

type Text = (tr: (key: MessageKey, params?: Record<string, string>) => string) => string;

function entry(id: string, tab: SettingsTab, section: Text, label: Text, keywords: Text, needs?: SettingEntry["needs"]): SettingEntry {
  return {
    id,
    tab,
    needs,
    get section() { return section(t); },
    get label() { return label(t); },
    get keywords() { return keywords(t); },
    en: {
      get section() { return section(tEn); },
      get label() { return label(tEn); },
      get keywords() { return keywords(tEn); },
    },
  };
}

const e = (
  id: string,
  tab: SettingsTab,
  section: MessageKey,
  label: MessageKey,
  keywords: MessageKey,
  needs?: SettingEntry["needs"],
): SettingEntry => entry(id, tab, (tr) => tr(section), (tr) => tr(label), (tr) => tr(keywords), needs);

/// Every setting, in page order (ties in search keep this order). The ids are
/// checked against the page sources by `settingsIndex.test.ts`, so a renamed
/// or removed row cannot silently leave a dead result.
export const SETTINGS_INDEX: SettingEntry[] = [
  e("language", "appearance", "settings.section.lookAndFeel", "settings.language.label", "settings.keywords.language"),
  e("theme", "appearance", "settings.section.lookAndFeel", "settings.theme.label", "settings.keywords.theme"),
  e("mode", "appearance", "settings.section.lookAndFeel", "settings.mode.label", "settings.keywords.mode"),
  e("studio-tone", "appearance", "settings.section.lookAndFeel", "settings.studioTone.label", "settings.keywords.studioTone", "studio"),
  e("dynamic-color", "appearance", "settings.section.lookAndFeel", "settings.dynamicColor.label", "settings.keywords.dynamicColor", "notRack"),
  e("typeface", "appearance", "settings.section.typographyAndLayout", "settings.typeface.label", "settings.keywords.typeface"),
  e("density", "appearance", "settings.section.typographyAndLayout", "settings.density.label", "settings.keywords.density"),
  e("row-separators", "appearance", "settings.section.typographyAndLayout", "settings.rowSeparators.label", "settings.keywords.rowSeparators"),
  e("cover-shape", "appearance", "settings.section.typographyAndLayout", "settings.coverShape.label", "settings.keywords.coverShape"),
  e("track-sort-memory", "appearance", "settings.section.typographyAndLayout", "settings.trackSortMemory.label", "settings.keywords.trackSortMemory"),
  e("player-bar-style", "appearance", "settings.section.typographyAndLayout", "settings.playerBarStyle.label", "settings.keywords.playerBarStyle", "notRack"),
  e("queue-style", "appearance", "settings.section.typographyAndLayout", "settings.queueStyle.label", "settings.keywords.queueStyle", "notRack"),
  e("hover-panels", "appearance", "settings.section.typographyAndLayout", "settings.hoverPanels.label", "settings.keywords.hoverPanels", "notRack"),
  e("rack-deck", "appearance", "settings.section.hiFiRack", "settings.rackDeck.label", "settings.keywords.rackDeck", "rack"),
  e("rack-meters", "appearance", "settings.section.hiFiRack", "settings.rackMeters.label", "settings.keywords.rackMeters", "rack"),
  e("rack-analyzer", "appearance", "settings.section.hiFiRack", "settings.rackAnalyzer.label", "settings.keywords.rackAnalyzer", "rack"),
  e("rack-lamp", "appearance", "settings.section.hiFiRack", "settings.rackLamp.label", "settings.keywords.rackLamp", "rack"),
  e("rack-brightness", "appearance", "settings.section.hiFiRack", "settings.rackBrightness.label", "settings.keywords.rackBrightness", "rack"),
  e("rack-eq", "appearance", "settings.section.hiFiRack", "settings.rackEq.label", "settings.keywords.rackEq", "rack"),
  e("cat", "appearance", "settings.section.cat", "settings.cat.label", "settings.keywords.cat"),
  e("catjam", "appearance", "settings.section.cat", "settings.catjam.label", "settings.keywords.catjam"),

  e("quality", "playback", "settings.section.streaming", "settings.quality.label", "settings.keywords.quality"),
  e("allow-fallback", "playback", "settings.section.streaming", "settings.allowFallback.label", "settings.keywords.allowFallback"),
  e("quality-badge", "playback", "settings.section.streaming", "settings.qualityBadge.label", "settings.keywords.qualityBadge"),
  e("autoplay", "playback", "settings.section.queue", "settings.autoplay.label", "settings.keywords.autoplay"),
  e("device", "playback", "settings.section.output", "settings.device.label", "settings.keywords.device"),
  e("exclusive-fallback", "playback", "settings.section.output", "settings.exclusiveFallback.label", "settings.keywords.exclusiveFallback", "exclusive"),
  e("device-icons", "playback", "settings.section.output", "settings.deviceIcons.label", "settings.keywords.deviceIcons"),
  e("replaygain", "playback", "settings.section.volumeLeveling", "settings.replaygain.label", "settings.keywords.replaygain"),
  e("rg-preamp", "playback", "settings.section.volumeLeveling", "settings.rgPreamp.label", "settings.keywords.rgPreamp"),
  e("rg-untagged", "playback", "settings.section.volumeLeveling", "settings.rgUntagged.label", "settings.keywords.rgUntagged"),
  e("rg-clipping", "playback", "settings.section.volumeLeveling", "settings.rgClipping.label", "settings.keywords.rgClipping"),
  e("crossfade", "playback", "settings.section.crossfade", "settings.crossfade.label", "settings.keywords.crossfade"),
  e("gapless", "playback", "settings.section.crossfade", "settings.gapless.label", "settings.keywords.gapless"),
  e("eq-preset", "playback", "settings.section.equalizer", "settings.section.equalizer", "settings.keywords.eqPreset"),
  e("eq-bands", "playback", "settings.section.equalizer", "settings.eqBandsLabel", "settings.keywords.eqBands"),
  e("eq-auto-preamp", "playback", "settings.section.equalizer", "settings.eqAutoPreamp.label", "settings.keywords.eqAutoPreamp"),
  e("eq-preamp", "playback", "settings.section.equalizer", "settings.eqPreampLabel", "settings.keywords.eqPreamp"),
  e("automix-style", "playback", "settings.section.automix", "settings.search.automixStyle", "settings.keywords.automixStyle"),
  e("automix-trim", "playback", "settings.section.automix", "settings.automixTrim.label", "settings.keywords.automixTrim"),
  e("automix-resample", "playback", "settings.section.automix", "settings.automixResample.label", "settings.keywords.automixResample"),
  e("automix-beat", "playback", "settings.section.automix", "settings.automixBeat.label", "settings.keywords.automixBeat"),
  e("automix-replay", "playback", "settings.section.automix", "settings.search.automixReplay", "settings.keywords.automixReplay"),

  e("block-explicit", "content", "settings.section.filtersAndLabels", "settings.blockExplicit.label", "settings.keywords.blockExplicit"),
  e("block-ai", "content", "settings.section.filtersAndLabels", "settings.blockAi.label", "settings.keywords.blockAi"),
  e("show-labels", "content", "settings.section.filtersAndLabels", "settings.showLabels.label", "settings.keywords.showLabels"),

  e("lastfm", "connections", "settings.section.scrobbling", "settings.lastfm.label", "settings.keywords.lastfm"),
  e("listenbrainz", "connections", "settings.section.scrobbling", "settings.listenbrainz.label", "settings.keywords.listenbrainz"),
  e("discord", "connections", "settings.section.presenceAndControls", "settings.discord.label", "settings.keywords.discord"),
  e("media-controls", "connections", "settings.section.presenceAndControls", "settings.mediaControls.label", "settings.keywords.mediaControls"),
  e("remote-enable", "connections", "settings.section.phoneRemote", "settings.remoteEnable.label", "settings.keywords.remoteEnable"),
  e("remote-pair", "connections", "settings.section.phoneRemote", "settings.remotePair.label", "settings.keywords.remotePair"),
  e("remote-address", "connections", "settings.section.phoneRemote", "settings.remoteAddress.label", "settings.keywords.remoteAddress"),
  e("remote-port", "connections", "settings.section.phoneRemote", "settings.remotePort.label", "settings.keywords.remotePort"),
  e("remote-approve", "connections", "settings.section.phoneRemote", "settings.remoteApprove.label", "settings.keywords.remoteApprove"),
  e("remote-devices", "connections", "settings.section.phoneRemote", "settings.remoteDevices.label", "settings.keywords.remoteDevices"),

  e("cache-usage", "storage", "settings.section.cache", "settings.cacheUsage.label", "settings.keywords.cacheUsage"),
  e("cache-max", "storage", "settings.section.cache", "settings.cacheMaxLabel", "settings.keywords.cacheMax"),
  e("cache-clear-exit", "storage", "settings.section.cache", "settings.cacheClearExit.label", "settings.keywords.cacheClearExit"),
  e("cache-location", "storage", "settings.section.cache", "settings.search.cacheLocation", "settings.keywords.cacheLocation"),
  e("history-record", "storage", "settings.section.listeningHistory", "settings.historyRecord.label", "settings.keywords.historyRecord"),
  e("history-max", "storage", "settings.section.listeningHistory", "settings.historyMaxLabel", "settings.keywords.historyMax"),
  e("history-clear", "storage", "settings.section.listeningHistory", "settings.historyClear.label", "settings.keywords.historyClear"),

  e("close-to-tray", "system", "settings.section.windowAndStartup", "settings.closeToTray.label", "settings.keywords.closeToTray"),
  e("start-mini", "system", "settings.section.windowAndStartup", "settings.startMini.label", "settings.keywords.startMini"),
  e("updates-check", "system", "settings.section.updates", "settings.updatesCheck.label", "settings.keywords.updatesCheck"),
  e("updates-start", "system", "settings.section.updates", "settings.updatesStart.label", "settings.keywords.updatesStart"),
  ...HOTKEY_ACTIONS.map((a) =>
    entry(
      `hotkey-${a.id}`,
      "system",
      (tr) => tr("settings.section.globalHotkeys"),
      (tr) => tr("settings.search.hotkey", { action: tr(a.key) }),
      (tr) => tr("settings.keywords.hotkey"),
    ),
  ),
  e("shortcuts", "system", "settings.section.keyboardShortcuts", "settings.section.keyboardShortcuts", "settings.keywords.shortcuts"),
  e("logs", "system", "settings.section.troubleshooting", "settings.logs.label", "settings.keywords.logs"),
  e("diagnostics", "system", "settings.section.troubleshooting", "settings.diagnostics.label", "settings.keywords.diagnostics"),
  e("logout", "system", "settings.section.account", "settings.logout.label", "settings.keywords.logout"),
];

function visible(entry: SettingEntry, ctx: SearchContext): boolean {
  if (entry.needs === "rack") return ctx.rack;
  if (entry.needs === "notRack") return !ctx.rack;
  if (entry.needs === "studio") return ctx.studio;
  if (entry.needs === "exclusive") return ctx.exclusive;
  return true;
}

function words(text: string): string[] {
  return text
    .toLocaleLowerCase(i18n.locale)
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .split(" ")
    .filter(Boolean);
}

/// How well one query token matches an entry's text, 0 = not at all. A label
/// word beats a keyword beats the section name; a prefix beats a mid-word hit.
/// The substring checks also cover scripts written without spaces (CJK).
function textScore(token: string, entry: EntryText): number {
  const label = words(entry.label);
  if (label.some((w) => w === token)) return 6;
  if (label.some((w) => w.startsWith(token))) return 5;
  if (entry.label.toLocaleLowerCase(i18n.locale).includes(token)) return 3;
  const keys = words(entry.keywords);
  if (keys.some((w) => w === token)) return 4;
  if (keys.some((w) => w.startsWith(token))) return 3;
  const section = words(entry.section);
  if (section.some((w) => w.startsWith(token))) return 2;
  if (keys.join(" ").includes(token)) return 1;
  return 0;
}

/// The better of the current language's and the English text.
function tokenScore(token: string, entry: SettingEntry): number {
  const own = textScore(token, entry);
  return i18n.language === "en" ? own : Math.max(own, textScore(token, entry.en));
}

/// The settings that match every word of `query`, best first (page order
/// breaks ties). Empty for a blank query.
export function searchSettings(
  query: string,
  ctx: SearchContext,
  entries: SettingEntry[] = SETTINGS_INDEX,
  limit = 8,
): SettingEntry[] {
  const tokens = words(query);
  if (tokens.length === 0) return [];
  return entries
    .map((entry, order) => {
      if (!visible(entry, ctx)) return { entry, order, score: 0 };
      let score = 0;
      for (const token of tokens) {
        const s = tokenScore(token, entry);
        if (s === 0) return { entry, order, score: 0 };
        score += s;
      }
      return { entry, order, score };
    })
    .filter((hit) => hit.score > 0)
    .sort((a, b) => b.score - a.score || a.order - b.order)
    .slice(0, limit)
    .map((hit) => hit.entry);
}
