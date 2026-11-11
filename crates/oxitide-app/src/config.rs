use std::fs;
use std::path::{Path, PathBuf};

use oxitide_audio::dsp::DspSettings;
use oxitide_audio::output::{ExclusiveFallback, OutputMode};
use serde::{Deserialize, Serialize};

use crate::player::QualityDto;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub audio: AudioConfig,
    pub streaming: StreamingConfig,
    pub ui: UiConfig,
    pub cache: CacheConfig,
    pub integrations: IntegrationsConfig,
    pub scrobbling: ScrobblingConfig,
    pub history: HistoryConfig,
    pub content: ContentConfig,
    pub hotkeys: HotkeysConfig,
    /// Sound processing (Track 7): EQ, ReplayGain, crossfade. All off by default.
    pub dsp: DspSettings,
    /// Hi-Fi Rack mode options (Track 9.5).
    pub rack: RackConfig,
    /// Phone remote (Track 11). Off by default.
    pub remote: RemoteConfig,
    /// Update checks (Track 12). On by default.
    pub updates: UpdatesConfig,
    /// Queue behaviour (Autoplay).
    pub playback: PlaybackConfig,
}

/// `[playback]`: what happens around the queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    /// Keep playing similar tracks when the queue ends (TIDAL parity: on).
    pub autoplay: bool,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self { autoplay: true }
    }
}

pub const REMOTE_DEFAULT_PORT: u16 = 47820;

/// Phone remote preferences (`[remote]`, spec §5.3). Paired devices live in
/// `remote.json`, never here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteConfig {
    pub enabled: bool,
    /// First port tried; the server falls back through `port + 9`.
    pub port: u16,
    /// Address for the pairing QR; empty picks automatically.
    pub interface: String,
    /// New phones need a click on the PC before they pair.
    pub approve_new_devices: bool,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: REMOTE_DEFAULT_PORT,
            interface: String::new(),
            approve_new_devices: true,
        }
    }
}

/// Keep the port out of the privileged range and leave room for the
/// nine-port fallback.
pub fn clamp_remote_port(port: u16) -> u16 {
    port.clamp(1024, u16::MAX - 9)
}

/// One output device's picker settings (`[audio.devices."<id>"]`, spec
/// `2026-11-05-oxitide-output-device-picker-design.md` §6.3). Everything is
/// optional; an entry holding only defaults is not kept unless the device is
/// selected or pinned (it then remembers the device's last name).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DevicePrefs {
    /// The clean name last seen, for a "Not connected" row.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The system's full name last seen (id-drift matching).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub system_name: String,
    /// The connection last seen (`usb`, `hdmi`, …; id-drift matching).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub transport: String,
    /// Display-only name; empty shows the device's own name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nickname: String,
    /// `""` = from the device type, `"none"`, or a Lucide icon name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    #[serde(skip_serializing_if = "is_shared")]
    pub mode: OutputMode,
}

fn is_shared(mode: &OutputMode) -> bool {
    *mode == OutputMode::Shared
}

impl DevicePrefs {
    /// True when nothing the user chose is stored.
    pub fn is_default_choice(&self) -> bool {
        self.nickname.is_empty()
            && self.icon.is_empty()
            && !self.hidden
            && self.mode == OutputMode::Shared
    }
}

/// Icons a device can be given (`DevicePrefs::icon`).
pub const DEVICE_ICONS: &[&str] = &[
    "",
    "none",
    "speaker",
    "headphones",
    "radio-receiver",
    "monitor",
    "tv",
    "laptop",
    "usb",
    "bluetooth",
    "audio-lines",
    "cable",
    "radio",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Empty string means the system default device.
    pub output_device: String,
    /// The old global exclusive switch. Read once and moved onto the selected
    /// device (`migrate_devices`); never written.
    #[serde(skip_serializing)]
    pub exclusive: bool,
    pub exclusive_fallback: ExclusiveFallback,
    /// Slider position; the engine applies the audio taper
    /// (`oxitide_audio::output::volume_gain`) when it scales samples.
    pub volume: f32,
    /// Set once `volume` is a tapered slider position. Older configs stored
    /// it as a linear gain; `Config::load_from` converts them so a saved
    /// level keeps its loudness. Missing from a file means old (`false`),
    /// a fresh config starts tapered.
    #[serde(default)]
    pub volume_tapered: bool,
    /// Ask before turning exclusive mode on: it sets the in-app volume to
    /// 100% and leaves the level to the device. Off once the user ticks
    /// "Don't show again".
    pub exclusive_volume_warning: bool,
    /// Devices in the quality menu, in pin order.
    pub pinned: Vec<String>,
    /// Per-device settings, by device id (`""` is System default). Last, so
    /// TOML writes it as sub-tables after the plain values.
    pub devices: std::collections::BTreeMap<String, DevicePrefs>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device: String::new(),
            exclusive: false,
            exclusive_fallback: ExclusiveFallback::Shared,
            volume: 1.0,
            volume_tapered: true,
            exclusive_volume_warning: true,
            pinned: Vec::new(),
            devices: std::collections::BTreeMap::new(),
        }
    }
}

impl AudioConfig {
    /// The mode `id` plays in. ASIO is always exclusive.
    pub fn mode_for(&self, id: &str) -> OutputMode {
        if oxitide_audio::output::is_asio_device(id) {
            return OutputMode::Exclusive;
        }
        self.devices
            .get(id)
            .map(|prefs| prefs.mode)
            .unwrap_or_default()
    }

    /// Drop entries that hold no choice and belong to no selected or pinned
    /// device.
    pub fn prune_devices(&mut self) {
        let keep: std::collections::HashSet<String> = self
            .pinned
            .iter()
            .cloned()
            .chain(std::iter::once(self.output_device.clone()))
            .collect();
        self.devices
            .retain(|id, prefs| !prefs.is_default_choice() || keep.contains(id));
    }

    /// The old global `exclusive = true` becomes the selected device's mode
    /// (spec DP-D3); every other device starts Shared.
    fn migrate_exclusive(&mut self) {
        if self.exclusive {
            let id = self.output_device.clone();
            let prefs = self.devices.entry(id).or_default();
            prefs.mode = OutputMode::Exclusive;
            self.exclusive = false;
        }
    }

