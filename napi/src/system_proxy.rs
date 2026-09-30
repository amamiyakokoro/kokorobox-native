use crate::error::map_err;
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi(object)]
pub struct JsProxyEndpoint {
    pub host: String,
    pub port: u32,
}
#[napi(object)]
pub struct JsProxyProtocols {
    pub http: Option<JsProxyEndpoint>,
    pub https: Option<JsProxyEndpoint>,
    pub socks: Option<JsProxyEndpoint>,
}
#[napi(object)]
pub struct JsPacState {
    pub enabled: bool,
    pub url: Option<String>,
}
#[napi(object)]
pub struct JsWinHttpProxyState {
    pub status: String,
    pub mode: Option<String>,
    pub proxy: Option<String>,
    pub bypass: Option<String>,
    pub error_code: Option<String>,
}
#[napi(object)]
pub struct JsAppContainerState {
    pub supported: bool,
    pub status: String,
    pub loopback_exemption_count: Option<u32>,
    pub error_code: Option<String>,
}
#[napi(object)]
pub struct JsWindowsProxyDetails {
    pub proxy_server: Option<String>,
    pub proxy_override: Option<String>,
    pub auto_config_url: Option<String>,
    pub win_http: JsWinHttpProxyState,
    pub app_container: JsAppContainerState,
}
#[napi(object)]
pub struct JsProxyEnvironmentEntry {
    pub name: String,
    pub endpoint: Option<JsProxyEndpoint>,
    pub bypass: Vec<String>,
    pub valid: bool,
}
#[napi(object)]
pub struct JsProxyPortalState {
    pub status: String,
    pub direct: bool,
    pub proxies: Vec<JsProxyEndpoint>,
    pub error_code: Option<String>,
}
#[napi(object)]
pub struct JsLinuxProxyDetails {
    pub desktop_environment: String,
    pub backend: String,
    pub mode: Option<String>,
    pub reversed_bypass: bool,
    pub environment: Vec<JsProxyEnvironmentEntry>,
    pub portal: JsProxyPortalState,
}
#[napi(object)]
pub struct JsMacProxyProtocol {
    pub enabled: bool,
    pub endpoint: Option<JsProxyEndpoint>,
}
#[napi(object)]
pub struct JsMacProxyState {
    pub http: JsMacProxyProtocol,
    pub https: JsMacProxyProtocol,
    pub socks: JsMacProxyProtocol,
    pub pac_enabled: bool,
    pub pac_url: Option<String>,
    pub auto_discovery: bool,
    pub bypass: Vec<String>,
    pub exclude_simple_hostnames: bool,
}
#[napi(object)]
pub struct JsMacNetworkService {
    pub id: String,
    pub name: String,
    pub interface: Option<String>,
    pub enabled: bool,
    pub active: bool,
    pub primary: bool,
    pub status: String,
    pub proxies: Option<JsMacProxyState>,
}
#[napi(object)]
pub struct JsMacOSProxyDetails {
    pub effective: Option<JsMacProxyState>,
    pub active_service_ids: Vec<String>,
    pub services: Vec<JsMacNetworkService>,
    pub network_location: Option<String>,
    pub location_error_code: Option<String>,
    pub service_error_code: Option<String>,
}
#[napi(object)]
pub struct JsSystemProxyMutation {
    pub automatic_settings_preserved: bool,
}
impl From<kokorobox_native::MacProxyProtocol> for JsMacProxyProtocol {
    fn from(v: kokorobox_native::MacProxyProtocol) -> Self {
        Self {
            enabled: v.enabled,
            endpoint: v.endpoint.map(|e| JsProxyEndpoint {
                host: e.host,
                port: e.port.into(),
            }),
        }
    }
}
impl From<kokorobox_native::MacProxyState> for JsMacProxyState {
    fn from(v: kokorobox_native::MacProxyState) -> Self {
        Self {
            http: v.http.into(),
            https: v.https.into(),
            socks: v.socks.into(),
            pac_enabled: v.pac_enabled,
            pac_url: v.pac_url,
            auto_discovery: v.auto_discovery,
            bypass: v.bypass,
            exclude_simple_hostnames: v.exclude_simple_hostnames,
        }
    }
}
#[napi(object)]
pub struct JsSystemProxyDiagnostics {
    pub platform: String,
    pub status: String,
    pub error_code: Option<String>,
    pub enabled: Option<bool>,
    pub proxies: JsProxyProtocols,
    pub pac: Option<JsPacState>,
    pub bypass: Vec<String>,
    pub windows: Option<JsWindowsProxyDetails>,
    pub linux: Option<JsLinuxProxyDetails>,
    pub macos: Option<JsMacOSProxyDetails>,
}
#[napi(object)]
pub struct JsSystemProxySettings {
    pub mode: String,
    pub host: Option<String>,
    pub port: Option<u32>,
    pub bypass: Vec<String>,
    pub pac_url: Option<String>,
    pub only_active_device: Option<bool>,
}

