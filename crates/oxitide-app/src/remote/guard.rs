//! Pure request guards for the phone remote (spec §6.1, §6.2): peer
//! admission, Host/Origin validation, cookie parsing, and the failure rate
//! limiter. Nothing here touches sockets, so every rule is unit-tested.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};

/// The device cookie name.
pub const COOKIE_NAME: &str = "oxi_remote";

/// RM-D3: loopback, RFC 1918, link-local, CGNAT `100.64/10` (Tailscale), and
/// IPv6 loopback/ULA/link-local. IPv4-mapped IPv6 peers are judged as IPv4.
pub fn is_lan_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_lan_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_lan_v4(v4),
            None => is_lan_v6(v6),
        },
    }
}

fn is_lan_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback() || ip.is_private() || ip.is_link_local() || (a == 100 && (64..128).contains(&b))
}

fn is_lan_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
}

/// Collapse an IPv4-mapped IPv6 address (a dual-stack socket's view of an
/// IPv4 peer) to plain IPv4.
pub fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)),
        v4 => v4,
    }
}

/// Split a `Host` header into host and optional port. IPv6 literals must be
/// bracketed (`[::1]:47820`).
fn split_host(host: &str) -> Option<(&str, Option<u16>)> {
    let host = host.trim();
    if let Some(rest) = host.strip_prefix('[') {
        let (inner, after) = rest.split_once(']')?;
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':')?.parse().ok()?),
        };
        return Some((inner, port));
    }
    match host.rsplit_once(':') {
        Some((name, port)) if !name.contains(':') => Some((name, Some(port.parse().ok()?))),
        Some(_) => None,
        None => Some((host, None)),
    }
}

/// DNS-rebinding defence: `Host` must be `localhost` or an IP literal equal to
/// the address the connection actually arrived on (which is by definition one
/// of this machine's), with the server's port when a port is given.
pub fn host_allowed(host: &str, local: IpAddr, port: u16) -> bool {
    let Some((name, host_port)) = split_host(host) else {
        return false;
    };
    if host_port.is_some_and(|p| p != port) {
        return false;
    }
    if name.eq_ignore_ascii_case("localhost") {
        return canonical(local).is_loopback();
    }
    match name.parse::<IpAddr>() {
        Ok(ip) => canonical(ip) == canonical(local),
        Err(_) => false,
    }
}

/// `Origin`, when present, must be exactly the `Host` origin over HTTP.
pub fn origin_allowed(origin: Option<&str>, host: &str) -> bool {
    match origin {
        None => true,
        Some(origin) => origin.trim() == format!("http://{}", host.trim()),
    }
}

/// The value of `name` in a `Cookie` header.
pub fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|pair| {
        let (key, value) = pair.trim().split_once('=')?;
        (key.trim() == name).then(|| value.trim()).filter(|v| !v.is_empty())
    })
}

/// Sliding-window limiter for failed pairings and cookie checks (spec §6.2):
/// `per_ip` failures per window per peer, `global` across all peers.
pub struct RateLimiter {
    window: Duration,
    per_ip: usize,
    global: usize,
    by_ip: HashMap<IpAddr, VecDeque<Instant>>,
    all: VecDeque<Instant>,
}

impl RateLimiter {
    pub fn new(window: Duration, per_ip: usize, global: usize) -> Self {
        Self { window, per_ip, global, by_ip: HashMap::new(), all: VecDeque::new() }
    }

    /// The spec default: 5 per minute per IP, 30 per minute overall.
    pub fn standard() -> Self {
        Self::new(Duration::from_secs(60), 5, 30)
    }

    fn prune(&mut self, now: Instant) {
        let window = self.window;
        let expired = |t: &Instant| now.saturating_duration_since(*t) >= window;
        while self.all.front().is_some_and(expired) {
            self.all.pop_front();
        }
        self.by_ip.retain(|_, hits| {
            while hits.front().is_some_and(expired) {
                hits.pop_front();
            }
            !hits.is_empty()
        });
    }

    /// `Err(retry_after)` while `ip` (or everyone) is locked out.
    pub fn check(&mut self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
        self.prune(now);
        let ip = canonical(ip);
        let retry = |oldest: Instant, window: Duration| {
            window.saturating_sub(now.saturating_duration_since(oldest)).max(Duration::from_secs(1))
        };
        if let Some(hits) = self.by_ip.get(&ip) {
            if hits.len() >= self.per_ip {
                return Err(retry(hits[hits.len() - self.per_ip], self.window));
            }
        }
        if self.all.len() >= self.global {
            return Err(retry(self.all[self.all.len() - self.global], self.window));
        }
        Ok(())
    }

    pub fn record_failure(&mut self, ip: IpAddr, now: Instant) {
        self.prune(now);
        self.by_ip.entry(canonical(ip)).or_default().push_back(now);
        self.all.push_back(now);
    }
}

