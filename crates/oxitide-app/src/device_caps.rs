//! What each output device can play, for the picker's "Up to …" line (spec
//! `2026-11-05-oxitide-output-device-picker-design.md` §7).
//!
//! Two sources:
//! - **probed**: an active check per device, run on a worker thread after the
//!   list is returned, kept for the session and dropped when devices change;
//! - **played**: what an exclusive, bit-perfect playback proved, kept in
//!   `config\device_caps.json` so a card that can't be checked while busy
//!   (PipeWire, an open ASIO driver) still shows its tag after a restart.
//!
//! Learned records live in their own file, not `config.toml`: they are written
//! from the engine's event thread, and config writers must stay in command
//! order.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use oxitide_audio::devices::DeviceCaps;
use oxitide_audio::output::OutputDevice;
use serde::{Deserialize, Serialize};

/// `%APPDATA%\OxiTide\config\device_caps.json`.
fn caps_path() -> Option<PathBuf> {
    crate::config::config_path().map(|path| path.with_file_name("device_caps.json"))
}

#[derive(Default, Serialize, Deserialize)]
struct CapsFile {
    #[serde(default)]
    played: BTreeMap<String, DeviceCaps>,
}

pub struct DeviceCapsStore {
    path: Option<PathBuf>,
    probed: Mutex<HashMap<String, DeviceCaps>>,
    /// Ids checked (or being checked) since the last device change, so a
    /// device that can't be checked is not retried on every list.
    attempted: Mutex<HashSet<String>>,
    played: OnceLock<Mutex<BTreeMap<String, DeviceCaps>>>,
    probing: AtomicBool,
}

impl Default for DeviceCapsStore {
    fn default() -> Self {
        Self::with_path(caps_path())
    }
}

impl DeviceCapsStore {
    pub fn with_path(path: Option<PathBuf>) -> Self {
        Self {
            path,
            probed: Mutex::default(),
            attempted: Mutex::default(),
            played: OnceLock::new(),
            probing: AtomicBool::new(false),
        }
    }

    fn played(&self) -> &Mutex<BTreeMap<String, DeviceCaps>> {
        self.played.get_or_init(|| {
            let file = self
                .path
                .as_ref()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .and_then(|text| serde_json::from_str::<CapsFile>(&text).ok())
                .unwrap_or_default();
            Mutex::new(file.played)
        })
    }

    /// The best known caps: a probe beats a learned record, which fills in
    /// when the device couldn't be checked.
    pub fn caps_for(&self, id: &str) -> Option<DeviceCaps> {
        if let Some(caps) = self.probed.lock().unwrap().get(id) {
            if caps.exclusive || !self.played().lock().unwrap().contains_key(id) {
                return Some(caps.clone());
            }
        }
        self.played().lock().unwrap().get(id).cloned()
    }

    /// Record a bit-perfect exclusive playback. Writes the file only when the
    /// record grows. Returns whether it did.
    pub fn record_played(&self, id: &str, rate: u32, bits: u16) -> bool {
        if id.is_empty() || rate == 0 || bits == 0 {
            return false;
        }
        let mut played = self.played().lock().unwrap();
        let next = match played.get(id) {
            Some(existing) => {
                let mut widened = existing.clone();
                widened.widen(&DeviceCaps::played(rate, bits));
                if &widened == existing {
                    return false;
                }
                widened
            }
            None => DeviceCaps::played(rate, bits),
        };
        played.insert(id.to_string(), next);
        self.persist(&played);
        true
    }

    fn persist(&self, played: &BTreeMap<String, DeviceCaps>) {
        let Some(path) = &self.path else { return };
        let file = CapsFile { played: played.clone() };
        let Ok(text) = serde_json::to_string_pretty(&file) else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }

    /// Devices changed (added, removed, default moved): check again.
    pub fn invalidate(&self) {
        self.probed.lock().unwrap().clear();
        self.attempted.lock().unwrap().clear();
    }

    /// The listed devices not yet checked this session, marked as checked.
    /// `skip` is left for later (the device playing exclusively right now: a
    /// check would see it busy and report no exclusive support).
    pub fn take_unchecked(&self, devices: &[OutputDevice], skip: Option<&str>) -> Vec<OutputDevice> {
        let mut attempted = self.attempted.lock().unwrap();
        devices
            .iter()
            .filter(|device| device.present && Some(device.id.as_str()) != skip)
            .filter(|device| attempted.insert(device.id.clone()))
            .cloned()
            .collect()
    }

    pub fn store_probed(&self, id: &str, caps: DeviceCaps) {
        self.probed.lock().unwrap().insert(id.to_string(), caps);
    }

    /// Claim the single checking worker. `false` while one is running.
    pub fn begin_probing(&self) -> bool {
        !self.probing.swap(true, Ordering::SeqCst)
    }

    pub fn end_probing(&self) {
        self.probing.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_audio::devices::{CapsSource, DeviceGroup};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oxitide-caps-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn played_records_grow_and_persist() {
        let dir = temp_dir("played");
        let path = dir.join("device_caps.json");
        let store = DeviceCapsStore::with_path(Some(path.clone()));
        assert!(store.record_played("dac", 44_100, 16));
        assert!(!store.record_played("dac", 44_100, 16), "same record, no write");
        assert!(store.record_played("dac", 96_000, 24));
        assert!(!store.record_played("", 96_000, 24), "system default is not a device");
        let reloaded = DeviceCapsStore::with_path(Some(path));
        let caps = reloaded.caps_for("dac").unwrap();
        assert_eq!((caps.max_rate, caps.max_bits, caps.source), (Some(96_000), Some(24), CapsSource::Played));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_probe_wins_unless_it_found_nothing_a_playback_proved() {
        let store = DeviceCapsStore::with_path(None);
        store.record_played("dac", 96_000, 24);
        store.store_probed("dac", DeviceCaps::from_formats(&[]));
        assert_eq!(store.caps_for("dac").unwrap().source, CapsSource::Played);
        store.store_probed("dac", DeviceCaps::from_formats(&[(384_000, 32)]));
        assert_eq!(store.caps_for("dac").unwrap().max_rate, Some(384_000));
    }

    #[test]
    fn each_device_is_checked_once_until_devices_change() {
        let store = DeviceCapsStore::with_path(None);
        let mut absent = OutputDevice::new("hdmi2", "HDMI 2", DeviceGroup::Alsa);
        absent.present = false;
        let devices = vec![
            OutputDevice::new("a", "A", DeviceGroup::Wasapi),
            OutputDevice::new("b", "B", DeviceGroup::Wasapi),
            absent,
        ];
        let first: Vec<String> = store.take_unchecked(&devices, Some("b")).into_iter().map(|d| d.id).collect();
        assert_eq!(first, vec!["a"], "the playing device and empty outputs wait");
        assert!(store.take_unchecked(&devices, Some("b")).is_empty());
        let next: Vec<String> = store.take_unchecked(&devices, None).into_iter().map(|d| d.id).collect();
        assert_eq!(next, vec!["b"]);
        store.invalidate();
        assert_eq!(store.take_unchecked(&devices, None).len(), 2);
    }

    #[test]
    fn one_worker_at_a_time() {
        let store = DeviceCapsStore::with_path(None);
        assert!(store.begin_probing());
        assert!(!store.begin_probing());
        store.end_probing();
        assert!(store.begin_probing());
    }
}