impl From<kokorobox_native::SystemProxyDiagnostics> for JsSystemProxyDiagnostics {
    fn from(v: kokorobox_native::SystemProxyDiagnostics) -> Self {
        let endpoint = |v: kokorobox_native::ProxyEndpoint| JsProxyEndpoint {
            host: v.host,
            port: v.port.into(),
        };
        Self {
            platform: v.platform,
            status: v.status,
            error_code: v.error_code,
            enabled: v.enabled,
            proxies: JsProxyProtocols {
                http: v.http.map(endpoint),
                https: v.https.map(endpoint),
                socks: v.socks.map(endpoint),
            },
            pac: v.pac_enabled.map(|enabled| JsPacState {
                enabled,
                url: v.pac_url,
            }),
            bypass: v.bypass,
            macos: v.macos.map(|m| JsMacOSProxyDetails {
                effective: m.effective.map(Into::into),
                active_service_ids: m.active_service_ids,
                services: m
                    .services
                    .into_iter()
                    .map(|s| JsMacNetworkService {
                        id: s.id,
                        name: s.name,
                        interface: s.interface,
                        enabled: s.enabled,
                        active: s.active,
                        primary: s.primary,
                        status: s.status,
                        proxies: s.proxies.map(Into::into),
                    })
                    .collect(),
                network_location: m.network_location,
                location_error_code: m.location_error_code,
                service_error_code: m.service_error_code,
            }),
            linux: v.linux.map(|l| JsLinuxProxyDetails {
                desktop_environment: l.desktop_environment,
                backend: l.backend,
                mode: l.mode,
                reversed_bypass: l.reversed_bypass,
                environment: l
                    .environment
                    .into_iter()
                    .map(|e| JsProxyEnvironmentEntry {
                        name: e.name,
                        endpoint: e.endpoint.map(endpoint),
                        bypass: e.bypass,
                        valid: e.valid,
                    })
                    .collect(),
                portal: JsProxyPortalState {
                    status: l.portal.status,
                    direct: l.portal.direct,
                    proxies: l.portal.proxies.into_iter().map(endpoint).collect(),
                    error_code: l.portal.error_code,
                },
            }),
            windows: v.windows.map(|w| JsWindowsProxyDetails {
                proxy_server: w.proxy_server,
                proxy_override: w.proxy_override,
                auto_config_url: w.auto_config_url,
                win_http: JsWinHttpProxyState {
                    status: w.win_http.status,
                    mode: w.win_http.mode,
                    proxy: w.win_http.proxy,
                    bypass: w.win_http.bypass,
                    error_code: w.win_http.error_code,
                },
                app_container: JsAppContainerState {
                    supported: w.app_container.supported,
                    status: w.app_container.status,
                    loopback_exemption_count: w.app_container.loopback_exemption_count,
                    error_code: w.app_container.error_code,
                },
            }),
        }
    }
}
pub struct SystemProxyDiagnosticsTask;
#[napi]
impl Task for SystemProxyDiagnosticsTask {
    type Output = kokorobox_native::SystemProxyDiagnostics;
    type JsValue = JsSystemProxyDiagnostics;
    fn compute(&mut self) -> Result<Self::Output> {
        Ok(kokorobox_native::get_system_proxy_diagnostics())
    }
    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}
#[napi]
pub fn get_system_proxy_diagnostics() -> AsyncTask<SystemProxyDiagnosticsTask> {
    AsyncTask::new(SystemProxyDiagnosticsTask)
}

pub struct SetSystemProxyTask {
    settings: kokorobox_native::SystemProxySettings,
}
#[napi]
impl Task for SetSystemProxyTask {
    type Output = kokorobox_native::SystemProxyMutation;
    type JsValue = JsSystemProxyMutation;
    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::set_system_proxy(&self.settings).map_err(map_err)
    }
    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(JsSystemProxyMutation {
            automatic_settings_preserved: output.automatic_settings_preserved,
        })
    }
}
#[napi]
pub fn set_system_proxy(settings: JsSystemProxySettings) -> Result<AsyncTask<SetSystemProxyTask>> {
    let port = settings
        .port
        .map(u16::try_from)
        .transpose()
        .map_err(|_| Error::from_reason("invalid-proxy-endpoint"))?;
    Ok(AsyncTask::new(SetSystemProxyTask {
        settings: kokorobox_native::SystemProxySettings {
            mode: settings.mode,
            host: settings.host,
            port,
            bypass: settings.bypass,
            pac_url: settings.pac_url,
            only_active_device: settings.only_active_device.unwrap_or(false),
        },
    }))
}
