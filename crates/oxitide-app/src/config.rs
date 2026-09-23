use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub quality_display: QualityDisplay,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { quality_display: QualityDisplay::Compact }
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
                // 'low' is 96 kbps HE-AAC, which the built-in decoder cannot
                // play; retire it so old configs keep working at 320 kbps AAC.
                if config.streaming.quality == QualityDto::Low {
                    tracing::warn!("streaming quality 'low' (HE-AAC) is unsupported; using 'high'");
                    config.streaming.quality = QualityDto::High;
                }
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

/// `%APPDATA%\OxiTide\config.toml`.
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// `%APPDATA%\OxiTide\cache`.
pub fn default_cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide").map(|dirs| dirs.config_dir().join("cache"))
}

/// `%APPDATA%\OxiTide\logs`.
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
                output_device: "dev-1".into(),
                exclusive: true,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
                volume: 0.42,
            },
            streaming: StreamingConfig { quality: QualityDto::Max, allow_fallback: false },
            ui: UiConfig { quality_display: QualityDisplay::Detailed },
            cache: CacheConfig {
                dir: "D:\\cache".into(),
                max_size_mb: 512,
                clear_on_exit: true,
            },
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
        assert_eq!(config.cache.dir, "");
        assert_eq!(config.cache.max_size_mb, 2048);
        assert!(!config.cache.clear_on_exit);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn low_quality_is_coerced_to_high() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[streaming]\nquality = \"low\"\n").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.streaming.quality, QualityDto::High);
        assert!(config.streaming.allow_fallback);
        assert_eq!(config.cache.max_size_mb, 2048);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn logs_dir_ends_with_logs() {
        let dir = logs_dir().expect("no project directories available");
        assert!(dir.ends_with("logs"), "unexpected logs dir: {}", dir.display());
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
}
