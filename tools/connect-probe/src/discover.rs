//! mDNS discovery of TIDAL Connect receivers (`_tidalconnect._tcp`).

use std::collections::BTreeSet;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent};

use crate::log::Log;

pub const SERVICE_TYPE: &str = "_tidalconnect._tcp.local.";

#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub txt: Vec<(String, String)>,
}

impl Device {
    pub fn label(&self) -> String {
        format!("\"{}\" at {}:{}", self.name, self.host, self.port)
    }
}

/// Local check of the discovery path: advertises a dummy `_tidalconnect._tcp`
/// service for a few seconds and browses for it. Not shown in the help.
pub async fn self_test(log: &Log) -> Result<()> {
    let daemon = ServiceDaemon::new()?;
    let host = "oxitide-probe-selftest.local.";
    let info = mdns_sd::ServiceInfo::new(
        SERVICE_TYPE,
        "OxiTide probe self-test",
        host,
        "",
        2019,
        &[
            ("fn", "OxiTide probe self-test"),
            ("id", "selftest"),
            ("mn", "selftest"),
        ][..],
    )?
    .enable_addr_auto();
    daemon.register(info)?;
    let found = discover_type(log, SERVICE_TYPE, 5, false).await?;
    let _ = daemon.unregister(&format!("OxiTide probe self-test.{SERVICE_TYPE}"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = daemon.shutdown();
    if found.iter().any(|d| d.name == "OxiTide probe self-test") {
        log.line("mDNS self-test OK: the advertised service was discovered.");
        Ok(())
    } else {
        anyhow::bail!("mDNS self-test failed: the advertised service was not discovered")
    }
}

/// After the first device resolves, wait this much longer for others.
const SETTLE: Duration = Duration::from_secs(3);

/// Browses for up to `seconds` (enough time to answer a first-run Windows
/// Firewall prompt), finishing [`SETTLE`] after the first device appears.
pub async fn discover(log: &Log, seconds: u64) -> Result<Vec<Device>> {
    discover_type(log, SERVICE_TYPE, seconds, true).await
}

pub async fn discover_type(
    log: &Log,
    service_type: &str,
    seconds: u64,
    settle: bool,
) -> Result<Vec<Device>> {
    log.line(format!(
        "Looking for devices ({service_type}) for up to {seconds} s. If Windows Firewall asks, \
         allow access on private networks."
    ));
    let daemon = ServiceDaemon::new()?;
    let rx = daemon.browse(service_type)?;
    let mut deadline = Instant::now() + Duration::from_secs(seconds);
    // One entry per service instance: a multi-homed host resolves once per
    // address set, which must not look like several devices.
    let mut services: Vec<(String, BTreeSet<Ipv4Addr>, Device)> = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let event = tokio::select! {
            r = tokio::time::timeout(left, rx.recv_async()) => r,
            _ = crate::interrupt::wait() => break,
        };
        let Ok(Ok(event)) = event else {
            break;
        };
        if let ServiceEvent::ServiceResolved(info) = event {
            // TIDAL's controller only uses IPv4 addresses.
            let addrs: BTreeSet<Ipv4Addr> = info.get_addresses_v4().into_iter().collect();
            if addrs.is_empty() {
                continue;
            }
            let txt: Vec<(String, String)> = info
                .get_properties()
                .iter()
                .map(|p| (p.key().to_string(), p.val_str().to_string()))
                .collect();
            let name = txt
                .iter()
                .find(|(k, _)| k == "fn")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| info.get_fullname().to_string());
            let fullname = info.get_fullname().to_string();
            match services.iter_mut().find(|(f, _, _)| *f == fullname) {
                Some((_, known, device)) => {
                    known.extend(addrs);
                    device.port = info.get_port();
                    device.txt = txt;
                }
                None => {
                    if services.is_empty() && settle {
                        deadline = deadline.min(Instant::now() + SETTLE);
                    }
                    services.push((
                        fullname,
                        addrs,
                        Device {
                            name,
                            host: String::new(),
                            port: info.get_port(),
                            txt,
                        },
                    ))
                }
            }
        }
    }
    let _ = daemon.stop_browse(service_type);
    let _ = daemon.shutdown();
    let mut devices = Vec::new();
    for (_, addrs, mut device) in services {
        let Some(best) = best_address(&addrs) else {
            continue;
        };
        device.host = best.to_string();
        let txt = device
            .txt
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        let others = addrs.len() - 1;
        let others = if others > 0 {
            format!(", {others} other address(es)")
        } else {
            String::new()
        };
        log.line(format!("  found {}{others} (TXT: {txt})", device.label()));
        devices.push(device);
    }
    log.line(format!("{} device(s) found.", devices.len()));
    Ok(devices)
}

/// The address to connect to: a private LAN address first (192.168/16, then
/// 10/8, then 172.16/12), then anything else that is not loopback or
/// link-local.
fn best_address(addrs: &BTreeSet<Ipv4Addr>) -> Option<Ipv4Addr> {
    let rank = |ip: &Ipv4Addr| -> u8 {
        let o = ip.octets();
        match o {
            _ if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() => 9,
            [192, 168, ..] => 0,
            [10, ..] => 1,
            [172, b, ..] if (16..32).contains(&b) => 2,
            _ => 3,
        }
    };
    addrs.iter().copied().min_by_key(|ip| (rank(ip), *ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lan_address_beats_virtual_and_loopback_ones() {
        let addrs: BTreeSet<Ipv4Addr> = ["127.0.0.1", "172.20.0.1", "192.168.0.20"]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(best_address(&addrs), Some("192.168.0.20".parse().unwrap()));
    }

    #[test]
    fn loopback_is_used_only_when_nothing_else_exists() {
        let addrs: BTreeSet<Ipv4Addr> = ["127.0.0.1".parse().unwrap()].into_iter().collect();
        assert_eq!(best_address(&addrs), Some("127.0.0.1".parse().unwrap()));
    }
}
