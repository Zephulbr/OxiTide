//! What the output picker knows about a device beyond its id: type, connection,
//! a clean display name, and what it can play (spec
//! `2026-11-05-oxitide-output-device-picker-design.md`).
//!
//! The naming rules and the `/proc/asound` parsers are pure so they are tested on
//! every platform; the probes that touch a device live with their backends.

use serde::{Deserialize, Serialize};

use crate::output::OutputDevice;

/// The picker group a device is listed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceGroup {
    Wasapi,
    Asio,
    Alsa,
}

/// What the device is, for its icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Speakers,
    Headphones,
    Headset,
    Hdmi,
    Spdif,
    Line,
    Bluetooth,
    Usb,
    Virtual,
    Unknown,
}

/// How the device is connected, shown on the row's second line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Usb,
    Bluetooth,
    Hdmi,
    Internal,
    Virtual,
    Unknown,
}

/// Where a [`DeviceCaps`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapsSource {
    /// An active check of the device.
    Probed,
    /// Recorded from an exclusive open that succeeded.
    Played,
    #[default]
    Unknown,
}

/// What a device can play. `exclusive` means a bit-perfect exclusive open is
/// possible; the rates and depths are the best ones found.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeviceCaps {
    pub exclusive: bool,
    pub max_rate: Option<u32>,
    pub max_bits: Option<u16>,
    pub hw_volume: Option<bool>,
    pub shared_rate: Option<u32>,
    pub shared_bits: Option<u16>,
    pub source: CapsSource,
}

impl DeviceCaps {
    /// The capability an exclusive open that succeeded proves.
    pub fn played(rate: u32, bits: u16) -> Self {
        Self {
            exclusive: true,
            max_rate: Some(rate),
            max_bits: Some(bits),
            source: CapsSource::Played,
            ..Self::default()
        }
    }

    /// Build probed caps from the exclusive formats a device accepted.
    pub fn from_formats(formats: &[(u32, u16)]) -> Self {
        Self {
            exclusive: !formats.is_empty(),
            max_rate: formats.iter().map(|f| f.0).max(),
            max_bits: formats.iter().map(|f| f.1).max(),
            source: CapsSource::Probed,
            ..Self::default()
        }
    }

    /// Keep the larger of two learned records.
    pub fn widen(&mut self, other: &DeviceCaps) {
        self.exclusive |= other.exclusive;
        self.max_rate = self.max_rate.max(other.max_rate);
        self.max_bits = self.max_bits.max(other.max_bits);
    }
}

/// Rates an active check tries, highest first.
pub const PROBE_RATES: [u32; 8] = [384_000, 352_800, 192_000, 176_400, 96_000, 88_200, 48_000, 44_100];
/// Integer depths an active check tries, highest first.
pub const PROBE_BITS: [u16; 3] = [32, 24, 16];

/// Drop Windows' numbering in front of a name: the instance prefix of a second
/// identical device ("2- MOONDROP Dawn Pro") and the display number of an HDMI
/// or DisplayPort endpoint ("4 - LG TV").
pub fn strip_number_prefix(name: &str) -> &str {
    let trimmed = name.trim();
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return trimmed;
    }
    let rest = trimmed[digits..].trim_start();
    match rest.strip_prefix('-') {
        Some(after) if !after.trim().is_empty() => after.trim(),
        _ => trimmed,
    }
}

/// True for an endpoint name that only says what kind of output it is.
pub fn is_generic_endpoint_name(name: &str) -> bool {
    const GENERIC: &[&str] = &[
        "speakers",
        "speaker",
        "headphones",
        "headphone",
        "headset",
        "headset earphone",
        "earphones",
        "earphone",
        "line",
        "line out",
        "line-out",
        "digital output",
        "digital audio",
        "digital audio (s/pdif)",
        "s/pdif",
        "spdif",
        "spdif out",
        "optical",
        "speakers/headphones",
        "output",
        "analog output",
        "audio",
    ];
    let lower = name.trim().to_ascii_lowercase();
    GENERIC.contains(&lower.as_str())
}

