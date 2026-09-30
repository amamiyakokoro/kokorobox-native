//! Read-only Linux desktop proxy inspection. No shell, environment mutation,
//! network probes, or full configuration/credential reads.
use crate::system_proxy::{
    LinuxProxyDetails, ProxyEndpoint, ProxyEnvironmentEntry, SystemProxyDiagnostics,
};
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use crate::system_proxy::ProxyPortalState;

#[cfg(target_os = "linux")]
const BUDGET: Duration = Duration::from_millis(1500);
const MAX_OUTPUT: u64 = 65536;
const PROXY_KEYS: [&str; 8] = [
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
];

// All endpoints sent across IPC are stripped of URL userinfo, path and query.
fn endpoint(value: &str) -> Option<ProxyEndpoint> {
    let authority = value
        .trim()
        .split_once("://")
        .map_or(value.trim(), |(_, s)| s);
    let authority = authority
        .split(['/', '?', '#'])
        .next()?
        .rsplit('@')
        .next()?;
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        (host, port.parse::<u16>().ok()?)
    } else {
        let scheme = value.trim().split_once("://")?.0;
        (
            authority,
            match scheme {
                "http" => 80,
                "https" => 443,
                "socks" | "socks4" | "socks5" | "socks5h" => 1080,
                _ => return None,
            },
        )
    };
    let host = host.trim_matches(['[', ']']);
    if port == 0 {
        return None;
    }
    if host.is_empty() || host.contains([' ', '\r', '\n', '\0', '=']) {
        return None;
    }
    Some(ProxyEndpoint {
        host: host.into(),
        port,
    })
}
// KIO writes both URI authority ports and the legacy "scheme://host port" format.
fn kde_endpoint(value: &str) -> Option<ProxyEndpoint> {
    if let Some((host, port)) = value.trim().rsplit_once(' ')
        && !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
    {
        return endpoint(&format!("{host}:{port}"));
    }
    endpoint(value)
}
fn environment(get: impl Fn(&str) -> Option<String>) -> Vec<ProxyEnvironmentEntry> {
    PROXY_KEYS
        .iter()
        .filter_map(|key| {
            let value = get(key)?.trim().to_owned();
            if value.is_empty() {
                return None;
            }
            let bypass = key.eq_ignore_ascii_case("no_proxy");
            Some(ProxyEnvironmentEntry {
                name: (*key).into(),
                endpoint: if bypass { None } else { endpoint(&value) },
                bypass: if bypass {
                    value
                        .split(',')
                        .map(|s| s.trim().into())
                        .filter(|s: &String| !s.is_empty())
                        .collect()
                } else {
                    vec![]
                },
                // A configured but invalid URI is distinct from a missing variable.
                valid: bypass || endpoint(&value).is_some(),
            })
        })
        .collect()
}
fn desktop_name(current: &str, session: &str) -> String {
    let name = if current.is_empty() { session } else { current };
    let lower = name.to_ascii_lowercase();
    if matches!(lower.as_str(), "plasmawayland" | "plasmax11") {
        return "KDE Plasma".into();
    }
    // Do not expose arbitrary environment content as a desktop identifier.
    for (needle, display) in [
        ("kde", "KDE Plasma"),
        ("plasma", "KDE Plasma"),
        ("gnome", "GNOME"),
        ("unity", "Unity"),
        ("cinnamon", "Cinnamon"),
        ("xfce", "XFCE"),
        ("lxqt", "LXQt"),
        ("hyprland", "Hyprland"),
        ("sway", "sway"),
        ("i3", "i3"),
        ("mate", "MATE"),
    ] {
        if lower.split([':', ';', '-', '_']).any(|s| s == needle) {
            return display.into();
        }
    }
    "Unknown".into()
}

// Fixed utilities/arguments only. Kill and reap timed-out children; never surface stderr.
fn command(program: &str, args: &[&str], deadline: Instant) -> Result<String, &'static str> {
    if Instant::now() >= deadline {
        return Err("timeout");
    }
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => "backend-unavailable",
            std::io::ErrorKind::PermissionDenied => "backend-permission-denied",
            _ => "backend-read-failed",
        })?;
    let stdout = child.stdout.take().ok_or("backend-read-failed")?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(MAX_OUTPUT + 1).read_to_end(&mut bytes);
        let _ = tx.send((result, bytes));
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err("backend-read-failed");
                }
                let (read, bytes) = rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|_| "timeout")?;
                if read.is_err() || bytes.len() as u64 > MAX_OUTPUT {
                    return Err("backend-read-failed");
                }
                return String::from_utf8(bytes)
                    .map(|s| s.trim().to_owned())
                    .map_err(|_| "backend-read-failed");
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("timeout");
            }
        }
    }
}

