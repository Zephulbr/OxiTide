//! "Copy diagnostics" (Settings → Troubleshooting): a plain-text report of the
//! version, platform, output path and recent problems, for bug reports. It
//! never carries secrets: config contributes on/off switches only (no API
//! keys, no device names), and log lines are redacted (`redact`) before they
//! are included.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Problems quoted from the newest log.
const RECENT_PROBLEMS: usize = 20;
/// How much of the newest log is read, from its end.
const LOG_TAIL_BYTES: u64 = 512 * 1024;
/// Query parameters whose values are never quoted.
const SECRET_PARAMS: &[&str] = &["token", "code", "state", "access_token", "refresh_token", "code_verifier"];

/// Everything the report shows, gathered by the command.
#[derive(Debug, Default)]
pub struct Inputs {
    pub version: String,
    pub debug_build: bool,
    pub os: String,
    pub webview: String,
    pub signed_in: bool,
    /// `None` while signed out (no engine).
    pub audio: Option<Audio>,
    pub switches: Vec<(&'static str, bool)>,
    pub ui: String,
    pub recent_problems: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Audio {
    pub output: String,
    pub device: String,
    pub bit_perfect: bool,
    pub notice: Option<String>,
    pub quality: String,
    pub stream: Option<String>,
    pub dsp_stages: Vec<&'static str>,
    pub underruns: u64,
}

pub fn report(inputs: &Inputs) -> String {
    let mut out = String::new();
    let build = if inputs.debug_build { "debug" } else { "release" };
    let _ = writeln!(out, "OxiTide {} ({build} build)", inputs.version);
    let _ = writeln!(out, "OS: {}", inputs.os);
    let _ = writeln!(out, "WebView: {}", inputs.webview);
    let _ = writeln!(out, "Signed in: {}", if inputs.signed_in { "yes" } else { "no" });
    match &inputs.audio {
        Some(audio) => {
            let _ = writeln!(out, "Output: {} on {}", audio.output, audio.device);
            let _ = writeln!(out, "Bit-perfect: {}", if audio.bit_perfect { "yes" } else { "no" });
            if let Some(notice) = &audio.notice {
                let _ = writeln!(out, "Output notice: {notice}");
            }
            let _ = writeln!(out, "Preferred quality: {}", audio.quality);
            let _ = writeln!(out, "Current stream: {}", audio.stream.as_deref().unwrap_or("none"));
            let stages = if audio.dsp_stages.is_empty() { "none".to_string() } else { audio.dsp_stages.join(", ") };
            let _ = writeln!(out, "Sound processing: {stages}");
            let _ = writeln!(out, "Underruns this session: {}", audio.underruns);
        }
        None => {
            let _ = writeln!(out, "Output: not started (signed out)");
        }
    }
    let _ = writeln!(out, "Appearance: {}", inputs.ui);
    let on: Vec<&str> = inputs.switches.iter().filter(|(_, on)| *on).map(|(name, _)| *name).collect();
    let _ = writeln!(out, "Enabled: {}", if on.is_empty() { "none".to_string() } else { on.join(", ") });
    let _ = writeln!(out);
    if inputs.recent_problems.is_empty() {
        let _ = writeln!(out, "Recent problems: none in the current log");
    } else {
        let _ = writeln!(out, "Recent problems (newest log, last {}):", inputs.recent_problems.len());
        for line in &inputs.recent_problems {
            let _ = writeln!(out, "  {line}");
        }
    }
    out
}

/// The last `max` warning and error lines of `log`, redacted.
pub fn recent_problems(log: &str, home: Option<&str>, max: usize) -> Vec<String> {
    let problems: Vec<&str> = log
        .lines()
        .filter(|line| line.contains(" WARN ") || line.contains(" ERROR "))
        .collect();
    problems[problems.len().saturating_sub(max)..]
        .iter()
        .map(|line| redact(line.trim(), home))
        .collect()
}

/// Drop URL query strings (they carry country, locale and, for images,
/// tokens), any other secret value (`token=…`), bearer tokens, and the home
/// directory (it names the account).
pub fn redact(line: &str, home: Option<&str>) -> String {
    let mut text = strip_url_queries(line);
    if let Some(home) = home.filter(|h| h.len() > 1) {
        text = text.replace(home, "~");
    }
    for param in SECRET_PARAMS {
        text = mask_after(&text, &format!("{param}="), |c| c == '&' || c == '"' || c == '\'' || c.is_whitespace());
    }
    mask_after(&text, "Bearer ", |c| c.is_whitespace() || c == '"' || c == '\'')
}

/// `https://host/path?query` → `https://host/path?…`.
fn strip_url_queries(text: &str) -> String {
    let ends_url = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | '>' | ']');
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("http") {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let url_end = rest.find(ends_url).unwrap_or(rest.len());
        let url = &rest[..url_end];
        match url.find('?').filter(|_| url.starts_with("http://") || url.starts_with("https://")) {
            Some(query) => {
                out.push_str(&url[..=query]);
                out.push('…');
            }
            None => out.push_str(url),
        }
        rest = &rest[url_end..];
    }
    out.push_str(rest);
    out
}

/// Replace what follows each `marker` (up to `end`) with `…`. A marker must
/// start a word or a query parameter, so `xtoken=` is left alone.
fn mask_after(text: &str, marker: &str, end: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(marker) {
        let starts_word = at == 0 || !rest[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_');
        let value_start = at + marker.len();
        out.push_str(&rest[..value_start]);
        rest = &rest[value_start..];
        if starts_word {
            let value_end = rest.find(&end).unwrap_or(rest.len());
            if value_end > 0 {
                out.push('…');
            }
            rest = &rest[value_end..];
        }
    }
    out.push_str(rest);
    out
}

/// The most recently modified `oxitide*.log` in `dir`.
pub fn newest_log(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.starts_with("oxitide") && name.ends_with(".log")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// The last [`LOG_TAIL_BYTES`] of `path`, lossily decoded (a cut may split a
/// character or a line; the first partial line is dropped).
pub fn log_tail(path: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(LOG_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    let _ = file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0 {
        text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default()
    } else {
        text
    }
}

pub fn recent_problems_from_logs() -> Vec<String> {
    let Some(path) = crate::config::logs_dir().and_then(|dir| newest_log(&dir)) else {
        return Vec::new();
    };
    let home = home_dir();
    recent_problems(&log_tail(&path), home.as_deref(), RECENT_PROBLEMS)
}

fn home_dir() -> Option<String> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var(var).ok().filter(|h| !h.is_empty())
}

/// A one-line OS description, without extra dependencies.
pub fn os_description() -> String {
    let base = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    match os_version() {
        Some(version) => format!("{version} ({base})"),
        None => base,
    }
}

#[cfg(windows)]
fn os_version() -> Option<String> {
    use std::os::windows::process::CommandExt;
    /// No console window flashes up for the GUI build.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(target_os = "linux")]
fn os_version() -> Option<String> {
    let release = std::fs::read_to_string("/etc/os-release").ok()?;
    let name = release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|name| name.trim_matches('"').to_string())?;
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    Some(format!("{name}, kernel {}", kernel.trim()))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn os_version() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_and_the_home_directory_are_redacted() {
        let line = r#"2026-10-23T10:00:00Z  WARN oxitide_app: fetch failed for https://images.tidal.com/0/X?token=abc123&w=320 at C:\Users\V\AppData (Authorization: Bearer eyJhbG.x) code=XYZ state=s1"#;
        let out = redact(line, Some(r"C:\Users\V"));
        assert!(!out.contains("abc123") && !out.contains("eyJhbG") && !out.contains("XYZ") && !out.contains("s1"), "{out}");
        assert!(out.contains("https://images.tidal.com/0/X?… at"), "{out}");
        assert!(out.contains(r"~\AppData"), "{out}");
        assert!(out.contains("Bearer …"), "{out}");
        // A word that merely ends in a secret name is left alone.
        assert_eq!(redact("mytoken=1 xcode=2", None), "mytoken=1 xcode=2");
        assert_eq!(
            redact(r#"GET "https://api.tidal.com/v1/tracks/1?countryCode=SE&locale=en" failed; see https://docs.rs/x#y"#, None),
            r#"GET "https://api.tidal.com/v1/tracks/1?…" failed; see https://docs.rs/x#y"#
        );
    }

    #[test]
    fn only_the_newest_warnings_and_errors_are_quoted() {
        let log = (1..=30)
            .map(|i| match i % 3 {
                0 => format!("t{i}  WARN app: w{i}"),
                1 => format!("t{i}  INFO app: i{i}"),
                _ => format!("t{i} ERROR app: e{i}"),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let problems = recent_problems(&log, None, 5);
        assert_eq!(problems.len(), 5);
        assert!(problems.iter().all(|p| !p.contains(" INFO ")));
        assert_eq!(problems.last().unwrap(), "t30  WARN app: w30");
    }

    #[test]
    fn the_report_lists_the_output_path_and_enabled_switches_only() {
        let report = report(&Inputs {
            version: "0.4.0".into(),
            os: "Windows 11 (windows x86_64)".into(),
            webview: "130.0".into(),
            signed_in: true,
            audio: Some(Audio {
                output: "Exclusive".into(),
                device: "system default".into(),
                bit_perfect: true,
                quality: "Lossless".into(),
                stream: Some("flac 44.1 kHz / 16-bit".into()),
                dsp_stages: vec!["eq"],
                underruns: 2,
                ..Default::default()
            }),
            switches: vec![("Discord", true), ("Phone remote", false)],
            ui: "atmosphere, dark".into(),
            recent_problems: vec![],
            ..Default::default()
        });
        assert!(report.starts_with("OxiTide 0.4.0 (release build)\n"), "{report}");
        assert!(report.contains("Output: Exclusive on system default\n"));
        assert!(report.contains("Sound processing: eq\n"));
        assert!(report.contains("Underruns this session: 2\n"));
        assert!(report.contains("Enabled: Discord\n"));
        assert!(!report.contains("Phone remote"));
        assert!(report.contains("Recent problems: none"));
    }

    #[test]
    fn the_log_tail_drops_a_partial_first_line() {
        let dir = std::env::temp_dir().join(format!("oxitide-diag-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("oxitide.2026-10-23.log");
        let line = "x".repeat(1000);
        let text = (0..600).map(|i| format!("{i} {line}")).collect::<Vec<_>>().join("\n");
        std::fs::write(&path, &text).unwrap();
        let tail = log_tail(&path);
        assert!(tail.len() <= LOG_TAIL_BYTES as usize);
        assert!(tail.lines().next().unwrap().ends_with(&line), "the first line must be whole");
        assert!(tail.ends_with(&format!("599 {line}")));
        assert_eq!(newest_log(&dir), Some(path));
        let _ = std::fs::remove_dir_all(dir);
    }
}