    /// Saved ALSA routes (`sysdefault:`, `plughw:`, `default`, numeric `hw:`)
    /// become the one-row-per-PCM ids (spec §6.3), across the selection, the
    /// pins and the per-device settings.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn migrate_alsa_ids(&mut self, card_ids: &[String]) {
        use oxitide_audio::devices::migrate_alsa_id;
        self.output_device = migrate_alsa_id(&self.output_device, card_ids);
        let mut pinned = Vec::new();
        for id in self
            .pinned
            .drain(..)
            .map(|id| migrate_alsa_id(&id, card_ids))
        {
            if !id.is_empty() && !pinned.contains(&id) {
                pinned.push(id);
            }
        }
        self.pinned = pinned;
        let devices = std::mem::take(&mut self.devices);
        for (id, prefs) in devices {
            self.devices
                .entry(migrate_alsa_id(&id, card_ids))
                .or_insert(prefs);
        }
    }

    fn migrate_devices(&mut self) {
        #[cfg(target_os = "linux")]
        self.migrate_alsa_ids(&oxitide_audio::alsa_output::card_ids());
        self.migrate_exclusive();
    }

    /// A linear-gain volume from before the taper becomes the slider position
    /// with the same gain.
    fn migrate_volume(&mut self) {
        if !self.volume_tapered {
            self.volume = oxitide_audio::output::volume_position(self.volume);
            self.volume_tapered = true;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamingConfig {
    pub quality: QualityDto,
    /// Play the best available quality and report it instead of erroring.
    pub allow_fallback: bool,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            quality: QualityDto::Lossless,
            allow_fallback: true,
        }
    }
}

/// How the player bar renders the stream quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityDisplay {
    #[default]
    Compact,
    Detailed,
}

/// Visual mode. Plan 10 ships Instrument; Plan 11 adds `Atmosphere` and flips
/// the default. Instrument is labelled "Studio" in the UI; the stored id stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiMode {
    Instrument,
    #[default]
    Atmosphere,
    /// Hi-Fi Rack (Track 9.5): its own shell and custom units.
    Rack,
}

/// What the compact Rack deck shows (RK-D5). The expanded deck is always the
/// turntable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeckFace {
    Transport,
    Cd,
    #[default]
    Display,
}

/// The amplifier's meter style (RK-D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeterStyle {
    #[default]
    Needles,
    Bars,
    Led,
    Spectrum,
}

/// The expanded deck's extra unit (between the amp and Now playing). `Auto`
/// shows the level recorder when the amp already shows a spectrum, otherwise
/// the spectrum, so the two never duplicate. Kept after the owner's live
/// comparison (2026-10-09): spectrum, recorder, tuner, eq.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalyzerStyle {
    /// Briefly offered options that were dropped ("none", "cassette") load as Auto.
    #[default]
    #[serde(alias = "none", alias = "cassette")]
    Auto,
    Spectrum,
    /// Chart-recorder strip of L/R programme level. Briefly shipped as a
    /// waterfall, then a stereo scope; both values load as the recorder.
    #[serde(alias = "waterfall", alias = "scope")]
    Recorder,
    /// FM-tuner dial whose pointer travels the track's timeline.
    Tuner,
    /// The graphic EQ, reachable inside Now Playing.
    Eq,
}

/// Lamp colour (RK-D8): five presets or the cover's hue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LampColor {
    #[default]
    Amber,
    Blue,
    Green,
    Red,
    White,
    Cover,
}

pub const LAMP_BRIGHTNESS_MIN: u8 = 40;
pub const LAMP_BRIGHTNESS_MAX: u8 = 100;

pub fn clamp_lamp_brightness(value: u8) -> u8 {
    value.clamp(LAMP_BRIGHTNESS_MIN, LAMP_BRIGHTNESS_MAX)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RackConfig {
    pub deck_face: DeckFace,
    pub meters: MeterStyle,
    pub analyzer: AnalyzerStyle,
    pub lamp: LampColor,
    /// Show the optional graphic-EQ unit in the rack column.
    pub eq_unit: bool,
    /// Percent, 40–100: scales lamp lightness and glow.
    pub lamp_brightness: u8,
}

impl Default for RackConfig {
    fn default() -> Self {
        Self {
            deck_face: DeckFace::Display,
            meters: MeterStyle::Needles,
            analyzer: AnalyzerStyle::Auto,
            lamp: LampColor::Amber,
            eq_unit: true,
            lamp_brightness: LAMP_BRIGHTNESS_MAX,
        }
    }
}

impl RackConfig {
    pub fn clamped(mut self) -> Self {
        self.lamp_brightness = clamp_lamp_brightness(self.lamp_brightness);
        self
    }
}

/// Color theme. Orthogonal to `UiMode`; `System` follows the OS preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

/// Studio (Instrument) surface colours in the dark theme: the warm brown
/// original, neutral greys, or pure black for OLED screens. Light keeps its
/// warm paper whatever this says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstrumentTone {
    #[default]
    Warm,
    Neutral,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Typeface {
    /// Mode default: Manrope in Atmosphere, Inter in Instrument.
    #[default]
    Auto,
    Manrope,
    Inter,
    System,
    Custom,
}

/// Dynamic cover color (spec §5.4). Full by default in every mode (Rack ignores
/// it); the former per-mode `auto` reads back as Full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DynamicColor {
    #[default]
    #[serde(alias = "auto")]
    Full,
    Accent,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverShape {
    #[default]
    Disc,
    Sleeve,
}

/// How the player bar and the queue sit in the default shell (Atmosphere and
/// Instrument; Rack has its own shell): docked to the window edges, or
/// floating as inset cards over the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelStyle {
    #[default]
    Docked,
    Floating,
}

/// The jamming cat's coat (CatJam, CJ-D12); mirrors `COAT_IDS` in
/// `ui/src/lib/catjamSprite.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatjamCoat {
    #[default]
    White,
    Black,
    Ginger,
    Grey,
    Tuxedo,
    Cream,
}

/// The jamming cat's size in percent of its 32 px sprite. Capped so it fits
/// the player bar.
pub fn clamp_catjam_scale(percent: u32) -> u32 {
    percent.clamp(100, 200)
}