/// The endpoint properties the WASAPI picker reads (spike note
/// `plans/notes/2026-11-05-device-properties-spike.md`).
#[derive(Debug, Clone, Default)]
pub struct WasapiProps {
    /// `PKEY_Device_DeviceDesc`: "Speakers", "4 - LG TV", "DX5".
    pub endpoint: String,
    /// `PKEY_DeviceInterface_FriendlyName`: "TOPPING USB DAC".
    pub adapter: String,
    /// `PKEY_Device_FriendlyName`: "DX5 (TOPPING USB DAC)".
    pub friendly: String,
    /// `PKEY_Device_EnumeratorName`: "USB", "HDAUDIO", "TUSBAUDIO_ENUM", "BTHENUM".
    pub enumerator: String,
    /// `PKEY_AudioEndpoint_FormFactor` (`EndpointFormFactor`).
    pub form_factor: Option<u32>,
}

const FF_SPEAKERS: u32 = 1;
const FF_LINE: u32 = 2;
const FF_HEADPHONES: u32 = 3;
const FF_HEADSET: u32 = 5;
const FF_HANDSET: u32 = 6;
const FF_SPDIF: u32 = 8;
const FF_DISPLAY: u32 = 9;

/// Name, kind and connection of a WASAPI endpoint.
pub fn wasapi_identity(props: &WasapiProps) -> (String, DeviceKind, Transport) {
    let enumerator = props.enumerator.to_ascii_uppercase();
    let display = props.form_factor == Some(FF_DISPLAY);
    let transport = if enumerator.contains("USB") {
        Transport::Usb
    } else if enumerator.starts_with("BTH") {
        Transport::Bluetooth
    } else if display {
        Transport::Hdmi
    } else if enumerator.contains("HDAUDIO") || enumerator.contains("INTELAUDIO") || enumerator == "ACPI" {
        Transport::Internal
    } else if matches!(enumerator.as_str(), "ROOT" | "SW" | "SWD" | "MMDEVAPI") {
        Transport::Virtual
    } else {
        Transport::Unknown
    };
    let kind = match props.form_factor {
        Some(FF_DISPLAY) => DeviceKind::Hdmi,
        Some(FF_SPDIF) => DeviceKind::Spdif,
        Some(FF_HEADPHONES) => DeviceKind::Headphones,
        Some(FF_HEADSET) | Some(FF_HANDSET) => DeviceKind::Headset,
        Some(FF_LINE) => DeviceKind::Line,
        _ => match transport {
            Transport::Usb => DeviceKind::Usb,
            Transport::Bluetooth => DeviceKind::Bluetooth,
            Transport::Virtual => DeviceKind::Virtual,
            _ if props.form_factor == Some(FF_SPEAKERS) => DeviceKind::Speakers,
            _ => DeviceKind::Unknown,
        },
    };
    let endpoint = strip_number_prefix(&props.endpoint);
    let adapter = strip_number_prefix(&props.adapter);
    let name = if display && !endpoint.is_empty() {
        endpoint.to_string()
    } else if is_generic_endpoint_name(endpoint) && transport != Transport::Internal && !adapter.is_empty() {
        adapter.to_string()
    } else if !endpoint.is_empty() {
        endpoint.to_string()
    } else {
        strip_number_prefix(&props.friendly).to_string()
    };
    (name, kind, transport)
}

/// ASIO drivers that wrap the Windows audio engine instead of a device.
pub fn is_wrapper_asio(key: &str, description: &str) -> bool {
    const WRAPPERS: &[&str] = &["asio4all", "flexasio", "realtek asio", "asio2wasapi", "generic low latency"];
    let text = format!("{key} {description}").to_ascii_lowercase();
    WRAPPERS.iter().any(|w| text.contains(w))
}

/// Picker label for an ASIO driver: the registry description without a
/// trailing "(ASIO)" or " ASIO Driver" (the group says it).
pub fn asio_name(description: &str) -> String {
    let name = description.trim();
    for suffix in [" (ASIO)", " ASIO Driver", " ASIO driver"] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            if !stripped.trim().is_empty() {
                return stripped.trim().to_string();
            }
        }
    }
    name.to_string()
}

/// When two devices in one group would show the same name, both use their full
/// system name instead.
pub fn dedupe_names(devices: &mut [OutputDevice]) {
    use std::collections::HashMap;
    let mut counts: HashMap<(DeviceGroup, String), usize> = HashMap::new();
    for device in devices.iter() {
        *counts.entry((device.group, device.name.to_lowercase())).or_default() += 1;
    }
    for device in devices.iter_mut() {
        if counts[&(device.group, device.name.to_lowercase())] > 1 && !device.system_name.is_empty() {
            device.name = strip_number_prefix(&device.system_name).to_string();
        }
    }
}

