import { HOTKEY_ACTIONS } from "./hotkeys";
import type { SettingsTab } from "./settingsTabs";

/// One searchable setting. `id` is the row's `data-setting` (or a card's, for
/// `shortcuts`); `needs` hides entries whose control is not on the page in the
/// current state, so a result never lands on a row that is not there.
export interface SettingEntry {
  id: string;
  tab: SettingsTab;
  section: string;
  label: string;
  keywords?: string;
  needs?: "rack" | "notRack" | "exclusive";
}

export interface SearchContext {
  rack: boolean;
  exclusive: boolean;
}

const e = (
  id: string,
  tab: SettingsTab,
  section: string,
  label: string,
  keywords = "",
  needs?: SettingEntry["needs"],
): SettingEntry => ({ id, tab, section, label, keywords, needs });

/// Every setting, in page order (ties in search keep this order). The ids are
/// checked against the page sources by `settingsIndex.test.ts`, so a renamed
/// or removed row cannot silently leave a dead result.
export const SETTINGS_INDEX: SettingEntry[] = [
  e("theme", "appearance", "Look & feel", "Theme", "dark light system night day colour color scheme"),
  e("mode", "appearance", "Look & feel", "Mode", "atmosphere instrument hi-fi rack style skin"),
  e("dynamic-color", "appearance", "Look & feel", "Dynamic color", "album art cover tint accent colour", "notRack"),
  e("typeface", "appearance", "Typography & layout", "Typeface", "font text manrope inter system custom"),
  e("density", "appearance", "Typography & layout", "Density", "compact comfortable spacing rows size"),
  e("cover-shape", "appearance", "Typography & layout", "Cover shape", "disc sleeve artwork album art now playing"),
  e("rack-deck", "appearance", "Hi-Fi Rack", "Deck face", "transport cd player display", "rack"),
  e("rack-meters", "appearance", "Hi-Fi Rack", "Amp meters", "needles bars led spectrum vu", "rack"),
  e("rack-analyzer", "appearance", "Hi-Fi Rack", "Now Playing analyzer", "spectrum recorder tuner eq", "rack"),
  e("rack-lamp", "appearance", "Hi-Fi Rack", "Lamp color", "amber blue green red white cover glow colour", "rack"),
  e("rack-brightness", "appearance", "Hi-Fi Rack", "Lamp brightness", "dim glow", "rack"),
  e("rack-eq", "appearance", "Hi-Fi Rack", "Show the graphic EQ unit", "equalizer rack", "rack"),
  e("cat", "appearance", "Extras", "Cat", "oneko pet fun cursor"),

  e("quality", "playback", "Streaming", "Streaming quality", "audio hifi hi-res lossless max high aac flac bitrate"),
  e("allow-fallback", "playback", "Streaming", "Use the best available quality", "fallback unavailable"),
  e("quality-badge", "playback", "Streaming", "Quality badge", "display compact detailed bit depth sample rate"),
  e("device", "playback", "Output", "Audio device", "output speakers dac headphones sound card wasapi asio"),
  e("exclusive", "playback", "Output", "Bit-perfect output (exclusive mode)", "wasapi asio exclusive mixer unchanged"),
  e("exclusive-fallback", "playback", "Output", "If unsupported", "exclusive fallback shared", "exclusive"),
  e("replaygain", "playback", "Volume leveling", "ReplayGain", "loudness normalize normalise volume leveling track album gain"),
  e("rg-preamp", "playback", "Volume leveling", "Preamp", "replaygain gain boost"),
  e("rg-untagged", "playback", "Volume leveling", "Tracks without data", "replaygain untagged missing"),
  e("rg-clipping", "playback", "Volume leveling", "Prevent clipping", "replaygain peak distortion"),
  e("crossfade", "playback", "Crossfade", "Crossfade", "fade overlap transition seconds gapless"),
  e("gapless", "playback", "Crossfade", "Keep albums gapless", "album crossfade"),
  e("eq-preset", "playback", "Equalizer", "Equalizer", "eq preset bass treble vocal loudness tone sound"),
  e("eq-bands", "playback", "Equalizer", "Equalizer bands", "eq faders frequency hz gain"),
  e("eq-auto-preamp", "playback", "Equalizer", "Automatic preamp", "eq clip headroom"),
  e("eq-preamp", "playback", "Equalizer", "Equalizer preamp", "eq gain"),
  e("automix-style", "playback", "AutoMix", "AutoMix transition style", "classic quick blend auto crossfade"),
  e("automix-trim", "playback", "AutoMix", "Skip silence at track edges", "automix trim"),
  e("automix-resample", "playback", "AutoMix", "Overlap across sample-rate changes", "automix resample"),
  e("automix-beat", "playback", "AutoMix", "Beat-match when tempos fit", "automix bpm tempo"),
  e("automix-replay", "playback", "AutoMix", "Replay last transition", "automix lab compare"),

  e("block-explicit", "content", "Filters & labels", "Block explicit content", "filter parental clean"),
  e("block-ai", "content", "Filters & labels", "Block AI-generated content", "filter artificial"),
  e("show-labels", "content", "Filters & labels", "Show content labels", "explicit ai badge tag e"),

  e("lastfm", "connections", "Scrobbling", "Last.fm", "scrobble lastfm listening profile connect"),
  e("listenbrainz", "connections", "Scrobbling", "ListenBrainz", "scrobble token listening profile connect"),
  e("discord", "connections", "Presence & controls", "Discord Rich Presence", "status activity"),
  e("media-controls", "connections", "Presence & controls", "Media controls", "smtc media keys hardware overlay"),
  e("remote-enable", "connections", "Phone remote", "Control OxiTide from your phone", "remote phone lan web"),
  e("remote-pair", "connections", "Phone remote", "Pair a phone", "remote qr code"),
  e("remote-address", "connections", "Phone remote", "Address in the QR code", "remote network interface ip"),
  e("remote-port", "connections", "Phone remote", "Port", "remote network"),
  e("remote-approve", "connections", "Phone remote", "Approve new phones on this PC", "remote security"),
  e("remote-devices", "connections", "Phone remote", "Paired phones", "remote revoke rename devices"),

  e("cache-usage", "storage", "Cache", "Disk usage", "cache clear refresh space downloads"),
  e("cache-max", "storage", "Cache", "Maximum cache size", "cache limit mb gb"),
  e("cache-clear-exit", "storage", "Cache", "Clear cache on exit", "cache"),
  e("cache-location", "storage", "Cache", "Cache location", "cache directory folder path"),
  e("history-record", "storage", "Listening history", "Record listening history", "stats plays privacy"),
  e("history-max", "storage", "Listening history", "Maximum history entries", "plays limit"),
  e("history-clear", "storage", "Listening history", "Clear history", "delete plays stats"),

  e("close-to-tray", "system", "Window & startup", "Close to tray", "minimize background quit"),
  e("start-mini", "system", "Window & startup", "Start in mini player", "startup launch compact"),
  e("updates-check", "system", "Updates", "Check for updates", "version upgrade release install"),
  e("updates-start", "system", "Updates", "Check for updates when OxiTide starts", "startup automatic"),
  ...HOTKEY_ACTIONS.map((a) =>
    e(`hotkey-${a.id}`, "system", "Global hotkeys", `Hotkey: ${a.label}`, "global shortcut keybinding record"),
  ),
  e("shortcuts", "system", "Keyboard shortcuts", "Keyboard shortcuts", "keys bindings reference seek volume mute"),
  e("logs", "system", "Troubleshooting", "Logs", "debug folder diagnostics"),
  e("logout", "system", "Account", "Log out", "sign out account tidal"),
];

function visible(entry: SettingEntry, ctx: SearchContext): boolean {
  if (entry.needs === "rack") return ctx.rack;
  if (entry.needs === "notRack") return !ctx.rack;
  if (entry.needs === "exclusive") return ctx.exclusive;
  return true;
}

function words(text: string): string[] {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .split(" ")
    .filter(Boolean);
}

/// How well one query token matches an entry, 0 = not at all. A label word
/// beats a keyword beats the section name; a prefix beats a mid-word hit.
function tokenScore(token: string, entry: SettingEntry): number {
  const label = words(entry.label);
  if (label.some((w) => w === token)) return 6;
  if (label.some((w) => w.startsWith(token))) return 5;
  if (entry.label.toLowerCase().includes(token)) return 3;
  const keys = words(entry.keywords ?? "");
  if (keys.some((w) => w === token)) return 4;
  if (keys.some((w) => w.startsWith(token))) return 3;
  const section = words(entry.section);
  if (section.some((w) => w.startsWith(token))) return 2;
  if (keys.join(" ").includes(token)) return 1;
  return 0;
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