/// Spec §7.1: the queue rail docks at 260–420px.
pub fn clamp_queue_width(px: u32) -> u32 {
    px.clamp(260, 420)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub quality_display: QualityDisplay,
    pub mode: UiMode,
    pub theme: Theme,
    pub instrument_tone: InstrumentTone,
    pub typeface: Typeface,
    /// Font family name used when `typeface == Custom`.
    pub custom_typeface: String,
    pub density: Density,
    pub cover_shape: CoverShape,
    pub dynamic_color: DynamicColor,
    pub queue_width: u32,
    pub player_bar_style: PanelStyle,
    pub queue_style: PanelStyle,
    /// Launch the window in the mini player (spec MP-D7); default off.
    pub start_in_mini: bool,
    pub oneko: bool,
    /// The jamming cat on the player bar cover (CatJam); default off.
    pub catjam: bool,
    pub catjam_coat: CatjamCoat,
    /// Percent of the sprite's 32 px (100–200).
    pub catjam_scale: u32,
    /// The collapsed sidebar and the closed queue expand as overlays on hover.
    pub hover_panels: bool,
    /// Hairlines between the rows of track lists; default on.
    pub row_separators: bool,
    /// Close hides the window to the tray instead of exiting (QoL-D3).
    pub close_to_tray: bool,
    /// Icons on the output picker's rows and the quality menu; default on.
    pub device_icons: bool,
    pub window_width: u32,
    pub window_height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
    pub window_maximized: bool,
    /// The look each mode had when it was last left (`[ui.looks.<mode>]`).
    /// The flat fields above are always the active mode's.
    pub looks: ModeLooks,
}

/// One mode's Typography & layout and Dynamic color settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeLook {
    pub typeface: Typeface,
    pub custom_typeface: String,
    pub density: Density,
    pub row_separators: bool,
    pub cover_shape: CoverShape,
    pub player_bar_style: PanelStyle,
    pub queue_style: PanelStyle,
    pub hover_panels: bool,
    pub dynamic_color: DynamicColor,
}

impl Default for ModeLook {
    fn default() -> Self {
        UiConfig::default().look()
    }
}

/// A mode is `None` until it is first left, so its first visit keeps the
/// look it is switched in with.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeLooks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<ModeLook>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<ModeLook>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rack: Option<ModeLook>,
}

impl ModeLooks {
    fn slot(&mut self, mode: UiMode) -> &mut Option<ModeLook> {
        match mode {
            UiMode::Atmosphere => &mut self.atmosphere,
            UiMode::Instrument => &mut self.instrument,
            UiMode::Rack => &mut self.rack,
        }
    }
}

impl UiConfig {
    pub fn look(&self) -> ModeLook {
        ModeLook {
            typeface: self.typeface,
            custom_typeface: self.custom_typeface.clone(),
            density: self.density,
            row_separators: self.row_separators,
            cover_shape: self.cover_shape,
            player_bar_style: self.player_bar_style,
            queue_style: self.queue_style,
            hover_panels: self.hover_panels,
            dynamic_color: self.dynamic_color,
        }
    }

    fn apply_look(&mut self, look: ModeLook) {
        self.typeface = look.typeface;
        self.custom_typeface = look.custom_typeface;
        self.density = look.density;
        self.row_separators = look.row_separators;
        self.cover_shape = look.cover_shape;
        self.player_bar_style = look.player_bar_style;
        self.queue_style = look.queue_style;
        self.hover_panels = look.hover_panels;
        self.dynamic_color = look.dynamic_color;
    }

    /// Stores the active look under the mode being left and restores the
    /// next mode's, if it was ever left before. Returns the look now active.
    pub fn switch_mode(&mut self, mode: UiMode) -> ModeLook {
        if mode != self.mode {
            let leaving = self.mode;
            *self.looks.slot(leaving) = Some(self.look());
            if let Some(look) = self.looks.slot(mode).clone() {
                self.apply_look(look);
            } else if mode == UiMode::Instrument {
                // Studio's disc never spins, so its first look is the sleeve.
                self.cover_shape = CoverShape::Sleeve;
            }
            self.mode = mode;
        }
        self.look()
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            quality_display: QualityDisplay::Compact,
            mode: UiMode::Atmosphere,
            theme: Theme::Dark,
            instrument_tone: InstrumentTone::Warm,
            typeface: Typeface::Auto,
            custom_typeface: String::new(),
            density: Density::Comfortable,
            cover_shape: CoverShape::Disc,
            dynamic_color: DynamicColor::Full,
            queue_width: 320,
            player_bar_style: PanelStyle::Docked,
            queue_style: PanelStyle::Docked,
            start_in_mini: false,
            oneko: false,
            catjam: false,
            catjam_coat: CatjamCoat::White,
            catjam_scale: 125,
            hover_panels: false,
            row_separators: true,
            close_to_tray: true,
            device_icons: true,
            window_width: 1200,
            window_height: 800,
            window_x: None,
            window_y: None,
            window_maximized: false,
            looks: ModeLooks::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Empty string means `%APPDATA%\OxiTide\cache`.
    pub dir: String,
    pub max_size_mb: u64,
    pub clear_on_exit: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            dir: String::new(),
            max_size_mb: 2048,
            clear_on_exit: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationsConfig {
    /// Windows System Media Transport Controls (media keys + now-playing).
    pub smtc: bool,
    /// Discord Rich Presence ("Listening to OxiTide" profile activity).
    pub discord: bool,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            smtc: true,
            discord: false,
        }
    }
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrobblingConfig {
    pub lastfm_enabled: bool,
    pub listenbrainz_enabled: bool,
    pub lastfm_api_key: String,
    pub lastfm_api_secret: String,
}

impl std::fmt::Debug for ScrobblingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrobblingConfig")
            .field("lastfm_enabled", &self.lastfm_enabled)
            .field("listenbrainz_enabled", &self.listenbrainz_enabled)
            .field("lastfm_api_key", &self.lastfm_api_key)
            .field("lastfm_api_secret", &"<redacted>")
            .finish()
    }
}

impl ScrobblingConfig {
    /// Env wins over config (spec S-D3). Empty values never shadow a fallback.
    pub fn pick(env: Option<String>, config: &str) -> String {
        env.filter(|value| !value.is_empty())
            .unwrap_or_else(|| config.to_string())
    }

    pub fn lastfm_credentials(&self) -> (String, String) {
        Self::with_builtin(
            (
                Self::pick(
                    std::env::var("OXITIDE_LASTFM_API_KEY").ok(),
                    &self.lastfm_api_key,
                ),
                Self::pick(
                    std::env::var("OXITIDE_LASTFM_API_SECRET").ok(),
                    &self.lastfm_api_secret,
                ),
            ),
            (
                crate::scrobble::lastfm::LASTFM_API_KEY,
                crate::scrobble::lastfm::LASTFM_API_SECRET,
            ),
        )
    }

