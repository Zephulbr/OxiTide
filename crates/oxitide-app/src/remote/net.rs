//! Which address goes into the pairing QR (spec §6.1): the interface with the
//! default route, preferring home-network ranges and skipping virtual
//! adapters (WSL/Hyper-V `vEthernet`, VirtualBox, Docker, …). Settings can
//! override the pick from the detected list.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use serde::Serialize;

use super::guard::is_lan_peer;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InterfaceDto {
    pub name: String,
    pub ip: String,
    /// A virtual/host-only adapter, listed last and never auto-picked first.
    pub virtual_adapter: bool,
}

/// Adapter names that are almost never the phone's network.
pub fn is_virtual(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "vethernet", "wsl", "hyper-v", "virtualbox", "vmware", "vmnet", "docker", "veth", "br-",
        "virbr", "loopback", "zerotier", "npcap",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

fn range_rank(ip: Ipv4Addr) -> u8 {
    match ip.octets() {
        [192, 168, ..] => 0,
        [10, ..] => 1,
        [172, b, ..] if (16..32).contains(&b) => 2,
        [100, b, ..] if (64..128).contains(&b) => 3,
        _ => 4,
    }
}

/// Up, non-loopback, LAN-range IPv4 interfaces in preference order.
pub fn list_interfaces() -> Vec<InterfaceDto> {
    let mut found: Vec<(InterfaceDto, Ipv4Addr)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|iface| iface.is_oper_up() && !iface.is_loopback())
        .filter_map(|iface| match iface.ip() {
            IpAddr::V4(ip) if is_lan_peer(IpAddr::V4(ip)) => Some((
                InterfaceDto {
                    virtual_adapter: is_virtual(&iface.name),
                    name: iface.name,
                    ip: ip.to_string(),
                },
                ip,
            )),
            _ => None,
        })
        .collect();
    found.sort_by_key(|(dto, ip)| (dto.virtual_adapter, range_rank(*ip)));
    found.dedup_by(|a, b| a.0.ip == b.0.ip);
    found.into_iter().map(|(dto, _)| dto).collect()
}

/// The source address the OS would use for the internet: a UDP `connect`
/// selects a route without sending a packet.
pub fn default_route_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}

/// The address for the QR URL: an explicit `preferred` match, else the
/// default-route address when it is a non-virtual candidate, else the first
/// candidate (`interfaces` is already in preference order), else loopback.
pub fn pick(interfaces: &[InterfaceDto], preferred: &str, default_route: Option<IpAddr>) -> IpAddr {
    let preferred = preferred.trim();
    if !preferred.is_empty() {
        if let Some(ip) = interfaces
            .iter()
            .find(|i| i.ip == preferred)
            .and_then(|i| i.ip.parse().ok())
        {
            return ip;
        }
    }
    if let Some(route) = default_route {
        let route = route.to_string();
        if let Some(ip) = interfaces
            .iter()
            .find(|i| i.ip == route && !i.virtual_adapter)
            .and_then(|i| i.ip.parse().ok())
        {
            return ip;
        }
    }
    interfaces
        .first()
        .and_then(|i| i.ip.parse().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(name: &str, ip: &str) -> InterfaceDto {
        InterfaceDto { name: name.into(), ip: ip.into(), virtual_adapter: is_virtual(name) }
    }

    #[test]
    fn virtual_adapters_are_recognised() {
        assert!(is_virtual("vEthernet (WSL (Hyper-V firewall))"));
        assert!(is_virtual("VirtualBox Host-Only Network"));
        assert!(is_virtual("docker0"));
        assert!(!is_virtual("Wi-Fi"));
        assert!(!is_virtual("Ethernet 2"));
        assert!(!is_virtual("wlan0"));
    }

    #[test]
    fn explicit_choice_wins() {
        let list = [iface("Wi-Fi", "192.168.1.5"), iface("Tailscale", "100.70.1.2")];
        assert_eq!(pick(&list, "100.70.1.2", None).to_string(), "100.70.1.2");
        assert_eq!(pick(&list, "10.9.9.9", None).to_string(), "192.168.1.5", "stale override");
    }

    #[test]
    fn default_route_wins_unless_virtual() {
        let list = [iface("Wi-Fi", "192.168.1.5"), iface("Ethernet", "10.0.0.4")];
        assert_eq!(pick(&list, "", Some("10.0.0.4".parse().unwrap())).to_string(), "10.0.0.4");
        let list = [iface("Wi-Fi", "192.168.1.5"), iface("vEthernet (WSL)", "172.20.0.1")];
        assert_eq!(
            pick(&list, "", Some("172.20.0.1".parse().unwrap())).to_string(),
            "192.168.1.5"
        );
    }

    #[test]
    fn falls_back_to_first_candidate_or_loopback() {
        let list = [iface("Wi-Fi", "192.168.1.5")];
        assert_eq!(pick(&list, "", None).to_string(), "192.168.1.5");
        assert_eq!(pick(&[], "", None).to_string(), "127.0.0.1");
    }

    #[test]
    fn ranges_rank_home_networks_first() {
        assert!(range_rank("192.168.0.2".parse().unwrap()) < range_rank("10.0.0.2".parse().unwrap()));
        assert!(range_rank("10.0.0.2".parse().unwrap()) < range_rank("172.16.0.2".parse().unwrap()));
    }
}
