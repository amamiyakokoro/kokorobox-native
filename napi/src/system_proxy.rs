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
pub struct JsSystemProxyDiagnostics {
    pub platform: String,
    pub status: String,
    pub error_code: Option<String>,
    pub enabled: Option<bool>,
    pub proxies: JsProxyProtocols,
    pub pac: Option<JsPacState>,
    pub bypass: Vec<String>,
    pub windows: Option<JsWindowsProxyDetails>,
}
#[napi(object)]
pub struct JsSystemProxySettings {
    pub mode: String,
    pub host: Option<String>,
    pub port: Option<u32>,
    pub bypass: Vec<String>,
    pub pac_url: Option<String>,
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
    type Output = ();
    type JsValue = ();
    fn compute(&mut self) -> Result<()> {
        kokorobox_native::set_system_proxy(&self.settings).map_err(map_err)
    }
    fn resolve(&mut self, _env: Env, _output: ()) -> Result<()> {
        Ok(())
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
        },
    }))
}
