use std::fs;
use std::path::{Path, PathBuf};

use oxitide_audio::dsp::DspSettings;
use oxitide_audio::output::ExclusiveFallback;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Empty string means the system default device.
    pub output_device: String,
    pub exclusive: bool,
    pub exclusive_fallback: ExclusiveFallback,
    pub volume: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device: String::new(),
            exclusive: false,
            exclusive_fallback: ExclusiveFallback::Shared,
            volume: 1.0,
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
        Self { quality: QualityDto::Lossless, allow_fallback: true }
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
/// the default.
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

/// Dynamic cover color (spec §5.4). `Auto` = Full in Atmosphere, Off in Instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DynamicColor {
    #[default]
    Auto,
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
    /// Close hides the window to the tray instead of exiting (QoL-D3).
    pub close_to_tray: bool,
    pub window_width: u32,
    pub window_height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
    pub window_maximized: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            quality_display: QualityDisplay::Compact,
            mode: UiMode::Atmosphere,
            theme: Theme::Dark,
            typeface: Typeface::Auto,
            custom_typeface: String::new(),
            density: Density::Comfortable,
            cover_shape: CoverShape::Disc,
            dynamic_color: DynamicColor::Auto,
            queue_width: 320,
            player_bar_style: PanelStyle::Docked,
            queue_style: PanelStyle::Docked,
            start_in_mini: false,
            oneko: false,
            catjam: false,
            catjam_coat: CatjamCoat::White,
            catjam_scale: 125,
            hover_panels: false,
            close_to_tray: true,
            window_width: 1200,
            window_height: 800,
            window_x: None,
            window_y: None,
            window_maximized: false,
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
        Self { dir: String::new(), max_size_mb: 2048, clear_on_exit: false }
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
        Self { smtc: true, discord: false }
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
        (
            Self::pick(std::env::var("OXITIDE_LASTFM_API_KEY").ok(), &self.lastfm_api_key),
            Self::pick(std::env::var("OXITIDE_LASTFM_API_SECRET").ok(), &self.lastfm_api_secret),
        )
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
        Self { enabled: true, max_entries: crate::history::DEFAULT_MAX_ENTRIES }
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
        Self { check_on_start: true }
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
        Self { block_explicit: false, block_ai: false, show_labels: true }
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
            Ok(config) => config,
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
pub fn logs_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.config_dir().join("OxiTide").join("logs"))
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
                output_device: "dev-1".into(),
                exclusive: true,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
                volume: 0.42,
            },
            streaming: StreamingConfig { quality: QualityDto::Max, allow_fallback: false },
            ui: UiConfig {
                quality_display: QualityDisplay::Detailed,
                mode: UiMode::Instrument,
                theme: Theme::Dark,
                typeface: Typeface::Manrope,
                custom_typeface: String::new(),
                density: Density::Comfortable,
                cover_shape: CoverShape::Disc,
                dynamic_color: DynamicColor::Auto,
                queue_width: 320,
                player_bar_style: PanelStyle::Floating,
                queue_style: PanelStyle::Floating,
                start_in_mini: false,
                oneko: true,
                catjam: true,
                catjam_coat: CatjamCoat::Tuxedo,
                catjam_scale: 150,
                hover_panels: true,
                close_to_tray: true,
                window_width: 1280,
                window_height: 720,
                window_x: Some(40),
                window_y: Some(60),
                window_maximized: true,
            },
            cache: CacheConfig {
                dir: "D:\\cache".into(),
                max_size_mb: 512,
                clear_on_exit: true,
            },
            integrations: IntegrationsConfig { smtc: false, discord: true },
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
            updates: UpdatesConfig { check_on_start: false },
        };
        config.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), config);
        assert!(!dir.join("config.toml.tmp").exists(), "temp file left behind");
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
        fs::write(&path, "[dsp.replay_gain]
mode = \"track\"

[dsp.crossfade]
duration_ms = 4000
")
            .unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.dsp.replay_gain.mode, oxitide_audio::dsp::ReplayGainMode::Track);
        assert!(config.dsp.replay_gain.prevent_clipping);
        assert_eq!(config.dsp.crossfade.duration_ms, 4000);
        assert!(config.dsp.crossfade.skip_same_album);
        assert!(!config.dsp.eq.enabled);
        assert_eq!(config.dsp.eq.bands_db.len(), 10);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[audio]\nexclusive = true\n").unwrap();
        let config = Config::load_from(&path);
        assert!(config.audio.exclusive);
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
        assert!(!text.contains("window_x"), "window_x written before the first save:\n{text}");
        assert!(!text.contains("window_y"), "window_y written before the first save:\n{text}");
        assert!(text.contains("window_width = 1200"));
        assert!(text.contains("window_maximized = false"));
        let loaded = Config::load_from(&path);
        assert_eq!(loaded.ui.window_x, None);
        assert_eq!(loaded.ui.window_maximized, false);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: the logs directory must be `%APPDATA%\OxiTide\logs`, not
    /// the config directory (`ProjectDirs::config_dir()` appends `\config`).
    #[test]
    fn logs_dir_is_under_the_oxitide_app_dir() {
        let dir = logs_dir().expect("no project directories available");
        assert!(dir.ends_with("logs"), "unexpected logs dir: {}", dir.display());
        assert_eq!(
            dir.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()),
            Some("OxiTide"),
            "logs must live directly under the app dir: {}",
            dir.display()
        );
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
        assert_eq!(config.ui.dynamic_color, DynamicColor::Auto);
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
        assert_eq!(config.ui.dynamic_color, DynamicColor::Auto);
    }

    /// Review Focus: Plan 10 wrote explicit values; they stay honored.
    #[test]
    fn plan10_configs_load_untouched() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[ui]\nmode = \"instrument\"\ntypeface = \"manrope\"\n").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.ui.mode, UiMode::Instrument);
        assert_eq!(config.ui.typeface, Typeface::Manrope);
        assert_eq!(config.ui.dynamic_color, DynamicColor::Auto);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dynamic_color_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.ui.dynamic_color = DynamicColor::Accent;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("dynamic_color = \"accent\""));
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
        assert!(fs::read_to_string(&path).unwrap().contains("start_in_mini = true"));
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
        .map(|c| serde_json::to_value(c).unwrap().as_str().unwrap().to_string())
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
        assert!(fs::read_to_string(&path).unwrap().contains("hover_panels = true"));
        assert!(Config::load_from(&path).ui.hover_panels);
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
            history: HistoryConfig { enabled: false, max_entries: 500 },
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
        assert!(fs::read_to_string(&path).unwrap().contains("close_to_tray = false"));
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
        fs::write(&path, "[ui]
mode = \"rack\"
").unwrap();
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
        fs::write(&path, "[rack]
deck_face = \"transport\"
lamp = \"blue\"
analyzer = \"waterfall\"
").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.rack.deck_face, DeckFace::Transport);
        assert_eq!(config.rack.analyzer, AnalyzerStyle::Recorder, "retired analyzer values map to the recorder");
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
        let rack = RackConfig { lamp_brightness: 5, ..RackConfig::default() }.clamped();
        assert_eq!(rack.lamp_brightness, LAMP_BRIGHTNESS_MIN);
    }
}
