//! Platform-neutral diagnostic contract. OS integrations remain private to native.
use anyhow::{Result, bail};

#[derive(Debug, Clone, Default)]
pub struct ProxyEndpoint {
    pub host: String,
    pub port: u16,
}
#[derive(Debug, Clone, Default)]
pub struct SystemProxyDiagnostics {
    pub platform: String,
    pub status: String,
    pub error_code: Option<String>,
    pub enabled: Option<bool>,
    pub http: Option<ProxyEndpoint>,
    pub https: Option<ProxyEndpoint>,
    pub socks: Option<ProxyEndpoint>,
    pub pac_enabled: Option<bool>,
    pub pac_url: Option<String>,
    pub bypass: Vec<String>,
    pub windows: Option<WindowsProxyDetails>,
    pub linux: Option<LinuxProxyDetails>,
}
#[derive(Debug, Clone, Default)]
pub struct LinuxProxyDetails {
    pub desktop_environment: String,
    pub backend: String,
    pub mode: Option<String>,
    pub reversed_bypass: bool,
    pub environment: Vec<ProxyEnvironmentEntry>,
    pub portal: ProxyPortalState,
}
#[derive(Debug, Clone)]
pub struct ProxyEnvironmentEntry {
    pub name: String,
    pub endpoint: Option<ProxyEndpoint>,
    pub bypass: Vec<String>,
    pub valid: bool,
}
#[derive(Debug, Clone, Default)]
pub struct ProxyPortalState {
    pub status: String,
    pub direct: bool,
    pub proxies: Vec<ProxyEndpoint>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone)]
pub struct WindowsProxyDetails {
    pub proxy_server: Option<String>,
    pub proxy_override: Option<String>,
    pub auto_config_url: Option<String>,
    pub win_http: WinHttpProxyState,
    pub app_container: AppContainerState,
}
#[derive(Debug, Clone)]
pub struct WinHttpProxyState {
    pub status: String,
    pub mode: Option<String>,
    pub proxy: Option<String>,
    pub bypass: Option<String>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone)]
pub struct AppContainerState {
    pub supported: bool,
    pub status: String,
    pub loopback_exemption_count: Option<u32>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone)]
pub struct SystemProxySettings {
    pub mode: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub bypass: Vec<String>,
    pub pac_url: Option<String>,
}

pub fn get_system_proxy_diagnostics() -> SystemProxyDiagnostics {
    #[cfg(target_os = "windows")]
    {
        crate::windows::system_proxy::get_diagnostics()
    }
    #[cfg(target_os = "linux")]
    {
        crate::linux_system_proxy::get_diagnostics()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        SystemProxyDiagnostics {
            platform: std::env::consts::OS.replace("macos", "darwin"),
            status: "unsupported".into(),
            error_code: Some("unsupported-platform".into()),
            ..Default::default()
        }
    }
}

pub fn set_system_proxy(settings: &SystemProxySettings) -> Result<()> {
    validate_settings(settings)?;
    #[cfg(target_os = "windows")]
    {
        crate::windows::system_proxy::set_proxy(settings)
    }
    #[cfg(not(target_os = "windows"))]
    {
        bail!("unsupported-platform")
    }
}

fn validate_settings(settings: &SystemProxySettings) -> Result<()> {
    match settings.mode.as_str() {
        "manual" => {
            // This operation intentionally supports only KokoroBox's local endpoint.
            if !matches!(
                settings.host.as_deref(),
                Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
            ) || settings.port.unwrap_or(0) == 0
            {
                bail!("invalid-proxy-endpoint");
            }
        }
        "auto" => {
            let url = settings.pac_url.as_deref().unwrap_or("");
            let valid = url
                .strip_prefix("http://127.0.0.1:")
                .and_then(|v| v.strip_suffix("/pac"))
                .and_then(|p| p.parse::<u16>().ok())
                .is_some_and(|p| p != 0);
            if !valid {
                bail!("invalid-pac-url");
            }
        }
        "disabled" => (),
        _ => bail!("invalid-proxy-mode"),
    }
    if settings
        .bypass
        .iter()
        .any(|v| v.contains(['\0', '\r', '\n']))
    {
        bail!("invalid-proxy-bypass");
    }
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn parse_endpoint(value: &str) -> Option<ProxyEndpoint> {
    let (host, port) = value.trim().rsplit_once(':')?;
    if host.is_empty() || host.contains(['@', '/', ' ', '=']) {
        return None;
    }
    let port = port.parse::<u16>().ok().filter(|p| *p != 0)?;
    Some(ProxyEndpoint {
        host: host.trim_matches(['[', ']']).into(),
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_and_settings_validation() {
        assert_eq!(parse_endpoint("[::1]:18423").unwrap().port, 18423);
        assert!(parse_endpoint("user:secret@localhost:18423").is_none());
        let mut settings = SystemProxySettings {
            mode: "manual".into(),
            host: Some("127.0.0.1".into()),
            port: Some(18423),
            bypass: vec!["<local>".into()],
            pac_url: None,
        };
        assert!(validate_settings(&settings).is_ok());
        settings.port = Some(0);
        assert!(validate_settings(&settings).is_err());
        settings.mode = "auto".into();
        settings.pac_url = Some("https://private/token".into());
        assert!(validate_settings(&settings).is_err());
    }
    #[test]
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn unsupported_is_unknown_not_disabled() {
        let result = get_system_proxy_diagnostics();
        assert_eq!(result.status, "unsupported");
        assert_eq!(result.enabled, None);
    }
}
