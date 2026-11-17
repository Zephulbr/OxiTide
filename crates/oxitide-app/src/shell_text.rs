//! Text the Rust shell shows itself: the tray menu and the taskbar buttons
//! (spec `2026-11-12-oxitide-v1.1-i18n-a11y-design.md` §10.1).
//!
//! The strings live in the UI's catalogs (`ui/src/locales/<code>/shell.json`,
//! plain strings, no ICU), embedded at build time so translators edit one
//! place. The UI resolves the language (the config's `"system"` follows the
//! webview's languages) and reports it with `set_shell_language`; until then
//! the saved explicit language, or English, is used.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// One embedded catalog per language that has a `shell.json`. Add a language
/// here when its folder is created; a test checks this list against the
/// catalog folders.
macro_rules! catalogs {
    ($($code:literal),* $(,)?) => {
        &[$((
            $code,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../ui/src/locales/",
                $code,
                "/shell.json"
            )),
        )),*]
    };
}

const CATALOGS: &[(&str, &str)] = catalogs![
    "en", "de", "fr", "es", "it", "pt-BR", "nl", "sv", "nb", "da", "fi", "pl", "cs", "hu", "uk",
    "ru", "ro", "el", "tr", "id", "ja", "ko", "zh-Hans", "zh-Hant"
];

/// A shell string. Keys match `shell.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellText {
    TrayShowHide,
    TrayPlayPause,
    TrayNext,
    TrayPrevious,
    TrayQuit,
    TaskbarPrevious,
    TaskbarPlay,
    TaskbarPause,
    TaskbarNext,
}

impl ShellText {
    #[cfg(test)]
    pub const ALL: [ShellText; 9] = [
        ShellText::TrayShowHide,
        ShellText::TrayPlayPause,
        ShellText::TrayNext,
        ShellText::TrayPrevious,
        ShellText::TrayQuit,
        ShellText::TaskbarPrevious,
        ShellText::TaskbarPlay,
        ShellText::TaskbarPause,
        ShellText::TaskbarNext,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ShellText::TrayShowHide => "tray.showHide",
            ShellText::TrayPlayPause => "tray.playPause",
            ShellText::TrayNext => "tray.next",
            ShellText::TrayPrevious => "tray.previous",
            ShellText::TrayQuit => "tray.quit",
            ShellText::TaskbarPrevious => "taskbar.previous",
            ShellText::TaskbarPlay => "taskbar.play",
            ShellText::TaskbarPause => "taskbar.pause",
            ShellText::TaskbarNext => "taskbar.next",
        }
    }
}

type Table = HashMap<&'static str, HashMap<String, String>>;

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        CATALOGS
            .iter()
            .filter_map(|(code, text)| match serde_json::from_str(text) {
                Ok(map) => Some((*code, map)),
                Err(e) => {
                    tracing::warn!("shell text for {code} unreadable: {e}");
                    None
                }
            })
            .collect()
    })
}

static CURRENT: Mutex<&'static str> = Mutex::new("en");

/// The embedded code matching `code`, if that language has shell text.
fn embedded(code: &str) -> Option<&'static str> {
    CATALOGS.iter().map(|(c, _)| *c).find(|c| *c == code)
}

/// Use `code` from now on; an unknown code (or `"system"`) means English.
/// Returns the code in effect.
pub fn set_language(code: &str) -> &'static str {
    let next = embedded(code).unwrap_or("en");
    *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = next;
    next
}

/// `text` in `code`, falling back to English.
pub fn text_in(code: &str, text: ShellText) -> String {
    let table = table();
    table
        .get(code)
        .and_then(|map| map.get(text.key()))
        .or_else(|| table.get("en").and_then(|map| map.get(text.key())))
        .cloned()
        .unwrap_or_else(|| text.key().to_string())
}

/// `text` in the current language.
pub fn text(text: ShellText) -> String {
    let code = *CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    text_in(code, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn locales() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/locales")
    }

    #[test]
    fn english_has_every_key() {
        for text in ShellText::ALL {
            assert_ne!(text_in("en", text), text.key(), "{text:?} missing in en");
        }
        assert_eq!(text_in("en", ShellText::TrayQuit), "Quit");
        assert_eq!(text_in("en", ShellText::TaskbarPause), "Pause");
    }

    #[test]
    fn every_embedded_catalog_parses_and_is_complete() {
        for (code, _) in CATALOGS {
            let map = table()
                .get(code)
                .unwrap_or_else(|| panic!("{code} unreadable"));
            for text in ShellText::ALL {
                assert!(map.contains_key(text.key()), "{code} lacks {}", text.key());
            }
        }
    }

    #[test]
    fn the_keys_are_exactly_the_english_catalog() {
        let mut keys: Vec<&str> = table()["en"].keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected: Vec<&str> = ShellText::ALL.iter().map(|t| t.key()).collect();
        expected.sort_unstable();
        assert_eq!(keys, expected);
    }

    #[test]
    fn every_catalog_folder_with_shell_text_is_embedded() {
        let mut on_disk: Vec<String> = std::fs::read_dir(locales())
            .expect("locales folder")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().join("shell.json").is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            // `notes/` holds translator notes; the pseudo-locale is
            // generated for development only.
            .filter(|code| code != "notes" && code != "en-XA")
            .collect();
        on_disk.sort();
        let mut embedded: Vec<String> = CATALOGS.iter().map(|(c, _)| c.to_string()).collect();
        embedded.sort();
        assert_eq!(on_disk, embedded, "update catalogs![...] in shell_text.rs");
    }

    #[test]
    fn translations_are_used_when_present() {
        assert_eq!(text_in("de", ShellText::TrayQuit), "Beenden");
        assert_eq!(embedded("de"), Some("de"));
    }

    #[test]
    fn unknown_languages_fall_back_to_english() {
        assert_eq!(text_in("xx", ShellText::TrayNext), "Next");
        assert_eq!(embedded("system"), None);
    }
}
