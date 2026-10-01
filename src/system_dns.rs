//! Read-only user-process DNS inspection. Bootstrap repair belongs to Desktop configuration.
use anyhow::Result;
#[cfg(target_os = "linux")]
use anyhow::bail;
use std::{
    net::{IpAddr, ToSocketAddrs},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub const DNS_DIAGNOSTIC_DOMAINS: [&str; 2] = ["www.gstatic.com", "example.com"];

#[derive(Debug, Clone)]
pub struct DNSQuery {
    pub domain: String,
    pub outcome: String,
}
#[derive(Debug, Clone)]
pub struct SystemDNSDiagnostics {
    pub outcome: String,
    pub queries: Vec<DNSQuery>,
    pub interface: Option<String>,
    pub service: Option<String>,
    pub servers: Vec<String>,
}

#[cfg(target_os = "linux")]
use std::{
    io::Read,
    process::{Command, Stdio},
};

// Read utilities cannot block a diagnostic indefinitely or return unbounded output.
#[cfg(target_os = "linux")]
fn output(program: &str, args: &[&str], seconds: u64) -> Result<String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| anyhow::anyhow!("dns-backend-unavailable"))?;
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.by_ref().take(65537).read_to_end(&mut bytes);
        let _ = tx.send((result, bytes));
    });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    bail!("dns-operation-failed");
                }
                let (read, bytes) = rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|_| anyhow::anyhow!("dns-timeout"))?;
                if read.is_err() || bytes.len() > 65536 {
                    bail!("dns-response-invalid");
                }
                return String::from_utf8(bytes)
                    .map_err(|_| anyhow::anyhow!("dns-response-invalid"));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("dns-timeout");
            }
        }
    }
}

fn snapshot() -> Result<SystemDNSDiagnostics> {
    #[cfg(not(target_os = "linux"))]
    let context = crate::get_network_context()?;
    #[cfg(target_os = "linux")]
    let context = linux_context();
    let servers = context
        .dns_servers
        .into_iter()
        .filter(|v| v.parse::<IpAddr>().is_ok())
        .collect();
    Ok(SystemDNSDiagnostics {
        outcome: "unavailable".into(),
        queries: vec![],
        interface: context.default_interface,
        service: context.default_service,
        servers,
    })
}

pub fn get_system_dns_diagnostics() -> Result<SystemDNSDiagnostics> {
    let (tx, rx) = mpsc::channel();
    for domain in DNS_DIAGNOSTIC_DOMAINS {
        let tx = tx.clone();
        thread::spawn(move || {
            let success = (domain, 443)
                .to_socket_addrs()
                .is_ok_and(|mut addresses| addresses.next().is_some());
            let _ = tx.send(DNSQuery {
                domain: domain.into(),
                outcome: if success { "success" } else { "failed" }.into(),
            });
        });
    }
    // Share one deadline for both queries; a hung OS resolver is reported failed.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut result = snapshot()?;
    for _ in DNS_DIAGNOSTIC_DOMAINS {
        if let Ok(query) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            result.queries.push(query);
        }
    }
    for domain in DNS_DIAGNOSTIC_DOMAINS {
        if !result.queries.iter().any(|q| q.domain == domain) {
            result.queries.push(DNSQuery {
                domain: domain.into(),
                outcome: "failed".into(),
            });
        }
    }
    result.queries.sort_by(|a, b| a.domain.cmp(&b.domain));
    result.outcome = if result.queries.iter().all(|q| q.outcome == "success") {
        "success"
    } else {
        "failed"
    }
    .into();
    Ok(result)
}

#[cfg(target_os = "linux")]
fn linux_context() -> crate::NetworkContext {
    let interface = linux_default_interface();
    let servers = interface
        .as_deref()
        .and_then(|iface| {
            output(
                "/usr/bin/nmcli",
                &[
                    "--escape",
                    "no",
                    "-g",
                    "IP4.DNS,IP6.DNS",
                    "device",
                    "show",
                    iface,
                ],
                1,
            )
            .ok()
            .map(|s| {
                s.lines()
                    .filter(|v| v.parse::<IpAddr>().is_ok())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty())
            .or_else(|| {
                output("/usr/bin/resolvectl", &["dns", iface], 1)
                    .ok()
                    .map(|s| {
                        s.split_once(':')
                            .map(|(_, v)| {
                                v.split_whitespace()
                                    .filter(|v| v.parse::<IpAddr>().is_ok())
                                    .map(String::from)
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
            })
        })
        .unwrap_or_default();
    let servers = if servers.is_empty() {
        std::fs::read_to_string("/etc/resolv.conf")
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                (words.next() == Some("nameserver"))
                    .then(|| words.next())
                    .flatten()
                    .filter(|v| v.parse::<IpAddr>().is_ok())
                    .map(String::from)
            })
            .collect()
    } else {
        servers
    };
    crate::NetworkContext {
        online: interface.is_some(),
        default_interface: interface,
        default_service: None,
        dns_servers: servers,
        ssid: None,
    }
}

#[cfg(any(target_os = "linux", test))]
fn route_interface(v4: &str, v6: &str) -> Option<String> {
    let best4 = v4
        .lines()
        .skip(1)
        .filter_map(|line| {
            let v = line.split_whitespace().collect::<Vec<_>>();
            if v.len() < 8 || v[1] != "00000000" || u32::from_str_radix(v[3], 16).ok()? & 3 != 3 {
                return None;
            }
            Some((v[6].parse::<u32>().ok()?, v[0].to_owned()))
        })
        .min_by_key(|v| v.0);
    best4
        .or_else(|| {
            v6.lines()
                .filter_map(|line| {
                    let v = line.split_whitespace().collect::<Vec<_>>();
                    if v.len() != 10 || v[0] != "00000000000000000000000000000000" || v[1] != "00" {
                        return None;
                    }
                    let flags = u32::from_str_radix(v[8], 16).ok()?;
                    if flags & 1 == 0 || flags & 0x200 != 0 {
                        return None;
                    }
                    Some((u32::from_str_radix(v[5], 16).ok()?, v[9].to_owned()))
                })
                .min_by_key(|v| v.0)
        })
        .map(|v| v.1)
}
#[cfg(target_os = "linux")]
fn linux_default_interface() -> Option<String> {
    route_interface(
        &std::fs::read_to_string("/proc/net/route").unwrap_or_default(),
        &std::fs::read_to_string("/proc/net/ipv6_route").unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linux_routes_choose_lowest_metric_and_support_ipv6_only() {
        assert_eq!(
            route_interface(
                "Iface Destination Gateway Flags RefCnt Use Metric Mask\neth1 00000000 01000000 0003 0 0 200 00000000\neth0 00000000 01000000 0003 0 0 100 00000000",
                ""
            ),
            Some("eth0".into())
        );
        assert_eq!(
            route_interface(
                "",
                "00000000000000000000000000000000 00 00000000000000000000000000000000 00 fe800000000000000000000000000001 00000064 0 0 00000001 eth0"
            ),
            Some("eth0".into())
        );
        assert_eq!(route_interface("", ""), None);
    }
}