    /// A user's own key and secret win as a pair; a half-set pair falls back to
    /// the shipped app key (spec S-D3 amendment, 2026-10-29). A secret only
    /// signs requests for that key, so the two never mix.
    pub fn with_builtin(user: (String, String), builtin: (&str, &str)) -> (String, String) {
        if !user.0.is_empty() && !user.1.is_empty() {
            user
        } else {
            (builtin.0.to_string(), builtin.1.to_string())
        }
    }
}

/// Local playback history (spec §5.1, §6.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub enabled: bool,
    pub max_entries: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: crate::history::DEFAULT_MAX_ENTRIES,
        }
    }
}

/// Self-update (Track 12): whether to look for a newer release on startup.
/// Nothing is ever downloaded without an explicit click.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdatesConfig {
    pub check_on_start: bool,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        Self {
            check_on_start: true,
        }
    }
}

/// Content blocking (spec §6.5): client-side filters for explicit and
/// AI-labeled tracks. Both filters default off; content labels default on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentConfig {
    pub block_explicit: bool,
    pub block_ai: bool,
    /// Tag `E`/`AI` on known flagged tracks even when not blocking them.
    pub show_labels: bool,
}

impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            block_explicit: false,
            block_ai: false,
            show_labels: true,
        }
    }
}

/// Global hotkeys (spec §6.6): one accelerator per action, empty means
/// unbound. Values are parsed by `tauri-plugin-global-shortcut`; the UI keeps
/// them in the plugin's format (`"Ctrl+Alt+KeyP"`, `"MediaPlayPause"`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeysConfig {
    pub play_pause: String,
    pub next: String,
    pub prev: String,
    pub volume_up: String,
    pub volume_down: String,
    pub show_hide: String,
}

impl Config {
    /// Load `%APPDATA%\OxiTide\config.toml`; a missing file yields defaults and
    /// a malformed file is backed up to `config.toml.bak` and replaced with
    /// defaults. Never fails.
    pub fn load() -> Self {
        match config_path() {
            Some(path) => Self::load_from(&path),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        match toml::from_str::<Config>(&text) {
            Ok(mut config) => {
                config.audio.migrate_volume();
                config.audio.migrate_devices();
                config
            }
            Err(e) => {
                tracing::warn!("malformed config at {}: {e}", path.display());
                let backup = path.with_extension("toml.bak");
                let _ = fs::rename(path, &backup);
                Self::default()
            }
        }
    }

    /// Atomic write (temp file + rename).
    pub fn save(&self) -> Result<(), String> {
        match config_path() {
            Some(path) => self.save_to(&path),
            None => Err("no config directory available".to_string()),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, text).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// `%APPDATA%\OxiTide\config\config.toml`.
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// `%APPDATA%\OxiTide\cache`.
pub fn default_cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide").map(|dirs| dirs.config_dir().join("cache"))
}

/// `%APPDATA%\OxiTide\logs` (the app root, not the config directory:
/// `ProjectDirs::config_dir()` appends `\config`).
#[cfg(windows)]
pub fn logs_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.config_dir().join("OxiTide").join("logs"))
}