/// An HDMI sink's EDID-Like Data, from `/proc/asound/cardN/eld#C.P`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eld {
    pub monitor_present: bool,
    pub monitor_name: String,
}

/// Parse an ELD file. `None` when it carries no `monitor_present` line.
pub fn parse_eld(text: &str) -> Option<Eld> {
    let mut present = None;
    let mut name = String::new();
    for line in text.lines() {
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or_default().trim();
        let value = parts.next().unwrap_or_default().trim();
        match key {
            "monitor_present" => present = Some(value == "1"),
            "monitor_name" => name = value.to_string(),
            _ => {}
        }
    }
    present.map(|monitor_present| Eld { monitor_present, monitor_name: name })
}

/// The best playback rate and depth a USB card advertises in
/// `/proc/asound/cardN/stream0` (playback section only).
pub fn parse_usb_stream(text: &str) -> Option<(u32, u16)> {
    let mut in_playback = false;
    let mut best_rate = 0u32;
    let mut best_bits = 0u16;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("Playback:") {
            in_playback = true;
            continue;
        }
        if line.starts_with("Capture:") {
            in_playback = false;
            continue;
        }
        if !in_playback {
            continue;
        }
        if let Some(format) = line.strip_prefix("Format:") {
            for token in format.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()) {
                best_bits = best_bits.max(alsa_format_bits(token));
            }
        } else if let Some(bits) = line.strip_prefix("Bits:") {
            if let Ok(bits) = bits.trim().parse::<u16>() {
                best_bits = best_bits.max(bits);
            }
        } else if let Some(rates) = line.strip_prefix("Rates:") {
            for number in rates.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()) {
                if let Ok(rate) = number.parse::<u32>() {
                    best_rate = best_rate.max(rate);
                }
            }
        }
    }
    (best_rate > 0 && best_bits > 0).then_some((best_rate, best_bits))
}

fn alsa_format_bits(token: &str) -> u16 {
    match token.trim().to_ascii_uppercase().as_str() {
        "S32_LE" | "S32_BE" | "U32_LE" => 32,
        "S24_LE" | "S24_3LE" | "S24_BE" | "S24_3BE" => 24,
        "S16_LE" | "S16_BE" => 16,
        _ => 0,
    }
}

/// One playback PCM of a card, with what `/proc` says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlsaPcmInfo {
    pub card_id: String,
    pub card_name: String,
    pub device: u32,
    pub pcm_name: String,
    /// The card is a USB device (`/proc/asound/cardN/usbid` exists).
    pub usb: bool,
    /// The card has more than one playback PCM.
    pub multi: bool,
    /// The ELD matched to an HDMI PCM, if any.
    pub eld: Option<Eld>,
}

/// Name, system name, kind, connection and presence of an ALSA PCM row.
pub fn alsa_identity(pcm: &AlsaPcmInfo) -> (String, String, DeviceKind, Transport, bool) {
    let card = if pcm.card_name.trim().is_empty() { pcm.card_id.trim() } else { pcm.card_name.trim() };
    let pcm_name = pcm.pcm_name.trim();
    let system_name = if pcm_name.is_empty() { card.to_string() } else { format!("{card}, {pcm_name}") };
    let upper = pcm_name.to_ascii_uppercase();
    let hdmi = upper.contains("HDMI") || upper.contains("DISPLAYPORT") || upper.starts_with("DP ");
    let transport = if pcm.usb {
        Transport::Usb
    } else if hdmi {
        Transport::Hdmi
    } else {
        Transport::Internal
    };
    let lower = pcm_name.to_ascii_lowercase();
    let kind = if pcm.usb {
        DeviceKind::Usb
    } else if hdmi {
        DeviceKind::Hdmi
    } else if lower.contains("digital") || lower.contains("spdif") || lower.contains("iec958") {
        DeviceKind::Spdif
    } else {
        DeviceKind::Speakers
    };
    let present = pcm.eld.as_ref().is_none_or(|eld| eld.monitor_present);
    let name = match &pcm.eld {
        Some(eld) if eld.monitor_present && !eld.monitor_name.trim().is_empty() => eld.monitor_name.trim().to_string(),
        _ if hdmi => {
            if pcm.multi && !pcm_name.is_empty() {
                pcm_name.to_string()
            } else {
                card.to_string()
            }
        }
        _ if !pcm.multi || pcm.usb => card.to_string(),
        _ if kind == DeviceKind::Spdif => "Digital output".to_string(),
        _ if lower.contains("analog") => "Speakers".to_string(),
        _ if pcm_name.is_empty() => card.to_string(),
        _ => format!("{card} {pcm_name}"),
    };
    (name, system_name, kind, transport, present)
}