// Only parse string/string-array GVariants from the explicitly requested keys.
fn variant_strings(value: &str) -> Option<Vec<String>> {
    let mut chars = value
        .trim()
        .strip_prefix("@as ")
        .unwrap_or(value.trim())
        .chars()
        .peekable();
    let array = chars.peek() == Some(&'[');
    if array {
        chars.next();
    }
    let mut out = vec![];
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        if array && chars.peek() == Some(&']') {
            chars.next();
            break;
        }
        if !array && !out.is_empty() {
            break;
        }
        let quote = chars.next()?;
        if !matches!(quote, '\'' | '"') {
            return None;
        }
        let mut s = String::new();
        loop {
            let c = chars.next()?;
            if c == quote {
                break;
            }
            if c == '\\' {
                s.push(match chars.next()? {
                    '\\' => '\\',
                    '\'' => '\'',
                    '"' => '"',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => return None,
                });
            } else {
                s.push(c);
            }
        }
        out.push(s);
    }
    if chars.any(|c| !c.is_whitespace()) {
        return None;
    }
    Some(out)
}
fn string(value: &str) -> Option<String> {
    let v = variant_strings(value)?;
    if v.len() == 1 {
        v.into_iter().next()
    } else {
        None
    }
}
fn gnome(
    read: impl Fn(&str, &str) -> Result<String, &'static str> + Sync,
) -> Result<SystemProxyDiagnostics, &'static str> {
    let keys = [
        ("org.gnome.system.proxy", "mode"),
        ("org.gnome.system.proxy", "autoconfig-url"),
        ("org.gnome.system.proxy", "ignore-hosts"),
        ("org.gnome.system.proxy.http", "host"),
        ("org.gnome.system.proxy.http", "port"),
        ("org.gnome.system.proxy.https", "host"),
        ("org.gnome.system.proxy.https", "port"),
        ("org.gnome.system.proxy.socks", "host"),
        ("org.gnome.system.proxy.socks", "port"),
    ];
    let values = thread::scope(|scope| {
        let jobs: Vec<_> = keys
            .iter()
            .map(|(s, k)| scope.spawn(|| read(s, k)))
            .collect();
        jobs.into_iter()
            .map(|j| j.join().unwrap_or(Err("backend-read-failed")))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let mode = string(&values[0]).ok_or("backend-read-failed")?;
    if !matches!(mode.as_str(), "none" | "manual" | "auto") {
        return Err("backend-read-failed");
    }
    let proxy = |i: usize| -> Result<Option<ProxyEndpoint>, &'static str> {
        let host = string(&values[i]).ok_or("backend-read-failed")?;
        let port = values[i + 1]
            .parse::<u16>()
            .map_err(|_| "backend-read-failed")?;
        Ok(if host.is_empty() || port == 0 {
            None
        } else {
            Some(ProxyEndpoint { host, port })
        })
    };
    let pac_url = string(&values[1]).ok_or("backend-read-failed")?;
    Ok(SystemProxyDiagnostics {
        platform: "linux".into(),
        status: "available".into(),
        enabled: Some(mode != "none"),
        http: proxy(3)?,
        https: proxy(5)?,
        socks: proxy(7)?,
        pac_enabled: Some(mode == "auto"),
        pac_url: (!pac_url.is_empty()).then_some(pac_url),
        bypass: variant_strings(&values[2]).ok_or("backend-read-failed")?,
        linux: Some(LinuxProxyDetails {
            backend: "gnome".into(),
            mode: Some(mode),
            ..Default::default()
        }),
        ..Default::default()
    })
}
fn kde_with_reader(
    read: impl Fn(&str, &str, &str) -> Result<String, &'static str> + Sync,
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<SystemProxyDiagnostics, &'static str> {
    for tool in ["kreadconfig6", "kreadconfig5"] {
        // kreadconfig supports configuration queries, but has no --version option.
        // Read the actual mode to select the tool and reuse it in the snapshot.
        match read(tool, "ProxyType", "0") {
            Ok(mode) => {
                return kde(
                    |key, default| {
                        if key == "ProxyType" {
                            Ok(mode.clone())
                        } else {
                            read(tool, key, default)
                        }
                    },
                    get_env,
                );
            }
            Err("backend-unavailable") => continue,
            Err(code) => return Err(code),
        }
    }
    Err("backend-unavailable")
}