/// `~/.config/oxitide/logs`: next to the config, session, and cache, which
/// `ProjectDirs` puts in the lowercase `oxitide` directory on Linux. Joining
/// the literal `OxiTide` here split the app across two directories.
#[cfg(not(windows))]
pub fn logs_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide").map(|dirs| dirs.config_dir().join("logs"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-config-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = temp_dir();
        let config = Config::load_from(&dir.join("config.toml"));
        assert_eq!(config, Config::default());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let config = Config {
            audio: AudioConfig {
                output_device: "{0.0.0.00000000}.{dev-1}".into(),
                exclusive: false,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
                volume: 0.42,
                volume_tapered: true,
                exclusive_volume_warning: false,
                pinned: vec![
                    "{0.0.0.00000000}.{dev-1}".into(),
                    "asio:Focusrite USB ASIO".into(),
                ],
                devices: [(
                    "{0.0.0.00000000}.{dev-1}".to_string(),
                    DevicePrefs {
                        name: "DX5".into(),
                        nickname: "Desk DAC".into(),
                        icon: "headphones".into(),
                        mode: OutputMode::Exclusive,
                        ..DevicePrefs::default()
                    },
                )]
                .into_iter()
                .collect(),
            },
            streaming: StreamingConfig {
                quality: QualityDto::Max,
                allow_fallback: false,
            },
            ui: UiConfig {
                quality_display: QualityDisplay::Detailed,
                mode: UiMode::Instrument,
                theme: Theme::Dark,
                instrument_tone: InstrumentTone::Black,
                typeface: Typeface::Manrope,
                custom_typeface: String::new(),
                density: Density::Comfortable,
                cover_shape: CoverShape::Disc,
                dynamic_color: DynamicColor::Full,
                queue_width: 320,
                player_bar_style: PanelStyle::Floating,
                queue_style: PanelStyle::Floating,
                start_in_mini: false,
                oneko: true,
                catjam: true,
                catjam_coat: CatjamCoat::Tuxedo,
                catjam_scale: 150,
                hover_panels: true,
                row_separators: false,
                close_to_tray: true,
                device_icons: false,
                window_width: 1280,
                window_height: 720,
                window_x: Some(40),
                window_y: Some(60),
                window_maximized: true,
                looks: ModeLooks {
                    rack: Some(ModeLook {
                        density: Density::Compact,
                        ..ModeLook::default()
                    }),
                    ..ModeLooks::default()
                },
            },
            cache: CacheConfig {
                dir: "D:\\cache".into(),
                max_size_mb: 512,
                clear_on_exit: true,
            },
            integrations: IntegrationsConfig {
                smtc: false,
                discord: true,
            },
            scrobbling: ScrobblingConfig::default(),
            history: HistoryConfig::default(),
            content: ContentConfig::default(),
            hotkeys: HotkeysConfig::default(),
            dsp: {
                let mut dsp = DspSettings::default();
                dsp.eq.enabled = true;
                dsp.eq.bands_db[0] = 4.5;
                dsp.replay_gain.mode = oxitide_audio::dsp::ReplayGainMode::Album;
                dsp.replay_gain.preamp_db = -2.0;
                dsp.crossfade.duration_ms = 6000;
                dsp
            },
            rack: RackConfig {
                deck_face: DeckFace::Cd,
                meters: MeterStyle::Led,
                analyzer: AnalyzerStyle::Recorder,
                lamp: LampColor::Cover,
                eq_unit: false,
                lamp_brightness: 70,
            },
            remote: RemoteConfig {
                enabled: true,
                port: 50000,
                interface: "192.168.1.5".into(),
                approve_new_devices: false,
            },
            updates: UpdatesConfig {
                check_on_start: false,
            },
            playback: PlaybackConfig { autoplay: false },
        };
        config.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), config);
        assert!(
            !dir.join("config.toml.tmp").exists(),
            "temp file left behind"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// Configs written before Autoplay get it on, like TIDAL.
    #[test]
    fn autoplay_defaults_on_for_older_configs() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[content]\nblock_explicit = true\n").unwrap();
        assert!(Config::load_from(&path).playback.autoplay);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: a malformed config must never crash startup.
    #[test]
    fn malformed_file_is_backed_up_and_defaults_used() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "not = [valid").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config, Config::default());
        assert!(!path.exists());
        assert!(dir.join("config.toml.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn partial_dsp_table_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "[dsp.replay_gain]
mode = \"track\"

[dsp.crossfade]
duration_ms = 4000
",
        )
        .unwrap();
        let config = Config::load_from(&path);
        assert_eq!(
            config.dsp.replay_gain.mode,
            oxitide_audio::dsp::ReplayGainMode::Track
        );
        assert!(config.dsp.replay_gain.prevent_clipping);
        assert_eq!(config.dsp.crossfade.duration_ms, 4000);
        assert!(config.dsp.crossfade.skip_same_album);
        assert!(!config.dsp.eq.enabled);
        assert_eq!(config.dsp.eq.bands_db.len(), 10);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_global_exclusive_switch_moves_onto_the_selected_device() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "[audio]\noutput_device = \"dac\"\nexclusive = true\n",
        )
        .unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.audio.mode_for("dac"), OutputMode::Exclusive);
        assert_eq!(config.audio.mode_for("speakers"), OutputMode::Shared);
        assert_eq!(
            config.audio.mode_for("asio:Focusrite"),
            OutputMode::Exclusive,
            "ASIO always is"
        );
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("exclusive = true"),
            "the old switch is not written back:\n{text}"
        );
        assert_eq!(
            Config::load_from(&path).audio.mode_for("dac"),
            OutputMode::Exclusive
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn exclusive_off_leaves_every_device_shared() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "[audio]\noutput_device = \"dac\"\nexclusive = false\n",
        )
        .unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.audio.mode_for("dac"), OutputMode::Shared);
        assert!(config.audio.devices.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn alsa_routes_migrate_to_row_ids() {
        let mut audio = AudioConfig {
            output_device: "sysdefault:CARD=Pro".into(),
            pinned: vec![
                "plughw:CARD=Pro,DEV=0".into(),
                "hw:0,3".into(),
                "default".into(),
            ],
            ..AudioConfig::default()
        };
        audio.devices.insert(
            "plughw:CARD=Pro,DEV=0".into(),
            DevicePrefs {
                nickname: "Desk DAC".into(),
                ..DevicePrefs::default()
            },
        );
        audio.migrate_alsa_ids(&["PCH".to_string(), "Pro".to_string()]);
        assert_eq!(audio.output_device, "hw:CARD=Pro,DEV=0");
        assert_eq!(
            audio.pinned,
            vec![
                "hw:CARD=Pro,DEV=0".to_string(),
                "hw:CARD=PCH,DEV=3".to_string()
            ]
        );
        assert_eq!(audio.devices["hw:CARD=Pro,DEV=0"].nickname, "Desk DAC");
    }

    #[test]
    fn prune_keeps_choices_and_selected_or_pinned_devices() {
        let mut audio = AudioConfig {
            output_device: "sel".into(),
            pinned: vec!["pin".into()],
            ..AudioConfig::default()
        };
        for id in ["sel", "pin", "gone"] {
            audio.devices.insert(
                id.into(),
                DevicePrefs {
                    name: id.into(),
                    ..DevicePrefs::default()
                },
            );
        }
        audio.devices.insert(
            "named".into(),
            DevicePrefs {
                nickname: "TV".into(),
                ..DevicePrefs::default()
            },
        );
        audio.prune_devices();
        let ids: Vec<&str> = audio.devices.keys().map(String::as_str).collect();
        assert_eq!(ids, vec!["named", "pin", "sel"]);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[audio]\nexclusive = true\n").unwrap();
        let config = Config::load_from(&path);
        assert!(!config.audio.exclusive, "migrated onto System default");
        assert_eq!(config.audio.mode_for(""), OutputMode::Exclusive);
        assert!(config.ui.device_icons);
        assert_eq!(config.audio.output_device, "");
        assert_eq!(config.audio.exclusive_fallback, ExclusiveFallback::Shared);
        assert_eq!(config.audio.volume, 1.0);
        assert_eq!(config.streaming.quality, QualityDto::Lossless);
        assert!(config.streaming.allow_fallback);
        assert_eq!(config.ui.quality_display, QualityDisplay::Compact);
        assert_eq!(config.ui.window_width, 1200);
        assert_eq!(config.ui.window_height, 800);
        assert_eq!(config.ui.window_x, None);
        assert_eq!(config.ui.window_y, None);
        assert!(!config.ui.window_maximized);
        assert!(!config.ui.start_in_mini);
        assert!(!config.ui.oneko);
        assert_eq!(config.cache.dir, "");
        assert_eq!(config.cache.max_size_mb, 2048);
        assert!(!config.cache.clear_on_exit);
        assert!(config.integrations.smtc);
        assert!(!config.integrations.discord);
        assert!(!config.scrobbling.lastfm_enabled);
        assert_eq!(config.scrobbling.lastfm_api_key, "");
        assert!(!config.content.block_explicit);
        assert!(!config.content.block_ai);
        assert!(config.content.show_labels);
        assert!(config.ui.close_to_tray);
        assert_eq!(config.hotkeys.play_pause, "");
        assert_eq!(config.hotkeys.next, "");
        assert_eq!(config.hotkeys.prev, "");
        assert_eq!(config.hotkeys.volume_up, "");
        assert_eq!(config.hotkeys.volume_down, "");
        assert_eq!(config.hotkeys.show_hide, "");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn low_quality_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        assert_eq!(config.streaming.quality, QualityDto::Lossless);
        config.streaming.quality = QualityDto::Low;
        config.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: `toml` serialization errors on `None` fields, so the
    /// window position must be omitted (not written as a null) before the
    /// first exit, and every other save must keep working.
    #[test]
    fn default_config_omits_window_position_before_the_first_save() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        Config::default().save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("window_x"),
            "window_x written before the first save:\n{text}"
        );
        assert!(
            !text.contains("window_y"),
            "window_y written before the first save:\n{text}"
        );
        assert!(text.contains("window_width = 1200"));
        assert!(text.contains("window_maximized = false"));
        let loaded = Config::load_from(&path);
        assert_eq!(loaded.ui.window_x, None);
        assert_eq!(loaded.ui.window_maximized, false);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: the logs directory must be `%APPDATA%\OxiTide\logs`, not
    /// the config directory (`ProjectDirs::config_dir()` appends `\config`).
    #[cfg(windows)]
    #[test]
    fn logs_dir_is_under_the_oxitide_app_dir() {
        let dir = logs_dir().expect("no project directories available");
        assert!(
            dir.ends_with("logs"),
            "unexpected logs dir: {}",
            dir.display()
        );
        assert_eq!(
            dir.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str()),
            Some("OxiTide"),
            "logs must live directly under the app dir: {}",
            dir.display()
        );
    }

    /// Linux: logs share the config's directory instead of a second
    /// `OxiTide` directory beside it.
    #[cfg(not(windows))]
    #[test]
    fn logs_dir_shares_the_config_directory() {
        let logs = logs_dir().expect("no project directories available");
        let config = config_path().expect("no project directories available");
        assert_eq!(logs.parent(), config.parent(), "logs: {}", logs.display());
        assert!(logs.ends_with("logs"));
    }

    #[test]
    fn new_sections_round_trip_as_documented_toml() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.streaming.allow_fallback = false;
        config.ui.quality_display = QualityDisplay::Detailed;
        config.cache.max_size_mb = 1024;
        config.cache.clear_on_exit = true;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[streaming]"));
        assert!(text.contains("allow_fallback = false"));
        assert!(text.contains("quality_display = \"detailed\""));
        assert!(text.contains("[cache]"));
        assert!(text.contains("max_size_mb = 1024"));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn appearance_keys_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.ui.typeface = Typeface::Inter;
        config.ui.custom_typeface = "Commit Mono".into();
        config.ui.density = Density::Compact;
        config.ui.cover_shape = CoverShape::Sleeve;
        config.ui.queue_width = 400;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("typeface = \"inter\""), "{text}");
        assert!(text.contains("density = \"compact\""), "{text}");
        assert!(text.contains("cover_shape = \"sleeve\""), "{text}");
        assert!(text.contains("queue_width = 400"), "{text}");
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn theme_round_trips_and_defaults_to_dark() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        assert_eq!(config.ui.theme, Theme::Dark);
        config.ui.theme = Theme::System;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("theme = \"system\""), "{text}");
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: a config written before Plan 10 lacks every appearance
    /// key; serde defaults must fill them.
    #[test]
    fn appearance_defaults_fill_a_pre_plan10_file() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[ui]\nquality_display = \"detailed\"\n").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.ui.quality_display, QualityDisplay::Detailed);
        assert_eq!(config.ui.mode, UiMode::Atmosphere);
        assert_eq!(config.ui.theme, Theme::Dark);
        assert_eq!(config.ui.typeface, Typeface::Auto);
        assert_eq!(config.ui.dynamic_color, DynamicColor::Full);
        assert_eq!(config.ui.instrument_tone, InstrumentTone::Warm);
        assert_eq!(config.ui.custom_typeface, "");
        assert_eq!(config.ui.density, Density::Comfortable);
        assert_eq!(config.ui.cover_shape, CoverShape::Disc);
        assert_eq!(config.ui.queue_width, 320);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: queue width is clamped to the spec range on both sides.
    #[test]
    fn queue_width_clamps_to_260_420() {
        assert_eq!(clamp_queue_width(0), 260);
        assert_eq!(clamp_queue_width(259), 260);
        assert_eq!(clamp_queue_width(260), 260);
        assert_eq!(clamp_queue_width(320), 320);
        assert_eq!(clamp_queue_width(420), 420);
        assert_eq!(clamp_queue_width(9999), 420);
    }

    #[test]
    fn atmosphere_defaults_for_fresh_configs() {
        let config = Config::default();
        assert_eq!(config.ui.mode, UiMode::Atmosphere);
        assert_eq!(config.ui.typeface, Typeface::Auto);
        assert_eq!(config.ui.dynamic_color, DynamicColor::Full);
    }

    /// Review Focus: Plan 10 wrote explicit values; they stay honored.
    #[test]
    fn plan10_configs_load_untouched() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "[ui]\nmode = \"instrument\"\ntypeface = \"manrope\"\n",
        )
        .unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.ui.mode, UiMode::Instrument);
        assert_eq!(config.ui.typeface, Typeface::Manrope);
        assert_eq!(config.ui.dynamic_color, DynamicColor::Full);
        let _ = fs::remove_dir_all(dir);
    }

    /// The removed per-mode `auto` setting reads back as Full.
    #[test]
    fn legacy_auto_dynamic_color_loads_as_full() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[ui]\ndynamic_color = \"auto\"\n").unwrap();
        assert_eq!(
            Config::load_from(&path).ui.dynamic_color,
            DynamicColor::Full
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dynamic_color_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.ui.dynamic_color = DynamicColor::Accent;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("dynamic_color = \"accent\""));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn instrument_tone_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.ui.instrument_tone = InstrumentTone::Neutral;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("instrument_tone = \"neutral\""));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn start_in_mini_defaults_false_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(!Config::default().ui.start_in_mini);
        let mut config = Config::default();
        config.ui.start_in_mini = true;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("start_in_mini = true"));
        assert!(Config::load_from(&path).ui.start_in_mini);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn oneko_defaults_false_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(!Config::default().ui.oneko);
        let mut config = Config::default();
        config.ui.oneko = true;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("oneko = true"));
        assert!(Config::load_from(&path).ui.oneko);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn catjam_defaults_off_and_white_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let defaults = Config::default();
        assert!(!defaults.ui.catjam);
        assert_eq!(defaults.ui.catjam_coat, CatjamCoat::White);
        let mut config = Config::default();
        config.ui.catjam = true;
        config.ui.catjam_coat = CatjamCoat::Ginger;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("catjam = true"), "{text}");
        assert!(text.contains("catjam_coat = \"ginger\""), "{text}");
        let loaded = Config::load_from(&path);
        assert!(loaded.ui.catjam);
        assert_eq!(loaded.ui.catjam_coat, CatjamCoat::Ginger);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn catjam_scale_defaults_to_125_and_is_capped() {
        assert_eq!(Config::default().ui.catjam_scale, 125);
        assert_eq!(clamp_catjam_scale(50), 100);
        assert_eq!(clamp_catjam_scale(150), 150);
        assert_eq!(clamp_catjam_scale(400), 200);
    }

    #[test]
    fn catjam_coats_use_the_ui_ids() {
        let ids: Vec<String> = [
            CatjamCoat::White,
            CatjamCoat::Black,
            CatjamCoat::Ginger,
            CatjamCoat::Grey,
            CatjamCoat::Tuxedo,
            CatjamCoat::Cream,
        ]
        .iter()
        .map(|c| {
            serde_json::to_value(c)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
        assert_eq!(ids, ["white", "black", "ginger", "grey", "tuxedo", "cream"]);
        assert!(serde_json::from_value::<CatjamCoat>(serde_json::json!("calico")).is_err());
    }

    #[test]
    fn hover_panels_defaults_false_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(!Config::default().ui.hover_panels);
        let mut config = Config::default();
        config.ui.hover_panels = true;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("hover_panels = true"));
        assert!(Config::load_from(&path).ui.hover_panels);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn row_separators_default_on_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(Config::default().ui.row_separators);
        let mut config = Config::default();
        config.ui.row_separators = false;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("row_separators = false"));
        assert!(!Config::load_from(&path).ui.row_separators);
        // A config written before the setting existed keeps the hairlines.
        fs::write(&path, "[ui]\nhover_panels = true\n").unwrap();
        assert!(Config::load_from(&path).ui.row_separators);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn each_mode_remembers_its_own_look() {
        let mut ui = UiConfig::default();
        ui.density = Density::Compact;
        ui.dynamic_color = DynamicColor::Accent;
        // A first visit keeps the look it arrives with (Studio starts on the sleeve)...
        let first = ui.switch_mode(UiMode::Instrument);
        assert_eq!(first.density, Density::Compact);
        assert_eq!(first.cover_shape, CoverShape::Sleeve);
        ui.density = Density::Comfortable;
        ui.typeface = Typeface::System;
        ui.dynamic_color = DynamicColor::Off;
        // ...and leaving stores it for the mode being left.
        let back = ui.switch_mode(UiMode::Atmosphere);
        assert_eq!(back.density, Density::Compact);
        assert_eq!(back.typeface, Typeface::Auto);
        assert_eq!(back.dynamic_color, DynamicColor::Accent);
        assert_eq!(ui.density, Density::Compact);
        let studio = ui.switch_mode(UiMode::Instrument);
        assert_eq!(studio.typeface, Typeface::System);
        assert_eq!(studio.dynamic_color, DynamicColor::Off);
        // Switching to the active mode changes nothing.
        assert_eq!(ui.switch_mode(UiMode::Instrument), studio);
    }

    #[test]
    fn mode_looks_round_trip_and_older_files_have_none() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[ui]\nmode = \"rack\"\n").unwrap();
        assert_eq!(Config::load_from(&path).ui.looks, ModeLooks::default());
        let mut config = Config::default();
        config.ui.switch_mode(UiMode::Rack);
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[ui.looks.atmosphere]"), "{text}");
        assert!(!text.contains("[ui.looks.rack]"), "{text}");
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn panel_styles_default_docked_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert_eq!(Config::default().ui.player_bar_style, PanelStyle::Docked);
        assert_eq!(Config::default().ui.queue_style, PanelStyle::Docked);
        let mut config = Config::default();
        config.ui.player_bar_style = PanelStyle::Floating;
        config.ui.queue_style = PanelStyle::Floating;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("player_bar_style = \"floating\""));
        assert!(text.contains("queue_style = \"floating\""));
        let loaded = Config::load_from(&path);
        assert_eq!(loaded.ui.player_bar_style, PanelStyle::Floating);
        assert_eq!(loaded.ui.queue_style, PanelStyle::Floating);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn scrobbling_defaults_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let fresh = Config::default();
        assert!(!fresh.scrobbling.lastfm_enabled);
        assert!(!fresh.scrobbling.listenbrainz_enabled);
        assert_eq!(fresh.scrobbling.lastfm_api_key, "");
        let mut config = Config::default();
        config.scrobbling.lastfm_enabled = true;
        config.scrobbling.lastfm_api_key = "key".into();
        config.scrobbling.lastfm_api_secret = "secret".into();
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[scrobbling]"), "{text}");
        assert!(text.contains("lastfm_enabled = true"));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pre_scrobbling_file_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[ui]\nquality_display = \"detailed\"\n").unwrap();
        let config = Config::load_from(&path);
        assert!(!config.scrobbling.lastfm_enabled);
        assert_eq!(config.scrobbling.lastfm_api_key, "");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn scrobbling_debug_redacts_the_secret() {
        let config = ScrobblingConfig {
            lastfm_api_secret: "super-secret".into(),
            ..ScrobblingConfig::default()
        };
        let text = format!("{config:?}");
        assert!(!text.contains("super-secret"), "{text}");
        assert!(text.contains("<redacted>"), "{text}");
    }

    #[test]
    fn credential_pick_prefers_non_empty_env() {
        assert_eq!(ScrobblingConfig::pick(Some("env".into()), "cfg"), "env");
        assert_eq!(ScrobblingConfig::pick(Some(String::new()), "cfg"), "cfg");
        assert_eq!(ScrobblingConfig::pick(None, "cfg"), "cfg");
    }

    #[test]
    fn user_lastfm_pair_wins_over_the_builtin_key() {
        let pair = |k: &str, s: &str| (k.to_string(), s.to_string());
        assert_eq!(
            ScrobblingConfig::with_builtin(pair("k", "s"), ("bk", "bs")),
            pair("k", "s")
        );
        assert_eq!(
            ScrobblingConfig::with_builtin(pair("k", ""), ("bk", "bs")),
            pair("bk", "bs")
        );
        assert_eq!(
            ScrobblingConfig::with_builtin(pair("", "s"), ("bk", "bs")),
            pair("bk", "bs")
        );
        assert_eq!(
            ScrobblingConfig::with_builtin(pair("", ""), ("bk", "bs")),
            pair("bk", "bs")
        );
    }

    #[test]
    fn smtc_defaults_true_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(Config::default().integrations.smtc);
        let mut config = Config::default();
        config.integrations.smtc = false;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[integrations]"), "{text}");
        assert!(text.contains("smtc = false"), "{text}");
        assert!(!Config::load_from(&path).integrations.smtc);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn discord_defaults_false_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(!Config::default().integrations.discord);
        let mut config = Config::default();
        config.integrations.discord = true;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[integrations]"), "{text}");
        assert!(text.contains("discord = true"), "{text}");
        assert!(Config::load_from(&path).integrations.discord);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn history_defaults_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let config = Config {
            history: HistoryConfig {
                enabled: false,
                max_entries: 500,
            },
            ..Config::default()
        };
        config.save_to(&path).unwrap();
        let loaded = Config::load_from(&path);
        assert!(!loaded.history.enabled);
        assert_eq!(loaded.history.max_entries, 500);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pre_history_file_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[audio]\nvolume = 0.5\n").unwrap();
        let loaded = Config::load_from(&path);
        assert!(loaded.history.enabled);
        assert_eq!(loaded.history.max_entries, 10_000);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_linear_volume_from_an_old_config_keeps_its_loudness() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[audio]\nvolume = 0.316\n").unwrap();
        let loaded = Config::load_from(&path);
        // A linear 0.316 is -10 dB, which is 50% on the taper.
        assert!(
            (loaded.audio.volume - 0.5).abs() < 1e-3,
            "{}",
            loaded.audio.volume
        );
        assert!(loaded.audio.volume_tapered);

        // Once saved, the position loads unchanged.
        loaded.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path).audio, loaded.audio);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn content_defaults_are_off() {
        let config = Config::default();
        assert!(!config.content.block_explicit);
        assert!(!config.content.block_ai);
        assert!(config.content.show_labels, "labels default on");
    }

    #[test]
    fn content_round_trips_as_documented_toml() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.content.block_explicit = true;
        config.content.block_ai = true;
        config.content.show_labels = false;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[content]"));
        assert!(text.contains("show_labels = false"));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn update_checks_default_on_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(Config::default().updates.check_on_start);
        let mut config = Config::default();
        config.updates.check_on_start = false;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[updates]"));
        assert!(text.contains("check_on_start = false"));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn close_to_tray_defaults_true_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(Config::default().ui.close_to_tray);
        let mut config = Config::default();
        config.ui.close_to_tray = false;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("close_to_tray = false"));
        assert!(!Config::load_from(&path).ui.close_to_tray);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn hotkeys_default_to_empty_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let fresh = Config::default();
        assert_eq!(fresh.hotkeys.play_pause, "");
        assert_eq!(fresh.hotkeys.show_hide, "");
        let mut config = Config::default();
        config.hotkeys.play_pause = "Ctrl+Alt+Space".into();
        config.hotkeys.volume_up = "VolumeUp".into();
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[hotkeys]"), "{text}");
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rack_defaults_and_mode_parse() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "[ui]
mode = \"rack\"
",
        )
        .unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.ui.mode, UiMode::Rack);
        assert_eq!(config.rack, RackConfig::default());
        assert_eq!(config.rack.deck_face, DeckFace::Display);
        assert_eq!(config.rack.meters, MeterStyle::Needles);
        assert_eq!(config.rack.analyzer, AnalyzerStyle::Auto);
        assert_eq!(config.rack.lamp, LampColor::Amber);
        assert!(config.rack.eq_unit);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rack_section_parses_partial_values() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "[rack]
