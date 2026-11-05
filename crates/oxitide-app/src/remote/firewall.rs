//! Linux host firewall detection for the phone remote's Settings guidance.
//!
//! A phone on the same Wi-Fi has to reach the remote's TCP port. An active
//! `ufw` (its default policy drops incoming connections) or `firewalld` blocks
//! it until the user allows the port, so Settings names the firewall, says
//! why, and gives the exact command. Nothing here needs root or spawns a
//! process: `ufw`'s state and rules are read from `/etc/ufw` when readable,
//! and `firewalld` is found by its process name.

use serde::Serialize;

/// A host firewall that may block the remote's port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FirewallDto {
    /// `ufw` or `firewalld`.
    pub name: String,
    /// Whether the rules allow the port: `None` when they cannot be read
    /// without root (`firewalld` always, `ufw` on some distributions).
    pub port_open: Option<bool>,
    /// The command that allows the port, for the user to run in a terminal.
    pub command: String,
}

/// Whether `ufw.conf` turns the firewall on (`ENABLED=yes`).
pub fn ufw_enabled(conf: &str) -> bool {
    conf.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("ENABLED="))
        .any(|value| value.trim().trim_matches('"').eq_ignore_ascii_case("yes"))
}

/// Whether ufw's `user.rules` allow incoming TCP on `port`. Reads the
/// `### tuple ### allow <proto> <ports> <dst> <dport-any> <src> … in` lines
/// ufw writes for each rule: `<proto>` is `tcp` or `any`, and `<ports>` a
/// port, a `lo:hi` range, or a comma list of either.
pub fn ufw_allows(rules: &str, port: u16) -> bool {
    rules.lines().any(|line| {
        let Some(tuple) = line.trim().strip_prefix("### tuple ###") else {
            return false;
        };
        let fields: Vec<&str> = tuple.split_whitespace().collect();
        let [action, proto, ports, ..] = fields.as_slice() else {
            return false;
        };
        let inbound = fields.contains(&"in");
        (*action == "allow" || *action == "limit")
            && (*proto == "tcp" || *proto == "any")
            && inbound
            && ports_match(ports, port)
    })
}

fn ports_match(spec: &str, port: u16) -> bool {
    spec.split(',').any(|part| match part.split_once(':') {
        Some((lo, hi)) => match (lo.parse::<u16>(), hi.parse::<u16>()) {
            (Ok(lo), Ok(hi)) => (lo..=hi).contains(&port),
            _ => false,
        },
        None => part.parse::<u16>().is_ok_and(|p| p == port),
    })
}

fn ufw_command(port: u16) -> String {
    format!("sudo ufw allow {port}/tcp")
}

fn firewalld_command(port: u16) -> String {
    format!("sudo firewall-cmd --permanent --add-port={port}/tcp && sudo firewall-cmd --reload")
}

/// The active host firewall, if any, and whether it allows `port`.
#[cfg(target_os = "linux")]
pub fn detect(port: u16) -> Option<FirewallDto> {
    let read = |path: &str| std::fs::read_to_string(path).ok();
    if read("/etc/ufw/ufw.conf").is_some_and(|conf| ufw_enabled(&conf)) {
        // Both address families must allow it; either file may be root-only.
        let open = match (read("/etc/ufw/user.rules"), read("/etc/ufw/user6.rules")) {
            (Some(v4), Some(v6)) => Some(ufw_allows(&v4, port) && ufw_allows(&v6, port)),
            (Some(v4), None) => Some(ufw_allows(&v4, port)),
            _ => None,
        };
        return Some(FirewallDto { name: "ufw".into(), port_open: open, command: ufw_command(port) });
    }
    if firewalld_running() {
        return Some(FirewallDto {
            name: "firewalld".into(),
            port_open: None,
            command: firewalld_command(port),
        });
    }
    None
}

#[cfg(not(target_os = "linux"))]
pub fn detect(_port: u16) -> Option<FirewallDto> {
    None
}

#[cfg(target_os = "linux")]
fn firewalld_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.file_name().to_str().is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit()))
            && std::fs::read_to_string(entry.path().join("comm"))
                .is_ok_and(|comm| comm.trim() == "firewalld")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ufw_conf_switch_is_read() {
        assert!(ufw_enabled("# comment\nENABLED=yes\nLOGLEVEL=low\n"));
        assert!(ufw_enabled("ENABLED=\"yes\""));
        assert!(!ufw_enabled("ENABLED=no\n"));
        assert!(!ufw_enabled("#ENABLED=yes\n"));
        assert!(!ufw_enabled(""));
    }

    const RULES: &str = "\
*filter
### RULES ###

### tuple ### allow udp 1714:1764 0.0.0.0/0 any 0.0.0.0/0 KDE%20Connect - in
-A ufw-user-input -p udp --dport 1714:1764 -j ACCEPT

### tuple ### allow tcp 3773 0.0.0.0/0 any 192.168.1.0/24 in comment=7433206c616e
### tuple ### allow tcp 47820 0.0.0.0/0 any 0.0.0.0/0 in
-A ufw-user-input -p tcp --dport 47820 -j ACCEPT
### tuple ### allow any 8000:8010,9000 0.0.0.0/0 any 0.0.0.0/0 in
### tuple ### deny tcp 5000 0.0.0.0/0 any 0.0.0.0/0 in
### tuple ### allow tcp 6000 0.0.0.0/0 any 0.0.0.0/0 out
### END RULES ###
";

    #[test]
    fn ufw_rules_allow_the_port_by_number_range_or_list() {
        assert!(ufw_allows(RULES, 47820));
        assert!(ufw_allows(RULES, 8005), "a range in an `any` rule");
        assert!(ufw_allows(RULES, 9000), "a comma list");
        assert!(!ufw_allows(RULES, 47821));
        assert!(!ufw_allows(RULES, 1714), "udp only");
        assert!(!ufw_allows(RULES, 5000), "deny");
        assert!(!ufw_allows(RULES, 6000), "outbound only");
        assert!(!ufw_allows("", 47820));
    }

    #[test]
    fn commands_name_the_port() {
        assert_eq!(ufw_command(47820), "sudo ufw allow 47820/tcp");
        assert!(firewalld_command(47821).contains("--add-port=47821/tcp"));
    }
}