/// The HDMI index of a PCM name ("HDMI 2" → 2), for matching ELD files.
pub fn hdmi_index(pcm_name: &str) -> Option<usize> {
    let upper = pcm_name.to_ascii_uppercase();
    let at = upper.find("HDMI")?;
    upper[at + 4..].trim().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

/// Map a saved ALSA device name onto the one-row-per-PCM id
/// (`hw:CARD=<id>,DEV=<n>`). `default` means the system default (`""`).
/// `card_ids` lists the present cards' ids by index, for numeric `hw:0,0`.
/// Anything else is returned unchanged.
pub fn migrate_alsa_id(id: &str, card_ids: &[String]) -> String {
    if id == "default" || id == "sysdefault" {
        return String::new();
    }
    let rest = if let Some(rest) = id.strip_prefix("plughw:") {
        rest
    } else if let Some(rest) = id.strip_prefix("sysdefault:") {
        rest
    } else if let Some(rest) = id.strip_prefix("hw:") {
        rest
    } else {
        return id.to_string();
    };
    let mut card = None;
    let mut device = 0u32;
    for (index, part) in rest.split(',').enumerate() {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("CARD=") {
            card = Some(value.to_string());
        } else if let Some(value) = part.strip_prefix("DEV=") {
            device = value.parse().unwrap_or(0);
        } else if index == 0 {
            card = match part.parse::<usize>() {
                Ok(n) => card_ids.get(n).cloned(),
                Err(_) => Some(part.to_string()),
            };
        } else if index == 1 {
            device = part.parse().unwrap_or(0);
        }
    }
    match card {
        Some(card) if !card.is_empty() => format!("hw:CARD={card},DEV={device}"),
        _ => id.to_string(),
    }
}

/// Call `on_change` (debounced) whenever an output device is added, removed,
/// enabled, disabled or becomes the default. Windows only; elsewhere the picker
/// re-reads the list when it opens (spec DP-D12). The watch lasts for the
/// process.
pub fn watch_devices(on_change: impl Fn() + Send + Sync + 'static) {
    #[cfg(windows)]
    crate::output::platform::watch(Box::new(on_change));
    #[cfg(not(windows))]
    drop(on_change);
}

/// Actively check what `device` can play. May block for a moment (a WASAPI
/// format check, an ASIO driver load, an ALSA open); call it off the UI path.
/// `None` when the device can't be checked right now (busy, missing).
pub fn probe_caps(device: &OutputDevice) -> Option<DeviceCaps> {
    #[cfg(windows)]
    {
        match device.group {
            DeviceGroup::Asio => crate::asio_output::probe_caps(&device.id),
            _ => crate::output::platform::probe_caps(&device.id),
        }
    }
    #[cfg(target_os = "linux")]
    {
        crate::alsa_output::probe_caps(&device.id)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = device;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(endpoint: &str, adapter: &str, enumerator: &str, ff: u32) -> WasapiProps {
        WasapiProps {
            endpoint: endpoint.into(),
            adapter: adapter.into(),
            friendly: format!("{endpoint} ({adapter})"),
            enumerator: enumerator.into(),
            form_factor: Some(ff),
        }
    }

    #[test]
    fn number_prefixes_are_stripped() {
        assert_eq!(strip_number_prefix("4 - LG TV"), "LG TV");
        assert_eq!(strip_number_prefix("2- MOONDROP Dawn Pro"), "MOONDROP Dawn Pro");
        assert_eq!(strip_number_prefix("DX5"), "DX5");
        assert_eq!(strip_number_prefix("1080p"), "1080p");
        assert_eq!(strip_number_prefix("3 -"), "3 -");
    }

    #[test]
    fn wasapi_names_follow_the_spike_rules() {
        // Values from the 2026-11-05 property spike.
        assert_eq!(
            wasapi_identity(&props("4 - LG TV", "AMD High Definition Audio Device", "HDAUDIO", 9)),
            ("LG TV".into(), DeviceKind::Hdmi, Transport::Hdmi)
        );
        assert_eq!(
            wasapi_identity(&props("Realtek Digital Output", "Realtek(R) Audio", "HDAUDIO", 8)),
            ("Realtek Digital Output".into(), DeviceKind::Spdif, Transport::Internal)
        );
        assert_eq!(
            wasapi_identity(&props("Speakers", "HyperX DuoCast", "USB", 1)),
            ("HyperX DuoCast".into(), DeviceKind::Usb, Transport::Usb)
        );
        assert_eq!(
            wasapi_identity(&props("DX5", "TOPPING USB DAC", "TUSBAUDIO_ENUM", 1)),
            ("DX5".into(), DeviceKind::Usb, Transport::Usb)
        );
        assert_eq!(
            wasapi_identity(&props("Speakers", "Realtek(R) Audio", "HDAUDIO", 1)),
            ("Speakers".into(), DeviceKind::Speakers, Transport::Internal)
        );
        assert_eq!(
            wasapi_identity(&props("Headphones", "WH-1000XM5", "BTHENUM", 3)),
            ("WH-1000XM5".into(), DeviceKind::Headphones, Transport::Bluetooth)
        );
        assert_eq!(
            wasapi_identity(&props("CABLE Input", "VB-Audio Virtual Cable", "ROOT", 10)),
            ("CABLE Input".into(), DeviceKind::Virtual, Transport::Virtual)
        );
    }

    #[test]
    fn colliding_names_fall_back_to_the_system_name() {
        let mut devices = vec![
            OutputDevice::new("a", "Speakers", DeviceGroup::Wasapi).with_system_name("Speakers (Realtek(R) Audio)"),
            OutputDevice::new("b", "Speakers", DeviceGroup::Wasapi).with_system_name("Speakers (2- Realtek(R) Audio)"),
            OutputDevice::new("c", "Speakers", DeviceGroup::Asio),
        ];
        dedupe_names(&mut devices);
        assert_eq!(devices[0].name, "Speakers (Realtek(R) Audio)");
        assert_eq!(devices[1].name, "Speakers (2- Realtek(R) Audio)");
        assert_eq!(devices[2].name, "Speakers", "other groups do not collide");
    }

    #[test]
    fn asio_wrappers_and_names() {
        assert!(is_wrapper_asio("ASIO4ALL v2", "ASIO4ALL v2"));
        assert!(is_wrapper_asio("FlexASIO", "FlexASIO"));
        assert!(is_wrapper_asio("Realtek ASIO", "Realtek ASIO"));
        assert!(!is_wrapper_asio("Focusrite USB ASIO", "Focusrite USB ASIO"));
        assert_eq!(asio_name("Focusrite USB ASIO"), "Focusrite USB ASIO");
        assert_eq!(asio_name("TOPPING USB DAC (ASIO)"), "TOPPING USB DAC");
        assert_eq!(asio_name("Yamaha Steinberg USB ASIO Driver"), "Yamaha Steinberg USB");
    }

    #[test]
    fn parses_eld() {
        let text = "monitor_present\t\t1\neld_valid\t\t1\nmonitor_name\t\tDELL U2723QE\nconnection_type\t\tHDMI\n";
        assert_eq!(parse_eld(text), Some(Eld { monitor_present: true, monitor_name: "DELL U2723QE".into() }));
        assert_eq!(
            parse_eld("monitor_present\t\t0\neld_valid\t\t0\n"),
            Some(Eld { monitor_present: false, monitor_name: String::new() })
        );
        assert_eq!(parse_eld("garbage"), None);
    }

    #[test]
    fn parses_usb_stream_playback_only() {
        let text = "\
MOONDROP Dawn Pro at usb-0000:00:14.0-2, high speed : USB Audio

Playback:
  Status: Stop
  Interface 1
    Altset 1
    Format: S16_LE
    Channels: 2
    Rates: 44100, 48000, 88200, 96000
    Bits: 16
  Interface 1
    Altset 2
    Format: S32_LE
    Channels: 2
    Rates: 44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000
    Bits: 32

Capture:
  Interface 2
    Altset 1
    Format: S32_LE
    Rates: 768000
";
        assert_eq!(parse_usb_stream(text), Some((384_000, 32)));
        assert_eq!(parse_usb_stream("Playback:\n  Format: S24_3LE\n  Rates: 8000 - 96000 (continuous)\n"), Some((96_000, 24)));
        assert_eq!(parse_usb_stream("Capture:\n  Format: S16_LE\n  Rates: 48000\n"), None);
    }

    fn pcm(card: &str, pcm_name: &str, device: u32, usb: bool, multi: bool, eld: Option<Eld>) -> AlsaPcmInfo {
        AlsaPcmInfo {
            card_id: card.split_whitespace().next().unwrap_or(card).into(),
            card_name: card.into(),
            device,
            pcm_name: pcm_name.into(),
            usb,
            multi,
            eld,
        }
    }

    #[test]
    fn alsa_rows_get_clean_names() {
        let dac = alsa_identity(&pcm("MOONDROP Dawn Pro", "USB Audio", 0, true, false, None));
        assert_eq!(dac, ("MOONDROP Dawn Pro".into(), "MOONDROP Dawn Pro, USB Audio".into(), DeviceKind::Usb, Transport::Usb, true));
        let analog = alsa_identity(&pcm("HDA Intel PCH", "ALC3246 Analog", 0, false, true, None));
        assert_eq!(analog.0, "Speakers");
        assert_eq!(analog.2, DeviceKind::Speakers);
        assert_eq!(analog.3, Transport::Internal);
        let monitor = alsa_identity(&pcm(
            "HDA Intel PCH",
            "HDMI 1",
            7,
            false,
            true,
            Some(Eld { monitor_present: true, monitor_name: "DELL U2723QE".into() }),
        ));
        assert_eq!((monitor.0.as_str(), monitor.2, monitor.3, monitor.4), ("DELL U2723QE", DeviceKind::Hdmi, Transport::Hdmi, true));
        let empty = alsa_identity(&pcm(
            "HDA Intel PCH",
            "HDMI 2",
            8,
            false,
            true,
            Some(Eld { monitor_present: false, monitor_name: String::new() }),
        ));
        assert_eq!((empty.0.as_str(), empty.4), ("HDMI 2", false));
    }

    #[test]
    fn hdmi_indices() {
        assert_eq!(hdmi_index("HDMI 0"), Some(0));
        assert_eq!(hdmi_index("HDMI 3"), Some(3));
        assert_eq!(hdmi_index("ALC3246 Analog"), None);
    }

    #[test]
    fn migrates_saved_alsa_ids() {
        let cards = vec!["PCH".to_string(), "Pro".to_string()];
        assert_eq!(migrate_alsa_id("default", &cards), "");
        assert_eq!(migrate_alsa_id("sysdefault:CARD=Pro", &cards), "hw:CARD=Pro,DEV=0");
        assert_eq!(migrate_alsa_id("plughw:CARD=Pro,DEV=0", &cards), "hw:CARD=Pro,DEV=0");
        assert_eq!(migrate_alsa_id("hw:CARD=PCH,DEV=3", &cards), "hw:CARD=PCH,DEV=3");
        assert_eq!(migrate_alsa_id("hw:1,0", &cards), "hw:CARD=Pro,DEV=0");
        assert_eq!(migrate_alsa_id("hw:7,0", &cards), "hw:7,0", "an absent card stays as it was");
        assert_eq!(migrate_alsa_id("asio:Foo", &cards), "asio:Foo");
        assert_eq!(migrate_alsa_id("{0.0.0.00000000}.{abc}", &cards), "{0.0.0.00000000}.{abc}");
    }

    #[test]
    fn caps_from_formats_and_widen() {
        let caps = DeviceCaps::from_formats(&[(44_100, 24), (192_000, 24), (96_000, 32)]);
        assert!(caps.exclusive);
        assert_eq!((caps.max_rate, caps.max_bits), (Some(192_000), Some(32)));
        assert!(!DeviceCaps::from_formats(&[]).exclusive);
        let mut learned = DeviceCaps::played(44_100, 16);
        learned.widen(&DeviceCaps::played(96_000, 24));
        assert_eq!((learned.max_rate, learned.max_bits), (Some(96_000), Some(24)));
    }
}