deck_face = \"transport\"
lamp = \"blue\"
analyzer = \"waterfall\"
",
        )
        .unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.rack.deck_face, DeckFace::Transport);
        assert_eq!(
            config.rack.analyzer,
            AnalyzerStyle::Recorder,
            "retired analyzer values map to the recorder"
        );
        for (text, style) in [
            ("cassette", AnalyzerStyle::Auto),
            ("tuner", AnalyzerStyle::Tuner),
            ("eq", AnalyzerStyle::Eq),
            ("none", AnalyzerStyle::Auto),
        ] {
            let rack: RackConfig = toml::from_str(&format!("analyzer = \"{text}\"")).unwrap();
            assert_eq!(rack.analyzer, style);
        }
        assert_eq!(config.rack.lamp, LampColor::Blue);
        assert_eq!(config.rack.meters, MeterStyle::Needles);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn remote_defaults_off_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let fresh = Config::default();
        assert!(!fresh.remote.enabled, "the phone remote is opt-in");
        assert_eq!(fresh.remote.port, REMOTE_DEFAULT_PORT);
        assert_eq!(fresh.remote.interface, "");
        assert!(fresh.remote.approve_new_devices);
        fs::write(&path, "[remote]\nenabled = true\n").unwrap();
        let partial = Config::load_from(&path);
        assert!(partial.remote.enabled);
        assert_eq!(partial.remote.port, REMOTE_DEFAULT_PORT);
        assert!(partial.remote.approve_new_devices);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn remote_port_is_clamped() {
        assert_eq!(clamp_remote_port(80), 1024);
        assert_eq!(clamp_remote_port(47820), 47820);
        assert_eq!(clamp_remote_port(u16::MAX), u16::MAX - 9);
    }

    #[test]
    fn lamp_brightness_is_clamped() {
        assert_eq!(clamp_lamp_brightness(0), LAMP_BRIGHTNESS_MIN);
        assert_eq!(clamp_lamp_brightness(255), LAMP_BRIGHTNESS_MAX);
        assert_eq!(clamp_lamp_brightness(70), 70);
        let rack = RackConfig {
            lamp_brightness: 5,
            ..RackConfig::default()
        }
        .clamped();
        assert_eq!(rack.lamp_brightness, LAMP_BRIGHTNESS_MIN);
    }
}