fn kde(
    read: impl Fn(&str, &str) -> Result<String, &'static str> + Sync,
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<SystemProxyDiagnostics, &'static str> {
    let keys = [
        ("ProxyType", "0"),
        ("httpProxy", ""),
        ("httpsProxy", ""),
        ("socksProxy", ""),
        ("NoProxyFor", ""),
        ("Proxy Config Script", ""),
        ("ReversedException", "false"),
    ];
    let v = thread::scope(|scope| {
        let jobs: Vec<_> = keys
            .iter()
            .map(|(k, d)| scope.spawn(|| read(k, d)))
            .collect();
        jobs.into_iter()
            .map(|j| j.join().unwrap_or(Err("backend-read-failed")))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let mode = match v[0].as_str() {
        "0" => "none",
        "1" => "manual",
        "2" => "auto",
        "3" => "wpad",
        "4" => "environment",
        _ => return Err("backend-read-failed"),
    };
    // KIO stores the variable *names* in EnvVarProxy mode, not proxy URIs.
    let value = |i: usize| {
        if mode == "environment" {
            get_env(&v[i]).unwrap_or_default()
        } else {
            v[i].clone()
        }
    };
    let reversed = match v[6].as_str() {
        "true" => true,
        "false" => false,
        _ => return Err("backend-read-failed"),
    };
    Ok(SystemProxyDiagnostics {
        platform: "linux".into(),
        status: "available".into(),
        enabled: Some(mode != "none"),
        http: kde_endpoint(&value(1)),
        https: kde_endpoint(&value(2)),
        socks: kde_endpoint(&value(3)),
        pac_enabled: Some(matches!(mode, "auto" | "wpad")),
        pac_url: (!v[5].is_empty()).then(|| v[5].clone()),
        bypass: value(4)
            .split(',')
            .map(|s| s.trim().into())
            .filter(|s: &String| !s.is_empty())
            .collect(),
        linux: Some(LinuxProxyDetails {
            backend: "kde".into(),
            mode: Some(mode.into()),
            reversed_bypass: reversed,
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[cfg(target_os = "linux")]
fn portal() -> ProxyPortalState {
    use zbus::blocking::{Proxy, connection::Builder};
    let result = (|| -> zbus::Result<Vec<String>> {
        let connection = Builder::session()?
            .method_timeout(Duration::from_millis(1000))
            .build()?;
        let proxy = Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.ProxyResolver",
        )?;
        // Lookup is a configuration query, not another connectivity probe.
        proxy.call("Lookup", &("https://www.gstatic.com/generate_204",))
    })();
    match result {
        Ok(values) => ProxyPortalState {
            status: "available".into(),
            direct: values.iter().any(|s| s == "direct://"),
            proxies: values.iter().filter_map(|s| endpoint(s)).collect(),
            error_code: None,
        },
        Err(_) => ProxyPortalState {
            status: "unavailable".into(),
            error_code: Some("portal-unavailable".into()),
            ..Default::default()
        },
    }
}

#[cfg(target_os = "linux")]
pub fn get_diagnostics() -> SystemProxyDiagnostics {
    let env = |key: &str| std::env::var(key).ok();
    let desktop = desktop_name(
        &env("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        &env("DESKTOP_SESSION").unwrap_or_default(),
    );
    let entries = environment(env);
    let fallback = if entries
        .iter()
        .any(|v| !v.name.eq_ignore_ascii_case("no_proxy"))
    {
        "environment"
    } else {
        "unsupported"
    };
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(portal());
    });
    let deadline = Instant::now() + BUDGET;
    let result = if matches!(desktop.as_str(), "GNOME" | "Unity" | "Cinnamon") {
        gnome(|s, k| command("gsettings", &["get", s, k], deadline))
    } else if desktop == "KDE Plasma" {
        // A working KDE configuration tool confirms availability and honors KConfig
        // defaults, cascaded XDG locations and immutable entries; no home-file parser.
        kde_with_reader(
            |tool, key, default| {
                command(
                    tool,
                    &[
                        "--file",
                        "kioslaverc",
                        "--group",
                        "Proxy Settings",
                        "--key",
                        key,
                        "--default",
                        default,
                    ],
                    deadline,
                )
            },
            env,
        )
    } else {
        Ok(SystemProxyDiagnostics {
            platform: "linux".into(),
            status: "unsupported".into(),
            linux: Some(LinuxProxyDetails {
                backend: fallback.into(),
                ..Default::default()
            }),
            ..Default::default()
        })
    };
    let mut state = result.unwrap_or_else(|code| SystemProxyDiagnostics {
        platform: "linux".into(),
        status: "unavailable".into(),
        error_code: Some(code.into()),
        linux: Some(LinuxProxyDetails {
            backend: if desktop == "KDE Plasma" {
                "kde"
            } else {
                "gnome"
            }
            .into(),
            ..Default::default()
        }),
        ..Default::default()
    });
    let details = state.linux.as_mut().expect("Linux diagnostic details");
    details.desktop_environment = desktop;
    details.environment = entries;
    details.portal = rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_else(|_| ProxyPortalState {
            status: "unavailable".into(),
            error_code: Some("timeout".into()),
            ..Default::default()
        });
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn environment_is_process_scoped_and_strips_credentials() {
        let env = BTreeMap::from([
            (
                "HTTPS_PROXY",
                "http://user:SECRET@127.0.0.1:18423/private?token=SECRET",
            ),
            ("https_proxy", "http://localhost:19351"),
            ("no_proxy", "localhost,127.0.0.1"),
            ("ALL_PROXY", "bad-SECRET"),
        ]);
        let entries = environment(|k| env.get(k).map(|s| (*s).into()));
        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries
                .iter()
                .find(|v| v.name == "HTTPS_PROXY")
                .unwrap()
                .endpoint
                .as_ref()
                .unwrap()
                .port,
            18423
        );
        assert!(!format!("{entries:?}").contains("SECRET"));
        assert!(
            !entries
                .iter()
                .find(|v| v.name == "ALL_PROXY")
                .unwrap()
                .valid
        );
    }
    #[test]
    fn desktop_detection_does_not_assume_installed_gnome_is_active() {
        assert_eq!(desktop_name("KDE", ""), "KDE Plasma");
        assert_eq!(desktop_name("ubuntu:GNOME", ""), "GNOME");
        assert_eq!(desktop_name("", "plasmawayland"), "KDE Plasma");
        assert_eq!(desktop_name("Hyprland", "gnome"), "Hyprland");
        assert_eq!(desktop_name("secret", ""), "Unknown");
    }
    #[test]
    fn gnome_modes_ports_and_bypass_are_typed() {
        for mode in ["none", "manual", "auto"] {
            let result = gnome(|s, k| {
                Ok(match k {
                    "mode" => format!("'{mode}'"),
                    "autoconfig-url" => "'https://private/PAC?secret'".into(),
                    "ignore-hosts" => "['localhost', '127.0.0.0/8']".into(),
                    "host" => "'127.0.0.1'".into(),
                    "port" if s.ends_with("https") => "19351".into(),
                    "port" => "18423".into(),
                    _ => unreachable!(),
                })
            })
            .unwrap();
            assert_eq!(result.enabled, Some(mode != "none"));
            assert_eq!(result.http.unwrap().port, 18423);
            assert_eq!(result.https.unwrap().port, 19351);
            assert_eq!(result.bypass.len(), 2);
        }
        assert_eq!(
            variant_strings("['a\\'b', \"c\\\\d\"]").unwrap(),
            vec!["a'b", "c\\d"]
        );
        assert!(variant_strings("[] garbage").is_none());
        assert!(variant_strings("@as []").unwrap().is_empty());
        assert!(gnome(|_, _| Err("timeout")).is_err());
    }
    #[test]
    fn kde_modes_environment_and_reverse_exceptions() {
        assert_eq!(kde_endpoint("http://127.0.0.1 18423").unwrap().port, 18423);
        assert_eq!(kde_endpoint("socks://[::1] 19351").unwrap().host, "::1");
        assert_eq!(
            endpoint("http://user:SECRET@proxy.example").unwrap().port,
            80
        );
        assert!(endpoint("http://proxy.example:invalid").is_none());
        for (number, mode) in [
            ("0", "none"),
            ("1", "manual"),
            ("2", "auto"),
            ("3", "wpad"),
            ("4", "environment"),
        ] {
            let result = kde(
                |k, d| {
                    Ok(match k {
                        "ProxyType" => number,
                        "httpProxy" => {
                            if number == "4" {
                                "HTTP_PROXY"
                            } else {
                                "http://127.0.0.1:18423"
                            }
                        }
                        "httpsProxy" => "http://127.0.0.1:18423",
                        "ReversedException" => "true",
                        _ => d,
                    }
                    .into())
                },
                |key| (key == "HTTP_PROXY").then(|| "http://user:secret@127.0.0.1:19351".into()),
            )
            .unwrap();
            assert_eq!(result.linux.as_ref().unwrap().mode.as_deref(), Some(mode));
            assert!(result.linux.unwrap().reversed_bypass);
            assert_eq!(
                result.http.unwrap().port,
                if number == "4" { 19351 } else { 18423 }
            );
        }
    }
    #[test]
    fn kde_reader_selection_uses_a_real_mode_query_and_reuses_it() {
        let calls = std::sync::Mutex::new(Vec::new());
        let result = kde_with_reader(
            |tool, key, default| {
                calls
                    .lock()
                    .unwrap()
                    .push((tool.to_owned(), key.to_owned()));
                assert_eq!(tool, "kreadconfig6");
                Ok(match key {
                    "ProxyType" => "1",
                    "httpProxy" | "httpsProxy" => "http://127.0.0.1 18423",
                    _ => default,
                }
                .into())
            },
            |_| None,
        )
        .unwrap();
        assert_eq!(result.status, "available");
        assert_eq!(result.http.unwrap().port, 18423);
        assert_eq!(result.https.unwrap().port, 18423);
        let calls = calls.into_inner().unwrap();
        assert_eq!(calls.len(), 7);
        assert_eq!(calls[0], ("kreadconfig6".into(), "ProxyType".into()));
        assert_eq!(
            calls.iter().filter(|(_, key)| key == "ProxyType").count(),
            1
        );
    }
    #[test]
    fn kde_reader_selection_falls_back_when_kde6_is_missing() {
        let result = kde_with_reader(
            |tool, key, default| {
                if tool == "kreadconfig6" {
                    assert_eq!(key, "ProxyType");
                    return Err("backend-unavailable");
                }
                assert_eq!(tool, "kreadconfig5");
                Ok(default.into())
            },
            |_| None,
        )
        .unwrap();
        assert_eq!(result.enabled, Some(false));
        assert_eq!(result.linux.unwrap().mode.as_deref(), Some("none"));
        assert_eq!(
            kde_with_reader(|_, _, _| Err("backend-unavailable"), |_| None).unwrap_err(),
            "backend-unavailable"
        );
    }
    #[test]
    fn kde_reader_selection_preserves_query_failures() {
        for code in [
            "timeout",
            "backend-permission-denied",
            "backend-read-failed",
        ] {
            let result = kde_with_reader(
                |tool, key, _| {
                    assert_eq!(tool, "kreadconfig6");
                    assert_eq!(key, "ProxyType");
                    Err(code)
                },
                |_| None,
            );
            assert_eq!(result.unwrap_err(), code);
        }
        assert_eq!(
            kde_with_reader(|_, _, _| Ok("invalid".into()), |_| None).unwrap_err(),
            "backend-read-failed"
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn subprocess_start_errors_distinguish_missing_and_permission_denied() {
        let deadline = Instant::now() + Duration::from_secs(1);
        assert_eq!(
            command("/kokorobox-nonexistent-config-reader", &[], deadline),
            Err("backend-unavailable")
        );
        assert_eq!(
            command("/", &[], deadline),
            Err("backend-permission-denied")
        );
    }
    #[test]
    fn subprocess_timeout_is_bounded() {
        let started = Instant::now();
        assert_eq!(
            command("sleep", &["2"], started + Duration::from_millis(30)),
            Err("timeout")
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
