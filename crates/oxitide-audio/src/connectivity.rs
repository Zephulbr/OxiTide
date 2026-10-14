//! What the operating system says about the network, so playback can resume
//! the moment it returns instead of on its next timer, and can wait out a
//! known outage without counting it as failure.
//!
//! Windows: `NotifyNetworkConnectivityHintChange` (Windows 10 2004+), pushed
//! by the system. Linux: a 2 s check for a default route in
//! `/proc/net/route` / `/proc/net/ipv6_route`. Elsewhere, or if the watcher
//! cannot start, the state stays unknown and recovery falls back to its timer.

use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

pub struct Connectivity {
    state: Mutex<State>,
    cond: Condvar,
}

#[derive(Default)]
struct State {
    /// `Some(false)`: the system reports no internet access. `None`: unknown.
    online: Option<bool>,
    /// Bumped on every reported change.
    epoch: u64,
}

impl Connectivity {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(State::default()), cond: Condvar::new() })
    }

    /// True only when the system reports the network down.
    pub fn is_offline(&self) -> bool {
        self.state.lock().unwrap().online == Some(false)
    }

    pub fn epoch(&self) -> u64 {
        self.state.lock().unwrap().epoch
    }

    /// Record a reported state; waiters wake when it differs from the last.
    pub fn set_online(&self, online: Option<bool>) {
        let mut state = self.state.lock().unwrap();
        if state.online != online {
            if state.online.is_some() || online.is_some() {
                tracing::info!(
                    "network {}",
                    match online {
                        Some(true) => "up",
                        Some(false) => "down",
                        None => "state unknown",
                    }
                );
            }
            state.online = online;
            state.epoch += 1;
            self.cond.notify_all();
        }
    }

    /// Block until the next reported change or `timeout`.
    pub fn wait_change(&self, timeout: Duration) {
        let state = self.state.lock().unwrap();
        let epoch = state.epoch;
        let _ = self.cond.wait_timeout_while(state, timeout, |s| s.epoch == epoch).unwrap();
    }
}

/// The process-wide monitor; the platform watcher starts on first use.
pub fn global() -> &'static Arc<Connectivity> {
    static GLOBAL: OnceLock<Arc<Connectivity>> = OnceLock::new();
    GLOBAL.get_or_init(|| {
        let conn = Connectivity::new();
        // Tests must not depend on the machine's network.
        if !cfg!(test) {
            platform::watch(conn.clone());
        }
        conn
    })
}

#[cfg(windows)]
mod platform {
    use super::Connectivity;
    use std::sync::Arc;
    use windows::Win32::Foundation::{HANDLE, NO_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::NotifyNetworkConnectivityHintChange;
    use windows::Win32::Networking::WinSock::{
        NetworkConnectivityLevelHintConstrainedInternetAccess,
        NetworkConnectivityLevelHintInternetAccess, NetworkConnectivityLevelHintLocalAccess,
        NetworkConnectivityLevelHintNone, NL_NETWORK_CONNECTIVITY_HINT,
    };

    unsafe extern "system" fn on_hint(context: *const core::ffi::c_void, hint: NL_NETWORK_CONNECTIVITY_HINT) {
        // SAFETY: `context` is the `Connectivity` leaked in `watch`, which
        // lives for the rest of the process.
        let conn = unsafe { &*(context as *const Connectivity) };
        let level = hint.ConnectivityLevel;
        let online = if level == NetworkConnectivityLevelHintInternetAccess
            || level == NetworkConnectivityLevelHintConstrainedInternetAccess
        {
            Some(true)
        } else if level == NetworkConnectivityLevelHintNone
            || level == NetworkConnectivityLevelHintLocalAccess
        {
            Some(false)
        } else {
            None // unknown or hidden
        };
        conn.set_online(online);
    }

    pub fn watch(conn: Arc<Connectivity>) {
        // The registration lives as long as the process, so the context
        // pointer is leaked deliberately and never cancelled.
        let context = Arc::into_raw(conn) as *const core::ffi::c_void;
        let mut handle = HANDLE::default();
        // SAFETY: `on_hint` matches the callback signature and `context`
        // stays valid forever (leaked above).
        let status = unsafe {
            NotifyNetworkConnectivityHintChange(Some(on_hint), Some(context), true, &mut handle)
        };
        if status != NO_ERROR {
            tracing::warn!("network change notifications unavailable ({status:?}); using timers");
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::Connectivity;
    use std::sync::Arc;
    use std::time::Duration;

    pub fn watch(conn: Arc<Connectivity>) {
        let spawned = std::thread::Builder::new().name("oxitide-netwatch".into()).spawn(move || loop {
            conn.set_online(super::has_default_route());
            std::thread::sleep(Duration::from_secs(2));
        });
        if let Err(e) = spawned {
            tracing::warn!("network watcher unavailable ({e}); using timers");
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    pub fn watch(_conn: std::sync::Arc<super::Connectivity>) {}
}

/// Whether the routing tables hold a default route; `None` when neither
/// table can be read.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn has_default_route() -> Option<bool> {
    let v4 = std::fs::read_to_string("/proc/net/route").ok();
    let v6 = std::fs::read_to_string("/proc/net/ipv6_route").ok();
    if v4.is_none() && v6.is_none() {
        return None;
    }
    Some(v4.as_deref().is_some_and(v4_has_default) || v6.as_deref().is_some_and(v6_has_default))
}

/// `/proc/net/route`: a row whose destination and mask are 0 (after the
/// header line). Columns: Iface Destination Gateway Flags ... Mask ...
fn v4_has_default(table: &str) -> bool {
    table.lines().skip(1).any(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();
        cols.len() > 7 && cols[1] == "00000000" && cols[7] == "00000000"
    })
}

/// `/proc/net/ipv6_route`: destination `::` with prefix length 0, through a
/// real interface (not the loopback `lo` reject route).
fn v6_has_default(table: &str) -> bool {
    table.lines().any(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();
        cols.len() >= 10
            && cols[0].chars().all(|c| c == '0')
            && cols[1] == "00"
            && cols[9] != "lo"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_wake_waiters_and_bump_the_epoch() {
        let conn = Connectivity::new();
        assert!(!conn.is_offline(), "unknown must not read as offline");
        let epoch = conn.epoch();
        conn.set_online(Some(false));
        assert!(conn.is_offline());
        assert_eq!(conn.epoch(), epoch + 1);
        conn.set_online(Some(false));
        assert_eq!(conn.epoch(), epoch + 1, "a repeated state is not a change");
    }

    #[test]
    fn parses_an_ipv4_default_route() {
        let table = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
                     eth0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
                     eth0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n";
        assert!(v4_has_default(table));
        let no_default = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
                          eth0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\n";
        assert!(!v4_has_default(no_default));
    }

    #[test]
    fn parses_an_ipv6_default_route_but_not_the_loopback_reject() {
        let default = "00000000000000000000000000000000 00 00000000000000000000000000000000 00 fe800000000000000000000000000001 00000400 00000001 00000000 00000003 eth0\n";
        let reject = "00000000000000000000000000000000 00 00000000000000000000000000000000 00 00000000000000000000000000000000 ffffffff 00000001 00000000 00200200 lo\n";
        assert!(v6_has_default(default));
        assert!(!v6_has_default(reject));
    }
}