/// Phone-supplied device label: control characters dropped, whitespace
/// collapsed, at most 40 characters; empty becomes "Phone".
pub fn sanitize_device_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(40)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "Phone".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn peer_admission_table() {
        for admitted in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.20",
            "169.254.10.10",
            "100.64.0.1",
            "100.127.255.254",
            "::1",
            "fd7a:115c:a1e0::1",
            "fc00::1",
            "fe80::1",
            "::ffff:192.168.1.20",
        ] {
            assert!(is_lan_peer(ip(admitted)), "{admitted} must be admitted");
        }
        for rejected in [
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "100.63.255.255",
            "1.1.1.1",
            "2001:4860:4860::8888",
            "::ffff:8.8.8.8",
            "0.0.0.0",
        ] {
            assert!(!is_lan_peer(ip(rejected)), "{rejected} must be rejected");
        }
    }

    #[test]
    fn host_must_match_the_arrival_address() {
        let local = ip("192.168.1.5");
        assert!(host_allowed("192.168.1.5:47820", local, 47820));
        assert!(host_allowed("192.168.1.5", local, 47820));
        assert!(!host_allowed("192.168.1.6:47820", local, 47820));
        assert!(!host_allowed("192.168.1.5:47821", local, 47820));
        // A dual-stack socket reports IPv4 arrivals as mapped IPv6.
        assert!(host_allowed("192.168.1.5:47820", ip("::ffff:192.168.1.5"), 47820));
        assert!(host_allowed("[fe80::1]:47820", ip("fe80::1"), 47820));
    }

    #[test]
    fn dns_rebinding_hosts_are_rejected() {
        let local = ip("192.168.1.5");
        assert!(!host_allowed("evil.example:47820", local, 47820));
        assert!(!host_allowed("192.168.1.5.nip.io:47820", local, 47820));
        assert!(!host_allowed("", local, 47820));
        assert!(!host_allowed("[::1", local, 47820));
        assert!(!host_allowed("localhost:47820", local, 47820), "localhost only over loopback");
        assert!(host_allowed("localhost:47820", ip("127.0.0.1"), 47820));
        assert!(host_allowed("LOCALHOST:47820", ip("::1"), 47820));
    }

    #[test]
    fn origin_must_equal_the_host_origin() {
        assert!(origin_allowed(None, "192.168.1.5:47820"));
        assert!(origin_allowed(Some("http://192.168.1.5:47820"), "192.168.1.5:47820"));
        assert!(!origin_allowed(Some("https://192.168.1.5:47820"), "192.168.1.5:47820"));
        assert!(!origin_allowed(Some("http://evil.example"), "192.168.1.5:47820"));
        assert!(!origin_allowed(Some("null"), "192.168.1.5:47820"));
    }

    #[test]
    fn cookie_parsing() {
        assert_eq!(cookie_value("oxi_remote=abc", COOKIE_NAME), Some("abc"));
        assert_eq!(cookie_value("a=1; oxi_remote=xyz; b=2", COOKIE_NAME), Some("xyz"));
        assert_eq!(cookie_value("a=1;oxi_remote=", COOKIE_NAME), None);
        assert_eq!(cookie_value("xoxi_remote=1", COOKIE_NAME), None);
        assert_eq!(cookie_value("", COOKIE_NAME), None);
    }

    #[test]
    fn limiter_locks_an_ip_after_five_failures() {
        let mut limiter = RateLimiter::standard();
        let t0 = Instant::now();
        let peer = ip("192.168.1.9");
        for i in 0..5 {
            assert!(limiter.check(peer, t0).is_ok(), "attempt {i}");
            limiter.record_failure(peer, t0 + Duration::from_secs(i));
        }
        let retry = limiter.check(peer, t0 + Duration::from_secs(10)).unwrap_err();
        assert_eq!(retry, Duration::from_secs(50));
        assert!(limiter.check(ip("192.168.1.10"), t0).is_ok(), "other peers are unaffected");
        assert!(limiter.check(peer, t0 + Duration::from_secs(61)).is_ok(), "window expires");
    }

    #[test]
    fn limiter_has_a_global_ceiling() {
        let mut limiter = RateLimiter::new(Duration::from_secs(60), 5, 30);
        let t0 = Instant::now();
        for i in 0..30u8 {
            limiter.record_failure(IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)), t0);
        }
        assert!(limiter.check(ip("10.0.1.1"), t0).is_err());
        assert!(limiter.check(ip("10.0.1.1"), t0 + Duration::from_secs(60)).is_ok());
    }

    #[test]
    fn mapped_and_plain_ipv4_share_a_bucket() {
        let mut limiter = RateLimiter::standard();
        let t0 = Instant::now();
        for _ in 0..5 {
            limiter.record_failure(ip("::ffff:192.168.1.9"), t0);
        }
        assert!(limiter.check(ip("192.168.1.9"), t0).is_err());
    }

    #[test]
    fn device_names_are_sanitized() {
        assert_eq!(sanitize_device_name("  Pixel\n8\u{0007} "), "Pixel 8");
        assert_eq!(sanitize_device_name(""), "Phone");
        assert_eq!(sanitize_device_name(&"x".repeat(100)).chars().count(), 40);
        assert_eq!(sanitize_device_name("<b>iPhone</b>"), "<b>iPhone</b>", "rendered as text only");
    }
}
